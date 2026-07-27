use super::durable_finality::{
    DurableInstallationFinalityErrorV1, DurableInstallationFinalityErrorV2,
    InstallationFinalityCurrentnessV1, PreStoreDecisionTupleV1, PreStoreFinalityOwnerV1,
    PreStoreFinalityOwnerV2, PreStoreFinalityRequestV1, PreStoreFinalityRequestV2,
    PreStoreOwnerValidationV1, PreStoreOwnerValidationV2, owner_sealed,
};
use crate::domain::vnext::persistence::protected_locator_lease::ProtectedLocatorCandidateInputV2;

pub(in crate::domain::vnext) struct Stage11PreStoreFinalitySeedV1 {
    _private: (),
}

impl Stage11PreStoreFinalitySeedV1 {
    #[cfg(test)]
    pub(in crate::domain::vnext) fn test_unavailable() -> Self {
        Self { _private: () }
    }
}

impl PreStoreFinalityOwnerV1 for Stage11PreStoreFinalitySeedV1 {
    fn prepare_request(
        &mut self,
    ) -> Result<PreStoreFinalityRequestV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }

    fn validate_inactive_candidate(
        &mut self,
        _expected: InstallationFinalityCurrentnessV1,
        _request: &PreStoreFinalityRequestV1,
    ) -> Result<PreStoreOwnerValidationV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }

    fn pre_dispatch_recheck(
        &mut self,
        _expected: InstallationFinalityCurrentnessV1,
        _request: &PreStoreFinalityRequestV1,
    ) -> Result<(), DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }

    fn final_recheck(
        &mut self,
        _expected: InstallationFinalityCurrentnessV1,
        _request: &PreStoreFinalityRequestV1,
        _outcome: crate::domain::vnext::persistence::protected_locator_lease::ProtectedLocatorFinalityDispositionV1,
    ) -> Result<(), DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }
}

impl owner_sealed::Sealed for Stage11PreStoreFinalitySeedV1 {}

pub(in crate::domain::vnext) struct Stage11PreStoreFinalitySeedV2 {
    request: Option<PreStoreFinalityRequestV2>,
    expected_currentness: InstallationFinalityCurrentnessV1,
    expected_decision: PreStoreDecisionTupleV1,
    validated: bool,
}

impl Stage11PreStoreFinalitySeedV2 {
    pub(in crate::domain::vnext) fn from_installation_owner(
        currentness: InstallationFinalityCurrentnessV1,
        decision: PreStoreDecisionTupleV1,
        candidate: ProtectedLocatorCandidateInputV2,
    ) -> Self {
        Self {
            request: Some(PreStoreFinalityRequestV2 {
                currentness,
                decision,
                candidate: Some(candidate),
                consumed: std::cell::Cell::new(false),
                _not_send_or_sync: std::marker::PhantomData,
            }),
            expected_currentness: currentness,
            expected_decision: decision,
            validated: false,
        }
    }

    #[cfg(test)]
    pub(in crate::domain::vnext) fn test_unavailable() -> Self {
        Self {
            request: None,
            expected_currentness: unavailable_currentness(),
            expected_decision: unavailable_decision(),
            validated: false,
        }
    }
}

impl owner_sealed::Sealed for Stage11PreStoreFinalitySeedV2 {}

impl PreStoreFinalityOwnerV2 for Stage11PreStoreFinalitySeedV2 {
    fn capture_pre_store_request(
        &mut self,
    ) -> Result<PreStoreFinalityRequestV2, DurableInstallationFinalityErrorV2> {
        self.request
            .take()
            .ok_or(DurableInstallationFinalityErrorV2::BackendUnavailable)
    }

    fn validate_inert_candidate(
        &mut self,
        request: &PreStoreFinalityRequestV2,
    ) -> Result<PreStoreOwnerValidationV2, DurableInstallationFinalityErrorV2> {
        if self.validated
            || request.currentness != self.expected_currentness
            || request.decision != self.expected_decision
            || request.candidate.is_none()
        {
            return Err(DurableInstallationFinalityErrorV2::CurrentnessMismatch);
        }
        self.validated = true;
        Ok(PreStoreOwnerValidationV2 {
            currentness: request.currentness,
            decision: request.decision,
            write_count: 0,
            _not_send_or_sync: std::marker::PhantomData,
        })
    }

