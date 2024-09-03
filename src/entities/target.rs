use super::issue;
use crate::cluster::ClusterTrait;
use async_graphql::*;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, QueryOrder};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize, SimpleObject)]
#[sea_orm(table_name = "target")]
#[graphql(concrete(name = "Target", params()))]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    #[graphql(skip)]
    pub id: i32,
    pub name: String,
    pub status: TargetStatus,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::issue::Entity")]
    Issue,
}

impl Related<super::issue::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Issue.def()
    }
}

impl Model {
    #[instrument]
    pub fn issues(&self) -> Select<issue::Entity> {
        self.find_related(issue::Entity)
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Entity {
    #[instrument]
    pub fn all() -> Select<Entity> {
        Self::find().order_by_asc(Column::Name)
    }
    #[instrument]
    pub async fn from_name(
        name: &str,
        db: &DatabaseConnection,
        cluster: &dyn ClusterTrait,
    ) -> Option<Model> {
        if !cluster.real_node(name) {
            debug!("request node {} is not real", name);
            return None;
        }
        let target = Self::find().filter(Column::Name.eq(name)).one(db).await;
        if let Err(e) = target {
            warn!("Error getting target {} by name: {}", name, e);
            return None;
        }
        let target = target.unwrap();
        // add the target to the db is its a real node, but hasn't been added yet
        if target.is_none() {
            Self::create_target(name, TargetStatus::Online, db, cluster).await
        } else {
            target
        }
    }

    #[instrument]
    async fn create_target(
        name: &str,
        state: TargetStatus,
        db: &DatabaseConnection,
        cluster: &dyn ClusterTrait,
    ) -> Option<Model> {
        if !cluster.real_node(name) {
            warn!("Tried making target for fake node {}", name);
            return None;
        }
        let max = if let Some(t) = Self::find()
            .order_by_desc(Column::Id)
            .one(db)
            .await
            .unwrap()
        {
            t.id
        } else {
            0
        };
        let new_target = ActiveModel {
            name: ActiveValue::Set(name.to_string()),
            status: ActiveValue::Set(state),
            id: ActiveValue::Set(max + 1),
        };
        info!("Creating target {:?}", new_target);
        Some(new_target.insert(db).await.unwrap())
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
pub enum TargetStatus {
    #[sea_orm(string_value = "Online")]
    Online,
    #[sea_orm(string_value = "Draining")]
    Draining,
    #[sea_orm(string_value = "Offline")]
    Offline,
    #[sea_orm(string_value = "Down")]
    Down,
}

impl TargetStatus {
    pub fn from_str(state: &str) -> Option<Self> {
        match state {
            "Online" => Some(Self::Online),
            "Draining" => Some(Self::Draining),
            "Offline" => Some(Self::Offline),
            "Down" => Some(Self::Down),
            _ => None,
        }
    }
}
