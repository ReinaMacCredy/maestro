use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::domain::transport::{decode_packet_read_request, encode_packet_read_envelope};
use crate::interfaces::mcp::ProtectedPacketRuntimeV1;
use crate::operations::adapters::{
    GLOBAL_MCP_TOOLS_V1, GlobalMcpAdapterKindV1, LiveProjectionReadProviderV1,
    RunningBinaryIdentityV1, cli_search, decode_cli_search_request, encode_cli_search_envelope,
    global_mcp_adapter, packet_read,
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
    GLOBAL_MCP_TOOLS_V1
        .iter()
        .map(|definition| ToolDefinition {
            name: definition.name,
            description: definition.description,
            input_schema: match definition.kind {
                GlobalMcpAdapterKindV1::Packet => packet_schema(),
                GlobalMcpAdapterKindV1::CliSearch => cli_search_schema(),
            },
        })
        .collect()
}

/// Dispatch one canonical MCP tool without ambient host or repository discovery.
pub(crate) fn call_tool(
    name: &str,
    arguments: &Value,
    protected_packet: Option<&mut ProtectedPacketRuntimeV1<'_, '_, '_>>,
) -> Result<String> {
    let adapter = global_mcp_adapter(name).ok_or_else(|| anyhow!("unknown MCP tool: {name}"))?;
    match adapter.kind {
        GlobalMcpAdapterKindV1::Packet => packet(arguments, protected_packet),
        GlobalMcpAdapterKindV1::CliSearch => search(arguments),
    }
}

fn packet(
    arguments: &Value,
    protected_packet: Option<&mut ProtectedPacketRuntimeV1<'_, '_, '_>>,
) -> Result<String> {
    let encoded = serde_json::to_string(arguments).context("failed to encode packet request")?;
    let request = decode_packet_read_request(&encoded)
        .context("maestro_packet requires one exact McpPacketReadRequestV1")?;
    let envelope = match protected_packet {
        Some(runtime) => runtime
            .read_packet(&request)
            .map_err(|_| anyhow!("packet projection was rejected"))?,
        None => {
            let provider =
                LiveProjectionReadProviderV1::open_explicit_repository(&request.repository_locator)
                    .map_err(|_| anyhow!("failed to establish the live projection provider"))?;
            packet_read(&provider, &request)
                .map_err(|_| anyhow!("packet projection was rejected"))?
        }
    };
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