    fn final_recheck(
        &mut self,
        request: &PreStoreFinalityRequestV2,
        _disposition: crate::domain::vnext::persistence::protected_locator_v2::ProtectedLocatorFinalityDispositionV2,
    ) -> Result<(), DurableInstallationFinalityErrorV2> {
        if !self.validated
            || request.currentness != self.expected_currentness
            || request.decision != self.expected_decision
            || request.candidate.is_some()
        {
            return Err(DurableInstallationFinalityErrorV2::CurrentnessMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
fn unavailable_currentness() -> InstallationFinalityCurrentnessV1 {
    InstallationFinalityCurrentnessV1 {
        installation: [0; 32],
        tenant: [0; 32],
        principal: [0; 32],
        authority: [0; 32],
        realm: [0; 32],
        domain: [0; 32],
        store_instance: [0; 32],
        activation_incarnation: [0; 32],
        head: [0; 32],
        head_revision: 0,
        generation: [0; 32],
        generation_ordinal: 0,
        store_cas: [0; 32],
        host_connection: [0; 32],
        host_currentness: [0; 32],
        currentness: [0; 32],
        fence: [0; 32],
        revocation_revision: 0,
    }
}

#[cfg(test)]
fn unavailable_decision() -> PreStoreDecisionTupleV1 {
    PreStoreDecisionTupleV1 {
        operation: [0; 32],
        ceremony_spec: [0; 32],
        attempt: [0; 32],
        protected_attempt_currentness: [0; 32],
        release: [0; 32],
        facility: [0; 32],
        locator_identity: [0; 32],
        candidate_association: [0; 32],
        association_meaning: [0; 32],
        candidate_store_lineage: [0; 32],
        target: [0; 32],
        distribution_commit: [0; 32],
        source_carrier: [0; 32],
        candidate_carrier: [0; 32],
        writer_protocol_epoch: 0,
        schema_epoch: 0,
        migration_epoch: 0,
        census: [0; 32],
        consumer_set: [0; 32],
        invocation: [0; 32],
        idempotency_key: [0; 32],
        idempotency_meaning: [0; 32],
        host_guard: [0; 32],
        currentness_fence: [0; 32],
        candidate_postcondition: [0; 32],
        inert_marker: [0; 32],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn currentness() -> InstallationFinalityCurrentnessV1 {
        InstallationFinalityCurrentnessV1 {
            installation: [1; 32],
            tenant: [2; 32],
            principal: [3; 32],
            authority: [4; 32],
            realm: [2; 32],
            domain: [5; 32],
            store_instance: [6; 32],
            activation_incarnation: [7; 32],
            head: [8; 32],
            head_revision: 1,
            generation: [9; 32],
            generation_ordinal: 1,
            store_cas: [10; 32],
            host_connection: [11; 32],
            host_currentness: [12; 32],
            currentness: [13; 32],
            fence: [14; 32],
            revocation_revision: 1,
        }
    }

    fn decision() -> PreStoreDecisionTupleV1 {
        PreStoreDecisionTupleV1 {
            operation: [4; 32],
            ceremony_spec: [5; 32],
            attempt: [6; 32],
            protected_attempt_currentness: [15; 32],
            release: [16; 32],
            facility: [17; 32],
            locator_identity: [18; 32],
            candidate_association: [26; 32],
            association_meaning: [19; 32],
            candidate_store_lineage: [20; 32],
            target: [21; 32],
            distribution_commit: [22; 32],
            source_carrier: [7; 32],
            candidate_carrier: [28; 32],
            writer_protocol_epoch: 1,
            schema_epoch: 1,
            migration_epoch: 1,
            census: [23; 32],
            consumer_set: [24; 32],
            invocation: [9; 32],
            idempotency_key: [25; 32],
            idempotency_meaning: [27; 32],
            host_guard: [31; 32],
            currentness_fence: [32; 32],
            candidate_postcondition: [30; 32],
            inert_marker: [33; 32],
        }
    }

    #[test]
    fn stage11_owner_test_provider_is_constructible_only_in_its_owner_module() {
        let mut backend = Stage11PreStoreFinalitySeedV1::test_unavailable();
        assert!(matches!(
            super::super::durable_finality::prepare_pre_store_from_stage11_owner(&mut backend),
            Err(super::super::durable_finality::Stage11PreStoreFinalityErrorV1)
        ));
    }

    #[test]
    fn production_v2_seed_consumes_an_inert_candidate_without_store_effects() {
        let candidate = ProtectedLocatorCandidateInputV2::from_installation_owner(
            [1; 32], [2; 32], [5; 32], [6; 32], [26; 32], [27; 32], [28; 32], [29; 32], [30; 32],
            [9; 32],
        )
        .expect("candidate");
        let mut seed = Stage11PreStoreFinalitySeedV2::from_installation_owner(
            currentness(),
            decision(),
            candidate,
        );
        let (outcome, writes) =
            crate::domain::vnext::persistence::protected_locator_lease::v2_tests::with_test_lease_v2(
                |lease| {
                    super::super::durable_finality::DurableInstallationFinalityBackendV2::capture(
                        &mut seed,
                    )
                    .consume_pre_store(lease)
                },
            );
        assert_eq!(
            outcome,
            Ok(super::super::durable_finality::DurableInstallationFinalityOutcomeV2::Committed)
        );
        assert_eq!(writes, 1);
        assert!(seed.validated);
        assert!(seed.request.is_none());
    }
}
