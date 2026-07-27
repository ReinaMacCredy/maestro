#![allow(
    dead_code,
    reason = "Stage 10 MCP entrypoints remain crate-internal until top-level vNext activation"
)]

//! The exact two global, read-only MCP adapter declarations.

use crate::domain::vnext::{
    authority::{AuthorityFacadeV1, ContinuityReferenceV1},
    integration::LiveAuthenticatedHostConnectionV1,
    persistence::ProtectedDiagnosticCurrentViewProviderV1,
};
use crate::interfaces::vnext::connectors::acquire_trusted_host_diagnostic_connection;
use crate::operations::vnext::adapters::{
    GLOBAL_MCP_TOOLS_V1, GlobalMcpAdapterDefinitionV1, Stage10AdapterError,
    read_protected_continuity_diagnostic,
};

pub fn tools() -> &'static [GlobalMcpAdapterDefinitionV1; 2] {
    &GLOBAL_MCP_TOOLS_V1
}

/// Reads only through the Stage-5 trusted-host and Authority continuation.
///
/// The provider is required and sealed by Persistence, so this path accepts the
/// Stage 9 production currentness implementation without minting an adapter-
/// local substitute.
pub fn read_protected_continuity(
    authority: &mut AuthorityFacadeV1,
    host_profile_id: &str,
    live_connection: &mut dyn LiveAuthenticatedHostConnectionV1,
    current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1,
    requested_subject: ContinuityReferenceV1,
) -> Result<Box<[u8]>, Stage10AdapterError> {
    let mut connection =
        acquire_trusted_host_diagnostic_connection(host_profile_id, live_connection)
            .ok_or(Stage10AdapterError::TrustedHostAuthorityRejected)?;
    read_protected_continuity_diagnostic(
        authority,
        &mut connection,
        current_view_provider,
        requested_subject,
    )
}

pub const MCP_TOOL_SOURCE_JSON: &str =
    include_str!("../../../../embedded/vnext/adapter/mcp-tools.v1.json");
