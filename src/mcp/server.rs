use std::io::{self, BufRead, BufReader, Write};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::mcp::tools::{call_tool, tool_definitions};

/// Run the stdio MCP JSON-RPC server.
pub fn serve() -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout();

    while let Some(body) = read_frame(&mut reader)? {
        let response = handle_request(&paths, &body);
        if let Some(response) = response {
            write_frame(&mut stdout, &response)?;
        }
    }

    Ok(())
}

fn read_frame(reader: &mut impl BufRead) -> Result<Option<String>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .context("failed to read MCP frame header")?;
        if bytes == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .context("invalid MCP Content-Length")?,
            );
        }
    }

    let Some(content_length) = content_length else {
        bail!("missing MCP Content-Length header");
    };
    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .context("failed to read MCP frame body")?;
    String::from_utf8(body)
        .context("MCP frame body was not UTF-8")
        .map(Some)
}

fn write_frame(writer: &mut impl Write, response: &Value) -> Result<()> {
    let body = serde_json::to_vec(response).context("failed to encode MCP response")?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())
        .context("failed to write MCP response header")?;
    writer
        .write_all(&body)
        .context("failed to write MCP response body")?;
    writer.flush().context("failed to flush MCP response")
}

fn handle_request(paths: &MaestroPaths, body: &str) -> Option<Value> {
    let request = match serde_json::from_str::<Value>(body) {
        Ok(request) => request,
        Err(error) => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {"code": -32700, "message": error.to_string()}
            }));
        }
    };

    if let Some(batch) = request.as_array() {
        if batch.is_empty() {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": Value::Null,
                "error": {"code": -32600, "message": "empty batch"}
            }));
        }
        let responses = batch
            .iter()
            .filter_map(|request| handle_request_value(paths, request))
            .collect::<Vec<_>>();
        return if responses.is_empty() {
            None
        } else {
            Some(Value::Array(responses))
        };
    }

    handle_request_value(paths, &request)
}

fn handle_request_value(paths: &MaestroPaths, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return id.map(|id| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32600, "message": "missing method"}
            })
        });
    };

    match method {
        "initialize" => id.map(|id| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "maestro", "version": env!("MAESTRO_BUILD_VERSION")}
                }
            })
        }),
        "notifications/initialized" => None,
        "tools/list" => id.map(|id| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"tools": tools_json()}
            })
        }),
        "tools/call" => id.map(|id| tool_call_response(paths, id, request.get("params"))),
        _ => id.map(|id| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": format!("unknown method: {method}")}
            })
        }),
    }
}

fn tools_json() -> Vec<Value> {
    tool_definitions()
        .into_iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.input_schema
            })
        })
        .collect()
}

fn tool_call_response(paths: &MaestroPaths, id: Value, params: Option<&Value>) -> Value {
    let Some(params) = params else {
        return invalid_params(id, "missing params");
    };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return invalid_params(id, "missing tool name");
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match call_tool(paths, name, &arguments) {
        Ok(text) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {"content": [{"type": "text", "text": text}]}
        }),
        Err(error) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32000, "message": error.to_string()}
        }),
    }
}

fn invalid_params(id: Value, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": -32602, "message": message}
    })
}
