use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("CREATE EXTENSION IF NOT EXISTS vector")
            .await?;
        db.execute_unprepared(
            r#"
    CREATE TABLE feed (
      id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      text TEXT NOT NULL,
      is_viewed BOOLEAN NOT NULL DEFAULT false,
      first_seen_at TIMESTAMPTZ NOT NULL,
      channel_id BIGINT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
      embedding VECTOR(768) NOT NULL,
      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
    )
  "#,
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX feed_embedding_idx ON feed USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE feed")
            .await
            .map(|_| ())
    }
}

#[derive(Iden)]
#[allow(dead_code)]
pub enum Feed {
    Table,
    Id,
    Text,
    IsViewed,
    FirstSeenAt,
    ChannelId,
    Embedding,
    CreatedAt,
    UpdatedAt,
    PostType,
}
