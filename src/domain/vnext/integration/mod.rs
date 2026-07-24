#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the Integration-owned consumer closure before its Stage 10 consumer"
    )
)]
pub(in crate::domain::vnext) mod consumer_closure;
pub mod public_literals;

mod trusted_host_diagnostic;
#[cfg(test)]
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
