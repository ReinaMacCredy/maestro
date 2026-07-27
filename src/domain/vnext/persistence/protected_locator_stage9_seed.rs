use super::protected_locator_lease::{
    ProtectedLocatorAcquisitionRequestV2, ProtectedLocatorBackendV1, ProtectedLocatorBackendV2,
    ProtectedLocatorCandidateInputV2, ProtectedLocatorCandidateStateV2,
    ProtectedLocatorCandidateTransitionV1, ProtectedLocatorDispatchOccurrenceV1,
    ProtectedLocatorDispatchOccurrenceV2, ProtectedLocatorFinalReadbackV1,
    ProtectedLocatorFinalReadbackV2, ProtectedLocatorLeaseErrorV1, ProtectedLocatorLeaseErrorV2,
    ProtectedLocatorLeaseV1, ProtectedLocatorLeaseV2, ProtectedLocatorObservedStateV1,
    ProtectedLocatorObservedStateV2, ProtectedLocatorOperationRequestV1, owner_sealed,
    v2_owner_sealed,
};

pub(in crate::domain::vnext) struct Stage9ProtectedLocatorBackendSeedV2 {
    _private: (),
}

impl Stage9ProtectedLocatorBackendSeedV2 {
    #[cfg(test)]
    pub(in crate::domain::vnext) fn test_unavailable() -> Self {
        Self { _private: () }
    }
}

impl v2_owner_sealed::Sealed for Stage9ProtectedLocatorBackendSeedV2 {}

impl ProtectedLocatorBackendV2 for Stage9ProtectedLocatorBackendSeedV2 {
    fn acquire_pre_candidate(
        &mut self,
    ) -> Result<
        (
            ProtectedLocatorAcquisitionRequestV2,
            ProtectedLocatorObservedStateV2,
        ),
        ProtectedLocatorLeaseErrorV2,
    > {
        Err(ProtectedLocatorLeaseErrorV2::BackendUnavailable)
    }

    fn acquisition_recheck(
        &mut self,
    ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2> {
        Err(ProtectedLocatorLeaseErrorV2::BackendUnavailable)
    }

    fn prepare_candidate(
        &mut self,
        _request: &ProtectedLocatorAcquisitionRequestV2,
        _acquisition: &ProtectedLocatorObservedStateV2,
        _candidate: ProtectedLocatorCandidateInputV2,
    ) -> Result<ProtectedLocatorCandidateStateV2, ProtectedLocatorLeaseErrorV2> {
        Err(ProtectedLocatorLeaseErrorV2::BackendUnavailable)
    }

    fn pre_dispatch_recheck(
        &mut self,
    ) -> Result<ProtectedLocatorObservedStateV2, ProtectedLocatorLeaseErrorV2> {
        Err(ProtectedLocatorLeaseErrorV2::BackendUnavailable)
    }

    fn dispatch_exact_transition(
        &mut self,
        _expected_old: &ProtectedLocatorObservedStateV2,
        _candidate: &ProtectedLocatorCandidateStateV2,
    ) -> Result<ProtectedLocatorDispatchOccurrenceV2, ProtectedLocatorLeaseErrorV2> {
        Err(ProtectedLocatorLeaseErrorV2::BackendUnavailable)
    }

    fn final_readback(
        &mut self,
    ) -> Result<ProtectedLocatorFinalReadbackV2, ProtectedLocatorLeaseErrorV2> {
        Err(ProtectedLocatorLeaseErrorV2::BackendUnavailable)
    }
}

#[cfg(test)]
mod v2_owner_factory_compile_probe {
    use super::*;

