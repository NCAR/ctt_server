use crate::entities::target::TargetStatus;
use std::collections::HashMap;

pub trait ClusterTrait {
    fn siblings(&self, target: &str) -> Vec<String>;
    fn cousins(&self, target: &str) -> Vec<String>;
    fn real_node(&self, target: &str) -> bool;
    async fn nodes_status(
        &self,
        pbs_srv: &pbs::Server,
    ) -> Result<HashMap<String, (TargetStatus, String)>, ()>;
    async fn release_node(&self, target: &str, pbs_srv: &pbs::Server) -> Result<(), ()>;
    async fn offline_node(
        &self,
        target: &str,
        comment: &str,
        pbs_srv: &pbs::Server,
    ) -> Result<(), ()>;
}

mod regex_cluster;
mod scheduler;
pub use regex_cluster::RegexCluster;
