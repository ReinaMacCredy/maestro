pub mod public_literals;

mod trusted_host_diagnostic;

#[allow(
    unused_imports,
    reason = "Stage 5 freezes nominal trusted-host ports before the Stage 10 adapter"
)]
pub(crate) use trusted_host_diagnostic::{
    TrustedHostDiagnosticAttestationPortSealedV1, TrustedHostDiagnosticAttestationPortV1,
    TrustedHostDiagnosticConnectionPortSealedV1, TrustedHostDiagnosticConnectionPortV1,
    TrustedHostDiagnosticPresentationPortSealedV1, TrustedHostDiagnosticPresentationPortV1,
};
#[cfg(test)]
pub(crate) use trusted_host_diagnostic::{
    TrustedHostDiagnosticTestClaimsV1, TrustedHostDiagnosticTestConnectionV1,
    TrustedHostDiagnosticTestControlV1, TrustedHostDiagnosticTestOperatorIdentityV1,
};
