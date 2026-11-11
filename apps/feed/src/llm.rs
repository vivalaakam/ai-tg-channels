//! HTTP-клиенты эмбеддингов и LLM (аналог feed/src/embeddings.ts и llm.ts).
use anyhow::Context;
use common::{optional, require};
use entities::{app_config, prompt};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{Value, json};

async fn get_config_value(db: &DatabaseConnection, slug: &str) -> anyhow::Result<Option<String>> {
    let cfg = app_config::Entity::find_by_id(slug.to_owned())
        .one(db)
        .await?;
    Ok(cfg.map(|c| c.value))
}

async fn get_prompt_content(
    db: &DatabaseConnection,
    config_slug: &str,
    fallback: &str,
) -> anyhow::Result<String> {
    let Some(value) = get_config_value(db, config_slug).await? else {
        return Ok(fallback.to_owned());
    };
    let Some(p) = prompt::Entity::find_by_id(value.parse::<sea_orm::prelude::Uuid>()?)
        .one(db)
        .await?
    else {
        return Ok(fallback.to_owned());
    };
    Ok(p.content)
}

/// Получить эмбеддинг текста через OpenAI-совместимый /v1/embeddings.
pub async fn get_embedding(_db: &DatabaseConnection, text: &str) -> anyhow::Result<Vec<f64>> {
    let base = require("EMBEDDED_BASE_URL");
    let model = optional("CHUNK_MODEL").unwrap_or_else(|| "nomic-embed-text".into());

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/v1/embeddings"))
        .json(&json!({ "model": model, "input": text }))
        .send()
        .await
        .context("embeddings request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err = resp.text().await.unwrap_or_default();
        anyhow::bail!("Embeddings API error {status}: {err}");
    }

    let data: Value = resp.json().await.context("embeddings json")?;
    let embedding = data
        .pointer("/data/0/embedding")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
        .context("no embedding in response")?;
    Ok(embedding)
}

struct LlmCfg {
    base_url: String,
    model: String,
    bearer: Option<String>,
}

async fn llm_cfg(db: &DatabaseConnection) -> anyhow::Result<LlmCfg> {
    let base_url = get_config_value(db, "llm_text_base_url")
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "LLM base URL not configured. Please set the \"llm_text_base_url\" config."
            )
        })?;
    let model = get_config_value(db, "llm_text_model")
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!("LLM model not configured. Please set the \"llm_text_model\" config.")
        })?;
    let bearer = get_config_value(db, "llm_text_bearer").await?;
    Ok(LlmCfg {
        base_url,
        model,
        bearer,
    })
}

async fn chat_completion(
    db: &DatabaseConnection,
    system_prompt: &str,
    user_message: &str,
) -> anyhow::Result<String> {
    let cfg = llm_cfg(db).await?;

    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(b) = cfg.bearer
        && let Ok(v) = reqwest::header::HeaderValue::from_str(&format!("Bearer {b}"))
    {
        headers.insert(reqwest::header::AUTHORIZATION, v);
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/chat/completions", cfg.base_url))
        .headers(headers)
        .json(&json!({
            "model": cfg.model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_message },
            ],
            "think": false,
        }))
        .send()
        .await
        .context("llm request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err = resp.text().await.unwrap_or_default();
        anyhow::bail!("LLM API error {status}: {err}");
    }

    let data: Value = resp.json().await.context("llm json")?;
    let content = data
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .context("no content in llm response")?;
    Ok(content.to_owned())
}

const FALLBACK_NORMALIZE: &str = r#"You are a content normalizer for Telegram posts. Given a post text:
1. Rewrite it in a neutral, factual tone — remove excessive emotions, hype, and emoji.
2. Classify it as one of: "ad" (advertisement/promotion), "news" (news/announcement), "post" (opinion/discussion/other), "meme" (humor/meme).

Respond ONLY with valid JSON. No markdown, no explanation."#;

