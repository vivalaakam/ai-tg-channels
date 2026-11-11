use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FeedTable::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(PostTypeCol::PostType)
                            .text()
                            .null()
                            .default(Expr::value(None::<String>)),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FeedTable::Table)
                    .drop_column(PostTypeCol::PostType)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
#[iden(rename = "feed")]
enum FeedTable {
    Table,
}

#[derive(Iden)]
enum PostTypeCol {
    PostType,
}
