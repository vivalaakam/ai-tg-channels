//! Реестр JSON-RPC методов + параметры (аналог methods*.ts).
use serde_json::{Map, Value, json};

pub struct MethodDef {
    pub name: &'static str,
    pub description: &'static str,
    /// (name, required, json_schema)
    pub params: Vec<ParamDef>,
    pub result_schema: Value,
}

pub struct ParamDef {
    pub name: &'static str,
    pub required: bool,
    pub schema: Value,
    pub description: &'static str,
}

fn channel_shape() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "title": { "type": "string" },
            "username": { "type": ["string", "null"] },
            "createdAt": { "type": "string", "format": "date-time" },
        }
    })
}

fn feed_shape() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "format": "uuid" },
            "text": { "type": "string" },
            "isViewed": { "type": "boolean" },
            "postType": { "type": ["string", "null"] },
            "firstSeenAt": { "type": "string", "format": "date-time" },
            "channelId": { "type": "string" },
            "createdAt": { "type": "string", "format": "date-time" },
        }
    })
}

fn config_shape() -> Value {
    json!({
        "type": "object",
        "properties": { "slug": { "type": "string" }, "value": { "type": "string" } }
    })
}

fn prompt_shape() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string", "format": "uuid" },
            "content": { "type": "string" },
            "version": { "type": "integer" },
            "tags": { "type": "array", "items": { "type": "string" } },
            "previousId": { "type": ["string", "null"], "format": "uuid" },
        }
    })
}

