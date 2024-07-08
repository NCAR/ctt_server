use crate::entities::target::TargetStatus;
use std::collections::HashMap;

pub trait SchedulerTrait {
    fn nodes_status(&mut self) -> Result<HashMap<String, (TargetStatus, String)>, String>;
    fn release_node(&mut self, target: &str) -> Result<(), ()>;
    fn offline_node(&mut self, target: &str, comment: &str) -> Result<(), ()>;
}

mod pbs_scheduler;
pub use pbs_scheduler::PbsScheduler;
