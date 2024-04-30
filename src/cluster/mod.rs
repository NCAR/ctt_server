use crate::{entities::target::TargetStatus, ChangeLogMsg};
use std::collections::HashMap;
use tokio::sync::mpsc;

pub trait ClusterTrait {
    fn siblings(&self, target: &str) -> Vec<String>;
    fn cousins(&self, target: &str) -> Vec<String>;
    fn real_node(&self, target: &str) -> bool;
    async fn nodes_status(
        &self,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<ChangeLogMsg>,
    ) -> Result<HashMap<String, (TargetStatus, String)>, ()>;
    async fn release_node(
        &self,
        target: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<ChangeLogMsg>,
    ) -> Result<(), ()>;
    async fn offline_node(
        &self,
        target: &str,
        comment: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<ChangeLogMsg>,
    ) -> Result<(), ()>;
}

mod regex_cluster;
mod scheduler;
pub use regex_cluster::RegexCluster;
