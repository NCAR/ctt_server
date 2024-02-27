#![allow(unused_variables)]
use super::scheduler;
use crate::cluster::ClusterTrait;
use crate::entities::target::TargetStatus;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::mpsc;
use tracing::instrument;
use tracing::warn;

#[derive(Debug)]
pub struct Shasta {
    prefix: String,
}

impl Shasta {
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }
}

impl ClusterTrait for Shasta {
    fn siblings(&self, target: &str) -> Vec<String> {
        //TODO should be "guc" not "gu"
        if let Some(val) = target.strip_prefix(&format!("{}c", self.prefix)) {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = (((num - 1) / 2) * 2) + 1;
            //println!("target: {}, blade_start: {}", num, blade_start);
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                //TODO should be guc
                cousins.push(format!("{}c{:0>4}", self.prefix, i));
            }
            cousins
        } else if let Some(val) = target.strip_prefix(&format!("{}g", self.prefix)) {
            vec![target.to_string()]
        } else if let Some(val) = target.strip_prefix(&self.prefix) {
            // same code as if, special case for gust pre renaming
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = (((num - 1) / 2) * 2) + 1;
            //println!("target: {}, blade_start: {}", num, blade_start);
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                //TODO should be guc
                cousins.push(format!("{}{:0>4}", self.prefix, i));
            }
            cousins
        } else {
            warn!("{} is not a {} node", target, self.prefix);
            vec![target.to_string()]
        }
    }
    fn cousins(&self, target: &str) -> Vec<String> {
        //TODO should be "guc" not "gu"
        if let Some(val) = target.strip_prefix(&format!("{}c", self.prefix)) {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = (((num - 1) / 4) * 4) + 1;
            let mut cousins = Vec::with_capacity(4);
            for i in blade_start..blade_start + 4 {
                //TODO should be guc
                cousins.push(format!("{}c{:0>4}", self.prefix, i));
            }
            cousins
        } else if let Some(val) = target.strip_prefix(&format!("{}g", self.prefix)) {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = (((num - 1) / 2) * 2) + 1;
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                cousins.push(format!("{}g{:0>4}", self.prefix, i));
            }
            cousins
        } else if let Some(val) = target.strip_prefix(&self.prefix) {
            // same code as if, special case for gust pre renaming
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = (((num - 1) / 4) * 4) + 1;
            let mut cousins = Vec::with_capacity(4);
            for i in blade_start..blade_start + 4 {
                //TODO should be guc
                cousins.push(format!("{}{:0>4}", self.prefix, i));
            }
            cousins
        } else {
            warn!("{} is not a {} node", target, self.prefix);
            vec![target.to_string()]
        }
    }
    #[instrument]
    fn real_node(&self, target: &str) -> bool {
        if let Some(val) = target.strip_prefix(&self.prefix) {
            true
        } else if let Some(val) = target.strip_prefix(&format!("{}c", &self.prefix)) {
            true
        } else if let Some(val) = target.strip_prefix(&format!("{}g", &self.prefix)) {
            true
        } else {
            false
        }
    }

    #[instrument(skip(pbs_srv))]
    async fn nodes_status(
        &self,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> HashMap<String, (TargetStatus, String)> {
        scheduler::nodes_status(pbs_srv, tx).await
    }
    async fn release_node(
        &self,
        target: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), ()> {
        scheduler::release_node(target, operator, pbs_srv, tx).await
    }
    async fn offline_node(
        &self,
        target: &str,
        comment: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), ()> {
        scheduler::offline_node(target, comment, operator, pbs_srv, tx).await
    }
}

#[test]
fn siblings() {
    let gust = Shasta::new("gu".to_string());
    let expected = vec![
        vec!["gu0001", "gu0002"],
        vec!["gu0003", "gu0004"],
        vec!["gu0005", "gu0006"],
    ];
    for e in &expected {
        for s in e.iter() {
            let actual = gust.siblings(s);
            println!("expected: {:?} actual: {:?}", &e, &actual);
            assert!(e.eq(&actual));
        }
    }
}

#[test]
fn cousins() {
    let gust = Shasta::new("gu".to_string());
    let expected = vec![
        vec!["gu0001", "gu0002", "gu0003", "gu0004"],
        vec!["gu0005", "gu0006", "gu0007", "gu0008"],
    ];
    for e in &expected {
        for s in e.iter() {
            let actual = gust.cousins(s);
            println!("expected: {:?} actual: {:?}", &e, &actual);
            assert!(e.eq(&actual));
        }
    }
}

#[test]
fn real_node() {
    let gust = Shasta::new("gu".to_string());
    let expected_true = vec!["gu0001", "gu0002", "gu0015", "gu0016", "gu0017", "gu0018"];
    let expected_false = vec!["gu1", "gu0000", "NotANode", "gu-001", "gu0019"];
    for n in &expected_true {
        let actual = gust.real_node(n);
        println!("for {} expected: true, actual: {}", n, actual);
        assert!(actual);
    }
    for n in &expected_false {
        let actual = gust.real_node(n);
        println!("for {} expected: false, actual: {}", n, actual);
        assert!(!actual);
    }
}
