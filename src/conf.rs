use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
const DEFAULT_CONF_FILE: &str = "/opt/ncar/etc/ctt.yml";

pub fn get_config(path: Option<String>) -> Result<Conf, ConfigError> {
    let mut conf = Config::builder();
    //TODO read from default source only if it exists, warn otherwise
    //.add_source(File::with_name(DEFAULT_CONF_FILE));
    if let Some(p) = path {
        conf = conf.add_source(File::with_name(&p));
    }
    let conf = conf.build()?;
    conf.try_deserialize()
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Conf {
    pub poll_interval: u64,
    pub slack: Slack,
    pub cluster: Cluster,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Slack {
    pub channel: String,
    pub token: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Cluster {
    pub prefix: String,
}
