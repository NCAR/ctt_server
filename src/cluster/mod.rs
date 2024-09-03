use crate::conf::{Cluster, Scheduler};
use crate::entities::target::TargetStatus;
#[cfg(feature = "pbs")]
use scheduler::PbsScheduler;
use std::collections::HashMap;
mod regex_cluster;
mod shell_cluster;
pub use regex_cluster::RegexCluster;
pub use shell_cluster::ShellCluster;
pub mod scheduler;

pub trait ClusterInnerTrait {
    fn siblings(&self, target: &str) -> Vec<String>;
    fn cousins(&self, target: &str) -> Vec<String>;
    fn real_node(&self, target: &str) -> bool;
    fn nodes_status(&mut self) -> Result<HashMap<String, (TargetStatus, String)>, String>;
    fn release_node(&mut self, target: &str) -> Result<(), String>;
    fn offline_node(&mut self, target: &str, comment: &str) -> Result<(), String>;
}

pub trait ClusterTrait: ClusterInnerTrait + std::fmt::Debug + Send + Sync {}

impl<T: std::fmt::Debug + ClusterInnerTrait + Send + Sync> ClusterTrait for T {}

pub fn new(c: Cluster, s: Scheduler) -> Box<dyn ClusterTrait> {
    let sched: Box<dyn scheduler::SchedulerTrait + Send + Sync> = match s {
        #[cfg(feature = "pbs")]
        Scheduler::Pbs => Box::new(PbsScheduler::new()),
        #[cfg(not(feature = "pbs"))]
        Scheduler::Pbs => panic!("pbs feature not enabled!"),
        Scheduler::Shell(sh) => Box::new(sh),
    };

    let cluster: Box<dyn ClusterTrait> = match c {
        Cluster::Regex(node_types) => Box::new(RegexCluster::new(node_types, sched)),
        Cluster::Shell(conf) => Box::new(ShellCluster::new(conf, sched)),
    };
    cluster
}
