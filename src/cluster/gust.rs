#![allow(unused_variables)]
use crate::cluster::ClusterTrait;
use std::str::FromStr;
pub struct Gust;

impl ClusterTrait for Gust {
    fn siblings(target: &str) -> Vec<String> {
        if let Some(val) = target.strip_prefix("guc") {
            let num: u32 = FromStr::from_str(val).unwrap();
            let blade_start = ((num / 2) * 2) + 1;
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                cousins.push(format!("guc{:0>4}", i));
            }
            cousins
        } else if let Some(val) = target.strip_prefix("gug") {
            vec![target.to_string()]
        } else {
            panic!("{} is not a gust node", target);
        }
    }
    fn cousins(target: &str) -> Vec<String> {
        if let Some(val) = target.strip_prefix("guc") {
            let num: u32 = FromStr::from_str(val).unwrap();
            let blade_start = ((num / 4) * 4) + 1;
            let mut cousins = Vec::with_capacity(4);
            for i in blade_start..blade_start + 4 {
                cousins.push(format!("guc{:0>4}", i));
            }
            cousins
        } else if let Some(val) = target.strip_prefix("gug") {
            let num: u32 = FromStr::from_str(val).unwrap();
            let blade_start = ((num / 2) * 2) + 1;
            let mut cousins = Vec::with_capacity(2);
            for i in blade_start..blade_start + 2 {
                cousins.push(format!("guc{:0>4}", i));
            }
            cousins
        } else {
            panic!("{} is not a gust node", target);
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
