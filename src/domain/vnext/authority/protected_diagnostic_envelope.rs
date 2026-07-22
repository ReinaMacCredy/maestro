use super::ContinuityReferenceV1;

pub(super) const ENVELOPE_DOMAIN_V1: &[u8] =
    b"maestro.vnext.protected-continuity-diagnostic-envelope.v1";
const MAX_ENVELOPE_BYTES_V1: usize = 1024;

pub(super) trait ProtectedContinuityDiagnosticReadGuardMarkerV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(super) enum ProtectedContinuityDiagnosticReasonClassV1 {
    CurrentProtectedSnapshot = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(super) enum ProtectedContinuityDiagnosticCarrierStateV1 {
    ProtectedSnapshot = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(super) enum ProtectedContinuityDiagnosticFreshnessClassV1 {
    VerifiedCurrent = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(super) enum ProtectedContinuityDiagnosticRemediationClassV1 {
    InspectCurrentState = 1,
}

pub(super) struct ProtectedContinuityDiagnosticEnvelopeInputV1<'guard> {
    _guard: &'guard dyn ProtectedContinuityDiagnosticReadGuardMarkerV1,
    admission_ref: ContinuityReferenceV1,
    attempt_ref: ContinuityReferenceV1,
    generation_ref: ContinuityReferenceV1,
    reason_class: ProtectedContinuityDiagnosticReasonClassV1,
    expected_carrier_state: ProtectedContinuityDiagnosticCarrierStateV1,
    observed_carrier_state: ProtectedContinuityDiagnosticCarrierStateV1,
    freshness_class: ProtectedContinuityDiagnosticFreshnessClassV1,
    remediation_class: ProtectedContinuityDiagnosticRemediationClassV1,
    fence_subject_ref: ContinuityReferenceV1,
    fence_carrier_ref: ContinuityReferenceV1,
    semantic_point_ref: ContinuityReferenceV1,
    covered_closure_ref: ContinuityReferenceV1,
    conservative_point_envelope_ref: ContinuityReferenceV1,
    carrier_revision_ref: ContinuityReferenceV1,
    current_view_anchor_ref: ContinuityReferenceV1,
    authority_snapshot_ref: ContinuityReferenceV1,
    attestation_carrier_ref: ContinuityReferenceV1,
}

impl<'guard> ProtectedContinuityDiagnosticEnvelopeInputV1<'guard> {
    #[expect(
        clippy::too_many_arguments,
        reason = "the closed diagnostic envelope input binds every allowlisted field explicitly"
    )]
    pub(super) fn new(
        guard: &'guard dyn ProtectedContinuityDiagnosticReadGuardMarkerV1,
        admission_ref: ContinuityReferenceV1,
        attempt_ref: ContinuityReferenceV1,
        generation_ref: ContinuityReferenceV1,
        fence_subject_ref: ContinuityReferenceV1,
        fence_carrier_ref: ContinuityReferenceV1,
        semantic_point_ref: ContinuityReferenceV1,
        covered_closure_ref: ContinuityReferenceV1,
        conservative_point_envelope_ref: ContinuityReferenceV1,
        carrier_revision_ref: ContinuityReferenceV1,
        current_view_anchor_ref: ContinuityReferenceV1,
        authority_snapshot_ref: ContinuityReferenceV1,
        attestation_carrier_ref: ContinuityReferenceV1,
    ) -> Option<Self> {
        let references = [
            admission_ref,
            attempt_ref,
            generation_ref,
            fence_subject_ref,
            fence_carrier_ref,
            semantic_point_ref,
            covered_closure_ref,
            conservative_point_envelope_ref,
            carrier_revision_ref,
            current_view_anchor_ref,
            authority_snapshot_ref,
            attestation_carrier_ref,
        ];
        if references
            .iter()
            .any(|reference| reference.as_bytes() == &[0; 32])
        {
            return None;
        }
        Some(Self {
            _guard: guard,
            admission_ref,
            attempt_ref,
            generation_ref,
            reason_class: ProtectedContinuityDiagnosticReasonClassV1::CurrentProtectedSnapshot,
            expected_carrier_state: ProtectedContinuityDiagnosticCarrierStateV1::ProtectedSnapshot,
            observed_carrier_state: ProtectedContinuityDiagnosticCarrierStateV1::ProtectedSnapshot,
            freshness_class: ProtectedContinuityDiagnosticFreshnessClassV1::VerifiedCurrent,
            remediation_class: ProtectedContinuityDiagnosticRemediationClassV1::InspectCurrentState,
            fence_subject_ref,
            fence_carrier_ref,
            semantic_point_ref,
            covered_closure_ref,
            conservative_point_envelope_ref,
            carrier_revision_ref,
            current_view_anchor_ref,
            authority_snapshot_ref,
            attestation_carrier_ref,
        })
    }

