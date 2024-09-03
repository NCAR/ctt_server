use crate::entities::target::TargetStatus;
use std::collections::HashMap;

pub(crate) trait SchedulerTrait:
    SchedulerInnerTrait + std::fmt::Debug + Send + Sync
{
}

pub(crate) trait SchedulerInnerTrait {
    fn nodes_status(&mut self) -> Result<HashMap<String, (TargetStatus, String)>, String>;
    fn release_node(&mut self, target: &str) -> Result<(), String>;
    fn offline_node(&mut self, target: &str, comment: &str) -> Result<(), String>;
}

#[cfg(feature = "pbs")]
mod pbs_scheduler;
mod shell_scheduler;
#[cfg(feature = "pbs")]
pub use pbs_scheduler::PbsScheduler;
pub use shell_scheduler::ShellScheduler;

impl<T: std::fmt::Debug + SchedulerInnerTrait + Send + Sync> SchedulerTrait for T {}
