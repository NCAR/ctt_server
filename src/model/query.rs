use crate::auth::{Role, RoleChecker};
use crate::entities::issue::{self, IssueStatus};
use crate::entities::prelude::*;
use crate::entities::target;
use async_graphql::{Context, Object};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::sync::Arc;
use tracing::instrument;

#[derive(Debug)]
pub struct Query;

#[Object]
impl Query {
    #[graphql(guard = "RoleChecker::new(Role::Admin).or(RoleChecker::new(Role::Guest))")]
    #[instrument(skip(ctx))]
    async fn issue<'a>(&self, ctx: &Context<'a>, issue: i32) -> Option<issue::Model> {
        let db = ctx.data::<Arc<DatabaseConnection>>().unwrap().as_ref();
        Issue::find_by_id(issue).one(db).await.unwrap()
    }

    #[graphql(guard = "RoleChecker::new(Role::Admin).or(RoleChecker::new(Role::Guest))")]
    #[instrument(skip(ctx))]
    async fn issues<'a>(
        &self,
        ctx: &Context<'a>,
        issue_status: Option<issue::IssueStatus>,
        target: Option<String>,
    ) -> Vec<issue::Model> {
        let db = ctx.data::<Arc<DatabaseConnection>>().unwrap().as_ref();
        let mut select = target::Entity::find().find_with_related(issue::Entity);
        if let Some(status) = issue_status {
            select =
                select.filter(<issue::Entity as sea_orm::EntityTrait>::Column::Status.eq(status));
        } else {
            select = select.filter(
                <issue::Entity as sea_orm::EntityTrait>::Column::Status.ne(IssueStatus::Closed),
            );
        }
        if let Some(t) = target {
            select = select.filter(<target::Entity as sea_orm::EntityTrait>::Column::Name.eq(t));
        }
        select
            .order_by_asc(crate::entities::target::Column::Name)
            .all(db)
            .await
            .unwrap()
            .into_iter()
            .map(|(_, i)| i)
            .reduce(|mut acc, mut c| {
                acc.append(&mut c);
                acc
            })
            .unwrap_or(vec![])
    }
}
