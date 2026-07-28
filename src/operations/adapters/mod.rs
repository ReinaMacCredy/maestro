#![allow(
    dead_code,
    reason = "canonical adapter entrypoints remain crate-internal"
)]

//! Stage-10 public-adapter preparation.
//!
//! Stage 5 owns host attestation and Authority validation. Stage 10 only
//! presents their released result. The currentness input is the sealed Stage 5
//! port implemented by Stage 9's production provider; Stage 10 does not mint a
//! substitute provider.

mod live_projection;

#[allow(
    unused_imports,
    reason = "MainIntegration consumes the narrow Stage 12 adapter facade after shared wiring"
)]
pub(crate) use live_projection::{
    LiveProjectionReadProviderV1, RunningBinaryIdentityV1, cli_search, decode_cli_search_request,
    encode_cli_search_envelope,
};

use crate::domain::integration::public_literals::{
    McpPacketReadEnvelopeV1, McpPacketReadRequestV1,
};
use crate::domain::{
    authority::{AuthorityFacadeV1, ContinuityReferenceV1},
    integration::TrustedHostDiagnosticConnectionPortV1,
    persistence::ProtectedDiagnosticCurrentViewProviderV1,
    projection::{
        LegacySuccessorRefusalV1, LegacySuccessorSurfaceV1, ProjectionReadPortV1, read_packet,
        refuse_legacy_successor_surface,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GlobalMcpAdapterDefinitionV1 {
    pub name: &'static str,
    pub read_only: bool,
    pub writes: bool,
    pub network_io: bool,
}

pub const GLOBAL_MCP_TOOLS_V1: [GlobalMcpAdapterDefinitionV1; 2] = [
    GlobalMcpAdapterDefinitionV1 {
        name: "maestro_packet",
        read_only: true,
        writes: false,
        network_io: false,
    },
    GlobalMcpAdapterDefinitionV1 {
        name: "maestro_cli_search",
        read_only: true,
        writes: false,
        network_io: false,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage10AdapterError {
    TrustedHostAuthorityRejected,
    InvalidFrame,
    PacketReadRejected,
    CapabilityCatalogUnavailable,
    SearchCurrentnessRejected,
    SearchEnvelopeRejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFrameV1 {
    pub release_ref: String,
    pub public_catalog_ref: String,
    pub semantic_ref: String,
    pub outcome: String,
    pub summary: String,
}

impl AdapterFrameV1 {
    pub fn validate(&self) -> Result<(), Stage10AdapterError> {
        if self.release_ref.is_empty()
            || self.public_catalog_ref.is_empty()
            || self.semantic_ref.is_empty()
            || self.outcome.is_empty()
            || self.summary.is_empty()
        {
            return Err(Stage10AdapterError::InvalidFrame);
        }
        Ok(())
    }
}

pub fn read_protected_continuity_diagnostic(
    authority: &mut AuthorityFacadeV1,
    connection: &mut dyn TrustedHostDiagnosticConnectionPortV1,
    current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1,
    requested_subject: ContinuityReferenceV1,
) -> Result<Box<[u8]>, Stage10AdapterError> {
    authority
        .protected_continuity_diagnostic_with_ports(
            connection,
            current_view_provider,
            requested_subject,
        )
        .map(|released| released.into_bytes())
        .map_err(|_| Stage10AdapterError::TrustedHostAuthorityRejected)
}

pub(crate) fn packet_read(
    projection: &dyn ProjectionReadPortV1,
    request: &McpPacketReadRequestV1,
) -> Result<McpPacketReadEnvelopeV1, Stage10AdapterError> {
    read_packet(projection, request).map_err(|_| Stage10AdapterError::PacketReadRejected)
}

pub(crate) fn legacy_successor_refusal(
    surface: LegacySuccessorSurfaceV1<'_>,
) -> Option<LegacySuccessorRefusalV1> {
    refuse_legacy_successor_surface(surface)
}

#[cfg(test)]
mod successor_route_tests {
    use super::*;

    #[test]
    fn legacy_surface_refusal_names_only_the_frozen_successor() {
        let refusal = legacy_successor_refusal(LegacySuccessorSurfaceV1::TaskNext).unwrap();
        assert_eq!(refusal.code, "unsupported_legacy_successor_surface");
        assert_eq!(refusal.canonical_replacement, "maestro packet read");
    }
}
