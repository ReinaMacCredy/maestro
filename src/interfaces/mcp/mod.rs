pub mod server;
pub mod tools;

use crate::domain::{
    authority::{AuthorityFacadeV1, ContinuityReferenceV1},
    integration::LiveAuthenticatedHostConnectionV1,
    persistence::ProtectedDiagnosticCurrentViewProviderV1,
};
use crate::interfaces::connectors::acquire_trusted_host_diagnostic_connection;
use crate::operations::adapters::{
    GLOBAL_MCP_TOOLS_V1, GlobalMcpAdapterDefinitionV1, Stage10AdapterError,
    read_protected_continuity_diagnostic,
};

#[allow(
    dead_code,
    reason = "the canonical MCP declarations are exercised by adapter contract tests"
)]
pub(crate) fn canonical_tools() -> &'static [GlobalMcpAdapterDefinitionV1; 2] {
    &GLOBAL_MCP_TOOLS_V1
}

#[allow(
    dead_code,
    reason = "the canonical protected read is exercised by adapter contract tests"
)]
pub(crate) fn read_protected_continuity(
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

#[allow(
    dead_code,
    reason = "the canonical MCP resource is checked by adapter contract tests"
)]
pub(crate) const MCP_TOOL_SOURCE_JSON: &str =
    include_str!("../../../embedded/vnext/adapter/mcp-tools.v1.json");
