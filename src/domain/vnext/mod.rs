//! Candidate-only Maestro vNext domain contracts.
//!
//! Modules under this namespace are inert until an exact candidate Contract
//! Root is separately authorized and published.

pub mod authority;
pub mod capability;
pub mod contract;
pub mod design;
pub mod distribution;
pub mod evidence;
pub mod execution;
pub mod gate;
pub mod identity;
pub mod integration;
pub mod migration;
pub mod orchestration;
pub mod persistence;
pub mod repository;
pub mod step;
pub mod work;

#[cfg(test)]
mod protected_diagnostic_sibling_port_compile_probe {
    use super::authority::{
        ProtectedContinuityDiagnosticEnvelopeBuilderSealedV1,
        ProtectedContinuityDiagnosticEnvelopeBuilderV1,
        ProtectedContinuityDiagnosticEnvelopeInputV1,
        ProtectedContinuityDiagnosticPreparedEnvelopeSealedV1,
        ProtectedContinuityDiagnosticPreparedEnvelopeV1, TrustedHostDiagnosticChallengeV1,
    };
    use super::integration::{
        TrustedHostDiagnosticAttestationPortSealedV1, TrustedHostDiagnosticAttestationPortV1,
        TrustedHostDiagnosticConnectionPortSealedV1, TrustedHostDiagnosticConnectionPortV1,
        TrustedHostDiagnosticPresentationPortSealedV1, TrustedHostDiagnosticPresentationPortV1,
    };
    use super::persistence::{
        ProtectedDiagnosticCurrentViewProviderSealedV1, ProtectedDiagnosticCurrentViewProviderV1,
        ProtectedDiagnosticObservedCurrentViewV1, ProtectedDiagnosticProviderCurrentnessV1,
    };

    struct Stage8EnvelopeBuilderProbeV1;
    struct Stage8PreparedEnvelopeProbeV1;
    struct Stage9CurrentViewProviderProbeV1;
    struct Stage10ConnectionProbeV1;
    struct Stage10AttestationProbeV1;
    struct Stage10PresentationProbeV1;

    impl ProtectedContinuityDiagnosticEnvelopeBuilderSealedV1 for Stage8EnvelopeBuilderProbeV1 {}
    impl ProtectedContinuityDiagnosticPreparedEnvelopeSealedV1 for Stage8PreparedEnvelopeProbeV1 {}
    impl ProtectedDiagnosticCurrentViewProviderSealedV1 for Stage9CurrentViewProviderProbeV1 {}
    impl TrustedHostDiagnosticConnectionPortSealedV1 for Stage10ConnectionProbeV1 {}
    impl TrustedHostDiagnosticAttestationPortSealedV1 for Stage10AttestationProbeV1 {}
    impl TrustedHostDiagnosticPresentationPortSealedV1 for Stage10PresentationProbeV1 {}

    impl ProtectedContinuityDiagnosticEnvelopeBuilderV1 for Stage8EnvelopeBuilderProbeV1 {
        fn prepare_current_protected_snapshot(
            &mut self,
            input: ProtectedContinuityDiagnosticEnvelopeInputV1<'_>,
        ) -> Option<Box<dyn ProtectedContinuityDiagnosticPreparedEnvelopeV1>> {
            let _allowlisted_snapshot_relative_input = [
                input.fence_subject_ref(),
                input.fence_carrier_ref(),
                input.attempt_ref(),
                input.semantic_point_ref(),
                input.covered_closure_ref(),
                input.conservative_point_envelope_ref(),
                input.carrier_revision_ref(),
            ];
            Some(Box::new(Stage8PreparedEnvelopeProbeV1))
        }
    }

    impl ProtectedContinuityDiagnosticPreparedEnvelopeV1 for Stage8PreparedEnvelopeProbeV1 {
        fn commitment(&self) -> [u8; 32] {
            use sha2::{Digest, Sha256};

            Sha256::digest([1]).into()
        }

