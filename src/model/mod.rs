use async_graphql::{EmptySubscription, Schema};
pub mod mutation;
mod query;
pub use mutation::{Mutation, NewIssue};
pub use query::Query;

pub type CttSchema = Schema<query::Query, mutation::Mutation, EmptySubscription>;