    pub(super) const fn admission_ref(&self) -> ContinuityReferenceV1 {
        self.admission_ref
    }

    pub(super) const fn attempt_ref(&self) -> ContinuityReferenceV1 {
        self.attempt_ref
    }

    pub(super) const fn generation_ref(&self) -> ContinuityReferenceV1 {
        self.generation_ref
    }

    pub(super) const fn reason_class(&self) -> ProtectedContinuityDiagnosticReasonClassV1 {
        self.reason_class
    }

    pub(super) const fn expected_carrier_state(
        &self,
    ) -> ProtectedContinuityDiagnosticCarrierStateV1 {
        self.expected_carrier_state
    }

    pub(super) const fn observed_carrier_state(
        &self,
    ) -> ProtectedContinuityDiagnosticCarrierStateV1 {
        self.observed_carrier_state
    }

    pub(super) const fn freshness_class(&self) -> ProtectedContinuityDiagnosticFreshnessClassV1 {
        self.freshness_class
    }

    pub(super) const fn remediation_class(
        &self,
    ) -> ProtectedContinuityDiagnosticRemediationClassV1 {
        self.remediation_class
    }

    pub(super) const fn fence_subject_ref(&self) -> ContinuityReferenceV1 {
        self.fence_subject_ref
    }

    pub(super) const fn fence_carrier_ref(&self) -> ContinuityReferenceV1 {
        self.fence_carrier_ref
    }

    pub(super) const fn semantic_point_ref(&self) -> ContinuityReferenceV1 {
        self.semantic_point_ref
    }

    pub(super) const fn covered_closure_ref(&self) -> ContinuityReferenceV1 {
        self.covered_closure_ref
    }

    pub(super) const fn conservative_point_envelope_ref(&self) -> ContinuityReferenceV1 {
        self.conservative_point_envelope_ref
    }

    pub(super) const fn carrier_revision_ref(&self) -> ContinuityReferenceV1 {
        self.carrier_revision_ref
    }

    pub(super) const fn current_view_anchor_ref(&self) -> ContinuityReferenceV1 {
        self.current_view_anchor_ref
    }

    pub(super) const fn authority_snapshot_ref(&self) -> ContinuityReferenceV1 {
        self.authority_snapshot_ref
    }

    pub(super) const fn attestation_carrier_ref(&self) -> ContinuityReferenceV1 {
        self.attestation_carrier_ref
    }
}

pub(super) struct ProtectedContinuityDiagnosticCandidateEnvelopeV1 {
    pub(super) bytes: [u8; MAX_ENVELOPE_BYTES_V1],
    pub(super) len: usize,
}

pub(crate) struct ProtectedContinuityDiagnosticReleasedEnvelopeV1 {
    prepared: ProtectedContinuityDiagnosticPreparedCarrierV1,
}

