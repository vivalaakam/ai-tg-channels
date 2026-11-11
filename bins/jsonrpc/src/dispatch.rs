//! Dispatch-функции (аналог jsonrpc/src/dispatch.ts).
use anyhow::Context;
use entities::{app_config, channel, feed, feed_message, message, prompt};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    Order, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Statement,
};
use serde_json::{Value, json};
use uuid::Uuid;

fn err_anyhow(code: i32, message: &str) -> anyhow::Error {
    anyhow::anyhow!("rpc_error:{code}:{message}")
}

pub fn try_parse_rpc_error(e: &anyhow::Error) -> Option<(i32, String)> {
    let msg = format!("{e:#}");
    msg.strip_prefix("rpc_error:").and_then(|rest| {
        let mut parts = rest.splitn(2, ':');
        let code = parts.next()?.parse::<i32>().ok()?;
        Some((code, parts.next().unwrap_or("").to_owned()))
    })
}

// --- channels ---

pub async fn list_channels(db: &DatabaseConnection, limit: Option<u64>) -> anyhow::Result<Value> {
    let chans = channel::Entity::find()
        .column_as(channel::Column::Id, "id")
        .limit(limit.unwrap_or(100))
        .into_json()
        .all(db)
        .await?;
    Ok(serde_json::to_value(chans)?)
}

pub async fn get_channel(db: &DatabaseConnection, id_str: &str) -> anyhow::Result<Value> {
    let id: i64 = id_str.parse().context("channel id must be integer")?;
    let ch = channel::Entity::find_by_id(id)
        .into_json()
        .one(db)
        .await?
        .context_anyhow(-32001, "Channel not found")?;
    Ok(ch)
}

pub async fn get_messages(
    db: &DatabaseConnection,
    channel_id_str: &str,
    limit: Option<u64>,
    offset: Option<u64>,
) -> anyhow::Result<Value> {
    let channel_id: i64 = channel_id_str
        .parse()
        .context("channel id must be integer")?;
    let msgs = message::Entity::find()
        .filter(message::Column::ChannelId.eq(channel_id))
        .order_by(message::Column::Date, Order::Desc)
        .limit(limit.unwrap_or(20))
        .offset(offset.unwrap_or(0))
        .columns([
            message::Column::ChannelId,
            message::Column::MessageId,
            message::Column::Date,
            message::Column::EditDate,
            message::Column::ContentTextText,
            message::Column::IsPinned,
        ])
        .into_json()
        .all(db)
        .await?;
    Ok(serde_json::to_value(msgs)?)
}

// --- feed ---

pub async fn list_feed(
    db: &DatabaseConnection,
    limit: Option<u64>,
    offset: Option<u64>,
    post_type: Option<&str>,
    is_viewed: Option<bool>,
    after: Option<String>,
    before: Option<String>,
) -> anyhow::Result<Value> {
    let mut cond = Condition::all();
    if let Some(pt) = post_type {
        cond = cond.add(feed::Column::PostType.eq(pt));
    }
    if let Some(v) = is_viewed {
        cond = cond.add(feed::Column::IsViewed.eq(v));
    }
    if let Some(after) = after {
        // ISO 8601 → timestamptz
        let dt = chrono::DateTime::parse_from_rfc3339(&after).context("bad `after` datetime")?;
        cond = cond.add(feed::Column::FirstSeenAt.gte(dt));
    }
    if let Some(before) = before {
        let dt = chrono::DateTime::parse_from_rfc3339(&before).context("bad `before` datetime")?;
        cond = cond.add(feed::Column::FirstSeenAt.lte(dt));
    }

    let rows = feed::Entity::find()
        .filter(cond)
        .order_by(feed::Column::FirstSeenAt, Order::Desc)
        .limit(limit.unwrap_or(20))
        .offset(offset.unwrap_or(0))
        .columns([
            feed::Column::Id,
            feed::Column::Text,
            feed::Column::IsViewed,
            feed::Column::PostType,
            feed::Column::FirstSeenAt,
            feed::Column::ChannelId,
            feed::Column::CreatedAt,
        ])
        .into_json()
        .all(db)
        .await?;
    Ok(serde_json::to_value(rows)?)
}

pub async fn get_feed(db: &DatabaseConnection, id_str: &str) -> anyhow::Result<Value> {
    let id: Uuid = id_str.parse().context("bad feed UUID")?;
    let row = feed::Entity::find_by_id(id)
        .columns([
            feed::Column::Id,
            feed::Column::Text,
            feed::Column::IsViewed,
            feed::Column::PostType,
            feed::Column::FirstSeenAt,
            feed::Column::ChannelId,
            feed::Column::CreatedAt,
        ])
        .into_json()
        .one(db)
        .await?
        .context_anyhow(-32001, "Feed item not found")?;
    Ok(row)
}

