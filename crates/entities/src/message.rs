use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_name = "channel_id")]
    pub channel_id: i64,
    #[sea_orm(primary_key, auto_increment = false, column_name = "message_id")]
    pub message_id: i64,
    pub date: DateTimeWithTimeZone,
    #[sea_orm(column_name = "edit_date", nullable)]
    pub edit_date: Option<DateTimeWithTimeZone>,
    #[sea_orm(column_name = "effect_id", nullable, column_type = "Text")]
    pub effect_id: Option<String>,
    #[sea_orm(column_name = "is_pinned")]
    pub is_pinned: bool,
    #[sea_orm(column_name = "sender_id", nullable, column_type = "Json")]
    pub sender_id: Option<Json>,
    #[sea_orm(column_name = "reply_to", nullable, column_type = "Json")]
    pub reply_to: Option<Json>,
    #[sea_orm(column_type = "Json", nullable)]
    pub content: Option<Json>,
    #[sea_orm(column_name = "content_text", column_type = "Json", nullable)]
    pub content_text: Option<Json>,
    #[sea_orm(column_name = "content_photo", column_type = "Json", nullable)]
    pub content_photo: Option<Json>,
    #[sea_orm(column_name = "content_video", column_type = "Json", nullable)]
    pub content_video: Option<Json>,
    #[sea_orm(column_name = "content_video_cover", column_type = "Json", nullable)]
    pub content_video_cover: Option<Json>,
    #[sea_orm(
        column_name = "content_video_storyboards",
        column_type = "Json",
        nullable
    )]
    pub content_video_storyboards: Option<Json>,
    #[sea_orm(
        column_name = "content_alternative_videos",
        column_type = "Json",
        nullable
    )]
    pub content_alternative_videos: Option<Json>,
    #[sea_orm(column_name = "content_animation", column_type = "Json", nullable)]
    pub content_animation: Option<Json>,
    #[sea_orm(
        column_name = "content_link_preview_options",
        column_type = "Json",
        nullable
    )]
    pub content_link_preview_options: Option<Json>,
    #[sea_orm(column_name = "content_link_preview", column_type = "Json", nullable)]
    pub content_link_preview: Option<Json>,
    #[sea_orm(column_name = "content_text_text", nullable, column_type = "Text")]
    pub content_text_text: Option<String>,
    #[sea_orm(column_name = "content_text_entities", column_type = "Json", nullable)]
    pub content_text_entities: Option<Json>,
    #[sea_orm(column_type = "Json")]
    pub raw: Json,
    #[sea_orm(column_name = "is_checked", nullable)]
    pub is_checked: Option<bool>,
    #[sea_orm(column_name = "created_at")]
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::channel::Entity",
        from = "Column::ChannelId",
        to = "super::channel::Column::Id"
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Channel,
}

impl Related<super::channel::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Channel.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
