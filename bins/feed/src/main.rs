//! Процессор ленты (аналог feed/src/processor.ts).
mod feed_repo;
mod llm;

use anyhow::Context;
use bin_common::{init_tracing, load_env};
use entities::message;
use migration::MigratorTrait;
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Statement,
};

const SIMILARITY_THRESHOLD: f64 = 0.85;

/// Разбор первого непроверенного сообщения с текстом, эмбеддинг, поиск похожих,
/// вставка/дедупликация через LLM.
async fn process_messages(db: &DatabaseConnection) -> anyhow::Result<()> {
    let mut processed: u64 = 0;

    loop {
        let cond = sea_orm::Condition::any()
            .add(message::Column::IsChecked.is_null())
            .add(message::Column::IsChecked.eq(false));
        let Some(msg) = message::Entity::find()
            .filter(message::Column::ContentTextText.is_not_null())
            .filter(cond)
            .order_by_asc(message::Column::Date)
            .one(db)
            .await?
        else {
            break;
        };

        let Some(text) = msg.content_text_text.clone() else {
            // нет текста — помечаем и идём дальше
            mark_checked(db, &msg).await?;
            continue;
        };

        if text.trim().is_empty() {
            println!(
                "[feed] msg {} from channel {} empty",
                msg.message_id, msg.channel_id
            );
            mark_checked(db, &msg).await?;
            continue;
        }

        println!(
            "[feed] Processing msg {} from channel {}...",
            msg.message_id, msg.channel_id
        );

        let result: anyhow::Result<()> = async {
            let embedding = llm::get_embedding(db, &text).await?;
            let similar = feed_repo::find_similar(db, &embedding, SIMILARITY_THRESHOLD).await?;

            if similar.is_empty() {
                let normalized = llm::normalize_and_classify(db, &text).await?;
                let normalized_embedding =
                    llm::get_embedding(db, &normalized.normalized_text).await?;
                feed_repo::insert(
                    db,
                    &normalized.normalized_text,
                    Some(normalized.post_type.as_str()),
                    &msg.date.to_rfc3339(),
                    msg.channel_id,
                    msg.message_id,
                    &normalized_embedding,
                )
                .await?;
                println!(
                    "[feed] New item added from channel {} msg {} ({})",
                    msg.channel_id,
                    msg.message_id,
                    normalized.post_type.as_str()
                );
            } else {
                for row in &similar {
                    let dup = llm::check_duplication(db, &row.text, &text).await?;
                    if dup.same_news {
                        if let Some(merged) = &dup.merged_text {
                            if merged != &row.text {
                                let merged_embedding = llm::get_embedding(db, merged).await?;
                                feed_repo::update_text(db, &row.id, merged, &merged_embedding)
                                    .await?;
                                println!("[feed] Updated item {} with merged text", row.id);
                            } else {
                                println!(
                                    "[feed] Duplicate of {} (similarity {:.3}), linking message",
                                    row.id, row.similarity
                                );
                            }
                        } else {
                            println!(
                                "[feed] Duplicate of {} (similarity {:.3}), linking message",
                                row.id, row.similarity
                            );
                        }
                        feed_repo::add_message(db, &row.id, msg.channel_id, msg.message_id).await?;
                    } else {
                        let normalized = llm::normalize_and_classify(db, &text).await?;
                        let normalized_embedding =
                            llm::get_embedding(db, &normalized.normalized_text).await?;
                        feed_repo::insert(
                            db,
                            &normalized.normalized_text,
                            Some(normalized.post_type.as_str()),
                            &msg.date.to_rfc3339(),
                            msg.channel_id,
                            msg.message_id,
                            &normalized_embedding,
                        )
                        .await?;
                        println!(
                            "[feed] Different news despite vector similarity, added new item ({})",
                            normalized.post_type.as_str()
                        );
                    }
                }
            }
            Ok(())
        }
        .await;

        if let Err(err) = result {
            println!(
                "[feed] Error processing msg {} from channel {}: {err}",
                msg.message_id, msg.channel_id
            );
        }

        mark_checked(db, &msg).await?;
        processed += 1;
    }

    println!("[feed] Done processing {processed} messages");
    Ok(())
}

async fn mark_checked(db: &DatabaseConnection, m: &message::Model) -> anyhow::Result<()> {
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        "UPDATE messages SET is_checked = TRUE WHERE channel_id = $1 AND message_id = $2",
        [m.channel_id.into(), m.message_id.into()],
    ))
    .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env();
    init_tracing();

    println!("[feed] Starting feed processor...");
    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    let db = Database::connect(&db_url).await.context("DB connect")?;
    migration::Migrator::up(&db, None).await?;
    process_messages(&db).await?;
    println!("[feed] Done.");
    Ok(())
}
