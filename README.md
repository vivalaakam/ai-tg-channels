# ai-tg-channels-rs

Rust-конвертация проекта `ai-tg-channels`: сбор сообщений из Telegram-каналов
(TDLib), складывание их в Postgres + pgvector, дедупликация новостей через
эмбеддинги и LLM, JSON-RPC 2.0 API.

## Структура

- `crates/migration` — SeaORM-миграции (схема повторяет TS-версию)
- `crates/entities` — SeaORM-сущности (channels, messages, feed, feed_messages, app_config, prompts)
- `crates/common` — env-загрузка, workspace-корень, tracing
- `apps/scrapper` — TDLib-клиент: sync непрочитанных + live-обновления
- `apps/feed` — процессор ленты: эмбеддинги, нормализация/дедуп через LLM
- `apps/jsonrpc` — axum HTTP, JSON-RPC 2.0 + OpenRPC-спека
- `apps/migrate` — CLI для применения миграций

## Запуск

```bash
cp .env.example .env   # заполнить креды
cargo run -p migrate   # применить миграции (нужен Postgres с pgvector)
cargo run -p scrapper  # Telegram-скрапер
cargo run -p feed      # процессор ленты
cargo run -p jsonrpc   # JSON-RPC на http://localhost:3002
```

## Бинарная зависимость TDLib

`scrapper` грузит `libtdjson.dylib` через libloading. Ожидается рядом в
`node_modules` пребилда (`@prebuilt-tdlib/darwin-arm64`) либо системная;
путь переопределяется `TDJSON_PATH`.

## Методы JSON-RPC

channels.list/get/getMessages, feed.list/get/getMessages/markViewed,
config.list/get/set, prompts.list/get/create/update/history/render.
Спека: `GET /openrpc.json`.
