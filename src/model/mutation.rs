use crate::auth::{Role, RoleChecker, RoleGuard};
use crate::cluster::ClusterTrait;
#[cfg(feature = "gust")]
use crate::cluster::Gust as Cluster;
use crate::entities::comment;
use crate::entities::issue::{self, IssueStatus};
use crate::entities::prelude::*;
use crate::entities::target::{self, TargetStatus};
use async_graphql::{Context, InputObject, Object, Result};
#[cfg(feature = "pbs")]
use pbs::Server;
use sea_orm::entity::ActiveValue;
use sea_orm::EntityTrait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, QueryFilter};
use std::collections::HashMap;
use tokio::sync::mpsc;
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
    async fn update(&self, operator: &str, ctx: &Context<'_>) -> Result<issue::Model, String> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let issue = Issue::find_by_id(self.id).one(db).await.unwrap();
        if issue.is_none() {
            return Err(format!("Issue {} not found", self.id));
        }
        let issue = issue.unwrap();
        let mut updated_issue: issue::ActiveModel = issue.clone().into();
        if let Some(_) = &self.assigned_to
            && self.assigned_to != issue.assigned_to
        {
            updated_issue.assigned_to = ActiveValue::Set(self.assigned_to.clone());
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!(
                    "Updating assigned_to from {:?} to {:?}",
                    issue.assigned_to, self.assigned_to
                )),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
        }
        if let Some(d) = self.description.clone()
            && d != issue.description
        {
            updated_issue.description = ActiveValue::Set(d.clone());
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!(
                    "Updating description from {:?} to {:?}",
                    issue.description, d
                )),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
        }
        if let Some(t) = self.title.clone()
            && t != issue.title
        {
            updated_issue.title = ActiveValue::Set(t.to_string());
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!(
                    "Updating title from {:?} to {:?}",
                    issue.title, t
                )),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
        }
        if let Some(_) = self.to_offline
            && self.to_offline != issue.to_offline
        {
            updated_issue.to_offline = ActiveValue::Set(self.to_offline);
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!(
                    "Updating to_offline from {:?} to {:?}",
                    issue.to_offline, self.to_offline
                )),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
            #[cfg(feature = "pbs")]
            {
                let srv = Server::new();
                let status = srv.stat_host(&None, None).unwrap();
                let target = issue.target(ctx).await.unwrap().unwrap();
                for t in to_offline(&target.name, status, self.to_offline).into_iter() {
                    srv.offline_vnode(&t, Some(&format!("{} sibling", &t)))
                        .unwrap();
                    let mut sib: target::ActiveModel =
                        Target::from_name(&t, db).await.unwrap().into();
                    sib.status = ActiveValue::Set(TargetStatus::Draining);
                    sib.update(db).await.unwrap();
                }
                //TODO only try offlining if it isn't draining already
                if self.to_offline.is_some() {
                    let _ = srv.offline_vnode(&target.name, Some(&issue.title));
                    let mut t: target::ActiveModel = target.into();
                    t.status = ActiveValue::Set(TargetStatus::Draining);
                    t.update(db).await.unwrap();
                }
            }
        }
        if let Some(e) = self.enforce_down
            && e != issue.enforce_down
        {
            updated_issue.enforce_down = ActiveValue::Set(e);
            let c = comment::ActiveModel {
                created_by: ActiveValue::Set(operator.to_string()),
                comment: ActiveValue::Set(format!(
                    "Updating enforce_down from {:?} to {:?}",
                    issue.enforce_down, e
                )),
                issue_id: ActiveValue::Set(issue.id),
                ..Default::default()
            };
            c.insert(db).await.unwrap();
        }
        updated_issue.update(db).await.unwrap();
        check_blade(
            &Target::find_by_id(issue.id)
                .one(db)
                .await
                .unwrap()
                .unwrap()
                .name,
            db,
        )
        .await
        .unwrap();
        Ok(Issue::find_by_id(self.id).one(db).await.unwrap().unwrap())
    }
}

#[derive(InputObject)]
pub struct NewIssue {
    assigned_to: Option<String>,
    description: String,
    to_offline: Option<issue::ToOffline>,
    target: String,
    title: String,
}

