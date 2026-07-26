use super::protected_locator_lease::{
    ProtectedLocatorBackendV1, ProtectedLocatorCandidateTransitionV1,
    ProtectedLocatorDispatchOccurrenceV1,
    ProtectedLocatorFinalReadbackV1, ProtectedLocatorLeaseErrorV1, ProtectedLocatorLeaseV1,
    ProtectedLocatorObservedStateV1, ProtectedLocatorOperationRequestV1, owner_sealed,
};

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
