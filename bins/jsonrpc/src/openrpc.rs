//! OpenRPC-спека (аналог openrpc.ts).
use serde_json::{Value, json};

use crate::methods::methods;

pub fn openrpc_spec(port: u16) -> Value {
    let methods: Vec<Value> = methods()
        .iter()
        .map(|m| {
            let params: Vec<Value> = m
                .params
                .iter()
                .map(|p| {
                    let schema = p.schema.clone().as_object().cloned().unwrap_or_default();
                    json!({
                        "name": p.name,
                        "schema": schema,
                        "required": p.required,
                        "description": p.description,
                    })
                })
                .collect();
            json!({
                "name": m.name,
                "description": m.description,
                "params": params,
                "result": {
                    "name": "result",
                    "schema": m.result_schema,
                },
            })
        })
        .collect();

    json!({
        "openrpc": "1.2.6",
        "info": {
            "title": "ai-tg-channels",
            "version": "0.1.0",
        },
        "servers": [json!({ "url": format!("http://localhost:{port}") })],
        "methods": methods,
    })
}
