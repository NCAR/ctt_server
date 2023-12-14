use async_graphql::{EmptySubscription, Schema};
mod mutation;
mod query;
pub use mutation::{Mutation, NewIssue};
pub use query::Query;

pub type CttSchema = Schema<query::Query, mutation::Mutation, EmptySubscription>;
