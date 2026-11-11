use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "feed_messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_name = "feed_id")]
    pub feed_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false, column_name = "channel_id")]
    pub channel_id: i64,
    #[sea_orm(primary_key, auto_increment = false, column_name = "message_id")]
    pub message_id: i64,
    #[sea_orm(column_name = "created_at")]
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::feed::Entity",
        from = "Column::FeedId",
        to = "super::feed::Column::Id"
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Feed,
    #[sea_orm(
        belongs_to = "super::channel::Entity",
        from = "Column::ChannelId",
        to = "super::channel::Column::Id"
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Channel,
}

impl Related<super::feed::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Feed.def()
    }
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Channel.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
