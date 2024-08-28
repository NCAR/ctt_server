use crate::entities::target::TargetStatus;
use core::str;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::process::Command;
use tracing::instrument;
use tracing::{info, warn};

use super::SchedulerInnerTrait;

/*impl fmt::Debug for ShellScheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShellScheduler").finish()
    }
}*/

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ShellScheduler {
    pub status_cmd: String,
    pub release_cmd: String,
    pub offline_cmd: String,
}

fn vecu8_to_string(input: &[u8]) -> Result<String, String> {
    match str::from_utf8(input) {
        Ok(s) => Ok(s.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

fn cmd_to_string(mut c: Command) -> Result<String, String> {
    let resp = c.output();
    if let Err(e) = resp {
        return Err(e.to_string());
    }
    let resp = resp.unwrap();
    if resp.status.success() {
        vecu8_to_string(&resp.stdout)
    } else {
        vecu8_to_string(&resp.stderr)
    }
}

impl SchedulerInnerTrait for ShellScheduler {
    #[instrument]
    fn nodes_status(&mut self) -> Result<HashMap<String, (TargetStatus, String)>, String> {
        match cmd_to_string(Command::new(self.status_cmd.clone())) {
            Ok(s) => {
                let obj = serde_json::from_str(&s);
                if let Err(e) = obj {
                    Err(e.to_string())
                } else {
                    Ok(obj.unwrap())
                }
            }
            Err(e) => Err(e),
        }
    }

    #[instrument]
    fn release_node(&mut self, target: &str) -> Result<(), String> {
        info!("resuming node {}", target);
        match cmd_to_string(Command::new(self.release_cmd.clone())) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    #[instrument]
    fn offline_node(&mut self, target: &str, comment: &str) -> Result<(), String> {
        info!("offlining: {}, {}", target, comment);
        match cmd_to_string(Command::new(self.offline_cmd.clone())) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}
