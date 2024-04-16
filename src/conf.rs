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
    pub db: String,
    pub certs_dir: String,
    pub server_addr: String,
    pub node_types: Vec<NodeType>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Slack {
    pub channel: String,
    pub token: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeType {
    pub prefix: String,
    pub digits: Option<usize>,
    pub board: Option<u32>,
    pub first_num: Option<u32>,
    pub last_num: Option<u32>,
    pub slot: Option<u32>,
}
