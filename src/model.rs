use async_graphql::{EmptySubscription, Schema, SimpleObject};
use serde::{Deserialize, Serialize};
mod query;
use query::IssueStatus;
mod mutation;
pub use mutation::Mutation;
pub use query::Query;

pub type CttSchema = Schema<query::Query, mutation::Mutation, EmptySubscription>;

#[derive(Serialize, Deserialize, SimpleObject)]
#[graphql(complex)]
pub struct Issue {
    assigned_to: String,
    description: String,
    down_siblings: bool,
    enforce_down: bool,
    id: u32,
    issue_status: IssueStatus,
    target: String,
    title: String,
}
