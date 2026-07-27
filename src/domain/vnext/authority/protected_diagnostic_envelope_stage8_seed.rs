use super::facade::ProtectedDiagnosticCurrentViewProviderV1;
use super::protected_diagnostic_envelope::{
    ProtectedContinuityDiagnosticAssemblerModeV1, ProtectedContinuityDiagnosticCandidateEnvelopeV1,
    ProtectedContinuityDiagnosticEnvelopeInputV1,
};
use super::{AuthorityFacadeV1, AuthorityPublicationError, ContinuityReferenceV1};
use crate::domain::vnext::integration::TrustedHostDiagnosticConnectionPortV1;

const _: () = {
    fn bind_stage8_production_consumer<'store>(
        facade: &mut AuthorityFacadeV1<'store>,
        connection: &mut dyn TrustedHostDiagnosticConnectionPortV1,
        current_view_provider: &mut dyn ProtectedDiagnosticCurrentViewProviderV1,
        requested_subject: ContinuityReferenceV1,
    ) -> Result<Box<[u8]>, AuthorityPublicationError> {
        facade
            .protected_continuity_diagnostic_with_ports(
                connection,
                current_view_provider,
                requested_subject,
            )
            .map(|released| released.into_bytes())
    }

    let _ = bind_stage8_production_consumer;
};

pub(super) fn assemble(
    input: &ProtectedContinuityDiagnosticEnvelopeInputV1<'_>,
    mode: ProtectedContinuityDiagnosticAssemblerModeV1,
) -> Option<ProtectedContinuityDiagnosticCandidateEnvelopeV1> {
    let candidate = super::protected_diagnostic_envelope::encode_canonical_envelope(input)?;
    #[cfg(test)]
    let mut candidate = candidate;
    match mode {
        ProtectedContinuityDiagnosticAssemblerModeV1::Canonical => {}
        #[cfg(test)]
        ProtectedContinuityDiagnosticAssemblerModeV1::SubstituteAdmission => {
            let admission_offset =
                2 + super::protected_diagnostic_envelope::ENVELOPE_DOMAIN_V1.len() + 2;
            candidate.bytes[admission_offset] ^= 0xff;
        }
        #[cfg(test)]
        ProtectedContinuityDiagnosticAssemblerModeV1::IgnoreInput => {
            candidate.bytes.fill(0);
        }
    }
    #[cfg(test)]
    super::protected_diagnostic_envelope::observe_test_assembly();
    Some(candidate)
}

#[cfg(test)]
#[test]
fn stage8_owner_local_descendant_is_the_only_concrete_assembler_seed() {
    let source = include_str!("protected_diagnostic_envelope_stage8_seed.rs");
    assert!(source.contains("pub(super) fn assemble("));
    assert!(!source.contains(&["Box", "<dyn"].concat()));
}
