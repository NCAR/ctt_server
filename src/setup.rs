use crate::migrator::Migrator;
use sea_orm::*;
use sea_orm_migration::{MigratorTrait, SchemaManager};
use std::fs::File;

pub async fn setup_and_connect(db_url: &str) -> Result<DatabaseConnection, DbErr> {
    let _ = File::open(db_url).unwrap_or_else(|_| File::create(db_url).unwrap());
    let db = Database::connect(format!("sqlite://{}", db_url))
        .await
        .unwrap();

    let schema_manager = SchemaManager::new(&db);

    if !schema_manager.has_table("issue").await?
        || !schema_manager.has_table("comment").await?
        || !schema_manager.has_table("target").await?
    {
        Migrator::refresh(&db).await?;
    }
    assert!(schema_manager.has_table("issue").await?);
    assert!(schema_manager.has_table("comment").await?);
    assert!(schema_manager.has_table("target").await?);

    Ok(db)
}
