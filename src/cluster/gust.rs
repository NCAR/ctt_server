#![allow(unused_variables)]
use crate::cluster::ClusterTrait;
pub struct Gust;

impl ClusterTrait for Gust {
    fn siblings(target: &str) -> Vec<String> {
        todo!()
    }
    fn cousins(target: &str) -> Vec<String> {
        todo!()
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
