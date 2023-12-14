use sea_orm::{EnumIter, Iterable};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Target::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Target::Id)
                            .integer()
                            .not_null()
                            .unique_key()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Target::Name)
                            .string()
                            .unique_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Target::Status)
                            .enumeration(Target::Table, TargetStatus::iter().skip(1))
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Issue::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Issue::Id)
                            .integer()
                            .not_null()
                            .unique_key()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Issue::Title).string().not_null())
                    .col(ColumnDef::new(Issue::Description).string().not_null())
                    .col(
                        ColumnDef::new(Issue::Status)
                            .enumeration(IssueStatus::Table, IssueStatus::iter().skip(1))
                            .not_null(),
                    )
                    .col(ColumnDef::new(Issue::TargetId).integer().not_null())
                    .col(ColumnDef::new(Issue::CreatedBy).string().not_null())
                    .col(
                        ColumnDef::new(Issue::ToOffline)
                            .enumeration(ToOffline::Table, ToOffline::iter().skip(1)),
                    )
                    .col(ColumnDef::new(Issue::AssignedTo).string())
                    .col(
                        ColumnDef::new(Issue::CreatedAt)
                            .date_time()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(
                        //TODO make auto update to now() on row updates
                        ColumnDef::new(Issue::UpdatedAt)
                            .date_time()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("target")
                            .from(Issue::Table, Issue::TargetId)
                            .to(Target::Table, Target::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Comment::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Comment::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Comment::IssueId).integer().not_null())
                    .col(ColumnDef::new(Comment::CreatedBy).string().not_null())
                    .col(ColumnDef::new(Comment::Comment).string().not_null())
                    .col(
                        ColumnDef::new(Comment::CreatedAt)
                            .date_time()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("issue")
                            .from(Comment::Table, Comment::IssueId)
                            .to(Issue::Table, Issue::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Comment::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Issue::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Target::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Issue {
    Table,
    Id,
    Title,
    Description,
    Status,
    TargetId,
    ToOffline,
    AssignedTo,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Target {
    Table,
    Id,
    Name,
    Status,
}

#[derive(DeriveIden)]
enum Comment {
    Table,
    Id,
    IssueId,
    CreatedAt,
    CreatedBy,
    #[allow(clippy::enum_variant_names)]
    Comment,
}

#[derive(Iden, EnumIter)]
enum IssueStatus {
    Table,
    Open,
    Closed,
}

#[derive(Iden, EnumIter)]
//if null means don't enforce offlining
enum ToOffline {
    Table,
    Node,
    Card,
    Blade,
}

#[derive(Iden, EnumIter)]
enum TargetStatus {
    Table,
    Online,
    Draining,
    Offline,
    Down,
}
