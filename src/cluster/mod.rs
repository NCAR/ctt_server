use crate::entities::target::TargetStatus;
use std::collections::HashMap;
use tokio::sync::mpsc;

pub trait ClusterTrait {
    fn siblings(&self, target: &str) -> Vec<String>;
    fn cousins(&self, target: &str) -> Vec<String>;
    fn real_node(&self, target: &str) -> bool;
    async fn nodes_status(
        &self,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<HashMap<String, (TargetStatus, String)>, ()>;
    async fn release_node(
        &self,
        target: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), ()>;
    async fn offline_node(
        &self,
        target: &str,
        comment: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), ()>;
}

mod scheduler;
mod shasta;
pub use shasta::Shasta;
