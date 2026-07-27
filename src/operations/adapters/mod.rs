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

use crate::domain::integration::public_literals::{
    McpPacketReadEnvelopeV1, McpPacketReadRequestV1,
};
use crate::domain::{
    authority::{AuthorityFacadeV1, ContinuityReferenceV1},
    capability::generated_catalog::{GeneratedCapabilityCatalogV1, OperationCatalogKindV1},
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CliSearchMatchV1 {
    pub exact_literal: String,
    pub canonical_invocation: String,
}

pub(crate) fn cli_search(literal: &str) -> Result<Vec<CliSearchMatchV1>, Stage10AdapterError> {
    if literal == "maestro packet read" {
        return Ok(vec![CliSearchMatchV1 {
            exact_literal: literal.to_owned(),
            canonical_invocation: literal.to_owned(),
        }]);
    }
    let catalog = GeneratedCapabilityCatalogV1::load_frozen()
        .map_err(|_| Stage10AdapterError::CapabilityCatalogUnavailable)?;
    Ok(catalog
        .actions()
        .iter()
        .chain(catalog.ceremonies())
        .filter(|entry| entry.name() == literal || entry.descriptor_ref() == literal)
        .map(|entry| CliSearchMatchV1 {
            exact_literal: entry.name().to_owned(),
            canonical_invocation: format!(
                "maestro operation prepare {} {}",
                match entry.kind() {
                    OperationCatalogKindV1::Action => "action",
                    OperationCatalogKindV1::Ceremony => "ceremony",
                },
                entry.name()
            ),
        })
        .collect())
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
    fn cli_search_is_literal_and_does_not_compute_a_next_action() {
        assert_eq!(
            cli_search("maestro packet read").unwrap(),
            vec![CliSearchMatchV1 {
                exact_literal: "maestro packet read".to_owned(),
                canonical_invocation: "maestro packet read".to_owned(),
            }]
        );
        assert!(cli_search("next").unwrap().is_empty());
        assert!(cli_search("loop next").unwrap().is_empty());
    }

    #[test]
    fn legacy_surface_refusal_names_only_the_frozen_successor() {
        let refusal = legacy_successor_refusal(LegacySuccessorSurfaceV1::TaskNext).unwrap();
        assert_eq!(refusal.code, "unsupported_legacy_successor_surface");
        assert_eq!(refusal.canonical_replacement, "maestro packet read");
    }
}
