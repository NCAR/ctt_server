#![allow(unused_variables)]
use super::scheduler::{PbsScheduler, SchedulerTrait};
use crate::cluster::ClusterTrait;
use crate::conf::NodeType;
use crate::entities::target::TargetStatus;
use regex::Regex;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::instrument;
use tracing::warn;

#[derive(Debug)]
pub struct RegexCluster {
    node_types: Vec<NodeType>,
    //TODO have sched be of type SchedulerTrait instead
    sched: PbsScheduler,
}

impl RegexCluster {
    #[instrument]
    //TODO have sched be of type SchedulerTrait instead
    pub fn new(node_types: Vec<NodeType>, sched: PbsScheduler) -> Self {
        Self { sched, node_types }
    }

    #[instrument]
    fn get_node_type(&self, target: &str) -> Option<NodeType> {
        for ntype in self.node_types.clone() {
            //let re = Regex::new(&ntype.names).unwrap();
            let re = if let Some(digits) = ntype.digits {
                Regex::new(&format!(r"^{}\d{{{}}}$", ntype.prefix, digits)).unwrap()
            } else {
                Regex::new(&format!(r"^{}\d+$", ntype.prefix)).unwrap()
            };
            if re.is_match(target) {
                let val = target.strip_prefix(&ntype.prefix).unwrap();
                let num: u32 = FromStr::from_str(val).unwrap();
                if ntype.first_num.unwrap_or(1) <= num {
                    if let Some(last) = ntype.last_num {
                        if num <= last {
                            return Some(ntype);
                        }
                    } else {
                        return Some(ntype);
                    }
                }
            }
        }
        None
    }
    #[instrument]
    fn get_related(&self, target: &str, nodetype: NodeType, size: u32) -> Vec<String> {
        if size > 1 {
            let val = target.strip_prefix(&nodetype.prefix).unwrap();
            let num: u32 = FromStr::from_str(val).unwrap();
            let start = (((num - 1) / size) * size) + 1;
            let mut related = Vec::with_capacity(size.try_into().unwrap());
            for i in start..start + size {
                if let Some(digits) = nodetype.digits {
                    related.push(format!("{}{:0>width$}", nodetype.prefix, i, width = digits));
                } else {
                    related.push(format!("{}{}", nodetype.prefix, i,));
                }
            }
            related
        } else {
            vec![target.to_string()]
        }
    }
}

impl ClusterTrait for RegexCluster {
    #[instrument]
    fn siblings(&self, target: &str) -> Vec<String> {
        if let Some(nodetype) = self.get_node_type(target) {
            self.get_related(target, nodetype.clone(), nodetype.board.unwrap_or(1))
        } else {
            //TODO return None instead
            vec![]
        }
    }
    #[instrument]
    fn cousins(&self, target: &str) -> Vec<String> {
        if let Some(nodetype) = self.get_node_type(target) {
            self.get_related(
                target,
                nodetype.clone(),
                nodetype.slot.unwrap_or(nodetype.board.unwrap_or(1)),
            )
        } else {
            //TODO return None instead
            vec![]
        }
    }
    #[instrument]
    fn real_node(&self, target: &str) -> bool {
        self.get_node_type(target).is_some()
    }

    #[instrument]
    fn nodes_status(&self) -> Result<HashMap<String, (TargetStatus, String)>, String> {
        self.sched.nodes_status()
    }
    #[instrument]
    fn release_node(&self, target: &str) -> Result<(), ()> {
        self.sched.release_node(target)
    }
    #[instrument]
    fn offline_node(&self, target: &str, comment: &str) -> Result<(), ()> {
        self.sched.offline_node(target, comment)
    }
}

/*
// TODO FIXME fix tests need to mock out PbsScheduler
#[test]
fn siblings() {
    let gust = RegexCluster::new(vec![NodeType {
        prefix: "gu".to_string(),
        digits: Some(4),
        first_num: None,
        last_num: Some(18),
        board: Some(2),
        slot: Some(4),
    }]);
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
    let gust = RegexCluster::new(vec![NodeType {
        prefix: "gu".to_string(),
        digits: Some(4),
        first_num: None,
        last_num: Some(18),
        board: Some(2),
        slot: Some(4),
    }]);
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
    let gust = RegexCluster::new(vec![NodeType {
        prefix: "gu".to_string(),
        digits: Some(4),
        first_num: None,
        last_num: Some(18),
        board: Some(2),
        slot: Some(4),
    }]);
    let expected_true = vec!["gu0001", "gu0002", "gu0015", "gu0016", "gu0017", "gu0018"];
    let expected_false = vec!["gu1", "gu0000", "NotANode", "gu-001", "gu0019", "gu00017"];
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
*/
