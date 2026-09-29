//! 测试替身：最小 stdio MCP server（newline-delimited JSON-RPC 2.0）。
//! 三个工具：`echo`（原样回显 text 参数）、`boom`（工具级失败
//! isError=true）、`slow`（睡 900ms 再答——配短 timeout_ms 验证超时）。
//! 仅测试使用：tool-mcp 单测与 plugin-host 集成测试都以
//! `CARGO_BIN_EXE_fake-mcp-server` 方式 spawn 本二进制。

use std::io::{BufRead, Write};

use plugin_api::serde_json::{Value, json};

fn main() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let Ok(msg) = plugin_api::serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = msg.get("id").cloned();
        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let resp = match (method.as_str(), id.is_some()) {
            ("initialize", true) => json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "fake-mcp-server", "version": "1.0.0" }
                }
            }),
            ("tools/list", true) => json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "tools": [
                    {
                        "name": "echo",
                        "description": "Echo the input text back",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "text": { "type": "string" } },
                            "required": ["text"]
                        }
                    },
                    {
                        "name": "boom",
                        "description": "Always fails at the tool level",
                        "inputSchema": { "type": "object" }
                    },
                    {
                        "name": "slow",
                        "description": "Sleeps 900ms then answers",
                        "inputSchema": { "type": "object" }
                    }
                ] }
            }),
            ("tools/call", true) => {
                let name = msg["params"]["name"].as_str().unwrap_or("");
                match name {
                    "echo" => json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": { "content": [
                            { "type": "text",
                              "text": format!("echo: {}", msg["params"]["arguments"]["text"].as_str().unwrap_or("")) }
                        ] }
                    }),
                    "boom" => json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": { "content": [{ "type": "text", "text": "boom!" }], "isError": true }
                    }),
                    "slow" => {
                        std::thread::sleep(std::time::Duration::from_millis(900));
                        json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": { "content": [{ "type": "text", "text": "finally" }] }
                        })
                    }
                    _ => json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32602, "message": "unknown tool" }
                    }),
                }
            }
            // notification（notifications/initialized 等）不回
            _ => continue,
        };
        writeln!(out, "{resp}").ok();
        out.flush().ok();
    }
}
