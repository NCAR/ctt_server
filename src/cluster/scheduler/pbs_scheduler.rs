use crate::entities::target::TargetStatus;
use core::fmt;
use pbs::{Attrl, Op, Server};
use std::collections::HashMap;
use tracing::instrument;
use tracing::{info, warn};

use super::SchedulerTrait;

pub struct PbsScheduler {
    srv: Server,
}

impl PbsScheduler {
    pub fn new(srv: Server) -> Self {
        Self { srv }
    }

    pub fn refresh_conn(&mut self) {
        self.srv = Server::new()
    }
}

impl fmt::Debug for PbsScheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PbsScheduler").finish()
    }
}

impl SchedulerTrait for PbsScheduler {
    #[instrument]
    fn nodes_status(&self) -> Result<HashMap<String, (TargetStatus, String)>, String> {
        //TODO filter stat attribs (just need hostname, jobs, and state)
        //TODO consider calling pbs_srv.stat_vnode from a spawn_blocking task
        //TODO add a timeout
        let mut resp = HashMap::new();
        let vnode_stat = self.srv.stat_vnode(&None, None);
        if let Err(e) = vnode_stat {
            warn!("error statting vnode: {}", e);
            return Err(e);
        }
        for n in vnode_stat.unwrap().resources.iter() {
            let name = n.name();
            let jobs = {
                if let Some(Attrl::Value(Op::Default(j))) = n.attribs().get("jobs") {
                    !j.is_empty()
                } else {
                    false
                }
            };
            #[allow(clippy::manual_unwrap_or_default)]
            let comment =
                if let Some(pbs::Attrl::Value(pbs::Op::Default(c))) = n.attribs().get("comment") {
                    c
                } else {
                    ""
                };
            // vnodes always have a state attrib so the unwrap is safe
            let state = match n.attribs().get("state").unwrap() {
                Attrl::Value(Op::Default(j)) => j,
                x => {
                    println!("{:?}", x);
                    panic!("bad state");
                }
            };
            let state = match state.as_str() {
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
                //job-excl or resv-excl
                x if x.contains("exclusive") => TargetStatus::Online,
                "job-busy" => TargetStatus::Online,
                "free" => TargetStatus::Online,
                x => {
                    warn!("unrecognized node state, '{}'", x);
                    //TODO should we really offline nodes randomly while checking node status?
                    if let Err(e) = self.srv.offline_vnode(&name, None) {
                        warn!("Error offlining node {}: {}", name, e);
                    }
                    if jobs {
                        TargetStatus::Draining
                    } else {
                        TargetStatus::Down
                    }
                }
            };
            resp.insert(name, (state, comment.to_string()));
        }
        Ok(resp)
    }

    #[instrument]
    fn release_node(&self, target: &str) -> Result<(), ()> {
        info!("resuming node {}", target);
        if self.srv.clear_vnode(target, Some("")).is_err() {
            return Err(());
        }
        Ok(())
    }

    #[instrument]
    fn offline_node(&self, target: &str, comment: &str) -> Result<(), ()> {
        info!("offlining: {}, {}", target, comment);
        if self.srv.offline_vnode(target, Some(comment)).is_err() {
            return Err(());
        }
        Ok(())
    }
}
