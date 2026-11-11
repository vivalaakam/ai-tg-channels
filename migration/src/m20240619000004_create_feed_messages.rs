use sea_orm_migration::prelude::*;

use super::m20240619000003_create_feed::Feed;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FeedMessages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(FeedMessages::FeedId).uuid().not_null())
                    .col(
                        ColumnDef::new(FeedMessages::ChannelId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FeedMessages::MessageId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FeedMessages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("feed_messages_pkey")
                            .col(FeedMessages::FeedId)
                            .col(FeedMessages::ChannelId)
                            .col(FeedMessages::MessageId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("feed_messages_feed_id_fkey")
                            .from(FeedMessages::Table, FeedMessages::FeedId)
                            .to(Feed::Table, Feed::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FeedMessages::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum FeedMessages {
    Table,
    FeedId,
    ChannelId,
    MessageId,
    CreatedAt,
}
