#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the Integration-owned consumer closure before its Stage 10 consumer"
    )
)]
pub(in crate::domain) mod consumer_closure;
pub mod public_literals;

mod trusted_host_diagnostic;
mod trusted_host_diagnostic_stage10_seed;

#[allow(
    unused_imports,
    reason = "Stage 5 freezes nominal trusted-host ports before the Stage 10 adapter"
)]
pub(crate) use trusted_host_diagnostic::{
    TrustedHostDiagnosticAttestationPortV1, TrustedHostDiagnosticConnectionPortV1,
    TrustedHostDiagnosticPresentationPortV1,
};
#[cfg(test)]
pub(crate) use trusted_host_diagnostic::{
    TrustedHostDiagnosticTestClaimsV1, TrustedHostDiagnosticTestConnectionV1,
    TrustedHostDiagnosticTestControlV1, TrustedHostDiagnosticTestOperatorIdentityV1,
};
#[allow(
    unused_imports,
    reason = "host adapters need to name the authenticated snapshot callback contract"
)]
pub(crate) use trusted_host_diagnostic_stage10_seed::{
    AuthenticatedHostConnectionSnapshotV1, LiveAuthenticatedHostConnectionV1,
    Stage10OwnerLocalConnectionSeedV1,
};
