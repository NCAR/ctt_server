use crate::entities::target::TargetStatus;
use std::collections::HashMap;
use tokio::sync::mpsc;

pub trait ClusterTrait {
    fn siblings(target: &str) -> Vec<String>;
    fn cousins(target: &str) -> Vec<String>;
    fn logical_to_physical(targets: Vec<&str>) -> Vec<String>;
    fn physical_to_logical(targets: Vec<&str>) -> Vec<String>;
    fn all_nodes() -> Vec<String>;
    fn real_node(target: &str) -> bool;
    fn node_status(pbs_srv: &pbs::Server, target: &str) -> TargetStatus;
    fn nodes_status(pbs_srv: &pbs::Server) -> HashMap<String, TargetStatus>;
    async fn release_node(
        target: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), ()>;
    async fn offline_node(
        target: &str,
        comment: &str,
        operator: &str,
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> Result<(), ()>;
}

mod gust;
pub use gust::Gust;
