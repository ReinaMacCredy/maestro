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
