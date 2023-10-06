pub trait ClusterTrait {
    fn siblings(target: &str) -> Vec<String>;
    fn cousins(target: &str) -> Vec<String>;
    fn logical_to_physical(targets: Vec<&str>) -> Vec<String>;
    fn physical_to_logical(targets: Vec<&str>) -> Vec<String>;
    fn all_nodes() -> Vec<String>;
    fn real_node(target: &str) -> bool;
}

mod gust;
pub use gust::Gust;