impl ProtectedContinuityDiagnosticReleasedEnvelopeV1 {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Stage 5 freezes the released envelope before its Stage 8 consumer"
        )
    )]
    pub(crate) fn into_bytes(self) -> Box<[u8]> {
        self.prepared.bytes[..self.prepared.len]
            .to_vec()
            .into_boxed_slice()
    }
}

pub(super) struct ProtectedContinuityDiagnosticPreparedCarrierV1 {
    bytes: [u8; MAX_ENVELOPE_BYTES_V1],
    len: usize,
    #[cfg(test)]
    released: bool,
}

impl ProtectedContinuityDiagnosticPreparedCarrierV1 {
    fn validate(
        input: &ProtectedContinuityDiagnosticEnvelopeInputV1<'_>,
        candidate: ProtectedContinuityDiagnosticCandidateEnvelopeV1,
    ) -> Option<Self> {
        let expected = encode_canonical_envelope(input)?;
        if candidate.len != expected.len
            || candidate.bytes[..candidate.len] != expected.bytes[..expected.len]
        {
            return None;
        }
        Some(Self {
            bytes: candidate.bytes,
            len: candidate.len,
            #[cfg(test)]
            released: false,
        })
    }

    pub(super) fn release(self) -> ProtectedContinuityDiagnosticReleasedEnvelopeV1 {
        #[cfg(test)]
        {
            let mut prepared = self;
            prepared.released = true;
            TEST_RELEASED.with(|count| count.set(count.get() + 1));
            ProtectedContinuityDiagnosticReleasedEnvelopeV1 { prepared }
        }
        #[cfg(not(test))]
        {
            ProtectedContinuityDiagnosticReleasedEnvelopeV1 { prepared: self }
        }
    }
}

