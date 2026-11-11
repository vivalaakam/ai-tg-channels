use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AppConfig::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(AppConfig::Slug).text().primary_key())
                    .col(ColumnDef::new(AppConfig::Value).text().not_null())
                    .col(
                        ColumnDef::new(AppConfig::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(AppConfig::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AppConfig::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum AppConfig {
    Table,
    Slug,
    Value,
    CreatedAt,
    UpdatedAt,
}
