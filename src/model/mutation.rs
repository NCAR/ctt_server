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
    to_offline: Option<issue::ToOffline>,
    id: i32,
    title: Option<String>,
}

impl UpdateIssue {
    async fn update(&self, operator: &str, db: &DatabaseConnection) -> Result<issue::Model, String> {
        let issue = Issue::find_by_id(self.id).one(db).await.unwrap();
        if issue.is_none() {
            return Err(format!("Issue {} not found", self.id));
        }
        let issue = issue.unwrap();
        let mut updated_issue: issue::ActiveModel = issue.clone().into();
        if let Some(_) = &self.assigned_to && self.assigned_to != issue.assigned_to {
            updated_issue.assigned_to = ActiveValue::Set(self.assigned_to.clone());
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!("Updating assigned_to from {:?} to {:?}", issue.assigned_to, self.assigned_to)),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
        }
        if let Some(d) = self.description.clone() && d != issue.description {
            updated_issue.description = ActiveValue::Set(d.clone());
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!("Updating description from {:?} to {:?}", issue.description, d)),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
        }
        if let Some(t) = self.title.clone() && t != issue.title {
            updated_issue.title = ActiveValue::Set(t.to_string());
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!("Updating title from {:?} to {:?}", issue.title, t)),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
        }
        if let Some(_) = self.to_offline && self.to_offline != issue.to_offline {
            updated_issue.to_offline = ActiveValue::Set(self.to_offline);
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!("Updating to_offline from {:?} to {:?}", issue.to_offline, self.to_offline)),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
            //TODO offline/resume nodes that should be
        }
        if let Some(e) = self.enforce_down && e != issue.enforce_down {
            updated_issue.enforce_down = ActiveValue::Set(e);
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!("Updating enforce_down from {:?} to {:?}", issue.enforce_down, e)),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
        }
       Ok(updated_issue.update(db).await.unwrap())
    }
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
        // TODO only offline in pbs if not already offline
        #[cfg(feature="pbs")]
        let status = srv.stat_host(&None, None);
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
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn update_issue<'a>(&self, ctx: &Context<'a>, issue: UpdateIssue) -> Result<issue::Model, String> {
        let usr: String = ctx.data_opt::<RoleGuard>().unwrap().user.clone();
        let db = ctx.data::<DatabaseConnection>().unwrap();
        issue.update(&usr, &db).await
    }
}
