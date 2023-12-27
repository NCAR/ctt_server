use crate::migrator::Migrator;
use sea_orm::*;
use sea_orm_migration::{MigratorTrait, SchemaManager};

const DATABASE_URL: &str = "sqlite:///root/shanks/ctt/db.sqlite";
//const DB_NAME: &str = "mydb";

pub async fn setup_and_connect() -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect(DATABASE_URL).await.unwrap();
    let db = match db.get_database_backend() {
        /*
        DbBackend::MySql => {
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("CREATE DATABASE IF NOT EXISTS `{}`;", DB_NAME),
            ))
            .await?;

            let url = format!("{}/{}", DATABASE_URL, DB_NAME);
            Database::connect(&url).await?
        }
        DbBackend::Postgres => {
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("DROP DATABASE IF EXISTS \"{}\";", DB_NAME),
            ))
            .await?;
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("CREATE DATABASE \"{}\";", DB_NAME),
            ))
            .await?;

            let url = format!("{}/{}", DATABASE_URL, DB_NAME);
            Database::connect(&url).await?
        }
        */
        DbBackend::Sqlite => db,
        _ => panic!("only sqlite implemented"),
    };

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
