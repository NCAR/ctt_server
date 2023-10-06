use async_graphql::{EmptySubscription, Schema};
mod query;
mod mutation;
pub use mutation::Mutation;
pub use query::Query;

pub type CttSchema = Schema<query::Query, mutation::Mutation, EmptySubscription>;