pub async fn get_feed_messages(
    db: &DatabaseConnection,
    feed_id_str: &str,
) -> anyhow::Result<Value> {
    let feed_id: Uuid = feed_id_str.parse().context("bad feed UUID")?;
    if get_feed(db, feed_id_str).await.is_err() {
        return Err(err_anyhow(-32001, "Feed item not found"));
    }

    let rows = feed_message::Entity::find()
        .filter(feed_message::Column::FeedId.eq(feed_id))
        .join(
            sea_orm::JoinType::InnerJoin,
            feed_message::Relation::Channel.def(),
        )
        .column(channel::Column::Username)
        .into_json()
        .all(db)
        .await?;

    let mut out = Vec::new();
    for r in rows {
        let obj = r.as_object().cloned().unwrap_or_default();
        let channel_id = obj.get("channel_id").and_then(Value::as_i64).unwrap_or(0);
        let message_id = obj.get("message_id").and_then(Value::as_i64).unwrap_or(0);
        let username = obj
            .get("username")
            .and_then(Value::as_str)
            .map(str::to_owned);

        // TDLib кодирует ID сообщений каналов как (url_id << 20)
        let url_msg_id = message_id >> 20;
        let tg_link = match username {
            Some(u) => format!("https://t.me/{u}/{url_msg_id}"),
            None => format!(
                "https://t.me/c/{}/{}",
                channel_id.abs().saturating_sub(1_000_000_000_000) / 10,
                url_msg_id
            ),
        };
        out.push(json!({
            "channelId": channel_id,
            "messageId": message_id,
            "tgLink": tg_link,
        }));
    }
    Ok(Value::Array(out))
}

pub async fn mark_feed_viewed(db: &DatabaseConnection, id_str: &str) -> anyhow::Result<Value> {
    let id: Uuid = id_str.parse().context("bad feed UUID")?;
    let row = feed::Entity::find_by_id(id)
        .one(db)
        .await?
        .context_anyhow(-32001, "Feed item not found")?;
    let mut active: feed::ActiveModel = row.into();
    active.is_viewed = sea_orm::Set(true);
    active.update(db).await?;
    Ok(json!({ "ok": true }))
}

// --- config ---

pub async fn list_config(
    db: &DatabaseConnection,
    after: Option<String>,
    before: Option<String>,
) -> anyhow::Result<Value> {
    let mut cond = Condition::all();
    if let Some(after) = after {
        let dt = chrono::DateTime::parse_from_rfc3339(&after).context("bad `after` datetime")?;
        cond = cond.add(app_config::Column::UpdatedAt.gte(dt));
    }
    if let Some(before) = before {
        let dt = chrono::DateTime::parse_from_rfc3339(&before).context("bad `before` datetime")?;
        cond = cond.add(app_config::Column::UpdatedAt.lte(dt));
    }
    let rows = app_config::Entity::find()
        .filter(cond)
        .order_by(app_config::Column::Slug, Order::Asc)
        .into_json()
        .all(db)
        .await?;
    Ok(serde_json::to_value(rows)?)
}

pub async fn get_config(db: &DatabaseConnection, slug: &str) -> anyhow::Result<Value> {
    app_config::Entity::find_by_id(slug.to_owned())
        .into_json()
        .one(db)
        .await?
        .context_anyhow(-32001, "Config key not found")
}

pub async fn set_config(db: &DatabaseConnection, slug: &str, value: &str) -> anyhow::Result<Value> {
    let now = chrono::Utc::now().fixed_offset();
    let active = app_config::ActiveModel {
        slug: sea_orm::Set(slug.to_owned()),
        value: sea_orm::Set(value.to_owned()),
        created_at: sea_orm::Set(now),
        updated_at: sea_orm::Set(chrono::Utc::now().fixed_offset()),
    };
    app_config::Entity::insert(active)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(app_config::Column::Slug)
                .update_columns([app_config::Column::Value, app_config::Column::UpdatedAt])
                .to_owned(),
        )
        .exec(db)
        .await?;
    // created_at при upsert не трогаем — возвращаем актуальную строку
    let row = app_config::Entity::find_by_id(slug.to_owned())
        .into_json()
        .one(db)
        .await?
        .unwrap_or(Value::Null);
    let _ = now;
    Ok(row)
}

// --- prompts ---

