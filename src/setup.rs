use crate::migrator::Migrator;
use sea_orm::*;
use sea_orm_migration::{MigratorTrait, SchemaManager};
use std::{fs::File, time::Duration};

pub async fn setup_and_connect(db_url: &str) -> Result<DatabaseConnection, DbErr> {
    let _ = File::open(db_url).unwrap_or_else(|_| File::create(db_url).unwrap());
    let mut opt: ConnectOptions = ConnectOptions::new(format!("sqlite://{}", db_url));
    opt.max_connections(100)
        .min_connections(0)
        .connect_timeout(Duration::from_secs(100))
        .idle_timeout(Duration::from_secs(10))
        .acquire_timeout(Duration::from_secs(10))
        .max_lifetime(Duration::from_secs(120));
    let db = Database::connect(opt).await.unwrap();

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
