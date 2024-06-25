use crate::auth::{Role, RoleChecker, RoleGuard};
use crate::cluster::{ClusterTrait, RegexCluster};
use crate::entities::comment;
use crate::entities::issue::{self, IssueStatus};
use crate::entities::prelude::*;
use crate::entities::target::TargetStatus;
use crate::ChangeLogMsg;
use async_graphql::{Context, InputObject, Object, Result};
use chrono::Utc;
use sea_orm::entity::ActiveValue;
use sea_orm::EntityTrait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, QueryFilter};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, instrument, warn};

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
        cluster: &RegexCluster,
    ) -> Option<Self> {
        if cluster.real_node(&target) {
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
    let tx = &ctx.data_opt::<mpsc::Sender<ChangeLogMsg>>().unwrap();
    let issue = Issue::find_by_id(i.id).one(db).await.unwrap();
    if issue.is_none() {
        return Err(format!("Issue {} not found", i.id));
    }
    let issue = issue.unwrap();
    let mut updated_issue: issue::ActiveModel = issue.clone().into();
    if let Some(s) = &i.assigned_to
        && i.assigned_to != issue.assigned_to
    {
        if s.is_empty() {
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
    }
    info!("Updating issue {}: {:?}", issue.id, updated_issue);
    let _ = tx
        .send(ChangeLogMsg::Update {
            issue: issue.id,
            operator: operator.to_string(),
            title: issue.title.clone(),
        })
        .await;

    // needs to happen before node state check so that crate::sync::desired_state uses the new
    // to_offline value for this issue
    updated_issue.updated_at = ActiveValue::Set(Utc::now().naive_utc());
    updated_issue.update(db).await.unwrap();
    //TODO FIXME how to handle a reduction in to_offline? (blade->card->node)
    //sync code doesn't know a node was offline due to being a sibling, so it will
    //open a new ticket for the sibling instead of resuming it
    //resuming nodes here for now instead of the sync loop since its easier
    if let Some(t_o) = issue.to_offline
        && i.to_offline != issue.to_offline
    {
        let cluster = ctx.data::<RegexCluster>().unwrap();
        let target = issue.target(ctx).await.unwrap().unwrap().name;
        let cousins = cluster.cousins(&target);
        let siblings = cluster.siblings(&target);

        //t_o != i.to_offline therefore issue no longer enforces cousins being down
        if t_o == issue::ToOffline::Blade {
            //resume cousins (but not siblings)
            for c in cousins {
                if c == target || siblings.contains(&c) {
                    continue;
                }
                let (desired_node_state, _) = crate::sync::desired_state(&c, db, cluster).await;
                if desired_node_state == TargetStatus::Online {
                    //TODO add changelog msg
                    if cluster.release_node(&c).is_err() {
                        warn!("Error releasing node {}", c);
                    } else {
                        let _ = tx
                            .send(ChangeLogMsg::Resume {
                                target: c.to_string(),
                            })
                            .await;
                    }
                }
            }
        }

        //t_o is something, and != i.to_offline, so issue no longer enforces sibling being down
        if i.to_offline.unwrap() == issue::ToOffline::Node {
            for s in siblings {
                if s == target {
                    continue;
                }
                let (desired_node_state, _) = crate::sync::desired_state(&s, db, cluster).await;
                if desired_node_state == TargetStatus::Online {
                    //TODO add changelog msg
                    cluster.release_node(&s).unwrap();
                    if cluster.release_node(&s).is_err() {
                        warn!("Error releasing node {}", s);
                    } else {
                        let _ = tx
                            .send(ChangeLogMsg::Resume {
                                target: s.to_string(),
                            })
                            .await;
                    }
                }
            }
        }
    }
    Ok(Issue::find_by_id(i.id).one(db).await.unwrap().unwrap())
}

#[instrument]
fn node_group(
    target: &str,
    group: Option<issue::ToOffline>,
    cluster: &RegexCluster,
) -> Vec<String> {
    match group {
        None => vec![],
        Some(issue::ToOffline::Blade) => cluster.cousins(target),
        Some(issue::ToOffline::Card) => cluster.siblings(target),
        Some(issue::ToOffline::Node) => {
            vec![]
        }
    }
}

#[instrument(skip(status))]
fn to_offline(
    target: &str,
    status: pbs::StatResp,
    group: Option<issue::ToOffline>,
    cluster: &RegexCluster,
) -> Vec<String> {
    let to_offline = node_group(target, group, cluster);
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
    tx: &mpsc::Sender<ChangeLogMsg>,
    cluster: &RegexCluster,
) -> Result<issue::Model, String> {
    if !cluster.real_node(&i.target) {
        return Err(format!("{} is not a real node", &i.target));
    }
    let target = if let Some(t) = Target::from_name(&i.target, db, cluster).await {
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

    let new_issue = issue::ActiveModel {
        assigned_to: ActiveValue::Set(i.assigned_to.clone()),
        created_by: ActiveValue::Set(operator.to_string()),
        description: ActiveValue::Set(i.description.clone()),
        to_offline: ActiveValue::Set(i.to_offline),
        status: ActiveValue::Set(IssueStatus::Opening),
        target_id: ActiveValue::Set(target_id),
        title: ActiveValue::Set(i.title.clone()),
        ..Default::default()
    };
    let new_issue = new_issue.insert(db).await.unwrap();
    let _ = tx
        .send(ChangeLogMsg::Open {
            title: i.title.clone(),
            issue: new_issue.id,
            operator: operator.to_string(),
        })
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
    if issue.status == IssueStatus::Open || issue.status == IssueStatus::Opening {
        info!(
            "Closing ticket {} for {}: {}",
            cttissue, target.name, comment
        );
        let title = issue.title.clone();
        let mut issue: issue::ActiveModel = issue.into();
        issue.status = ActiveValue::Set(IssueStatus::Closing);
        issue.update(db).await.unwrap();
        let c = comment::ActiveModel {
            created_by: ActiveValue::Set(operator.clone()),
            comment: ActiveValue::Set(comment.clone()),
            issue_id: ActiveValue::Set(cttissue),
            ..Default::default()
        };
        c.insert(db).await.unwrap();
        let tx = &ctx.data_opt::<mpsc::Sender<ChangeLogMsg>>().unwrap();
        let _ = tx
            .send(ChangeLogMsg::Close {
                issue: cttissue,
                operator: operator.clone(),
                comment,
                title,
            })
            .await;
    }
    Ok(format!("closed {}", cttissue))
}

#[Object]
impl Mutation {
    #[graphql(guard = "RoleChecker::new(Role::Admin)")]
    #[instrument(skip(ctx))]
    async fn open<'a>(&self, ctx: &Context<'a>, issue: NewIssue) -> Result<issue::Model, String> {
        let usr = &ctx.data_opt::<RoleGuard>().unwrap().user;
        let tx = ctx.data_opt::<mpsc::Sender<ChangeLogMsg>>().unwrap();
        let db = ctx.data_opt::<Arc<DatabaseConnection>>().unwrap().as_ref();
        let cluster = ctx.data::<RegexCluster>().unwrap();

        issue_open(&issue, usr, db, tx, cluster).await
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
