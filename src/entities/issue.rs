use super::{comment, target};
use async_graphql::*;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize, SimpleObject)]
#[sea_orm(table_name = "issue")]
#[graphql(concrete(name = "Issue", params()), complex)]
pub struct Model {
    pub assigned_to: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub created_by: String,
    pub description: String,
    pub to_offline: Option<ToOffline>,
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,
    pub status: IssueStatus,
    #[graphql(skip)]
    pub target_id: i32,
    pub title: String,
}

#[ComplexObject]
impl Model {
    pub async fn comments(&self, ctx: &Context<'_>) -> Vec<comment::Model> {
        let db = ctx.data::<Arc<DatabaseConnection>>().unwrap().as_ref();
        let t = self.find_related(comment::Entity).all(db).await;
        if let Err(e) = t {
            warn!("Error getting target for issue {}: {}", self.id, e);
            vec![]
        } else {
            t.unwrap()
        }
    }
    pub async fn target(&self, ctx: &Context<'_>) -> Option<target::Model> {
        let db = ctx.data::<Arc<DatabaseConnection>>().unwrap().as_ref();
        let t = self.find_related(target::Entity).one(db).await;
        if let Err(e) = t {
            warn!("Error getting target for issue {}: {}", self.id, e);
            None
        } else {
            t.unwrap()
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::comment::Entity")]
    Comment,
    #[sea_orm(
        belongs_to = "super::target::Entity",
        from = "Column::TargetId",
        to = "super::target::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Target,
}

impl Related<super::comment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Comment.def()
    }
}

impl Related<super::target::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Target.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    pub async fn from_id(id: i32, db: &DatabaseConnection) -> Option<Model> {
        let issue = Self::find_by_id(id).one(db).await;
        if let Err(e) = issue {
            warn!("Error getting issue {} by id: {}", id, e);
            None
        } else {
            issue.unwrap()
        }
    }
}

#[derive(
    Copy,
    Debug,
    Clone,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    async_graphql::Enum,
    Serialize,
    Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "status")]
pub enum IssueStatus {
    #[sea_orm(string_value = "Open")]
    Open,
    #[sea_orm(string_value = "Closed")]
    Closed,
}

#[derive(
    Copy,
    Debug,
    Clone,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    async_graphql::Enum,
    Serialize,
    Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "to_offline")]
pub enum ToOffline {
    #[sea_orm(string_value = "Node")]
    Node,
    #[sea_orm(string_value = "Card")]
    Card,
    #[sea_orm(string_value = "Blade")]
    Blade,
}