    fn request() -> ProtectedLocatorAcquisitionRequestV2 {
        ProtectedLocatorAcquisitionRequestV2::from_stage9_owner(
            [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [7; 32], [8; 32], [9; 32],
        )
        .unwrap()
    }

    fn observed(request: &ProtectedLocatorAcquisitionRequestV2) -> ProtectedLocatorObservedStateV2 {
        ProtectedLocatorObservedStateV2::from_stage9_owner(
            request, [10; 32], [11; 32], [12; 32], [13; 32], [14; 32], [15; 32], [16; 32],
            [17; 32], 18, [19; 32], [20; 32], [21; 32], [22; 32], [23; 32], [24; 32], 25,
        )
        .unwrap()
    }

    #[test]
    fn stage9_owner_seed_can_construct_only_validated_v2_backend_outputs() {
        let request = request();
        let observed = observed(&request);
        let candidate = ProtectedLocatorCandidateInputV2::from_installation_owner(
            [1; 32], [2; 32], [5; 32], [6; 32], [26; 32], [27; 32], [28; 32], [29; 32], [30; 32],
            [9; 32],
        )
        .unwrap();
        let prepared =
            ProtectedLocatorCandidateStateV2::from_stage9_owner(&request, &observed, candidate)
                .unwrap();
        let dispatch = prepared.consume_stage9_dispatch_projection().unwrap();
        assert!(dispatch.matches_exact_owner_effect(
            &[26; 32],
            &[27; 32],
            &[28; 32],
            &[29; 32],
            &[30; 32],
            dispatch.transition_commitment(),
        ));
        assert!(!dispatch.matches_exact_owner_effect(
            &[26; 32],
            &[31; 32],
            &[28; 32],
            &[29; 32],
            &[30; 32],
            dispatch.transition_commitment(),
        ));
        assert_eq!(dispatch.candidate_association(), &[26; 32]);
        assert_eq!(dispatch.candidate_root(), &[27; 32]);
        assert_eq!(dispatch.candidate_carrier(), &[28; 32]);
        assert_eq!(dispatch.candidate_seal(), &[29; 32]);
        assert_eq!(dispatch.candidate_postcondition(), &[30; 32]);
        drop(dispatch);
        assert!(matches!(
            prepared.consume_stage9_dispatch_projection(),
            Err(ProtectedLocatorLeaseErrorV2::Replay)
        ));
        let _readback = ProtectedLocatorFinalReadbackV2::exact_candidate_from_stage9_owner(
            &request, observed, &prepared,
        )
        .unwrap();
    }

    #[test]
    fn stage9_owner_factories_reject_zero_or_unbound_currentness() {
        assert!(matches!(
            ProtectedLocatorAcquisitionRequestV2::from_stage9_owner(
                [0; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [7; 32], [8; 32], [9; 32],
            ),
            Err(ProtectedLocatorLeaseErrorV2::InvalidAcquisition)
        ));

        let request = request();
        assert!(matches!(
            ProtectedLocatorObservedStateV2::from_stage9_owner(
                &request, [10; 32], [11; 32], [12; 32], [13; 32], [14; 32], [15; 32], [16; 32],
                [0; 32], 18, [19; 32], [20; 32], [21; 32], [22; 32], [23; 32], [24; 32], 25,
            ),
            Err(ProtectedLocatorLeaseErrorV2::InvalidAcquisition)
        ));
    }
}

pub(in crate::domain::vnext::persistence) fn acquire_protected_locator_lease_v2(
    backend: &mut dyn ProtectedLocatorBackendV2,
) -> Result<ProtectedLocatorLeaseV2<'_>, ProtectedLocatorLeaseErrorV2> {
    ProtectedLocatorLeaseV2::acquire(backend)
}

pub(in crate::domain::vnext::persistence) struct Stage9ProtectedLocatorBackendSeedV1 {
    _private: (),
}

impl owner_sealed::Sealed for Stage9ProtectedLocatorBackendSeedV1 {}

impl ProtectedLocatorBackendV1 for Stage9ProtectedLocatorBackendSeedV1 {
    fn observe_no_follow(
        &mut self,
        _request: ProtectedLocatorOperationRequestV1,
    ) -> Result<ProtectedLocatorObservedStateV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }

    fn pre_dispatch_recheck(
        &mut self,
    ) -> Result<ProtectedLocatorObservedStateV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }

    fn dispatch_expected_old(
        &mut self,
        _expected_old: [u8; 32],
        _candidate: &ProtectedLocatorCandidateTransitionV1,
    ) -> Result<ProtectedLocatorDispatchOccurrenceV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }

    fn prepare_candidate_transition(
        &mut self,
        _request: ProtectedLocatorOperationRequestV1,
    ) -> Result<ProtectedLocatorCandidateTransitionV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }

    fn final_readback(
        &mut self,
    ) -> Result<ProtectedLocatorFinalReadbackV1, ProtectedLocatorLeaseErrorV1> {
        Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
    }
}

pub(in crate::domain::vnext::persistence) fn acquire_protected_locator_lease<'locator>(
    backend: &'locator mut Stage9ProtectedLocatorBackendSeedV1,
    request: ProtectedLocatorOperationRequestV1,
) -> Result<ProtectedLocatorLeaseV1<'locator>, ProtectedLocatorLeaseErrorV1> {
    ProtectedLocatorLeaseV1::acquire(backend, request)
}

pub(in crate::domain::vnext::persistence) fn acquire_stage9_backend()
-> Stage9ProtectedLocatorBackendSeedV1 {
    Stage9ProtectedLocatorBackendSeedV1 { _private: () }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage9_seed_is_callable_and_explicitly_fail_closed() {
        let request = ProtectedLocatorOperationRequestV1::from_installation_operation(
            [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], [7; 32], [8; 32], [9; 32],
            [10; 32],
        )
        .unwrap();
        let mut backend = acquire_stage9_backend();
        assert!(matches!(
            acquire_protected_locator_lease(&mut backend, request),
            Err(ProtectedLocatorLeaseErrorV1::ProviderUnavailable)
        ));
    }
}