const FALLBACK_DEDUP: &str = r#"You are a news deduplication assistant. Given two news texts from different Telegram channels, determine:
1. Are they reporting the same event/news?
2. If yes, produce the best merged version (preserve the most complete information, prefer clearer wording).

Respond ONLY with valid JSON. No markdown, no explanation."#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostType {
    Ad,
    News,
    Post,
    Meme,
    Other(String),
}

impl PostType {
    pub fn as_str(&self) -> &str {
        match self {
            PostType::Ad => "ad",
            PostType::News => "news",
            PostType::Post => "post",
            PostType::Meme => "meme",
            PostType::Other(s) => s,
        }
    }
}

impl std::str::FromStr for PostType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "ad" => PostType::Ad,
            "news" => PostType::News,
            "post" => PostType::Post,
            "meme" => PostType::Meme,
            other => PostType::Other(other.to_owned()),
        })
    }
}

/// Достать JSON-объект из ответа LLM (LLM может добавить markdown-обрамление).
fn extract_json(content: &str) -> anyhow::Result<Value> {
    let trimmed = content.trim();
    if trimmed.starts_with('{') {
        return Ok(serde_json::from_str(trimmed)?);
    }
    // ищем первый { и последний }
    let start = trimmed.find('{').context("no JSON in LLM response")?;
    let end = trimmed.rfind('}').context("no JSON in LLM response")?;
    Ok(serde_json::from_str(&trimmed[start..=end])?)
}

#[derive(Debug, Clone)]
pub struct NormalizeResult {
    pub normalized_text: String,
    pub post_type: PostType,
}

/// Нормализация и классификация поста через LLM.
pub async fn normalize_and_classify(
    db: &DatabaseConnection,
    text: &str,
) -> anyhow::Result<NormalizeResult> {
    let system_prompt = get_prompt_content(db, "normalize_system", FALLBACK_NORMALIZE).await?;
    let content = chat_completion(
        db,
        &system_prompt,
        &format!(
            "Post text:\n{text}\n\nRespond with JSON: {{\"normalized_text\": string, \"post_type\": \"ad\"|\"news\"|\"post\"|\"meme\"}}"
        ),
    )
    .await?;

    let parsed = extract_json(&content)?;
    let normalized_text = parsed
        .get("normalized_text")
        .and_then(Value::as_str)
        .context("missing normalized_text")?
        .to_owned();
    let post_type: PostType = parsed
        .get("post_type")
        .and_then(Value::as_str)
        .unwrap_or("post")
        .parse()?;
    Ok(NormalizeResult {
        normalized_text,
        post_type,
    })
}

#[derive(Debug, Clone)]
pub struct DeduplicationResult {
    pub same_news: bool,
    pub merged_text: Option<String>,
}

/// Проверка дубликатов двух новостей через LLM.
pub async fn check_duplication(
    db: &DatabaseConnection,
    existing_text: &str,
    new_text: &str,
) -> anyhow::Result<DeduplicationResult> {
    let system_prompt = get_prompt_content(db, "dedup_system", FALLBACK_DEDUP).await?;
    let content = chat_completion(
        db,
        &system_prompt,
        &format!(
            "Text 1 (existing):\n{existing_text}\n\nText 2 (new):\n{new_text}\n\nRespond with JSON: {{\"same_news\": boolean, \"merged_text\": string | null}}\nIf same_news is true, merged_text must contain the best combined version.\nIf same_news is false, merged_text must be null."
        ),
    )
    .await?;

    let parsed = extract_json(&content)?;
    let same_news = parsed
        .get("same_news")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let merged_text = parsed
        .get("merged_text")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(DeduplicationResult {
        same_news,
        merged_text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_from_markdown() {
        let v = extract_json("```json\n{\"a\": 1}\n```").unwrap();
        assert_eq!(v.get("a").and_then(Value::as_i64), Some(1));
    }
}
