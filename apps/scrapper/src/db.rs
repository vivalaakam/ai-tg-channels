//! Запись каналов и сообщений в БД (аналог scrapper/src/db.ts).
use std::collections::BTreeMap;

use entities::{channel, message};
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use serde_json::{Value, json};

/// Вставка/обновление канала (upsert по id).
pub async fn upsert_channel(
    db: &DatabaseConnection,
    id: i64,
    title: &str,
    username: Option<String>,
) -> anyhow::Result<()> {
    let active = channel::ActiveModel {
        id: Set(id),
        title: Set(title.to_owned()),
        username: Set(username),
        ..Default::default()
    };
    // upsert: повторная вставка того же id обновляет title/username
    channel::Entity::insert(active)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(channel::Column::Id)
                .update_columns([channel::Column::Title, channel::Column::Username])
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

/// Разбор сообщения TDLib и upsert в таблицу messages.
/// Возвращает `true`, если сообщение было вставлено впервые.
pub async fn save_message(
    db: &DatabaseConnection,
    channel_id: i64,
    message_id: i64,
    raw: &Value,
) -> anyhow::Result<bool> {
    let obj = raw
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("msg not object"))?;

    let date_secs = obj.get("date").and_then(Value::as_i64).unwrap_or_default();
    let msg_date = chrono::DateTime::from_timestamp(date_secs, 0)
        .unwrap_or_else(chrono::Utc::now)
        .fixed_offset();

    let edit_date = obj
        .get("edit_date")
        .and_then(Value::as_i64)
        .filter(|&d| d > 0)
        .and_then(|d| chrono::DateTime::from_timestamp(d, 0))
        .map(|d| d.fixed_offset());

    let effect_id = obj.get("effect_id").and_then(|e| match e {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) => Some(s.clone()),
        _ => None,
    });

    let is_pinned = obj
        .get("is_pinned")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let sender_id = obj.get("sender_id").cloned();

    // Контент: вырезаем важные поля, остальное — в content
    let raw_content = obj.get("content").cloned().unwrap_or_else(|| json!({}));
    let raw_content = raw_content.as_object().cloned().unwrap_or_default();

    let mut content = raw_content.clone();
    for key in [
        "is_secret",
        "has_spoiler",
        "show_caption_above_media",
        "start_timestamp",
        "text",
        "caption",
        "photo",
        "video",
        "cover",
        "storyboards",
        "animation",
        "link_preview_options",
        "link_preview",
        "alternative_videos",
    ] {
        content.remove(key);
    }

    let content_obj: Value = if content.is_empty() {
        Value::Null
    } else {
        Value::Object(content)
    };

    // Текст: из text или caption
    let (content_text_text, content_text_entities, content_text_value) =
        match raw_content.get("text").or(raw_content.get("caption")) {
            Some(t) => {
                let text_obj = t.as_object().cloned().unwrap_or_default();
                let text_text = text_obj
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let entities = match text_obj.get("entities") {
                    Some(e) if !e.is_null() => Some(e.clone()),
                    _ => None,
                };
                let mut rest = text_obj.clone();
                rest.remove("text");
                rest.remove("entities");
                let rest_val: Value = if rest.is_empty() {
                    Value::Null
                } else {
                    Value::Object(rest)
                };
                (text_text, entities, rest_val)
            }
            _ => (None, None, Value::Null),
        };

    let rest_content =
        |key: &str| -> Value { raw_content.get(key).cloned().unwrap_or(Value::Null) };
    let optional_value = |v: Value| -> Option<Value> { if v.is_null() { None } else { Some(v) } };

    let raw_rest: BTreeMap<String, Value> = obj
        .iter()
        .filter(|(k, _)| {
            ![
                "id",
                "chat_id",
                "date",
                "edit_date",
                "effect_id",
                "is_pinned",
                "sender_id",
                "reply_to",
                "content",
            ]
            .contains(&k.as_str())
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let new_active = || message::ActiveModel {
        channel_id: Set(channel_id),
        message_id: Set(message_id),
        date: Set(msg_date),
        edit_date: Set(edit_date),
        effect_id: Set(effect_id.clone()),
        is_pinned: Set(is_pinned),
        sender_id: Set(sender_id.clone()),
        reply_to: Set(obj.get("reply_to").cloned().filter(|v| !v.is_null())),
        content: Set(Some(content_obj.clone())),
        content_text: Set(Some(content_text_value.clone())),
        content_photo: Set(optional_value(rest_content("photo"))),
        content_video: Set(optional_value(rest_content("video"))),
        content_video_cover: Set(optional_value(rest_content("cover"))),
        content_video_storyboards: Set(optional_value(rest_content("storyboards"))),
        content_alternative_videos: Set(optional_value(rest_content("alternative_videos"))),
        content_animation: Set(optional_value(rest_content("animation"))),
        content_link_preview_options: Set(optional_value(rest_content("link_preview_options"))),
        content_link_preview: Set(optional_value(rest_content("link_preview"))),
        content_text_text: Set(content_text_text.clone()),
        content_text_entities: Set(content_text_entities.clone()),
        raw: Set(Value::Object(raw_rest.clone().into_iter().collect())),
        is_checked: Set(None),
        created_at: Set(chrono::Utc::now().fixed_offset()),
    };

    // Попытка вставить; если строка уже есть — не трогаем её (обновление только через edit)
    let res = message::Entity::insert(new_active())
        .on_conflict(
            sea_orm::sea_query::OnConflict::columns([
                message::Column::ChannelId,
                message::Column::MessageId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .do_nothing()
        .exec(db)
        .await;

    // TryInsertResult: Empty — строка уже существовала, вставки не было
    match res {
        Ok(sea_orm::TryInsertResult::Inserted(_)) => Ok(true),
        Ok(_) => Ok(false),
        Err(e) => Err(e.into()),
    }
}
