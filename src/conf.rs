use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};

pub fn get_config(path: Option<String>) -> Result<Conf, ConfigError> {
    let mut conf = Config::builder();
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
    pub auth: Auth,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Auth {
    pub admin: Vec<String>,
    pub guest: Vec<String>,
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
