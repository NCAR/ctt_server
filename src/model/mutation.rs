use crate::auth::{Role, RoleChecker, RoleGuard};
use crate::cluster::ClusterTrait;
#[cfg(feature = "gust")]
use crate::cluster::Gust as Cluster;
use crate::entities;
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
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, instrument, warn};

#[derive(InputObject, Debug)]
pub struct UpdateIssue {
    assigned_to: Option<String>,
    description: Option<String>,
    enforce_down: Option<bool>,
    to_offline: Option<issue::ToOffline>,
    id: i32,
    title: Option<String>,
}

#[derive(InputObject, Debug)]
pub struct NewIssue {
    assigned_to: Option<String>,
    description: String,
    to_offline: Option<issue::ToOffline>,
    target: String,
    title: String,
}

impl NewIssue {
    #[instrument]
    pub fn new(
        assigned_to: Option<String>,
        description: String,
        title: String,
        target: String,
        to_offline: Option<issue::ToOffline>,
    ) -> Option<Self> {
        if Cluster::real_node(&target) {
            Some(Self {
                assigned_to,
                description,
                to_offline,
                target,
                title,
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Mutation;

#[instrument(skip(ctx))]
async fn issue_update(
    i: UpdateIssue,
    operator: &str,
    ctx: &Context<'_>,
) -> Result<issue::Model, String> {
    let db = ctx.data::<Arc<DatabaseConnection>>().unwrap().as_ref();
    let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
    let issue = Issue::find_by_id(i.id).one(db).await.unwrap();
    if issue.is_none() {
        return Err(format!("Issue {} not found", i.id));
    }
    let issue = issue.unwrap();
    let mut updated_issue: issue::ActiveModel = issue.clone().into();
    if let Some(_) = &i.assigned_to
        && i.assigned_to != issue.assigned_to
    {
        if let Some(s) = &i.assigned_to
            && s.is_empty()
        {
            updated_issue.assigned_to = ActiveValue::Set(None);
        } else {
            updated_issue.assigned_to = ActiveValue::Set(i.assigned_to.clone());
        }
        let c = comment::ActiveModel {
            created_by: ActiveValue::Set(operator.to_string()),
            comment: ActiveValue::Set(format!(
                "Updating assigned_to from {:?} to {:?}",
                issue.assigned_to,
                updated_issue.assigned_to.clone().unwrap()
            )),
            issue_id: ActiveValue::Set(issue.id),
            ..Default::default()
        };
        c.insert(db).await.unwrap();
    }
    if let Some(d) = i.description.clone()
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
    if let Some(t) = i.title.clone()
        && t != issue.title
    {
        updated_issue.title = ActiveValue::Set(t.to_string());
        let c = comment::ActiveModel {
            created_by: ActiveValue::Set(operator.to_string()),
            comment: ActiveValue::Set(format!("Updating title from {:?} to {:?}", issue.title, t)),
            issue_id: ActiveValue::Set(issue.id),
            ..Default::default()
        };
        c.insert(db).await.unwrap();
    }
    if let Some(_) = i.to_offline
        && i.to_offline != issue.to_offline
    {
        info!("updating to_offline");
        updated_issue.to_offline = ActiveValue::Set(i.to_offline);
        let c = comment::ActiveModel {
            created_by: ActiveValue::Set(operator.to_string()),
            comment: ActiveValue::Set(format!(
                "Updating to_offline from {:?} to {:?}",
                issue.to_offline, i.to_offline
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
            for t in to_offline(&target.name, status, i.to_offline).into_iter() {
                srv.offline_vnode(
                    &t,
                    Some(&format!(
                        "{} sibling",
                        &issue.target(ctx).await.unwrap().unwrap().name
                    )),
                )
                .unwrap();
                let mut sib: target::ActiveModel = Target::from_name(&t, db).await.unwrap().into();
                sib.status = ActiveValue::Set(TargetStatus::Draining);
                sib.update(db).await.unwrap();
            }
            //TODO only try offlining if it isn't draining already
            if i.to_offline.is_some() {
                let _ = srv.offline_vnode(&target.name, Some(&issue.title));
                let mut t: target::ActiveModel = target.into();
                t.status = ActiveValue::Set(TargetStatus::Draining);
                t.update(db).await.unwrap();
            }
        }
    }
    info!("Updating issue {}: {:?}", issue.id, updated_issue);
    updated_issue.update(db).await.unwrap();
    check_blade(&issue.target(ctx).await.unwrap().unwrap().name, db, tx).await;
    Ok(Issue::find_by_id(i.id).one(db).await.unwrap().unwrap())
}

#[instrument]
async fn check_blade(target: &str, db: &DatabaseConnection, tx: &mpsc::Sender<String>) {
    debug!("Checking blade status for {}", target);
    let srv = Server::new();
    let nodes = Cluster::cousins(target);
    // current status of nodes in blade
    let current_status: HashMap<String, (TargetStatus, String)> = Cluster::nodes_status(&srv, tx)
        .await
        .into_iter()
        .filter(|n| nodes.iter().any(|t| n.0.eq(t)))
        .collect();

    for (target, (new_state, _new_comment)) in current_status {
        let (expected_state, comment) = crate::pbs_sync::desired_state(&target, db).await;
        //almost the same as crate::pbs_sync::handle_transition, but different logic
        //e.g. release a node if expected online but found offline
        //TODO combine with handle_transition since everything except expected online found not
        //online is the same

        let final_state = match expected_state {
            TargetStatus::Draining => panic!("Expected state is never Draining"),
            TargetStatus::Online => {
                if new_state != TargetStatus::Online {
                    Cluster::release_node(&target, "ctt", &srv, tx)
                        .await
                        .unwrap();
                }
                TargetStatus::Online
            }
            TargetStatus::Offline => match new_state {
                TargetStatus::Draining => TargetStatus::Draining,
                TargetStatus::Offline => TargetStatus::Offline,
                state => {
                    Cluster::offline_node(&target, &comment, "ctt", &srv, tx)
                        .await
                        .unwrap();
                    if state == TargetStatus::Down {
                        TargetStatus::Offline
                    } else {
                        // node was online, might have running jobs
                        TargetStatus::Draining
                    }
                }
            },
            TargetStatus::Down => match new_state {
                TargetStatus::Draining => TargetStatus::Draining,
                TargetStatus::Down => TargetStatus::Down,
                TargetStatus::Offline => TargetStatus::Offline,
                TargetStatus::Online => {
                    info!("Closing issues for {}", &target);
                    crate::pbs_sync::close_open_issues(&target, db).await;
                    TargetStatus::Online
                }
            },
        };
        debug!(
            "target: {}, current: {:?}, expected: {:?}, final: {:?}",
            &target, &new_state, &expected_state, &final_state
        );
        let old_state = crate::pbs_sync::get_ctt_nodes(db).await;
        //dont update state if it hasn't changed
        if *old_state.get(&target).unwrap() != final_state {
            let node = entities::target::Entity::from_name(&target, db)
                .await
                .unwrap();
            let mut updated_target: entities::target::ActiveModel = node.into();
            updated_target.status = ActiveValue::Set(final_state);
            updated_target.update(db).await.unwrap();
        }
    }
}

#[instrument]
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

#[instrument(skip(status))]
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
#[instrument]
pub async fn issue_open(
    i: &NewIssue,
    operator: &str,
    db: &DatabaseConnection,
    tx: &mpsc::Sender<String>,
) -> Result<issue::Model, String> {
    let target = if let Some(t) = Target::from_name(&i.target, db).await {
        t
    } else {
        warn!("Target {} not found", i.target);
        return Err(format!("Node {} does not exist", i.target));
    };
    if let Some(i) = target
        .issues()
        .filter(issue::Column::Status.eq(IssueStatus::Open))
        .filter(issue::Column::Title.eq(&i.title))
        .one(db)
        .await
        .unwrap()
    {
        return Ok(i);
    }
    let target_id = target.id;
    #[cfg(feature = "pbs")]
    {
        let srv = Server::new();
        let status = srv.stat_host(&None, None).unwrap();
        for t in to_offline(&i.target, status, i.to_offline).into_iter() {
            srv.offline_vnode(&t, Some(&format!("{} sibling", &i.target)))
                .unwrap();
            let mut sib: target::ActiveModel = Target::from_name(&t, db).await.unwrap().into();
            sib.status = ActiveValue::Set(TargetStatus::Draining);
            sib.update(db).await.unwrap();
        }
        if i.to_offline.is_some() {
            let _ = srv.offline_vnode(&i.target, Some(&i.title));
            let mut target: target::ActiveModel = target.into();
            target.status = ActiveValue::Set(TargetStatus::Draining);
            target.update(db).await.unwrap();
        }
    }

    let new_issue = issue::ActiveModel {
        assigned_to: ActiveValue::Set(i.assigned_to.clone()),
        created_by: ActiveValue::Set(operator.to_string()),
        description: ActiveValue::Set(i.description.clone()),
        to_offline: ActiveValue::Set(i.to_offline),
        status: ActiveValue::Set(IssueStatus::Open),
        target_id: ActiveValue::Set(target_id),
        title: ActiveValue::Set(i.title.clone()),
        ..Default::default()
    };
    let new_issue = new_issue.insert(db).await.unwrap();
    let _ = tx
        .send(format!(
            "{}: Opening issue for {}: {}",
            operator, i.target, i.title
        ))
        .await;
    let c = comment::ActiveModel {
        created_by: ActiveValue::Set(operator.to_string()),
        comment: ActiveValue::Set("Opening issue".to_string()),
        issue_id: ActiveValue::Set(new_issue.id),
        ..Default::default()
    };
    c.insert(db).await.unwrap();
    Ok(new_issue)
}

#[instrument(skip(ctx))]
async fn issue_close(
    cttissue: i32,
    operator: String,
    comment: String,
    ctx: &Context<'_>,
) -> Result<String, String> {
    let db = ctx.data::<Arc<DatabaseConnection>>().unwrap().as_ref();
    let issue = Issue::find_by_id(cttissue).one(db).await.unwrap().unwrap();
    let target = issue.target(ctx).await.unwrap().unwrap();
    if issue.status == IssueStatus::Open {
        info!(
            "Closing ticket {} for {}: {}",
            cttissue, target.name, comment
        );
        let mut issue: issue::ActiveModel = issue.into();
        issue.status = ActiveValue::Set(IssueStatus::Closed);
        issue.update(db).await.unwrap();
        let c = comment::ActiveModel {
            created_by: ActiveValue::Set(operator.clone()),
            comment: ActiveValue::Set(comment.clone()),
            issue_id: ActiveValue::Set(cttissue),
            ..Default::default()
        };
        c.insert(db).await.unwrap();
        let tx = &ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let _ = tx
            .send(format!(
                "{}: closing issue for {}: {}",
                operator, target.name, comment
            ))
            .await;
        check_blade(&target.name, db, tx).await;
    }
    Ok(format!("closed {}", cttissue))
}

#[Object]
impl Mutation {
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    #[instrument(skip(ctx))]
    async fn open<'a>(&self, ctx: &Context<'a>, issue: NewIssue) -> Result<issue::Model, String> {
        let usr = &ctx.data_opt::<RoleGuard>().unwrap().user;
        let tx = ctx.data_opt::<mpsc::Sender<String>>().unwrap();
        let db = ctx.data_opt::<Arc<DatabaseConnection>>().unwrap().as_ref();
        issue_open(&issue, usr, db, tx).await
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    #[instrument(skip(ctx))]
    async fn close<'a>(
        &self,
        ctx: &Context<'a>,
        issue: i32,
        comment: String,
    ) -> Result<String, String> {
        let usr: String = ctx.data_opt::<RoleGuard>().unwrap().user.clone();
        issue_close(issue, usr, comment, ctx).await
    }
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    #[instrument(skip(ctx))]
    async fn update_issue<'a>(
        &self,
        ctx: &Context<'a>,
        issue: UpdateIssue,
    ) -> Result<issue::Model, String> {
        let usr: String = ctx.data_opt::<RoleGuard>().unwrap().user.clone();
        issue_update(issue, &usr, ctx).await
    }
}