async fn check_blade(target: &str, db: &DatabaseConnection) -> Result<(), ()> {
    let srv = Server::new();
    let status = srv.stat_host(&None, None).unwrap();
    let nodes = Cluster::cousins(target);
    // current status of nodes in blade
    let current_status: HashMap<String, TargetStatus> = status
        .resources
        .into_iter()
        .filter(|n| nodes.iter().any(|t| n.name().eq(t)))
        //only care about ones that aren't already offline
        .map(|n| {
            (
                n.name(),
                if let pbs::Attrl::Value(pbs::Op::Equal(state)) = n.attribs().get("state").unwrap()
                {
                    //don't care about nuance here, make state binary, node is either offline or online
                    if state.contains("offline")
                        || state.contains("down")
                        || state.contains("unknown")
                    {
                        TargetStatus::Offline
                    } else {
                        TargetStatus::Online
                    }
                } else {
                    panic!()
                },
            )
        })
        .collect();

    // expected status of nodes in blade
    let mut expected_status: HashMap<String, (TargetStatus, bool)> = Cluster::cousins(target)
        .into_iter()
        .map(|t| (t, (TargetStatus::Online, false)))
        .collect();
    for node in Cluster::cousins(target) {
        if let Ok(issues) = Target::from_name(&node, db)
            .await
            .unwrap()
            .issues()
            .all(db)
            .await
        {
            for i in issues {
                for n in node_group(&node, i.to_offline) {
                    expected_status.insert(
                        n.clone(),
                        (
                            TargetStatus::Offline,
                            expected_status.get(&n).unwrap().1 || i.enforce_down,
                        ),
                    );
                }
            }
        }
    }
    // rectify differences
    for (node, state) in current_status {
        // check if expected is the same as actual
        let expected = expected_status.get(&node).unwrap();
        if expected.0 != state {
            if expected.0 == TargetStatus::Online {
                //node should be online but isn't, resume it
                srv.clear_vnode(&node, Some("")).unwrap();
            } else if expected.1 {
                // node is expected to be offline, and enforce flag is set
                // so offline it
                srv.offline_vnode(&node, Some("ctt enforcing node offline"))
                    .unwrap();
            } else {
                // node is expected to be offline, and no enforce flag
                // assume it is fixed and close any open issues on the target
                for issue in Target::from_name(&node, db)
                    .await
                    .unwrap()
                    .issues()
                    .filter(issue::Column::Status.eq(IssueStatus::Open))
                    .all(db)
                    .await
                    .unwrap()
                {
                    let mut i: issue::ActiveModel = issue.into();
                    i.status = ActiveValue::Set(IssueStatus::Closed);
                    i.update(db).await.unwrap();
                    //TODO add comment found node online, closing ticket
                }
                let mut target: target::ActiveModel =
                    Target::from_name(&node, db).await.unwrap().into();
                target.status = ActiveValue::Set(TargetStatus::Online);
                target.update(db).await.unwrap();
            }
        }
    }
    Ok(())
}

fn node_group(target: &str, group: Option<issue::ToOffline>) -> Vec<String> {
    match group {
        None => vec![],
        Some(issue::ToOffline::Blade) => Cluster::cousins(target),
        Some(issue::ToOffline::Card) => Cluster::siblings(target),
        Some(issue::ToOffline::Node) => {
            vec![]
        }
    }
}

fn to_offline(target: &str, status: pbs::StatResp, group: Option<issue::ToOffline>) -> Vec<String> {
    let to_offline = node_group(target, group);
    status
        .resources
        .into_iter()
        //only care about nodes in `to_offline`
        .filter(|n| to_offline.iter().any(|t| n.name().eq(t)))
        .filter(|t| t.name().ne(target))
        //only care about ones that aren't already offline
        .filter(|n| {
            &pbs::Attrl::Value(pbs::Op::Equal("offline".to_string()))
                != n.attribs().get("state").unwrap()
        })
        .map(|n| n.name())
        .collect()
}