pub fn methods() -> Vec<MethodDef> {
    vec![
        MethodDef {
            name: "channels.list",
            description: "Get list of all channels",
            params: vec![ParamDef {
                name: "limit",
                required: false,
                schema: json!({ "type": "integer", "default": 100, "description": "Max channels to return" }),
                description: "Max channels to return",
            }],
            result_schema: json!({ "type": "array", "items": channel_shape() }),
        },
        MethodDef {
            name: "channels.get",
            description: "Get channel info by ID",
            params: vec![ParamDef {
                name: "id",
                required: true,
                schema: json!({ "type": "string", "description": "Channel ID" }),
                description: "Channel ID",
            }],
            result_schema: channel_shape(),
        },
        MethodDef {
            name: "channels.getMessages",
            description: "Get last messages from a channel",
            params: vec![
                ParamDef {
                    name: "channelId",
                    required: true,
                    schema: json!({ "type": "string", "description": "Channel ID" }),
                    description: "Channel ID",
                },
                ParamDef {
                    name: "limit",
                    required: false,
                    schema: json!({ "type": "integer", "default": 20, "description": "Max messages to return" }),
                    description: "Max messages to return (default 20)",
                },
                ParamDef {
                    name: "offset",
                    required: false,
                    schema: json!({ "type": "integer", "default": 0, "description": "Offset for pagination" }),
                    description: "Offset for pagination (default 0)",
                },
            ],
            result_schema: json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "channelId": { "type": "string" },
                        "messageId": { "type": "integer" },
                        "date": { "type": "string", "format": "date-time" },
                        "editDate": { "type": ["string", "null"], "format": "date-time" },
                        "contentTextText": { "type": ["string", "null"] },
                        "isPinned": { "type": "boolean" },
                    }
                }
            }),
        },
        MethodDef {
            name: "feed.list",
            description: "List feed items (deduplicated news)",
            params: vec![
                ParamDef {
                    name: "limit",
                    required: false,
                    schema: json!({ "type": "integer", "default": 20, "description": "Max items to return" }),
                    description: "Max items to return (default 20)",
                },
                ParamDef {
                    name: "offset",
                    required: false,
                    schema: json!({ "type": "integer", "default": 0, "description": "Pagination offset" }),
                    description: "Pagination offset (default 0)",
                },
                ParamDef {
                    name: "postType",
                    required: false,
                    schema: json!({ "type": "string", "enum": ["ad", "news", "post", "meme"], "description": "Post type filter" }),
                    description: "Filter by post type: ad, news, post, meme",
                },
                ParamDef {
                    name: "isViewed",
                    required: false,
                    schema: json!({ "type": "boolean", "description": "Viewed status filter" }),
                    description: "Filter by viewed status",
                },
                ParamDef {
                    name: "after",
                    required: false,
                    schema: json!({ "type": "string", "format": "date-time", "description": "Filter: firstSeenAt after" }),
                    description: "Return items with firstSeenAt after this ISO 8601 datetime",
                },
                ParamDef {
                    name: "before",
                    required: false,
                    schema: json!({ "type": "string", "format": "date-time", "description": "Filter: firstSeenAt before" }),
                    description: "Return items with firstSeenAt before this ISO 8601 datetime",
                },
            ],
            result_schema: json!({ "type": "array", "items": feed_shape() }),
        },
        MethodDef {
            name: "feed.get",
            description: "Get a single feed item by ID",
            params: vec![ParamDef {
                name: "id",
                required: true,
                schema: json!({ "type": "string", "format": "uuid", "description": "Feed item UUID" }),
                description: "Feed item UUID",
            }],
            result_schema: feed_shape(),
        },
        MethodDef {
            name: "feed.getMessages",
            description: "Get all source messages linked to a feed item, with Telegram links",
            params: vec![ParamDef {
                name: "id",
                required: true,
                schema: json!({ "type": "string", "format": "uuid", "description": "Feed item UUID" }),
                description: "Feed item UUID",
            }],
            result_schema: json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "channelId": { "type": "string" },
                        "messageId": { "type": "integer" },
                        "tgLink": { "type": "string", "format": "uri" },
                    }
                }
            }),
        },
        MethodDef {
            name: "feed.markViewed",
            description: "Mark a feed item as viewed",
            params: vec![ParamDef {
                name: "id",
                required: true,
                schema: json!({ "type": "string", "format": "uuid", "description": "Feed item UUID" }),
                description: "Feed item UUID",
            }],
            result_schema: json!({ "type": "object", "properties": { "ok": { "type": "boolean" } } }),
        },
        MethodDef {
            name: "config.list",
            description: "List all config entries",
            params: vec![
                ParamDef {
                    name: "after",
                    required: false,
                    schema: json!({ "type": "string", "format": "date-time", "description": "Filter: updatedAt after" }),
                    description: "Return entries updated after this ISO 8601 datetime",
                },
                ParamDef {
                    name: "before",
                    required: false,
                    schema: json!({ "type": "string", "format": "date-time", "description": "Filter: updatedAt before" }),
                    description: "Return entries updated before this ISO 8601 datetime",
                },
            ],
            result_schema: json!({ "type": "array", "items": config_shape() }),
        },
        MethodDef {
            name: "config.get",
            description: "Get config value by slug",
            params: vec![ParamDef {
                name: "slug",
                required: true,
                schema: json!({ "type": "string", "description": "Config key slug" }),
                description: "Config key slug",
            }],
            result_schema: config_shape(),
        },
        MethodDef {
            name: "config.set",
            description: "Set (upsert) a config value",
            params: vec![
                ParamDef {
                    name: "slug",
                    required: true,
                    schema: json!({ "type": "string", "description": "Config key slug" }),
                    description: "Config key slug",
                },
                ParamDef {
                    name: "value",
                    required: true,
                    schema: json!({ "type": "string", "description": "Value to store" }),
                    description: "Value to store (e.g. a prompt UUID)",
                },
            ],
            result_schema: config_shape(),
        },
        MethodDef {
            name: "prompts.list",
            description: "List prompts, optionally filtered by tags and/or date range",
            params: vec![
                ParamDef {
                    name: "tags",
                    required: false,
                    schema: json!({ "type": "array", "items": { "type": "string" }, "description": "Tag filter" }),
                    description: "Filter prompts that have any of these tags",
                },
                ParamDef {
                    name: "after",
                    required: false,
                    schema: json!({ "type": "string", "format": "date-time", "description": "Filter: createdAt after" }),
                    description: "Return prompts created after this ISO 8601 datetime",
                },
                ParamDef {
                    name: "before",
                    required: false,
                    schema: json!({ "type": "string", "format": "date-time", "description": "Filter: createdAt before" }),
                    description: "Return prompts created before this ISO 8601 datetime",
                },
            ],
            result_schema: json!({ "type": "array", "items": prompt_shape() }),
        },
        MethodDef {
            name: "prompts.get",
            description: "Get prompt by UUID",
            params: vec![ParamDef {
                name: "id",
                required: true,
                schema: json!({ "type": "string", "format": "uuid", "description": "Prompt UUID" }),
                description: "Prompt UUID",
            }],
            result_schema: prompt_shape(),
        },
        MethodDef {
            name: "prompts.create",
            description: "Create a new prompt (version 1)",
            params: vec![
                ParamDef {
                    name: "content",
                    required: true,
                    schema: json!({ "type": "string", "description": "Prompt text content" }),
                    description: "Prompt text content (supports {{var}} template syntax)",
                },
                ParamDef {
                    name: "tags",
                    required: false,
                    schema: json!({ "type": "array", "items": { "type": "string" }, "description": "Tags" }),
                    description: "Tags for searching and grouping",
                },
            ],
            result_schema: prompt_shape(),
        },
        MethodDef {
            name: "prompts.update",
            description: "Create a new version of a prompt (previousId links to old)",
            params: vec![
                ParamDef {
                    name: "id",
                    required: true,
                    schema: json!({ "type": "string", "format": "uuid", "description": "Source prompt UUID" }),
                    description: "UUID of the prompt to base new version on",
                },
                ParamDef {
                    name: "content",
                    required: true,
                    schema: json!({ "type": "string", "description": "New content" }),
                    description: "New prompt text content",
                },
                ParamDef {
                    name: "tags",
                    required: false,
                    schema: json!({ "type": "array", "items": { "type": "string" }, "description": "Tags" }),
                    description: "Tags for the new version",
                },
            ],
            result_schema: prompt_shape(),
        },
        MethodDef {
            name: "prompts.history",
            description: "Get version chain for a prompt (newest first)",
            params: vec![ParamDef {
                name: "id",
                required: true,
                schema: json!({ "type": "string", "format": "uuid", "description": "Prompt UUID" }),
                description: "Any prompt UUID in the chain",
            }],
            result_schema: json!({ "type": "array", "items": prompt_shape() }),
        },
        MethodDef {
            name: "prompts.render",
            description: "Render a prompt with template variables",
            params: vec![
                ParamDef {
                    name: "id",
                    required: true,
                    schema: json!({ "type": "string", "format": "uuid", "description": "Prompt UUID" }),
                    description: "Prompt UUID",
                },
                ParamDef {
                    name: "vars",
                    required: false,
                    schema: json!({ "type": "object", "additionalProperties": { "type": "string" }, "description": "Template variables" }),
                    description: "Template variables ({{key}} → value)",
                },
            ],
            result_schema: json!({ "type": "object", "properties": { "rendered": { "type": "string" } } }),
        },
    ]
}

