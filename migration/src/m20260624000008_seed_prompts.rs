use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const NORMALIZE_SYSTEM: &str = r#"You are a content normalizer for Telegram posts. Given a post text:
1. Rewrite it in a neutral, factual tone — remove excessive emotions, hype, and emoji.
2. Classify it as one of: "ad" (advertisement/promotion), "news" (news/announcement), "post" (opinion/discussion/other), "meme" (humor/meme).

Respond ONLY with valid JSON. No markdown, no explanation."#;

const DEDUP_SYSTEM: &str = r#"You are a news deduplication assistant. Given two news texts from different Telegram channels, determine:
1. Are they reporting the same event/news?
2. If yes, produce the best merged version (preserve the most complete information, prefer clearer wording).

Respond ONLY with valid JSON. No markdown, no explanation."#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for (slug, content, tags) in [
            ("normalize_system", NORMALIZE_SYSTEM, "'normalize','system'"),
            ("dedup_system", DEDUP_SYSTEM, "'dedup','system'"),
        ] {
            db.execute_unprepared(&format!(
                "INSERT INTO prompts (id, content, version, tags, previous_id)                  VALUES (gen_random_uuid(), '{}', 1, ARRAY[{}], NULL)",
                content.replace(char::from(39u8), "''"),
                tags,
            ))
            .await
            .map(|_| ())?;
            db.execute_unprepared(&format!(
                "INSERT INTO app_config (slug, value) VALUES ('{}',                  (SELECT id FROM prompts WHERE content = '{}' LIMIT 1))",
                slug,
                content.replace(char::from(39u8), "''"),
            ))
            .await
            .map(|_| ())?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DELETE FROM prompts WHERE id IN                  (SELECT value::uuid FROM app_config WHERE slug IN ('normalize_system', 'dedup_system'))",
            )
            .await
            .map(|_| ())?;
        manager
            .get_connection()
            .execute_unprepared(
                "DELETE FROM app_config WHERE slug IN ('normalize_system', 'dedup_system')",
            )
            .await
            .map(|_| ())
    }
}
