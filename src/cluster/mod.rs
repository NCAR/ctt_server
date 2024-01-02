use crate::entities::target::TargetStatus;
use std::collections::HashMap;
use tokio::sync::mpsc;

pub trait ClusterTrait {
    fn siblings(target: &str) -> Vec<String>;
    fn cousins(target: &str) -> Vec<String>;
    fn real_node(target: &str) -> bool;
    async fn nodes_status(
        pbs_srv: &pbs::Server,
        tx: &mpsc::Sender<String>,
    ) -> HashMap<String, (TargetStatus, String)>;
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
