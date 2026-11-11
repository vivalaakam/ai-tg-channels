use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MessagesTable::Table)
                    .add_column_if_not_exists(ColumnDef::new(Cols::IsChecked).boolean().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MessagesTable::Table)
                    .drop_column(Cols::IsChecked)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
#[iden(rename = "messages")]
enum MessagesTable {
    Table,
}

#[derive(Iden)]
enum Cols {
    IsChecked,
}