#[cfg(test)]
impl Drop for ProtectedContinuityDiagnosticPreparedCarrierV1 {
    fn drop(&mut self) {
        if !self.released {
            TEST_DISCARDED.with(|count| count.set(count.get() + 1));
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProtectedContinuityDiagnosticAssemblerModeV1 {
    Canonical,
    #[cfg(test)]
    SubstituteAdmission,
    #[cfg(test)]
    IgnoreInput,
}

pub(super) fn prepare_current_protected_snapshot(
    input: &ProtectedContinuityDiagnosticEnvelopeInputV1<'_>,
    mode: ProtectedContinuityDiagnosticAssemblerModeV1,
) -> Option<ProtectedContinuityDiagnosticPreparedCarrierV1> {
    let candidate = super::protected_diagnostic_envelope_stage8_seed::assemble(input, mode)?;
    ProtectedContinuityDiagnosticPreparedCarrierV1::validate(input, candidate)
}

pub(super) fn encode_canonical_envelope(
    input: &ProtectedContinuityDiagnosticEnvelopeInputV1<'_>,
) -> Option<ProtectedContinuityDiagnosticCandidateEnvelopeV1> {
    let mut writer = BoundedEnvelopeWriterV1::new();
    writer.field(ENVELOPE_DOMAIN_V1)?;
    writer.reference(input.admission_ref())?;
    writer.reference(input.attempt_ref())?;
    writer.reference(input.generation_ref())?;
    writer.tag(input.reason_class() as u8)?;
    writer.tag(input.expected_carrier_state() as u8)?;
    writer.tag(input.observed_carrier_state() as u8)?;
    writer.tag(input.freshness_class() as u8)?;
    writer.tag(input.remediation_class() as u8)?;
    writer.reference(input.fence_subject_ref())?;
    writer.reference(input.fence_carrier_ref())?;
    writer.reference(input.semantic_point_ref())?;
    writer.reference(input.covered_closure_ref())?;
    writer.reference(input.conservative_point_envelope_ref())?;
    writer.reference(input.carrier_revision_ref())?;
    writer.reference(input.current_view_anchor_ref())?;
    writer.reference(input.authority_snapshot_ref())?;
    writer.reference(input.attestation_carrier_ref())?;
    Some(writer.finish())
}

struct BoundedEnvelopeWriterV1 {
    bytes: [u8; MAX_ENVELOPE_BYTES_V1],
    len: usize,
}

impl BoundedEnvelopeWriterV1 {
    const fn new() -> Self {
        Self {
            bytes: [0; MAX_ENVELOPE_BYTES_V1],
            len: 0,
        }
    }

    fn field(&mut self, value: &[u8]) -> Option<()> {
        let length = u16::try_from(value.len()).ok()?.to_be_bytes();
        self.write(&length)?;
        self.write(value)
    }

    fn tag(&mut self, value: u8) -> Option<()> {
        self.field(&[value])
    }

    fn reference(&mut self, value: ContinuityReferenceV1) -> Option<()> {
        self.field(value.as_bytes())
    }

    fn write(&mut self, value: &[u8]) -> Option<()> {
        let end = self.len.checked_add(value.len())?;
        let target = self.bytes.get_mut(self.len..end)?;
        target.copy_from_slice(value);
        self.len = end;
        Some(())
    }

    fn finish(self) -> ProtectedContinuityDiagnosticCandidateEnvelopeV1 {
        ProtectedContinuityDiagnosticCandidateEnvelopeV1 {
            bytes: self.bytes,
            len: self.len,
        }
    }
}

#[cfg(test)]
thread_local! {
    static TEST_ASSEMBLED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_DISCARDED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static TEST_RELEASED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(super) fn observe_test_assembly() {
    TEST_ASSEMBLED.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(crate) fn reset_protected_diagnostic_envelope_test_observation() {
    TEST_ASSEMBLED.with(|count| count.set(0));
    TEST_DISCARDED.with(|count| count.set(0));
    TEST_RELEASED.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn protected_diagnostic_envelope_test_observation() -> (u64, u64, u64) {
    (
        TEST_ASSEMBLED.with(std::cell::Cell::get),
        TEST_DISCARDED.with(std::cell::Cell::get),
        TEST_RELEASED.with(std::cell::Cell::get),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct GuardSeedV1;

    impl ProtectedContinuityDiagnosticReadGuardMarkerV1 for GuardSeedV1 {}

    fn input(guard: &GuardSeedV1) -> ProtectedContinuityDiagnosticEnvelopeInputV1<'_> {
        let reference = |tag| ContinuityReferenceV1::from_digest([tag; 32]);
        ProtectedContinuityDiagnosticEnvelopeInputV1::new(
            guard,
            reference(1),
            reference(2),
            reference(3),
            reference(4),
            reference(5),
            reference(6),
            reference(7),
            reference(8),
            reference(9),
            reference(10),
            reference(11),
            reference(12),
        )
        .unwrap()
    }

    #[test]
    fn owner_local_stage8_assembler_is_exact_bounded_and_authority_released() {
        let guard = GuardSeedV1;
        let input = input(&guard);
        reset_protected_diagnostic_envelope_test_observation();
        let prepared = prepare_current_protected_snapshot(
            &input,
            ProtectedContinuityDiagnosticAssemblerModeV1::Canonical,
        )
        .unwrap();
        assert_eq!(protected_diagnostic_envelope_test_observation(), (1, 0, 0));
        let released = prepared.release();
        let bytes = released.into_bytes();
        assert!(!bytes.is_empty());
        assert!(bytes.len() <= MAX_ENVELOPE_BYTES_V1);
        assert_eq!(protected_diagnostic_envelope_test_observation(), (1, 0, 1));

        for mode in [
            ProtectedContinuityDiagnosticAssemblerModeV1::SubstituteAdmission,
            ProtectedContinuityDiagnosticAssemblerModeV1::IgnoreInput,
        ] {
            reset_protected_diagnostic_envelope_test_observation();
            assert!(prepare_current_protected_snapshot(&input, mode).is_none());
            assert_eq!(protected_diagnostic_envelope_test_observation(), (1, 0, 0));
        }
    }
}
