use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::domain::integration::LiveAuthenticatedHostConnectionV1;
use crate::domain::transport::{decode_packet_read_request, encode_packet_read_envelope};
use crate::foundation::core::paths::MaestroPaths;
use crate::operations::adapters::{
    LiveProjectionReadProviderV1, RunningBinaryIdentityV1, cli_search, decode_cli_search_request,
    encode_cli_search_envelope, packet_read,
};

/// MCP tool metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

/// Return the exact global read-only MCP registry.
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        tool(
            "maestro_packet",
            "Read one canonical bounded Packet projection from an explicit repository locator.",
            packet_schema(),
        ),
        tool(
            "maestro_cli_search",
            "Search the running binary's frozen public operation catalog without repository state.",
            cli_search_schema(),
        ),
    ]
}

/// Dispatch one canonical MCP tool without ambient host or repository discovery.
pub(crate) fn call_tool(
    name: &str,
    arguments: &Value,
    _live_host: &mut Option<&mut dyn LiveAuthenticatedHostConnectionV1>,
) -> Result<String> {
    match name {
        "maestro_packet" => packet(arguments),
        "maestro_cli_search" => search(arguments),
        _ => bail!("unknown MCP tool: {name}"),
    }
}

fn packet(arguments: &Value) -> Result<String> {
    let encoded = serde_json::to_string(arguments).context("failed to encode packet request")?;
    let request = decode_packet_read_request(&encoded)
        .context("maestro_packet requires one exact McpPacketReadRequestV1")?;
    let root = explicit_repository_root(&request.repository_locator)?;
    let provider = LiveProjectionReadProviderV1::load(MaestroPaths::new(root))
        .map_err(|_| anyhow!("failed to establish the live projection provider"))?;
    let envelope =
        packet_read(&provider, &request).map_err(|_| anyhow!("packet projection was rejected"))?;
    encode_packet_read_envelope(&envelope).map_err(Into::into)
}

fn search(arguments: &Value) -> Result<String> {
    let encoded =
        serde_json::to_string(arguments).context("failed to encode CLI search request")?;
    let request = decode_cli_search_request(&encoded)
        .map_err(|_| anyhow!("maestro_cli_search requires one exact McpCliSearchRequestV1"))?;
    let running_binary = RunningBinaryIdentityV1::load()
        .map_err(|_| anyhow!("failed to establish the running binary identity"))?;
    let envelope = cli_search(&request, &running_binary)
        .map_err(|_| anyhow!("CLI catalog search was rejected"))?;
    encode_cli_search_envelope(&envelope, &request)
        .map_err(|_| anyhow!("CLI catalog search envelope was rejected"))
}

fn explicit_repository_root(repository_locator: &str) -> Result<PathBuf> {
    let supplied = Path::new(repository_locator);
    if !supplied.is_absolute() {
        bail!("repository_locator must be one explicit absolute path");
    }
    let canonical = supplied
        .canonicalize()
        .context("repository_locator must identify an existing repository root")?;
    if canonical != supplied {
        bail!("repository_locator must be alias-closed canonical path");
    }
    Ok(canonical)
}

fn tool(name: &'static str, description: &'static str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name,
        description,
        input_schema,
    }
}

fn packet_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "schema_version",
            "request_id",
            "repository_locator",
            "authenticated_host_connection_context_ref",
            "projection_scope",
            "expected_release_ref",
            "expected_public_catalog_ref",
            "bounded_response_redaction_profile",
            "read_mode"
        ],
        "properties": {
            "schema_version": {"const": 1},
            "request_id": {"type": "string"},
            "repository_locator": {"type": "string"},
            "authenticated_host_connection_context_ref": {"type": "string"},
            "projection_scope": {"type": "object"},
            "expected_release_ref": {"type": "string"},
            "expected_public_catalog_ref": {"type": "string"},
            "bounded_response_redaction_profile": {"type": "string"},
            "read_mode": {"type": "object"}
        },
        "additionalProperties": false
    })
}

fn cli_search_schema() -> Value {
    json!({
        "type": "object",
        "required": [
            "schema_version",
            "request_id",
            "query",
            "finite_bound",
            "expected_release_ref",
            "expected_public_catalog_ref"
        ],
        "properties": {
            "schema_version": {"const": 1},
            "request_id": {"type": "string"},
            "query": {"type": "object"},
            "finite_bound": {"type": "integer", "minimum": 1},
            "expected_release_ref": {"type": "string"},
            "expected_public_catalog_ref": {"type": "string"}
        },
        "additionalProperties": false
    })
}
