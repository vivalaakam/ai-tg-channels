//! HTTP-сервер JSON-RPC 2.0 (аналог jsonrpc/src/index.ts).
mod dispatch;
mod methods;
mod openrpc;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use axum::Router;
use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
use common::{init_tracing, load_env, optional};
use methods::{find, methods, validate_params};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde_json::{Map, Value, json};

struct AppState {
    db: DatabaseConnection,
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env();
    init_tracing();

    let port: u16 = optional("JSONRPC_PORT")
        .and_then(|p| p.parse().ok())
        .unwrap_or(3002);

    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL not set")?;
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(10);
    let db = Database::connect(opts).await.context("DB connect")?;
    Migrator::up(&db, None).await?;

    let state = Arc::new(AppState { db, port });
    let spec = openrpc::openrpc_spec(port);

    let app = Router::new()
        .route("/", axum::routing::get(list_root).post(handle_rpc))
        .route(
            "/openrpc.json",
            axum::routing::get(move || {
                let spec = spec.clone();
                async move {
                    let pretty =
                        serde_json::to_string_pretty(&spec).unwrap_or_else(|_| "{}".into());
                    ([(header::CONTENT_TYPE, "application/json")], pretty)
                }
            }),
        )
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("JSON-RPC server listening on http://localhost:{port}");
    println!("OpenRPC:   http://localhost:{port}/openrpc.json");
    println!(
        "Methods:   {}",
        methods()
            .iter()
            .map(|m| m.name)
            .collect::<Vec<_>>()
            .join(", ")
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// GET / → список методов (простой explorer)
async fn list_root(State(state): State<Arc<AppState>>) -> Html<String> {
    let mut items = String::new();
    for m in methods() {
        items.push_str(&format!(
            "<li><code>{}</code> — {}</li>",
            m.name,
            html_escape(m.description)
        ));
    }
    Html(format!(
        "<html><head><title>ai-tg-channels JSON-RPC</title></head><body>\
         <h1>ai-tg-channels JSON-RPC</h1>\
         <p>Endpoint: <code>POST http://localhost:{}/</code> (JSON-RPC 2.0)</p>\
         <p>OpenRPC spec: <a href=\"/openrpc.json\">/openrpc.json</a></p>\
         <h2>Methods ({})</h2><ul>{}</ul>\
         </body></html>",
        state.port,
        methods().len(),
        items
    ))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

async fn handle_rpc(
    State(state): State<Arc<AppState>>,
    axum::Json(body): axum::Json<Value>,
) -> Response {
    let resp = match handle_rpc_inner2(&state, body).await {
        Ok(v) => v,
        Err(v) => v,
    };
    println!("[rpc] ← {resp}");
    (
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&resp).unwrap_or_default(),
    )
        .into_response()
}

fn rpc_success(result: Value, id: Value) -> Value {
    json!({ "jsonrpc": "2.0", "result": result, "id": id })
}

fn rpc_failure(code: i32, message: &str, id: Value, data: Option<Value>) -> Value {
    let mut err = json!({ "code": code, "message": message });
    if let (Some(d), Some(o)) = (data, err.as_object_mut()) {
        o.insert("data".into(), d);
    }
    json!({ "jsonrpc": "2.0", "error": err, "id": id })
}

/// Диспетчер метода → результат.
async fn call_method(
    state: &AppState,
    name: &str,
    params: Map<String, Value>,
) -> anyhow::Result<Value> {
    let db = &state.db;
    let p = &params;
    match name {
        "channels.list" => {
            let limit = opt_u64(p, "limit")?;
            dispatch::list_channels(db, limit).await
        }
        "channels.get" => {
            let id = req_str(p, "id")?;
            dispatch::get_channel(db, &id).await
        }
        "channels.getMessages" => {
            let channel_id = req_str(p, "channelId")?;
            let limit = opt_u64(p, "limit")?;
            let offset = opt_u64(p, "offset")?;
            dispatch::get_messages(db, &channel_id, limit, offset).await
        }
        "feed.list" => {
            let limit = opt_u64(p, "limit")?;
            let offset = opt_u64(p, "offset")?;
            let post_type = opt_str(p, "postType")?;
            let is_viewed = p.get("isViewed").and_then(Value::as_bool);
            let after = opt_str(p, "after")?;
            let before = opt_str(p, "before")?;
            dispatch::list_feed(
                db,
                limit,
                offset,
                post_type.as_deref(),
                is_viewed,
                after,
                before,
            )
            .await
        }
        "feed.get" => {
            let id = req_str(p, "id")?;
            dispatch::get_feed(db, &id).await
        }
        "feed.getMessages" => {
            let id = req_str(p, "id")?;
            dispatch::get_feed_messages(db, &id).await
        }
        "feed.markViewed" => {
            let id = req_str(p, "id")?;
            dispatch::mark_feed_viewed(db, &id).await
        }
        "config.list" => {
            let after = opt_str(p, "after")?;
            let before = opt_str(p, "before")?;
            dispatch::list_config(db, after, before).await
        }
        "config.get" => {
            let slug = req_str(p, "slug")?;
            dispatch::get_config(db, &slug).await
        }
        "config.set" => {
            let slug = req_str(p, "slug")?;
            let value = req_str(p, "value")?;
            dispatch::set_config(db, &slug, &value).await
        }
        "prompts.list" => {
            let tags = p.get("tags").and_then(Value::as_array).map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            });
            let after = opt_str(p, "after")?;
            let before = opt_str(p, "before")?;
            dispatch::list_prompts(db, tags, after, before).await
        }
        "prompts.get" => {
            let id = req_str(p, "id")?;
            dispatch::get_prompt(db, &id).await
        }
        "prompts.create" => {
            let content = req_str(p, "content")?;
            let tags = tags_param(p)?;
            dispatch::create_prompt(db, &content, tags).await
        }
        "prompts.update" => {
            let id = req_str(p, "id")?;
            let content = req_str(p, "content")?;
            let tags = tags_param(p)?;
            dispatch::update_prompt(db, &id, &content, tags).await
        }
        "prompts.history" => {
            let id = req_str(p, "id")?;
            dispatch::get_prompt_history(db, &id).await
        }
        "prompts.render" => {
            let id = req_str(p, "id")?;
            let vars = p.get("vars").cloned().unwrap_or_else(|| json!({}));
            dispatch::render_prompt(db, &id, vars).await
        }
        other => Err(anyhow::anyhow!("unknown_method:{other}")),
    }
}

