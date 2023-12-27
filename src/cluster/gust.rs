#![allow(unused_variables)]
use crate::cluster::ClusterTrait;
use crate::entities::target::TargetStatus;
use pbs::{Attrl, Op};
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::mpsc;
use tracing::instrument;
use tracing::{info, warn};
pub struct Gust;

impl ClusterTrait for Gust {
    fn siblings(target: &str) -> Vec<String> {
        //TODO should be "guc" not "gu"
        if let Some(val) = target.strip_prefix("gu") {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = (((num - 1) / 2) * 2) + 1;
            //println!("target: {}, blade_start: {}", num, blade_start);
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                //TODO should be guc
                cousins.push(format!("gu{:0>4}", i));
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
        //TODO should be "guc" not "gu"
        if let Some(val) = target.strip_prefix("gu") {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = (((num - 1) / 4) * 4) + 1;
            let mut cousins = Vec::with_capacity(4);
            for i in blade_start..blade_start + 4 {
                //TODO should be guc
                cousins.push(format!("gu{:0>4}", i));
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
    fn real_node(target: &str) -> bool {
        todo!()
    }

    #[instrument(skip(pbs_srv))]
    async fn nodes_status(
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> HashMap<String, TargetStatus> {
        //TODO filter stat attribs (just need hostname, jobs, and state)
        //TODO need to handle err
        //TODO consider calling pbs_srv.stat_vnode from a spawn_blocking task
        //TODO add a timeout
        let mut resp = HashMap::new();
        for n in pbs_srv.stat_vnode(&None, None).unwrap().resources.iter() {
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
                    //TODO FIXME handle err
                    //TODO should we really offline nodes randomly while checking node status?
                    if let Err(e) = pbs_srv.offline_vnode(&name, None) {
                        warn!("Error offlining node {}: {}", name, e);
                    }
                    let comment = if let Some(pbs::Attrl::Value(pbs::Op::Default(c))) =
                        n.attribs().get("comment")
                    {
                        c
                    } else {
                        ""
                    };
                    let _ = tx
                        .send(format!("ctt offlining: {}, {}", name, comment))
                        .await;
                    if jobs {
                        TargetStatus::Draining
                    } else {
                        TargetStatus::Down
                    }
                }
            };
            resp.insert(name, state);
        }
        resp
    }
    async fn release_node(
        target: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), ()> {
        info!("{} resuming node {}", operator, target);
        pbs_srv.clear_vnode(target, Some("")).unwrap();
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
        info!("{} offlining: {}, {}", operator, target, comment);
        pbs_srv.offline_vnode(target, Some(comment)).unwrap();
        let _ = tx
            .send(format!("{} offlining: {}, {}", operator, target, comment))
            .await;
        Ok(())
    }
}

#[test]
fn siblings() {
    let expected = vec![
        vec!["gu0001", "gu0002"],
        vec!["gu0003", "gu0004"],
        vec!["gu0005", "gu0006"],
    ];
    for e in &expected {
        for s in e.iter() {
            let actual = Gust::siblings(s);
            println!("expected: {:?} actual: {:?}", &e, &actual);
            assert!(e.eq(&actual));
        }
    }
}

#[test]
fn cousins() {
    let expected = vec![
        vec!["gu0001", "gu0002", "gu0003", "gu0004"],
        vec!["gu0005", "gu0006", "gu0007", "gu0008"],
    ];
    for e in &expected {
        for s in e.iter() {
            let actual = Gust::cousins(s);
            println!("expected: {:?} actual: {:?}", &e, &actual);
            assert!(e.eq(&actual));
        }
    }
}
