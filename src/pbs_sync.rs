use crate::entities;
use crate::entities::target::TargetStatus;
use sea_orm::QuerySelect;
use std::collections::HashMap;

use pbs::{Attrl, Op};
use sea_orm::DatabaseConnection;
use std::time::Duration;
use tokio::time;
#[allow(unused_imports)]
use tracing::{info, warn, Level};

pub async fn pbs_sync(db: DatabaseConnection) {
    //TODO refactor this method into smaller ones
    //TODO add some yields into this method so it doesn't block too long
    //TODO get interval from config file
    let mut interval = time::interval(Duration::from_secs(30));
    // don't let ticks stack up if a sync takes longer than interval
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    loop {
        let pbs_srv = pbs::Server::new();
        //TODO filter stat attribs (just need hostname, jobs, and state)
        //TODO need to handle err
        //TODO consider calling pbs_srv.stat_vnode from a spawn_blocking task
        let pbs_node_state: HashMap<String, TargetStatus> = pbs_srv
            .stat_vnode(&None, None)
            .unwrap()
            .resources
            .iter()
            .map(|n| {
                let name = n.name();
                let jobs = match n.attribs().get("jobs").unwrap() {
                    Attrl::Value(Op::Default(j)) => j.is_empty(),
                    x => {
                        println!("{:?}", x);
                        panic!("bad job list");
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
                    "down" => {
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
                        if jobs {
                            TargetStatus::Draining
                        } else {
                            TargetStatus::Down
                        }
                    }
                };
                (name, state)
            })
            .collect();
        let ctt_node_state = entities::target::Entity::all()
            .select_only()
            .columns([
                entities::target::Column::Name,
                entities::target::Column::Status,
            ])
            .all(&db)
            .await
            .unwrap();
        let mut ctt_node_state: HashMap<String, TargetStatus> = ctt_node_state
            .iter()
            .map(|n| (n.name.clone(), n.status))
            .collect();
        // sync ctt and pbs

        //handle any pbs nodes not in ctt
        for target in pbs_node_state.keys() {
            if !ctt_node_state.contains_key(target) {
                // TODO create target in ctt db with state TargetStatus::Online
                ctt_node_state.insert(target.to_string(), TargetStatus::Online);
            }
        }

        for (target, old_state) in &ctt_node_state {
            let pbs_state = pbs_node_state.get(target);
            if let Some(new_state) = pbs_state {
                match old_state {
                    TargetStatus::Draining => {
                        match new_state {
                            TargetStatus::Draining => continue,
                            TargetStatus::Down => {
                                todo!();
                            },
                            TargetStatus::Offline => {
                                todo!();
                            },
                            TargetStatus::Online => {
                                todo!();
                            },
                        }
                    }
                    TargetStatus::Down => {
                        match new_state {
                            TargetStatus::Down => continue,
                            TargetStatus::Draining => {
                                todo!();
                            },
                            TargetStatus::Offline => {
                                todo!();
                            },
                            TargetStatus::Online => {
                                todo!();
                            },
                        }
                    }
                    TargetStatus::Offline => {
                        match new_state {
                            TargetStatus::Offline => continue,
                            TargetStatus::Draining => {
                                todo!();
                            },
                            TargetStatus::Down => {
                                todo!();
                            },
                            TargetStatus::Online => {
                                todo!();
                            },
                        }
                    }
                    TargetStatus::Online => {
                        match new_state {
                            TargetStatus::Online => continue,
                            TargetStatus::Draining => {
                                todo!();
                            },
                            TargetStatus::Down => {
                                todo!();
                            },
                            TargetStatus::Offline => {
                                todo!();
                            },
                        }
                    }
                }
                // TODO update node state
                //   be careful of enforce_down edge case
                //   since pbs_node_state won't be correct
            } else {
                warn!("{} not found in pbs", target);
                // TODO open issue, set target state to TargetStatus::Unknown
            }
        }
        interval.tick().await;
    }
}
