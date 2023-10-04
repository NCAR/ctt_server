use sea_orm::{DatabaseConnection, ActiveModelTrait};
use sea_orm::entity::ActiveValue;
use crate::auth::{Role, RoleChecker, RoleGuard};
use async_graphql::{Context, InputObject, Object, Result};
use tokio::sync::mpsc;
#[cfg(feature="pbs")]
use pbs::Server;
use crate::entities::issue::{self, IssueStatus};
use crate::entities::comment;
use crate::entities::prelude::*;
use crate::entities::target::{self, TargetStatus};
use tracing::log::warn;

#[derive(InputObject)]
pub struct UpdateIssue {
    assigned_to: Option<String>,
    description: Option<String>,
    enforce_down: Option<bool>,
    id: u32,
    title: Option<String>,
}

#[derive(InputObject)]
pub struct NewIssue {
    assigned_to: Option<String>,
    description: String,
    to_offline: Option<issue::ToOffline>,
    enforce_down: Option<bool>,
    target: String,
    title: String,
}

async fn create_target(target: &str, db: &DatabaseConnection) -> target::Model {
        let new_target = target::ActiveModel{
            name: ActiveValue::Set(target.to_string()),
            status: ActiveValue::Set(target::TargetStatus::Online),
            ..Default::default()
        };
        new_target.insert(db).await.unwrap()
}

impl NewIssue {
    async fn open(&self, operator: &str, db: &DatabaseConnection) -> Result<issue::Model, String> {
        if let Some(i) = Issue::already_open(&self.target, &self.title, db).await {
            return Ok(i)
        }
        let target = Target::find_by_name(&self.target).one(db).await.unwrap();
        let target = if target.is_none() {
            warn!("Target not found, creating {}", self.target);
            create_target(&self.target, db).await
        } else {
            let t = target.unwrap();
            warn!("Target exists, with id {}", t.id);
            t
        };
        let target_id = target.id;
        #[cfg(feature="pbs")]
        let srv = Server::new();
        // TODO only set offline if not already offline
        //let status = srv.stat_host(&None, None);
        let off: Result<(), String> = match self.to_offline {
            None => {
                #[cfg(feature="pbs")]
                srv.offline_vnode(&self.target, Some(&self.title))?;
                //TODO set target to draining
                let mut target: target::ActiveModel = target.into();
                target.status = ActiveValue::Set(TargetStatus::Draining);
                target.update(db).await.unwrap();
                Ok(())
            },
            Some(issue::ToOffline::Cousins) => {
                todo!()
                //srv.offline(cluster.blade(self.target), format!("{} sibling", self.target));
                //srv.offline(vec!(self.target), &self.title);
            },
            Some(issue::ToOffline::Siblings) => {
                todo!()
                //srv.offline(cluster.card(self.target), format!("{} sibling", self.target));
                //srv.offline(vec!(self.target), &self.title);
            },
            Some(issue::ToOffline::Target) => {
                #[cfg(feature="pbs")]
                srv.offline_vnode(&self.target, Some(&self.title))?;
                //TODO set target to draining
                let mut target: target::ActiveModel = target.into();
                target.status = ActiveValue::Set(TargetStatus::Draining);
                target.update(db).await.unwrap();
                Ok(())
            },
        };
        off.unwrap();
        let new_issue = issue::ActiveModel{
            assigned_to: ActiveValue::Set(self.assigned_to.clone()),
            created_by: ActiveValue::Set(operator.to_string()),
            description: ActiveValue::Set(self.description.clone()),
            to_offline: ActiveValue::Set(self.to_offline),
            enforce_down: ActiveValue::Set(self.enforce_down.unwrap_or(false)),
            issue_status: ActiveValue::Set(IssueStatus::Open),
            //TODO insert target if not exists
            target_id: ActiveValue::Set(target_id),
            title: ActiveValue::Set(self.title.clone()),
            ..Default::default()
        };
        let new_issue = new_issue.insert(db).await.unwrap();
        let c = comment::ActiveModel {
            created_by: ActiveValue::Set(operator.to_string()),
            comment: ActiveValue::Set("Opening issue".to_string()),
            issue_id: ActiveValue::Set(new_issue.id),
            ..Default::default()
        };
        c.insert(db).await.unwrap();
        Ok(new_issue)
    }
}

async fn issue_close(cttissue: i32, operator: String, comment: String, db: &DatabaseConnection) -> Result<String, String> {
    let issue = Issue::find_by_id(cttissue).one(db).await.unwrap().unwrap();
    let target_id = issue.target_id;
    if issue.issue_status == IssueStatus::Open {
        let mut issue: issue::ActiveModel = issue.into();
        issue.issue_status = ActiveValue::Set(IssueStatus::Closed);
        issue.reset(issue::Column::IssueStatus);
        issue.update(db).await.unwrap();
        let c = comment::ActiveModel {
            created_by: ActiveValue::Set(operator.clone()),
            comment: ActiveValue::Set(comment.clone()),
            issue_id: ActiveValue::Set(target_id),
            ..Default::default()
        };
        c.insert(db).await.unwrap();
        #[allow(unused_variables)]
        let target = Target::find_by_id(target_id).one(db).await.unwrap().unwrap();
        #[cfg(feature="pbs")]
        let srv = Server::new();
        #[cfg(feature="pbs")]
        srv.clear_vnode(&target.name, Some(""))?;
        //TODO check if there are any other issues on target, or siblings with correct to_offline flag before resuming
        let mut target: target::ActiveModel = target.into();
        target.status = ActiveValue::Set(TargetStatus::Online);
        target.update(db).await.unwrap();
    }
    Ok(format!("closed {}", cttissue))
}

pub struct Mutation;

#[Object]
impl Mutation {
    
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn open<'a>(&self, ctx: &Context<'a>, issue: NewIssue) -> Result<issue::Model, String> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        //TODO get operator from authentication
        let usr = &ctx.data_opt::<RoleGuard>().unwrap().user;
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx.send(format!("{}: Opening issue for {}: {}", usr, issue.target, issue.title)).await;
        issue.open(usr, db).await
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn close<'a>(&self, ctx: &Context<'a>, issue: i32, comment: String) -> Result<String, String> {
        let usr: String = ctx.data_opt::<RoleGuard>().unwrap().user.clone();
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx.send(format!("{}: closing issue for {}: {}", usr, issue, comment)).await;
        let db = ctx.data::<DatabaseConnection>().unwrap();
        issue_close(issue, usr, comment, db).await
    }
    /*
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn update<'a>(&self, ctx: &Context<'a>, issue: UpdateIssue) -> issue::Model {
        todo!()
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn drain<'a>(&self, ctx: &Context<'a>, issue: u32) -> String {
        let usr = &ctx.data_opt::<RoleGuard>().unwrap().user;
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx.send(format!("{}: draing nodes for issue {}", usr, issue)).await;
        todo!()
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn release<'a>(&self, ctx: &Context<'a>, issue: u32) -> String {
        let usr = &ctx.data_opt::<RoleGuard>().unwrap().user;
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx.send(format!("{}: resuming nodes for issue {}", usr, issue)).await;
        todo!()
    }
    */
}
