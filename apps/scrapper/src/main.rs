//! Telegram-скрапер: sync непрочитанных + live-подписка (аналог scrapper/src/main.ts).
mod db;
mod td;

use std::time::Duration;

use anyhow::Context;
use common::{init_tracing, load_env, optional, require, require_i64};
use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DatabaseConnection};
use serde_json::{Value, json};

use td::TdClient;

const REQUEST_TIMEOUT: f64 = 30.0;

fn tdjson_path() -> anyhow::Result<String> {
    if let Some(p) = optional("TDJSON_PATH") {
        return Ok(p);
    }
    // дефолт: prebuilt-tdlib из node_modules TS-проекта (macOS aarch64)
    let candidates = [
        "/Users/vivalaakam/work/ai-tg-channels/node_modules/.pnpm/@prebuilt-tdlib+darwin-arm64@0.1008065.0/node_modules/@prebuilt-tdlib/darwin-arm64/libtdjson.dylib",
        "/opt/homebrew/lib/libtdjson.dylib",
        "/usr/local/lib/libtdjson.dylib",
    ];
    for c in candidates {
        if std::path::Path::new(c).exists() {
            return Ok(c.to_owned());
        }
    }
    anyhow::bail!("libtdjson not found. Install TDLib (brew install tdlib) or set TDJSON_PATH.")
}

fn read_line(prompt: &str) -> anyhow::Result<String> {
    use std::io::BufRead;
    print!("{prompt}");
    use std::io::Write;
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_owned())
}

