//! A small MCP server over stdio. The tests start it inside the sandbox.
//!
//! The first argument selects a mode:
//! - `normal`: answers like a server with a handful of tools.
//! - `chatty`: sends a request and notifications of its own before it answers.
//! - `hang`: never answers `initialize`.
//! - `crash`: exits after it lists its tools.
//! - `old`: answers `initialize` with an unsupported protocol version.
//! - `paged`: lists its tools over two pages.
use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn send(message: &Value) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{message}").unwrap();
    out.flush().unwrap();
}

fn schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn tools(mode: &str) -> Vec<Value> {
    let mut all = vec![
        json!({
            "name": "echo",
            "description": "Echo the text back.",
            "inputSchema": schema(json!({ "text": { "type": "string" } }), &["text"])
        }),
        json!({
            "name": "fail",
            "description": "Always reports an error.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({
            "name": "slow",
            "description": "Waits for a long time.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({
            "name": "die",
            "description": "Exits the server.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({
            "name": "big",
            "description": "Returns a large text.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({
            "name": "read_file",
            "description": "Reads a file by path.",
            "inputSchema": schema(json!({ "path": { "type": "string" } }), &["path"])
        }),
        json!({
            "name": "env",
            "description": "Lists the names of the environment variables.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({
            "name": "mixed",
            "description": "Returns several kinds of content.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({
            "name": "structured",
            "description": "Returns only structured content.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({
            "name": "dup.name",
            "description": "Collides with dup_name after cleaning.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({
            "name": "dup_name",
            "description": "Collides with dup.name after cleaning.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({
            "name": "bad_schema",
            "description": "Has a schema that is not an object schema.",
            "inputSchema": { "type": "string" }
        }),
        json!({
            "name": "no_schema",
            "description": "Has no schema."
        }),
        json!({
            "name": "has:colon",
            "description": "Has a colon in its name.",
            "inputSchema": schema(json!({}), &[])
        }),
        json!({ "name": "plain", "inputSchema": schema(json!({}), &[]) }),
    ];
    if mode == "paged" {
        all.truncate(2);
    }
    all
}

fn call(name: &str, arguments: &Value) -> Value {
    match name {
        "echo" => json!({
            "content": [{ "type": "text", "text": format!("echo: {}", arguments["text"].as_str().unwrap_or_default()) }]
        }),
        "die" => std::process::exit(7),
        "fail" => json!({
            "isError": true,
            "content": [{ "type": "text", "text": "something broke" }]
        }),
        "slow" => {
            std::thread::sleep(std::time::Duration::from_secs(30));
            json!({ "content": [{ "type": "text", "text": "late" }] })
        }
        "big" => json!({
            "content": [{ "type": "text", "text": "x".repeat(200_000) }]
        }),
        "read_file" => {
            let path = arguments["path"].as_str().unwrap_or_default();
            match std::fs::read_to_string(path) {
                Ok(text) => json!({ "content": [{ "type": "text", "text": text }] }),
                Err(error) => json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": format!("cannot read: {}", error.kind()) }]
                }),
            }
        }
        "env" => {
            let mut names: Vec<String> = std::env::vars().map(|(name, _)| name).collect();
            names.sort();
            json!({ "content": [{ "type": "text", "text": names.join(",") }] })
        }
        "mixed" => json!({
            "content": [
                { "type": "text", "text": "first" },
                { "type": "image", "data": "AAAA", "mimeType": "image/png" },
                { "type": "resource", "resource": { "uri": "file:///a", "text": "resource text" } },
                { "type": "resource", "resource": { "uri": "file:///b", "blob": "AAAA" } },
                { "type": "resource_link", "uri": "file:///c" },
                { "type": "text", "text": "last" }
            ]
        }),
        "structured" => json!({ "structuredContent": { "answer": 42 } }),
        _ => json!({ "content": [{ "type": "text", "text": "ok" }] }),
    }
}

fn main() {
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "normal".to_string());
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let (Some(method), id) = (message["method"].as_str(), message.get("id")) else {
            continue;
        };
        let Some(id) = id else {
            continue;
        };
        match method {
            "initialize" => {
                if mode == "hang" {
                    continue;
                }
                if mode == "chatty" {
                    println!("this line is not JSON");
                    send(
                        &json!({ "jsonrpc": "2.0", "method": "notifications/message", "params": { "level": "info", "data": "hello" } }),
                    );
                    send(&json!({ "jsonrpc": "2.0", "id": 900, "method": "roots/list" }));
                }
                let version = if mode == "old" {
                    "1999-01-01"
                } else {
                    "2025-03-26"
                };
                send(&json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": version,
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "fake", "version": "1" }
                    }
                }));
            }
            "tools/list" => {
                let cursor = message["params"]["cursor"].as_str();
                let all = tools(&mode);
                let (page, next) = match (mode.as_str(), cursor) {
                    ("paged", None) => (all[..1].to_vec(), Some("page2")),
                    ("paged", Some(_)) => (all[1..].to_vec(), None),
                    _ => (all, None),
                };
                let mut result = json!({ "tools": page });
                if let Some(next) = next {
                    result["nextCursor"] = json!(next);
                }
                send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
                if mode == "crash" {
                    std::process::exit(3);
                }
            }
            "tools/call" => {
                let name = message["params"]["name"].as_str().unwrap_or_default();
                let arguments = &message["params"]["arguments"];
                send(&json!({ "jsonrpc": "2.0", "id": id, "result": call(name, arguments) }));
            }
            _ => send(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "Method not found" }
            })),
        }
    }
}
