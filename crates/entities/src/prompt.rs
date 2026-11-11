use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "prompts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    pub version: i32,
    #[sea_orm(column_name = "tags")]
    pub tags: Vec<String>,
    #[sea_orm(column_name = "previous_id", nullable)]
    pub previous_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::prompt::Entity",
        from = "Column::PreviousId",
        to = "super::prompt::Column::Id"
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Previous,
}

impl ActiveModelBehavior for ActiveModel {}
