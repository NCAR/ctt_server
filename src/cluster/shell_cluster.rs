#![allow(unused_variables)]
use super::scheduler::SchedulerTrait;
use crate::cluster::ClusterInnerTrait;
use crate::conf::ShellClusterConf;
use crate::entities::target::TargetStatus;
use std::collections::HashMap;
use std::process::Command;
use std::str;
use tracing::instrument;
use tracing::warn;

#[derive(Debug)]
pub struct ShellCluster {
    sched: Box<dyn SchedulerTrait + Send + Sync>,
    siblings_cmd: String,
    cousins_cmd: String,
    real_node_cmd: String,
}

impl ShellCluster {
    //TODO have sched be of type SchedulerTrait instead
    pub fn new(conf: ShellClusterConf, sched: Box<dyn SchedulerTrait + Send + Sync>) -> Self {
        Self {
            sched,
            siblings_cmd: conf.siblings_cmd,
            cousins_cmd: conf.cousins_cmd,
            real_node_cmd: conf.real_node_cmd,
        }
    }
}

fn vecu8_to_string(input: &[u8]) -> Result<String, String> {
    match str::from_utf8(input) {
        Ok(s) => Ok(s.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn cmd_to_string(mut c: Command) -> Result<String, String> {
    let resp = c.output();
    if let Err(e) = resp {
        return Err(e.to_string());
    }
    let resp = resp.unwrap();
    if resp.status.success() {
        vecu8_to_string(&resp.stdout)
    } else {
        vecu8_to_string(&resp.stderr)
    }
}

impl ClusterInnerTrait for ShellCluster {
    #[instrument]
    fn siblings(&self, target: &str) -> Vec<String> {
        match cmd_to_string(Command::new(self.siblings_cmd.clone())) {
            Ok(s) => {
                let obj = serde_json::from_str(&s);
                if let Err(e) = obj {
                    vec![]
                } else {
                    obj.unwrap()
                }
            }
            Err(e) => vec![],
        }
    }
    #[instrument]
    fn cousins(&self, target: &str) -> Vec<String> {
        match cmd_to_string(Command::new(self.cousins_cmd.clone())) {
            Ok(s) => {
                let obj = serde_json::from_str(&s);
                if let Err(e) = obj {
                    vec![]
                } else {
                    obj.unwrap()
                }
            }
            Err(e) => vec![],
        }
    }

    #[instrument]
    fn real_node(&self, target: &str) -> bool {
        match cmd_to_string(Command::new(self.real_node_cmd.clone())) {
            Ok(s) => {
                let obj = serde_json::from_str(&s);
                if let Err(e) = obj {
                    false
                } else {
                    obj.unwrap()
                }
            }
            Err(e) => false,
        }
    }

    #[instrument]
    fn nodes_status(&mut self) -> Result<HashMap<String, (TargetStatus, String)>, String> {
        self.sched.nodes_status()
    }
    #[instrument]
    fn release_node(&mut self, target: &str) -> Result<(), String> {
        self.sched.release_node(target)
    }
    #[instrument]
    fn offline_node(&mut self, target: &str, comment: &str) -> Result<(), String> {
        self.sched.offline_node(target, comment)
    }
}
