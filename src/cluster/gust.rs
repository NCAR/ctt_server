#![allow(unused_variables)]
use crate::cluster::ClusterTrait;
use crate::entities::target::TargetStatus;
use pbs::{Attrl, Op};
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::mpsc;
use tracing::log::warn;
pub struct Gust;

impl ClusterTrait for Gust {
    fn siblings(target: &str) -> Vec<String> {
        if let Some(val) = target.strip_prefix("guc") {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = ((num / 2) * 2) + 1;
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                cousins.push(format!("guc{:0>4}", i));
            }
            cousins
        } else if let Some(val) = target.strip_prefix("gug") {
            vec![target.to_string()]
        } else {
            warn!("{} is not a gust node", target);
            vec![target.to_string()]
        }
    }
    fn cousins(target: &str) -> Vec<String> {
        if let Some(val) = target.strip_prefix("guc") {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = ((num / 4) * 4) + 1;
            let mut cousins = Vec::with_capacity(4);
            for i in blade_start..blade_start + 4 {
                cousins.push(format!("guc{:0>4}", i));
            }
            cousins
        } else if let Some(val) = target.strip_prefix("gug") {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = ((num / 2) * 2) + 1;
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                cousins.push(format!("guc{:0>4}", i));
            }
            cousins
        } else {
            warn!("{} is not a gust node", target);
            vec![target.to_string()]
        }
    }
    fn logical_to_physical(targets: Vec<&str>) -> Vec<String> {
        todo!()
    }
    fn physical_to_logical(targets: Vec<&str>) -> Vec<String> {
        todo!()
    }
    fn all_nodes() -> Vec<String> {
        todo!()
    }
    fn real_node(target: &str) -> bool {
        todo!()
    }
    fn node_status(pbs_srv: &pbs::Server, target: &str) -> TargetStatus {
        todo!()
    }
    fn nodes_status(pbs_srv: &pbs::Server) -> HashMap<String, TargetStatus> {
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
                        //TODO FIXME handle err
                        //TODO should we really offline nodes randomly while checking node status?
                        if let Err(e) = pbs_srv.offline_vnode(&name, None) {
                            warn!("Error offlining node {}: {}", name, e);
                        }
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
    async fn release_node(
        target: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), ()> {
        pbs_srv.clear_vnode(target, None).unwrap();
        let _ = tx
            .send(format!("{} onlining node: {}", operator, target))
            .await;
        Ok(())
    }
    async fn offline_node(
        target: &str,
        comment: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), ()> {
        pbs_srv.offline_vnode(target, Some(comment)).unwrap();
        let _ = tx
            .send(format!(
                "{} {} found online, offlining: {}",
                operator, target, comment
            ))
            .await;
        Ok(())
    }
}
