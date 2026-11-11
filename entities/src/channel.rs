use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "channels")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,
    #[sea_orm(column_type = "Text", nullable)]
    pub username: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub title: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::message::Entity")]
    Message,
    #[sea_orm(has_many = "super::feed::Entity")]
    Feed,
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Message.def()
    }
}

impl Related<super::feed::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Feed.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
