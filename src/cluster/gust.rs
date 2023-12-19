#![allow(unused_variables)]
use crate::cluster::ClusterTrait;
use std::str::FromStr;
use tracing::log::warn;
pub struct Gust;

impl ClusterTrait for Gust {
    fn siblings(target: &str) -> Vec<String> {
        if let Some(val) = target.strip_prefix("guc") {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = ((num / 2) * 2) + 1;
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                cousins.push(format!("guc{:0>4}", i));
            }
            cousins
        } else if let Some(val) = target.strip_prefix("gug") {
            vec![target.to_string()]
        } else {
            warn!("{} is not a gust node", target);
            vec![target.to_string()]
        }
    }
    fn cousins(target: &str) -> Vec<String> {
        if let Some(val) = target.strip_prefix("guc") {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = ((num / 4) * 4) + 1;
            let mut cousins = Vec::with_capacity(4);
            for i in blade_start..blade_start + 4 {
                cousins.push(format!("guc{:0>4}", i));
            }
            cousins
        } else if let Some(val) = target.strip_prefix("gug") {
            let num: u32 = FromStr::from_str(val).unwrap();
            //TODO add sanity check, only 18ish nodes in gust
            let blade_start = ((num / 2) * 2) + 1;
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                cousins.push(format!("guc{:0>4}", i));
            }
            cousins
        } else {
            warn!("{} is not a gust node", target);
            vec![target.to_string()]
        }
    }
    fn logical_to_physical(targets: Vec<&str>) -> Vec<String> {
        todo!()
    }
    fn physical_to_logical(targets: Vec<&str>) -> Vec<String> {
        todo!()
    }
    fn all_nodes() -> Vec<String> {
        todo!()
    }
    fn real_node(target: &str) -> bool {
        todo!()
    }
}