pub fn find(name: &str) -> Option<MethodRef> {
    let list = methods();
    let idx = list.iter().position(|m| m.name == name)?;
    Some(MethodRef { list, idx })
}

/// Ссылка на элемент в глобальном метод-списке.
pub struct MethodRef {
    list: Vec<MethodDef>,
    idx: usize,
}

impl MethodRef {
    pub fn get(&self) -> &MethodDef {
        &self.list[self.idx]
    }
}

/// renderTemplate для методов (используется dispatch::render_prompt).
pub fn render_template(content: &str, vars: &Map<String, Value>) -> String {
    let mut out = content.to_owned();
    for (k, v) in vars {
        let pat = format!("{{{{{k}}}}}");
        out = out.replace(&pat, v.as_str().unwrap_or(""));
    }
    out
}

/// Валидация параметров против списка (type/check по JSON Schema минимально).
pub fn validate_params(m: &MethodDef, params: &Map<String, Value>) -> Result<(), String> {
    for p in &m.params {
        let val = params.get(p.name);
        match (p.required, val) {
            (true, None) | (true, Some(Value::Null)) => {
                return Err(format!("{}: required parameter missing", p.name));
            }
            _ => {}
        }
    }
    Ok(())
}

// Тест: методы уникальны и dispatch содержит все обработчики (компиляционный тест вызова)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_unique() {
        let list = methods();
        let mut names: Vec<_> = list.iter().map(|m| m.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len());
    }

    #[test]
    fn all_known_methods_present() {
        for name in [
            "channels.list",
            "channels.get",
            "channels.getMessages",
            "feed.list",
            "feed.get",
            "feed.getMessages",
            "feed.markViewed",
            "config.list",
            "config.get",
            "config.set",
            "prompts.list",
            "prompts.get",
            "prompts.create",
            "prompts.update",
            "prompts.history",
            "prompts.render",
        ] {
            assert!(find(name).is_some(), "missing {name}");
        }
    }

    #[test]
    fn renders() {
        let mut vars = Map::new();
        vars.insert("k".into(), json!("v"));
        assert_eq!(render_template("a {{k}} b {{zz}}", &vars), "a v b {{zz}}");
    }
}
