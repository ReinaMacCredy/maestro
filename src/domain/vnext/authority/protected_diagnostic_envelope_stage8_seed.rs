use super::protected_diagnostic_envelope::{
    ProtectedContinuityDiagnosticAssemblerModeV1, ProtectedContinuityDiagnosticCandidateEnvelopeV1,
    ProtectedContinuityDiagnosticEnvelopeInputV1,
};

pub(super) fn assemble(
    input: &ProtectedContinuityDiagnosticEnvelopeInputV1<'_>,
    mode: ProtectedContinuityDiagnosticAssemblerModeV1,
) -> Option<ProtectedContinuityDiagnosticCandidateEnvelopeV1> {
    #[cfg(not(test))]
    {
        let _ = (input, mode);
        None
    }
    #[cfg(test)]
    {
        let mut candidate = super::protected_diagnostic_envelope::encode_canonical_envelope(input)?;
        match mode {
            ProtectedContinuityDiagnosticAssemblerModeV1::Canonical => {}
            ProtectedContinuityDiagnosticAssemblerModeV1::SubstituteAdmission => {
                let admission_offset =
                    2 + super::protected_diagnostic_envelope::ENVELOPE_DOMAIN_V1.len() + 2;
                candidate.bytes[admission_offset] ^= 0xff;
            }
            ProtectedContinuityDiagnosticAssemblerModeV1::IgnoreInput => {
                candidate.bytes.fill(0);
            }
        }
        super::protected_diagnostic_envelope::observe_test_assembly();
        Some(candidate)
    }
}

#[cfg(test)]
#[test]
fn stage8_owner_local_descendant_is_the_only_concrete_assembler_seed() {
    let source = include_str!("protected_diagnostic_envelope_stage8_seed.rs");
    assert!(source.contains("pub(super) fn assemble("));
    assert!(!source.contains(&["Box", "<dyn"].concat()));
}
