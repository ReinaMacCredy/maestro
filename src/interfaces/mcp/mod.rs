pub mod server;
pub mod tools;

use anyhow::{Result, anyhow};

use crate::domain::{
    authority::{AuthorityFacadeV1, ContinuityReferenceV1},
    integration::{
        LiveAuthenticatedHostConnectionV1, Stage10OwnerLocalConnectionSeedV1,
        public_literals::{McpPacketReadEnvelopeV1, McpPacketReadRequestV1},
    },
    persistence::ProtectedDiagnosticCurrentViewProviderV1,
};
use crate::interfaces::connectors::acquire_trusted_host_diagnostic_connection;
use crate::operations::adapters::{
    LiveProjectionReadProviderV1, Stage10AdapterError, packet_read_with_protected_continuity,
};

pub(crate) struct ProtectedPacketRuntimeV1<'runtime, 'store, 'host> {
    authority: &'runtime mut AuthorityFacadeV1<'store>,
    connection: Stage10OwnerLocalConnectionSeedV1<'host>,
    current_view_provider: &'runtime mut dyn ProtectedDiagnosticCurrentViewProviderV1,
    requested_subject: ContinuityReferenceV1,
}

impl ProtectedPacketRuntimeV1<'_, '_, '_> {
    pub(crate) fn read_packet(
        &mut self,
        request: &McpPacketReadRequestV1,
    ) -> Result<McpPacketReadEnvelopeV1, Stage10AdapterError> {
        let projection =
            LiveProjectionReadProviderV1::open_explicit_repository(&request.repository_locator)
                .map_err(|_| Stage10AdapterError::PacketReadRejected)?;
        let protected = packet_read_with_protected_continuity(
            self.authority,
            &mut self.connection,
            self.current_view_provider,
            self.requested_subject,
            &projection,
            request,
        )?;
        let _diagnostic_bytes = protected.protected_continuity_diagnostic;
        Ok(protected.packet)
    }
}

#[allow(
    dead_code,
    reason = "supported host adapters call this crate-private entry when a provider is activated"
)]
pub(crate) fn serve_with_authenticated_host<'runtime, 'store, 'host>(
    authority: &'runtime mut AuthorityFacadeV1<'store>,
    host_profile_id: &str,
    live_connection: &'host mut dyn LiveAuthenticatedHostConnectionV1,
    current_view_provider: &'runtime mut dyn ProtectedDiagnosticCurrentViewProviderV1,
    requested_subject: ContinuityReferenceV1,
) -> Result<()> {
    let connection = acquire_trusted_host_diagnostic_connection(host_profile_id, live_connection)
        .ok_or(Stage10AdapterError::TrustedHostAuthorityRejected)
        .map_err(|_| anyhow!("trusted host authority was rejected"))?;
    let mut runtime = ProtectedPacketRuntimeV1 {
        authority,
        connection,
        current_view_provider,
        requested_subject,
    };
    server::serve_with_protected_packet(&mut runtime)
}

#[allow(
    dead_code,
    reason = "the canonical MCP resource is checked by adapter contract tests"
)]
pub(crate) const MCP_TOOL_SOURCE_JSON: &str =
    include_str!("../../../embedded/vnext/adapter/mcp-tools.v1.json");
