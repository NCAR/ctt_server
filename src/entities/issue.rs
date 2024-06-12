use super::{comment, target};
use crate::cluster::ClusterTrait;
use crate::cluster::RegexCluster;
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
    pub async fn related(&self, ctx: &Context<'_>) -> Vec<target::Model> {
        let db = ctx.data::<Arc<DatabaseConnection>>().unwrap().as_ref();
        let cluster = ctx.data::<RegexCluster>().unwrap();
        let mut related: Vec<target::Model> = vec![];
        let tar = self.target(ctx).await;
        if let Err(e) = tar {
            warn!("Error getting target for issue {}: {:?}", self.id, e);
            return related;
        };
        let tar = tar.unwrap().unwrap();
        match self.to_offline {
            Some(ToOffline::Card) => {
                for t in cluster.siblings(&tar.name) {
                    if let Some(tmp) = target::Entity::from_name(&t, db, cluster).await {
                        related.push(tmp);
                    }
                }
            }
            Some(ToOffline::Blade) => {
                for t in cluster.cousins(&tar.name) {
                    if let Some(tmp) = target::Entity::from_name(&t, db, cluster).await {
                        related.push(tmp);
                    }
                }
            }
            _ => {
                //target is related if ToOffline is Node or None
                related.push(tar)
            }
        }
        related
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

impl Entity {}

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
