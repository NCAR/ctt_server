use sea_orm::{DatabaseConnection, ColumnTrait, EntityTrait, QueryFilter};
use crate::auth::{Role, RoleChecker};
use async_graphql::{ComplexObject, Context, Object};
use crate::entities::prelude::*;
use crate::entities::issue;
use crate::entities::target;
use crate::entities::comment;

#[ComplexObject]
impl issue::Model {
    async fn comments(&self, ctx: &Context<'_>) -> Vec<comment::Model> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        Comment::for_issue(self.id).all(db).await.unwrap()
    }
    async fn target(&self, ctx: &Context<'_>) -> Option<target::Model> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        Target::find_by_id(self.target_id).one(db).await.unwrap()
    }
}


pub struct Query;

#[Object]
impl Query {
    #[graphql(guard = "RoleChecker::new(Role::Admin).or(RoleChecker::new(Role::Guest))")]
    async fn issue<'a>(&self, ctx: &Context<'a>, issue: i32) -> Option<issue::Model> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        Issue::find_by_id(issue)
            .one(db).await.unwrap()
    }

    #[graphql(guard = "RoleChecker::new(Role::Admin).or(RoleChecker::new(Role::Guest))")]
    async fn issues<'a>(
        &self,
        ctx: &Context<'a>,
        issue_status: Option<issue::IssueStatus>,
        target: Option<String>,
    ) -> Vec<issue::Model> {
        let db = ctx.data::<DatabaseConnection>().unwrap();
        let mut select = Issue::find();
        if let Some(status) =  issue_status {
            select = select.filter(<issue::Entity as sea_orm::EntityTrait>::Column::IssueStatus.eq(status));
        }
        if let Some(t) = target {
            select = select.filter(<issue::Entity as sea_orm::EntityTrait>::Column::TargetId.eq(Target::find_by_name(&t).one(db).await.unwrap().unwrap().id));
        }
        select.all(db).await.unwrap()
    }
}