impl NewIssue {
    pub fn new(
        assigned_to: Option<String>,
        description: String,
        title: String,
        target: String,
        to_offline: Option<issue::ToOffline>,
    ) -> Self {
        Self {
            assigned_to,
            description,
            to_offline,
            target,
            title,
        }
    }
    pub async fn open(
        &self,
        operator: &str,
        db: &DatabaseConnection,
    ) -> Result<issue::Model, String> {
        if let Some(i) = Target::from_name(&self.target, db)
            .await
            .unwrap()
            .issues()
            .filter(issue::Column::Status.eq(IssueStatus::Open))
            .filter(issue::Column::Title.eq(&self.title))
            .one(db)
            .await
            .unwrap()
        {
            return Ok(i);
        }
        let target = if let Some(t) = Target::from_name(&self.target, db).await {
            t
        } else {
            warn!("Target not found, creating {}", self.target);
            Target::create_target(&self.target, TargetStatus::Online, db)
                .await
                .unwrap()
        };
        let target_id = target.id;
        #[cfg(feature = "pbs")]
        {
            let srv = Server::new();
            let status = srv.stat_host(&None, None).unwrap();
            for t in to_offline(&self.target, status, self.to_offline).into_iter() {
                srv.offline_vnode(&t, Some(&format!("{} sibling", &t)))
                    .unwrap();
                let mut sib: target::ActiveModel = Target::from_name(&t, db).await.unwrap().into();
                sib.status = ActiveValue::Set(TargetStatus::Draining);
                sib.update(db).await.unwrap();
            }
            if self.to_offline.is_some() {
                let _ = srv.offline_vnode(&self.target, Some(&self.title));
                let mut target: target::ActiveModel = target.into();
                target.status = ActiveValue::Set(TargetStatus::Draining);
                target.update(db).await.unwrap();
            }
        }

        let new_issue = issue::ActiveModel {
            assigned_to: ActiveValue::Set(self.assigned_to.clone()),
            created_by: ActiveValue::Set(operator.to_string()),
            description: ActiveValue::Set(self.description.clone()),
            to_offline: ActiveValue::Set(self.to_offline),
            status: ActiveValue::Set(IssueStatus::Open),
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

async fn issue_close(
    cttissue: i32,
    operator: String,
    comment: String,
    db: &DatabaseConnection,
) -> Result<String, String> {
    let issue = Issue::find_by_id(cttissue).one(db).await.unwrap().unwrap();
    let target_id = issue.target_id;
    if issue.status == IssueStatus::Open {
        let mut issue: issue::ActiveModel = issue.into();
        issue.status = ActiveValue::Set(IssueStatus::Closed);
        issue.reset(issue::Column::Status);
        issue.update(db).await.unwrap();
        let c = comment::ActiveModel {
            created_by: ActiveValue::Set(operator.clone()),
            comment: ActiveValue::Set(comment.clone()),
            issue_id: ActiveValue::Set(target_id),
            ..Default::default()
        };
        c.insert(db).await.unwrap();
        let target = Target::find_by_id(target_id)
            .one(db)
            .await
            .unwrap()
            .unwrap();
        check_blade(&target.name, db).await.unwrap();
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
        let _ = tx
            .send(format!(
                "{}: Opening issue for {}: {}",
                usr, issue.target, issue.title
            ))
            .await;
        issue.open(usr, db).await
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn close<'a>(
        &self,
        ctx: &Context<'a>,
        issue: i32,
        comment: String,
    ) -> Result<String, String> {
        let usr: String = ctx.data_opt::<RoleGuard>().unwrap().user.clone();
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx
            .send(format!("{}: closing issue for {}: {}", usr, issue, comment))
            .await;
        let db = ctx.data::<DatabaseConnection>().unwrap();
        issue_close(issue, usr, comment, db).await
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    async fn update_issue<'a>(
        &self,
        ctx: &Context<'a>,
        issue: UpdateIssue,
    ) -> Result<issue::Model, String> {
        let usr: String = ctx.data_opt::<RoleGuard>().unwrap().user.clone();
        issue.update(&usr, ctx).await
    }
}