/// Извлекает username канала из supergroup-ответа.
fn channel_username(supergroup: &Value) -> Option<String> {
    let sg = supergroup.as_object()?;
    if let Some(usernames) = sg.get("usernames")
        && let Some(arr) = usernames.get("active_usernames").and_then(Value::as_array)
        && let Some(first) = arr.first().and_then(Value::as_str)
    {
        return Some(first.to_owned());
    }
    sg.get("username")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Обработка одного входящего сообщения.
async fn handle_message(
    td: &TdClient,
    db: &DatabaseConnection,
    chat_id: i64,
    msg: &Value,
) -> anyhow::Result<()> {
    let chat = td.request("getChat", &json!({ "chat_id": chat_id }), REQUEST_TIMEOUT)?;

    let chat_type = chat
        .get("type")
        .and_then(|t| t.get("@type"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if chat_type != "chatTypeSupergroup" {
        return Ok(());
    }
    let is_channel = chat
        .pointer("/type/is_channel")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !is_channel {
        return Ok(());
    }

    let title = chat.get("title").and_then(Value::as_str).unwrap_or("");
    let supergroup_id = chat
        .pointer("/type/supergroup_id")
        .and_then(Value::as_i64)
        .context("no supergroup_id")?;
    let supergroup = td.request(
        "getSupergroup",
        &json!({ "supergroup_id": supergroup_id }),
        REQUEST_TIMEOUT,
    )?;

    db::upsert_channel(db, chat_id, title, channel_username(&supergroup)).await?;

    let msg_id = msg.get("id").and_then(Value::as_i64).unwrap_or_default();
    let saved = db::save_message(db, chat_id, msg_id, msg).await?;
    if saved {
        println!("[{title}] saved msg {msg_id}");
    }

    td.send(
        "viewMessages",
        &json!({
            "chat_id": chat_id,
            "message_ids": [msg_id],
            "force_read": true,
        }),
    )?;
    Ok(())
}

/// Разовая синхронизация непрочитанных сообщений из main chat list.
async fn sync_unread(td: &TdClient, db: &DatabaseConnection) -> anyhow::Result<()> {
    let chat_list = td.request(
        "getChats",
        &json!({ "chat_list": { "@type": "chatListMain" }, "limit": 500 }),
        REQUEST_TIMEOUT,
    )?;

    let chat_ids: Vec<i64> = chat_list
        .get("chat_ids")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_i64).collect())
        .unwrap_or_default();

    for chat_id in chat_ids {
        let Ok(chat) = td.request("getChat", &json!({ "chat_id": chat_id }), REQUEST_TIMEOUT)
        else {
            println!("[sync] getChat {chat_id} failed (no response)");
            continue;
        };

        let chat_type = chat
            .get("type")
            .and_then(|t| t.get("@type"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if chat_type != "chatTypeSupergroup" {
            continue;
        }
        let is_channel = chat
            .pointer("/type/is_channel")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !is_channel {
            continue;
        }

        let unread_count = chat
            .get("unread_count")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if unread_count == 0 {
            continue;
        }

        let title = chat.get("title").and_then(Value::as_str).unwrap_or("");
        println!("[{title}] syncing {unread_count} unread...");

        let supergroup_id = match chat.pointer("/type/supergroup_id").and_then(Value::as_i64) {
            Some(id) => id,
            None => continue,
        };
        let Ok(supergroup) = td.request(
            "getSupergroup",
            &json!({ "supergroup_id": supergroup_id }),
            REQUEST_TIMEOUT,
        ) else {
            println!("[sync] getSupergroup failed (no response)");
            continue;
        };
        db::upsert_channel(db, chat_id, title, channel_username(&supergroup)).await?;

        let Ok(history) = td.request(
            "getChatHistory",
            &json!({
                "chat_id": chat_id,
                "from_message_id": 0,
                "offset": 0,
                "limit": unread_count.min(100),
                "only_local": false,
            }),
            REQUEST_TIMEOUT,
        ) else {
            println!("[sync] getChatHistory failed");
            continue;
        };

        let last_read = chat
            .get("last_read_inbox_message_id")
            .and_then(Value::as_i64)
            .unwrap_or(0);

        let mut message_ids = Vec::new();
        for msg in history
            .get("messages")
            .and_then(Value::as_array)
            .unwrap_or(&vec![])
        {
            let msg_id = msg.get("id").and_then(Value::as_i64).unwrap_or(0);
            if msg_id <= last_read {
                continue;
            }
            let saved = db::save_message(db, chat_id, msg_id, msg).await?;
            if saved {
                println!("  saved msg {msg_id}");
            }
            message_ids.push(msg_id);
        }

        if !message_ids.is_empty() {
            td.send(
                "viewMessages",
                &json!({
                    "chat_id": chat_id,
                    "message_ids": message_ids,
                    "force_read": true,
                }),
            )?;
        }
    }
    Ok(())
}

async fn authorize(td: &TdClient) -> anyhow::Result<()> {
    let phone = require("TG_PHONE");
    loop {
        let event = td.request("getAuthorizationState", &json!({}), REQUEST_TIMEOUT)?;
        // TDLib асинхронно шлёт updateAuthorizationState; getAuthorizationState
        // возвращает текущее состояние синхронно.
        let state = event
            .get("@type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();

        match state.as_str() {
            "authorizationStateWaitPhoneNumber" => {
                td.request(
                    "setAuthenticationPhoneNumber",
                    &json!({ "phone_number": phone, "allow_flash_call": false }),
                    REQUEST_TIMEOUT,
                )?;
            }
            "authorizationStateWaitCode" => {
                let code = read_line("Code (check Telegram app): ")?;
                td.request(
                    "checkAuthenticationCode",
                    &json!({ "code": code }),
                    REQUEST_TIMEOUT,
                )?;
            }
            "authorizationStateWaitPassword" => {
                let hint = event
                    .get("password_hint")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let password = read_line(&format!("2FA password (hint: {hint}): "))?;
                td.request(
                    "checkAuthenticationPassword",
                    &json!({ "password": password }),
                    REQUEST_TIMEOUT,
                )?;
            }
            "authorizationStateReady" => return Ok(()),
            "error" => anyhow::bail!("auth error: {event}"),
            _ => {
                // ждём updateAuthorizationState
                if let Some(u) = td.recv_update(Duration::from_secs(2)) {
                    let t = u.get("@type").and_then(Value::as_str).unwrap_or("");
                    if t == "updateAuthorizationState" {
                        let s = u
                            .pointer("/authorization_state/@type")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if s == "authorizationStateReady" {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env();
    init_tracing();

    let db_url = require("DATABASE_URL");
    let api_id: i32 = require_i64("TG_API_ID") as i32;
    let api_hash = require("TG_API_HASH");

    let db = Database::connect(&db_url).await.context("DB connect")?;
    Migrator::up(&db, None).await?;
    println!("DB ready");

    let path = tdjson_path()?;
    let td = TdClient::start(&path)?;

    let Some(root) = common::workspace_root() else {
        anyhow::bail!("cannot determine workspace root");
    };
    let td_dir = root.join(".tdlib");
    let files_dir = root.join(".tdlib-files");
    std::fs::create_dir_all(&td_dir)?;
    std::fs::create_dir_all(&files_dir)?;

    td.execute("setLogVerbosityLevel", &json!({ "new_verbosity_level": 1 }))?;

    // setTdlibParameters неявно: tdl делает это через login; здесь делаем руками.
    td.send(
        "setTdlibParameters",
        &json!({
            "use_test_dc": false,
            "database_directory": td_dir.to_string_lossy(),
            "files_directory": files_dir.to_string_lossy(),
            "database_encryption_key": "",
            "use_file_database": true,
            "use_chat_info_database": true,
            "use_message_database": true,
            "use_secret_chats": false,
            "api_id": api_id,
            "api_hash": api_hash,
            "system_language_code": "en",
            "device_model": "Desktop",
            "application_version": env!("CARGO_PKG_VERSION"),
        }),
    )?;

    println!("Authorizing...");
    authorize(&td).await?;
    println!("Telegram connected, syncing unread...");
    sync_unread(&td, &db).await?;
    println!("Sync done, listening for new messages...");

    // Основной цикл: апдейты → handle_message
    loop {
        let update = match td.recv_update(Duration::from_secs(1)) {
            Some(u) => u,
            None => continue,
        };
        let utype = update.get("@type").and_then(Value::as_str).unwrap_or("");
        if utype != "updateNewMessage" {
            continue;
        }
        let msg = match update.get("message") {
            Some(m) if m.is_object() => m.clone(),
            _ => continue,
        };
        let chat_id = match msg.get("chat_id").and_then(Value::as_i64) {
            Some(c) => c,
            None => continue,
        };

        if let Err(e) = handle_message(&td, &db, chat_id, &msg).await {
            println!("Error handling msg in {chat_id}: {e}");
        }
    }
}
