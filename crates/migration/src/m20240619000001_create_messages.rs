use sea_orm_migration::prelude::*;

use super::m20240619000000_create_channels::Channels;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Messages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Messages::ChannelId).big_integer().not_null())
                    .col(ColumnDef::new(Messages::MessageId).big_integer().not_null())
                    .col(
                        ColumnDef::new(Messages::Date)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Messages::EditDate)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(Messages::EffectId).text().null())
                    .col(
                        ColumnDef::new(Messages::IsPinned)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(Messages::SenderId).json_binary().null())
                    .col(ColumnDef::new(Messages::ReplyTo).json_binary().null())
                    .col(ColumnDef::new(Messages::Content).json_binary().null())
                    .col(ColumnDef::new(Messages::ContentText).json_binary().null())
                    .col(ColumnDef::new(Messages::ContentPhoto).json_binary().null())
                    .col(ColumnDef::new(Messages::ContentVideo).json_binary().null())
                    .col(
                        ColumnDef::new(Messages::ContentVideoCover)
                            .json_binary()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Messages::ContentVideoStoryboards)
                            .json_binary()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Messages::ContentAlternativeVideos)
                            .json_binary()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Messages::ContentAnimation)
                            .json_binary()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Messages::ContentLinkPreviewOptions)
                            .json_binary()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Messages::ContentLinkPreview)
                            .json_binary()
                            .null(),
                    )
                    .col(ColumnDef::new(Messages::ContentTextText).text().null())
                    .col(
                        ColumnDef::new(Messages::ContentTextEntities)
                            .json_binary()
                            .null(),
                    )
                    .col(ColumnDef::new(Messages::Raw).json_binary().not_null())
                    .col(
                        ColumnDef::new(Messages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("messages_pkey")
                            .col(Messages::ChannelId)
                            .col(Messages::MessageId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("messages_channel_id_fkey")
                            .from(Messages::Table, Messages::ChannelId)
                            .to(Channels::Table, Channels::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Messages::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Messages {
    Table,
    ChannelId,
    MessageId,
    Date,
    EditDate,
    EffectId,
    IsPinned,
    SenderId,
    ReplyTo,
    Content,
    ContentText,
    ContentPhoto,
    ContentVideo,
    ContentVideoCover,
    ContentVideoStoryboards,
    ContentAlternativeVideos,
    ContentAnimation,
    ContentLinkPreviewOptions,
    ContentLinkPreview,
    ContentTextText,
    ContentTextEntities,
    Raw,
    CreatedAt,
}
