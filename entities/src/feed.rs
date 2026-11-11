use sea_orm::entity::prelude::*;

/// Модель для таблицы `feed`. Колонка `embedding` (pgvector) не входит в модель —
/// все операции с вектором идут через raw SQL, SeaORM-слой работает только с метаданными.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "feed")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(column_type = "Text")]
    pub text: String,
    pub is_viewed: bool,
    #[sea_orm(nullable, column_type = "Text")]
    pub post_type: Option<String>,
    #[sea_orm(column_name = "first_seen_at")]
    pub first_seen_at: DateTimeWithTimeZone,
    #[sea_orm(column_name = "channel_id")]
    pub channel_id: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
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
    #[sea_orm(has_many = "super::feed_message::Entity")]
    FeedMessage,
}

impl Related<super::channel::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Channel.def()
    }
}

impl Related<super::feed_message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FeedMessage.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
