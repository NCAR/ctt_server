use crate::cluster::ClusterTrait;
#[cfg(feature = "gust")]
use crate::cluster::Gust as Cluster;
use crate::entities;
use crate::entities::issue::IssueStatus;
use crate::entities::issue::ToOffline;
use crate::entities::prelude::Target;
use crate::entities::target::TargetStatus;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, QueryFilter, QuerySelect};
use std::collections::HashMap;

use pbs::{Attrl, Op};
use sea_orm::DatabaseConnection;
use std::time::Duration;
use tokio::time;
#[allow(unused_imports)]
use tracing::{info, warn, Level};

pub async fn pbs_sync(db: DatabaseConnection) {
    //TODO get interval from config file
    let mut interval = time::interval(Duration::from_secs(60*5));
    // don't let ticks stack up if a sync takes longer than interval
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    loop {
        let pbs_srv = pbs::Server::new();
        let pbs_node_state = get_pbs_nodes(&pbs_srv).await;
        let mut ctt_node_state = get_ctt_nodes(&db).await;
        // sync ctt and pbs

        //handle any pbs nodes not in ctt
        pbs_node_state
            .keys()
            .filter(|t| !ctt_node_state.contains_key(*t))
            .collect::<Vec<&String>>()
            .iter()
            .for_each(|t| {
                ctt_node_state.insert(t.to_string(), TargetStatus::Online);
            });

        for (target, old_state) in &ctt_node_state {
            let pbs_state = pbs_node_state.get(target);
            if let Some(new_state) = pbs_state {
                handle_transition(target, old_state, new_state, &pbs_srv, &db).await;
            } else {
                warn!("{} not found in pbs", target);
                let new_issue = crate::model::NewIssue::new(
                    None,
                    "Node not found in pbs".to_string(),
                    "Node not found in pbs".to_string(),
                    target.to_string(),
                    None,
                );
                new_issue.open("ctt", &db).await.unwrap();
            }
        }
        interval.tick().await;
    }
}

pub async fn get_pbs_nodes(pbs_srv: &pbs::Server) -> HashMap<String, TargetStatus> {
    //TODO filter stat attribs (just need hostname, jobs, and state)
    //TODO need to handle err
    //TODO consider calling pbs_srv.stat_vnode from a spawn_blocking task
    //TODO add a timeout
    pbs_srv
        .stat_vnode(&None, None)
        .unwrap()
        .resources
        .iter()
        .map(|n| {
            let name = n.name();
            let jobs = {
                if let Some(Attrl::Value(Op::Default(j))) = n.attribs().get("jobs") {
                    j.is_empty()
                } else {
                    false
                }
            };
            let state = match n.attribs().get("state").unwrap() {
                Attrl::Value(Op::Default(j)) => j,
                x => {
                    println!("{:?}", x);
                    panic!("bad state");
                }
            };
            let state = match state.as_str() {
                //job-excl or resv-excl
                x if x.contains("exclusive") => TargetStatus::Online,
                //order matters, before "down" to capture down,offline nodes
                x if x.contains("offline") => {
                    if jobs {
                        TargetStatus::Draining
                    } else {
                        TargetStatus::Offline
                    }
                }
                x if x.contains("down") => {
                    if jobs {
                        TargetStatus::Draining
                    } else {
                        TargetStatus::Down
                    }
                }
                "job-busy" => TargetStatus::Online,
                "free" => TargetStatus::Online,
                x => {
                    warn!("Unrecognized node state, '{}'", x);
                    //TODO FIXME handle err
                    pbs_srv.offline_vnode(&name, None).unwrap();
                    if jobs {
                        TargetStatus::Draining
                    } else {
                        TargetStatus::Down
                    }
                }
            };
            (name, state)
        })
        .collect()
}

pub async fn get_ctt_nodes(db: &DatabaseConnection) -> HashMap<String, TargetStatus> {
    let ctt_node_state = entities::target::Entity::all()
        .select_only()
        .columns([
            entities::target::Column::Name,
            entities::target::Column::Status,
            entities::target::Column::Id,
        ])
        .all(db)
        .await
        .unwrap();
    ctt_node_state
        .iter()
        .map(|n| (n.name.clone(), n.status))
        .collect()
}

