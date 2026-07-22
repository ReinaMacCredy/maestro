pub mod public_literals;

mod trusted_host_diagnostic;

#[cfg(test)]
pub(crate) use trusted_host_diagnostic::{
    TrustedHostDiagnosticAttestationV1, TrustedHostDiagnosticTestClaimsV1,
    TrustedHostDiagnosticTestConnectionV1, TrustedHostDiagnosticTestControlV1,
    TrustedHostDiagnosticTestOperatorIdentityV1,
};
