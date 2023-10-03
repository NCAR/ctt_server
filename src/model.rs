use async_graphql::{EmptySubscription, Schema, SimpleObject};
use serde::{Deserialize, Serialize};
mod query;
mod mutation;
pub use mutation::Mutation;
pub use query::Query;

pub type CttSchema = Schema<query::Query, mutation::Mutation, EmptySubscription>;