pub async fn list_prompts(
    db: &DatabaseConnection,
    tags: Option<Vec<String>>,
    after: Option<String>,
    before: Option<String>,
) -> anyhow::Result<Value> {
    let mut cond = Condition::all();
    if let Some(tags) = tags.filter(|t| !t.is_empty()) {
        // overlap: tags && ARRAY[..]
        let arr = tags
            .iter()
            .map(|t| format!("'{}'", t.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id::text, content, version, tags, previous_id, created_at FROM prompts WHERE tags && ARRAY[{arr}]::text[] ORDER BY created_at DESC"
        );
        let rows = db
            .query_all(Statement::from_string(db.get_database_backend(), sql))
            .await?;
        let mut out = Vec::new();
        for row in rows {
            out.push(prompt_row_to_json(&row)?);
        }
        return Ok(Value::Array(out));
    }
    if let Some(after) = after {
        let dt = chrono::DateTime::parse_from_rfc3339(&after).context("bad `after` datetime")?;
        cond = cond.add(prompt::Column::CreatedAt.gte(dt));
    }
    if let Some(before) = before {
        let dt = chrono::DateTime::parse_from_rfc3339(&before).context("bad `before` datetime")?;
        cond = cond.add(prompt::Column::CreatedAt.lte(dt));
    }
    let rows = prompt::Entity::find()
        .filter(cond)
        .order_by(prompt::Column::CreatedAt, Order::Desc)
        .into_json()
        .all(db)
        .await?;
    Ok(serde_json::to_value(rows)?)
}

fn prompt_row_to_json(row: &sea_orm::QueryResult) -> anyhow::Result<Value> {
    let id: Uuid = row.try_get("", "id")?;
    let content: String = row.try_get("", "content")?;
    let version: i32 = row.try_get("", "version")?;
    let tags: Vec<String> = row.try_get("", "tags")?;
    let previous_id: Option<Uuid> = row.try_get("", "previous_id")?;
    let created_at: chrono::DateTime<chrono::FixedOffset> = row.try_get("", "created_at")?;
    Ok(json!({
        "id": id, "content": content, "version": version,
        "tags": tags, "previousId": previous_id,
        "createdAt": created_at.to_rfc3339(),
    }))
}

pub async fn get_prompt(db: &DatabaseConnection, id_str: &str) -> anyhow::Result<Value> {
    let id: Uuid = id_str.parse().context("bad prompt UUID")?;
    prompt::Entity::find_by_id(id)
        .into_json()
        .one(db)
        .await?
        .context_anyhow(-32001, "Prompt not found")
}

pub async fn create_prompt(
    db: &DatabaseConnection,
    content: &str,
    tags: Vec<String>,
) -> anyhow::Result<Value> {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    let active = prompt::ActiveModel {
        id: sea_orm::Set(id),
        content: sea_orm::Set(content.to_owned()),
        version: sea_orm::Set(1),
        tags: sea_orm::Set(tags),
        previous_id: sea_orm::Set(None),
        created_at: sea_orm::Set(now),
    };
    prompt::Entity::insert(active).exec(db).await?;
    get_prompt(db, &id.to_string()).await
}

pub async fn update_prompt(
    db: &DatabaseConnection,
    id_str: &str,
    content: &str,
    tags: Vec<String>,
) -> anyhow::Result<Value> {
    let prev_id: Uuid = id_str.parse().context("bad prompt UUID")?;
    let prev = prompt::Entity::find_by_id(prev_id)
        .one(db)
        .await?
        .context_anyhow(-32001, "Prompt not found")?;
    let next_version = (prev.version.max(0)) + 1;
    let id = Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    let active = prompt::ActiveModel {
        id: sea_orm::Set(id),
        content: sea_orm::Set(content.to_owned()),
        version: sea_orm::Set(next_version),
        tags: sea_orm::Set(tags),
        previous_id: sea_orm::Set(Some(prev_id)),
        created_at: sea_orm::Set(now),
    };
    prompt::Entity::insert(active).exec(db).await?;
    get_prompt(db, &id.to_string()).await
}

pub async fn get_prompt_history(db: &DatabaseConnection, id_str: &str) -> anyhow::Result<Value> {
    let mut id_opt: Option<Uuid> = Some(id_str.parse().context("bad prompt UUID")?);
    // ensure exists
    get_prompt(db, id_str).await?;
    let mut chain = Vec::new();
    while let Some(id) = id_opt {
        let Some(p) = prompt::Entity::find_by_id(id).one(db).await? else {
            break;
        };
        id_opt = p.previous_id;
        chain.push(json!({
            "id": p.id,
            "content": p.content,
            "version": p.version,
            "tags": p.tags,
            "previousId": p.previous_id,
            "createdAt": p.created_at.to_rfc3339(),
        }));
    }
    Ok(Value::Array(chain))
}

pub async fn render_prompt(
    db: &DatabaseConnection,
    id_str: &str,
    vars: Value,
) -> anyhow::Result<Value> {
    let id: Uuid = id_str.parse().context("bad prompt UUID")?;
    let row = prompt::Entity::find_by_id(id)
        .one(db)
        .await?
        .context_anyhow(-32001, "Prompt not found")?;
    let vars_map = vars.as_object().cloned().unwrap_or_default();
    // аналог renderTemplate(content, vars): {{k}} замена
    let rendered = crate::methods::render_template(&row.content, &vars_map);
    Ok(json!({ "rendered": rendered }))
}

/// расширение Context с rpc_error
trait ContextAnyhow<T> {
    fn context_anyhow(self, code: i32, message: &str) -> anyhow::Result<T>;
}

impl<T> ContextAnyhow<T> for Option<T> {
    fn context_anyhow(self, code: i32, message: &str) -> anyhow::Result<T> {
        self.ok_or_else(|| err_anyhow(code, message))
    }
}

impl<T, E> ContextAnyhow<T> for Result<T, E> {
    fn context_anyhow(self, code: i32, message: &str) -> anyhow::Result<T> {
        self.map_err(|_| err_anyhow(code, message))
    }
}
