use crate::entities::target::TargetStatus;
use std::collections::HashMap;

pub trait SchedulerTrait {
    fn nodes_status(&self) -> Result<HashMap<String, (TargetStatus, String)>, String>;
    fn release_node(&self, target: &str) -> Result<(), ()>;
    fn offline_node(&self, target: &str, comment: &str) -> Result<(), ()>;
}

mod pbs_scheduler;
pub use pbs_scheduler::PbsScheduler;
