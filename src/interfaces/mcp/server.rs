use std::io::{self, BufRead, BufReader, Write};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::interfaces::mcp::ProtectedPacketRuntimeV1;
use crate::interfaces::mcp::tools::{call_tool, tool_definitions};

const MAX_MCP_FRAME_BYTES: usize = 1024 * 1024;
const MAX_MCP_HEADER_BYTES: usize = 8 * 1024;
const MAX_MCP_HEADER_COUNT: usize = 32;

/// Run the stdio MCP JSON-RPC server.
pub fn serve() -> Result<()> {
    serve_stdio(None)
}

pub(crate) fn serve_with_protected_packet(
    runtime: &mut ProtectedPacketRuntimeV1<'_, '_, '_>,
) -> Result<()> {
    serve_stdio(Some(runtime))
}

fn serve_stdio(
    mut protected_packet: Option<&mut ProtectedPacketRuntimeV1<'_, '_, '_>>,
) -> Result<()> {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = io::stdout();

    while let Some(body) = read_message(&mut reader)? {
        let response = handle_request(&body, &mut protected_packet);
        if let Some(response) = response {
            write_frame(&mut stdout, &response)?;
        }
    }

    Ok(())
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<String>> {
    let buffer = reader.fill_buf().context("failed to read MCP input")?;
    if buffer.is_empty() {
        return Ok(None);
    }
    if buffer.starts_with(b"Content-Length:") {
        return read_frame(reader);
    }
    let Some(line) = read_bounded_line(
        reader,
        MAX_MCP_FRAME_BYTES,
        "MCP newline frame exceeds maximum size of 1048576 bytes",
        "failed to read JSON-RPC line",
    )?
    else {
        return Ok(None);
    };
    String::from_utf8(line)
        .context("MCP newline frame was not UTF-8")
        .map(Some)
}

fn read_frame(reader: &mut impl BufRead) -> Result<Option<String>> {
    let mut content_length = None;
    let mut header_bytes = 0usize;
    let mut header_count = 0usize;
    loop {
        let Some(line) = read_bounded_line(
            reader,
            MAX_MCP_HEADER_BYTES,
            "MCP frame header exceeds maximum size of 8192 bytes",
            "failed to read MCP frame header",
        )?
        else {
            return Ok(None);
        };
        header_bytes = header_bytes
            .checked_add(line.len())
            .filter(|total| *total <= MAX_MCP_HEADER_BYTES)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "MCP frame headers exceed maximum total size of {MAX_MCP_HEADER_BYTES} bytes"
                )
            })?;
        let line = std::str::from_utf8(&line).context("MCP frame header was not valid UTF-8")?;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        header_count = header_count.checked_add(1).ok_or_else(|| {
            anyhow::anyhow!("MCP frame exceeds maximum header count of {MAX_MCP_HEADER_COUNT}")
        })?;
        if header_count > MAX_MCP_HEADER_COUNT {
            bail!("MCP frame exceeds maximum header count of {MAX_MCP_HEADER_COUNT}");
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            if content_length.is_some() {
                bail!("duplicate MCP Content-Length header");
            }
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
    if content_length > MAX_MCP_FRAME_BYTES {
        bail!("MCP frame exceeds maximum size of {MAX_MCP_FRAME_BYTES} bytes");
    }
    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .context("failed to read MCP frame body")?;
    String::from_utf8(body)
        .context("MCP frame body was not UTF-8")
        .map(Some)
}

fn read_bounded_line(
    reader: &mut impl BufRead,
    maximum_bytes: usize,
    limit_error: &'static str,
    read_context: &'static str,
) -> Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf().context(read_context)?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(line))
            };
        }
        let consumed = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(buffer.len(), |index| index + 1);
        let complete = buffer[consumed - 1] == b'\n';
        if line
            .len()
            .checked_add(consumed)
            .is_none_or(|length| length > maximum_bytes)
        {
            bail!("{limit_error}");
        }
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        if complete {
            return Ok(Some(line));
        }
    }
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

fn handle_request(
    body: &str,
    protected_packet: &mut Option<&mut ProtectedPacketRuntimeV1<'_, '_, '_>>,
) -> Option<Value> {
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
            .filter_map(|request| handle_request_value(request, protected_packet))
            .collect::<Vec<_>>();
        return if responses.is_empty() {
            None
        } else {
            Some(Value::Array(responses))
        };
    }

    handle_request_value(&request, protected_packet)
}

fn handle_request_value(
    request: &Value,
    protected_packet: &mut Option<&mut ProtectedPacketRuntimeV1<'_, '_, '_>>,
) -> Option<Value> {
    let Some(request) = request.as_object() else {
        return Some(invalid_request(Value::Null));
    };
    let id = request.get("id").cloned();
    let response_id = id.clone().unwrap_or(Value::Null);
    if id
        .as_ref()
        .is_some_and(|id| !(id.is_string() || id.is_number() || id.is_null()))
        || request.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
    {
        return Some(invalid_request(response_id));
    }
    let Some(method) = request.get("method").and_then(Value::as_str) else {
        return Some(invalid_request(response_id));
    };
    let params = request.get("params");
    if params.is_some_and(|params| !(params.is_object() || params.is_array()))
        || (method == "tools/call"
            && params.and_then(Value::as_object).is_none_or(|params| {
                !params.get("name").is_some_and(Value::is_string)
                    || !params.get("arguments").is_some_and(Value::is_object)
            }))
    {
        return Some(invalid_request(response_id));
    };
    let tool_call = (method == "tools/call").then(|| {
        let params = params
            .and_then(Value::as_object)
            .expect("invariant: tools/call params were validated before dispatch");
        (
            params
                .get("name")
                .and_then(Value::as_str)
                .expect("invariant: tools/call name was validated before dispatch"),
            params
                .get("arguments")
                .expect("invariant: tools/call arguments were validated before dispatch"),
        )
    });

    match method {
        "initialize" => id.map(|id| {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "maestro", "version": env!("MAESTRO_VERSION")}
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
        "tools/call" => id.map(|id| {
            let (name, arguments) =
                tool_call.expect("invariant: tools/call shape was validated before dispatch");
            tool_call_response(id, name, arguments, protected_packet)
        }),
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

fn tool_call_response(
    id: Value,
    name: &str,
    arguments: &Value,
    protected_packet: &mut Option<&mut ProtectedPacketRuntimeV1<'_, '_, '_>>,
) -> Value {
    match call_tool(name, arguments, protected_packet.as_deref_mut()) {
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

fn invalid_request(id: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": -32600, "message": "invalid request"}
    })
}