fn req_str(p: &Map<String, Value>, key: &str) -> anyhow::Result<String> {
    p.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .context(format!("missing param {key}"))
}

fn opt_str(p: &Map<String, Value>, key: &str) -> anyhow::Result<Option<String>> {
    Ok(p.get(key).and_then(Value::as_str).map(str::to_owned))
}

fn opt_u64(p: &Map<String, Value>, key: &str) -> anyhow::Result<Option<u64>> {
    Ok(p.get(key)
        .and_then(Value::as_i64)
        .and_then(|v| u64::try_from(v).ok()))
}

fn tags_param(p: &Map<String, Value>) -> anyhow::Result<Vec<String>> {
    Ok(p.get("tags")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default())
}

/// Убираем дублирование: реальные версии ниже.
async fn handle_rpc_inner2(state: &AppState, body: Value) -> Result<Value, Value> {
    let obj = match body.as_object() {
        Some(o) => o,
        None => return Err(rpc_failure(-32600, "Invalid Request", Value::Null, None)),
    };
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(rpc_failure(-32600, "Invalid Request", Value::Null, None));
    }
    let id = obj.get("id").cloned().unwrap_or(Value::Null);
    let method = match obj.get("method").and_then(Value::as_str) {
        Some(m) => m.to_owned(),
        None => {
            return Err(rpc_failure(-32600, "Invalid Request", id, None));
        }
    };
    let params = match obj.get("params") {
        Some(Value::Object(m)) => m.clone(),
        None => Map::new(),
        Some(_) => {
            return Err(rpc_failure(
                -32602,
                "Invalid params: must be object",
                id,
                None,
            ));
        }
    };

    let Some(def) = find(&method) else {
        return Err(rpc_failure(-32601, "Method not found", id, None));
    };
    if let Err(e) = validate_params(def.get(), &params) {
        return Err(rpc_failure(
            -32602,
            &format!("Invalid params: {e}"),
            id,
            None,
        ));
    }

    // Логируем запрос
    println!(
        "[rpc] → {method} {}",
        serde_json::to_string(&params).unwrap_or_default()
    );

    match call_method(state, &method, params).await {
        Ok(result) => Ok(rpc_success(result, id)),
        Err(e) => {
            if let Some((code, message)) = dispatch::try_parse_rpc_error(&e) {
                Err(rpc_failure(code, &message, id, None))
            } else {
                Err(rpc_failure(
                    -32603,
                    "Internal error",
                    id,
                    Some(json!(format!("{e:#}"))),
                ))
            }
        }
    }
}
