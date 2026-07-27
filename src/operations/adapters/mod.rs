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

use crate::domain::{
    authority::{AuthorityFacadeV1, ContinuityReferenceV1},
    integration::TrustedHostDiagnosticConnectionPortV1,
    persistence::ProtectedDiagnosticCurrentViewProviderV1,
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
