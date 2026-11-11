use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
    CREATE TABLE prompts (
      id UUID PRIMARY KEY,
      content TEXT NOT NULL,
      version INTEGER NOT NULL DEFAULT 1,
      tags TEXT[] NOT NULL DEFAULT '{}',
      previous_id UUID REFERENCES prompts(id),
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
  "#,
        )
        .await
        .map(|_| ())?;
        db.execute_unprepared("CREATE INDEX prompts_tags_gin_idx ON prompts USING GIN (tags)")
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE prompts")
            .await
            .map(|_| ())
    }
}