pub async fn desired_state(target: &str, db: &DatabaseConnection) -> (TargetStatus, String) {
    for c in Cluster::cousins(target) {
        let t = entities::target::Entity::from_name(&c, db).await;
        let t = if let Some(tmp) = t {
            tmp
        } else {
            //TODO check if t is a valid node
            //if not give a warning and return (TargetStatus::Offline, "Invalid node")
            Target::create_target(target, TargetStatus::Online, db)
                .await
                .unwrap()
        };
        if t.issues()
            .filter(entities::issue::Column::Status.eq(IssueStatus::Open))
            .filter(entities::issue::Column::ToOffline.eq(ToOffline::Blade))
            .one(db)
            .await
            .unwrap()
            .is_some()
        {
            return (TargetStatus::Offline, "todo".to_string());
        }
    }
    for c in Cluster::siblings(target) {
        let t = entities::target::Entity::from_name(&c, db).await.unwrap();
        if t.issues()
            .filter(entities::issue::Column::Status.eq(IssueStatus::Open))
            .filter(entities::issue::Column::ToOffline.eq(ToOffline::Card))
            .one(db)
            .await
            .unwrap()
            .is_some()
        {
            return (TargetStatus::Offline, "todo".to_string());
        }
    }
    let t = entities::target::Entity::from_name(target, db)
        .await
        .unwrap();
    if t.issues()
        .filter(entities::target::Column::Status.eq(IssueStatus::Open))
        .filter(entities::issue::Column::ToOffline.eq(ToOffline::Node))
        .one(db)
        .await
        .unwrap()
        .is_some()
    {
        return (TargetStatus::Offline, "todo".to_string());
    }
    if t.issues()
        .filter(entities::issue::Column::Status.eq(IssueStatus::Open))
        .one(db)
        .await
        .unwrap()
        .is_some()
    {
        return (TargetStatus::Down, "todo".to_string());
    }
    (TargetStatus::Online, "todo".to_string())
}

pub async fn close_open_issues(target: &str, db: &DatabaseConnection) {
    for issue in entities::target::Entity::from_name(target, db)
        .await
        .unwrap()
        .issues()
        .filter(entities::issue::Column::Status.eq(IssueStatus::Open))
        .all(db)
        .await
        .unwrap()
    {
        let id = issue.id;
        let mut i: entities::issue::ActiveModel = issue.into();
        i.status = ActiveValue::Set(IssueStatus::Closed);
        i.update(db).await.unwrap();
        let c = entities::comment::ActiveModel {
            created_by: ActiveValue::Set("ctt".to_string()),
            comment: ActiveValue::Set("node found up, assuming issue is resolved".to_string()),
            issue_id: ActiveValue::Set(id),
            ..Default::default()
        };
        c.insert(db).await.unwrap();
    }
}

async fn handle_transition(
    target: &str,
    old_state: &TargetStatus,
    new_state: &TargetStatus,
    pbs_srv: &pbs::Server,
    db: &DatabaseConnection,
) {
    let (expected_state, comment) = desired_state(target, db).await;

    //dont use old_state to figure out how to handle nodes
    //things could have changed between when it was collected and now, so only consider
    //the current state (new_state) and the expected_state
    let final_state = match expected_state {
        TargetStatus::Draining => panic!("Expected state is never Draining"),
        TargetStatus::Online => {
            if *new_state == TargetStatus::Online {
                TargetStatus::Online
            } else {
                let new_issue = crate::model::NewIssue::new(
                    None,
                    comment.clone(),
                    comment,
                    target.to_string(),
                    Some(ToOffline::Node),
                );
                new_issue.open("ctt", db).await.unwrap();
                *new_state
            }
        }
        TargetStatus::Offline => match new_state {
            TargetStatus::Draining => TargetStatus::Draining,
            TargetStatus::Offline => TargetStatus::Offline,
            state => {
                pbs_srv.offline_vnode(target, Some(&comment)).unwrap();
                if *state == TargetStatus::Down {
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
                close_open_issues(target, db).await;
                TargetStatus::Online
            }
        },
    };
    //dont update state if it hasn't changed
    if *old_state != final_state {
        let node = entities::target::Entity::from_name(target, db)
            .await
            .unwrap();
        let mut updated_target: entities::target::ActiveModel = node.into();
        updated_target.status = ActiveValue::Set(final_state);
        updated_target.update(db).await.unwrap();
    }
}
