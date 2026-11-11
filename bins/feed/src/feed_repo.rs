//! Операции с таблицей feed через pgvector (аналог Feed-модели TS).
use anyhow::Context;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};

pub struct SimilarFeedItem {
    pub id: String,
    pub text: String,
    pub similarity: f64,
}

fn vector_str(embedding: &[f64]) -> String {
    let mut s = String::from("[");
    for (i, e) in embedding.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("{e}"));
    }
    s.push(']');
    s
}

/// Поиск похожих элементов ленты (косинусная близость через pgvector).
pub async fn find_similar(
    db: &DatabaseConnection,
    embedding: &[f64],
    threshold: f64,
) -> anyhow::Result<Vec<SimilarFeedItem>> {
    let vec = vector_str(embedding);
    let res = db
        .query_all(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT id::text, text, 1 - (embedding <=> $1::vector) AS similarity \
             FROM feed WHERE first_seen_at > NOW() - INTERVAL '48 hours' \
             ORDER BY embedding <=> $1::vector",
            [vec.into()],
        ))
        .await?;

    let mut out = Vec::new();
    for row in res {
        let id: String = row.try_get("", "id")?;
        let text: String = row.try_get("", "text")?;
        let similarity: f64 = row.try_get("", "similarity")?;
        if similarity >= threshold {
            out.push(SimilarFeedItem {
                id,
                text,
                similarity,
            });
        }
    }
    Ok(out)
}

/// Вставка нового элемента ленты + связка с сообщением.
pub async fn insert(
    db: &DatabaseConnection,
    text: &str,
    post_type: Option<&str>,
    first_seen_at: &str,
    channel_id: i64,
    message_id: i64,
    embedding: &[f64],
) -> anyhow::Result<String> {
    let vec = vector_str(embedding);
    let res = db
        .query_one(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO feed (text, is_viewed, post_type, first_seen_at, channel_id, embedding, created_at, updated_at) \
             VALUES ($1, false, $2, $3::timestamptz, $4, $5::vector, NOW(), NOW()) RETURNING id::text",
            [
                text.into(),
                post_type.map(|s| s.to_owned()).into(),
                first_seen_at.into(),
                channel_id.into(),
                vec.into(),
            ],
        ))
        .await?
        .context("Failed to insert feed item")?;
    let feed_id: String = res.try_get("", "id")?;
    add_message(db, &feed_id, channel_id, message_id).await?;
    Ok(feed_id)
}

/// Связать сообщение с элементом ленты (без дубликатов).
pub async fn add_message(
    db: &DatabaseConnection,
    feed_id: &str,
    channel_id: i64,
    message_id: i64,
) -> anyhow::Result<()> {
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT INTO feed_messages (feed_id, channel_id, message_id, created_at) \
         VALUES ($1::uuid, $2, $3, NOW()) ON CONFLICT DO NOTHING",
        [feed_id.into(), channel_id.into(), message_id.into()],
    ))
    .await?;
    Ok(())
}

/// Обновить текст и эмбеддинг элемента (merge дубликатов).
pub async fn update_text(
    db: &DatabaseConnection,
    id: &str,
    text: &str,
    embedding: &[f64],
) -> anyhow::Result<()> {
    let vec = vector_str(embedding);
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "UPDATE feed SET text = $1, embedding = $2::vector, updated_at = NOW() WHERE id = $3::uuid",
        [text.into(), vec.into(), id.into()],
    ))
    .await?;
    Ok(())
}