        fn into_bytes(self: Box<Self>) -> Vec<u8> {
            vec![1]
        }
    }

    impl ProtectedDiagnosticCurrentViewProviderV1 for Stage9CurrentViewProviderProbeV1 {
        fn bind_current_view(
            &mut self,
            _observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
        ) -> Option<ProtectedDiagnosticProviderCurrentnessV1> {
            ProtectedDiagnosticProviderCurrentnessV1::from_live_provider(
                [1; 32], [2; 32], [3; 32], 1, [4; 32], [5; 32], [6; 32], 1,
            )
        }

        fn final_recheck_current_view(
            &mut self,
            _initial_currentness: &ProtectedDiagnosticProviderCurrentnessV1,
            _observed: &ProtectedDiagnosticObservedCurrentViewV1<'_>,
        ) -> bool {
            false
        }

        fn abandon_current_view(&mut self) {}
    }

    impl TrustedHostDiagnosticConnectionPortV1 for Stage10ConnectionProbeV1 {
        fn attest_in_current_view<'scope, 'view>(
            &'scope mut self,
            _challenge: TrustedHostDiagnosticChallengeV1<'scope, 'view>,
        ) -> Option<Box<dyn TrustedHostDiagnosticAttestationPortV1 + 'scope>>
        where
            'view: 'scope,
        {
            Some(Box::new(Stage10AttestationProbeV1))
        }
    }

    impl TrustedHostDiagnosticAttestationPortV1 for Stage10AttestationProbeV1 {
        fn witness_carrier_commitment(&self) -> [u8; 32] {
            [0; 32]
        }

        fn present_once(
            &mut self,
            inspect: &mut dyn FnMut(&dyn TrustedHostDiagnosticPresentationPortV1) -> bool,
        ) -> bool {
            let _ = inspect(&Stage10PresentationProbeV1);
            false
        }

        fn final_recheck(self: Box<Self>) -> bool {
            false
        }
    }

    macro_rules! digest_getters {
        ($($name:ident),+ $(,)?) => {
            $(fn $name(&self) -> [u8; 32] { [0; 32] })+
        };
    }

    macro_rules! revision_getters {
        ($($name:ident),+ $(,)?) => {
            $(fn $name(&self) -> u64 { 0 })+
        };
    }

    impl TrustedHostDiagnosticPresentationPortV1 for Stage10PresentationProbeV1 {
        digest_getters!(
            anchor_commitment,
            authority_commitment,
            protected_subject_commitment,
            invocation_nonce,
            challenge_commitment,
            claims_commitment,
            principal_identity,
            binding_identity,
            session_identity,
            context_identity,
            domain_identity,
        );
        revision_getters!(
            trust_root_revision,
            assurance_revision,
            binding_not_before,
            binding_expires_at,
            session_not_before,
            session_expires_at,
            store_generation,
            authority_epoch,
            domain_role,
        );

        fn human_capable(&self) -> bool {
            false
        }
    }

    #[test]
    fn later_sibling_roots_can_implement_every_frozen_diagnostic_port() {
        fn require_stage8<T: ProtectedContinuityDiagnosticEnvelopeBuilderV1>() {}
        fn require_stage9<T: ProtectedDiagnosticCurrentViewProviderV1>() {}
        fn require_stage10_connection<T: TrustedHostDiagnosticConnectionPortV1>() {}
        fn require_stage10_attestation<T: TrustedHostDiagnosticAttestationPortV1>() {}
        fn require_stage10_presentation<T: TrustedHostDiagnosticPresentationPortV1>() {}
        require_stage8::<Stage8EnvelopeBuilderProbeV1>();
        require_stage9::<Stage9CurrentViewProviderProbeV1>();
        require_stage10_connection::<Stage10ConnectionProbeV1>();
        require_stage10_attestation::<Stage10AttestationProbeV1>();
        require_stage10_presentation::<Stage10PresentationProbeV1>();
    }
}
