use crate::entities::target::TargetStatus;
use std::collections::HashMap;

pub trait ClusterTrait {
    fn siblings(&self, target: &str) -> Vec<String>;
    fn cousins(&self, target: &str) -> Vec<String>;
    fn real_node(&self, target: &str) -> bool;
    fn nodes_status(&self) -> Result<HashMap<String, (TargetStatus, String)>, String>;
    fn release_node(&self, target: &str) -> Result<(), ()>;
    fn offline_node(&self, target: &str, comment: &str) -> Result<(), ()>;
}

mod regex_cluster;
pub mod scheduler;
pub use regex_cluster::RegexCluster;
