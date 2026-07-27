use super::durable_finality::{
    DurableInstallationFinalityErrorV1, DurableInstallationFinalityErrorV2,
    InstallationFinalityCurrentnessV1, PreStoreFinalityOwnerV1, PreStoreFinalityOwnerV2,
    PreStoreFinalityRequestV1, PreStoreFinalityRequestV2, PreStoreOwnerValidationV1,
    PreStoreOwnerValidationV2, owner_sealed,
};

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
    _private: (),
}

impl Stage11PreStoreFinalitySeedV2 {
    #[cfg(test)]
    pub(in crate::domain::vnext) fn test_unavailable() -> Self {
        Self { _private: () }
    }
}

impl owner_sealed::Sealed for Stage11PreStoreFinalitySeedV2 {}

impl PreStoreFinalityOwnerV2 for Stage11PreStoreFinalitySeedV2 {
    fn capture_pre_store_request(
        &mut self,
    ) -> Result<PreStoreFinalityRequestV2, DurableInstallationFinalityErrorV2> {
        Err(DurableInstallationFinalityErrorV2::BackendUnavailable)
    }

    fn validate_inert_candidate(
        &mut self,
        _request: &PreStoreFinalityRequestV2,
    ) -> Result<PreStoreOwnerValidationV2, DurableInstallationFinalityErrorV2> {
        Err(DurableInstallationFinalityErrorV2::BackendUnavailable)
    }

    fn final_recheck(
        &mut self,
        _request: &PreStoreFinalityRequestV2,
        _disposition: crate::domain::vnext::persistence::protected_locator_v2::ProtectedLocatorFinalityDispositionV2,
    ) -> Result<(), DurableInstallationFinalityErrorV2> {
        Err(DurableInstallationFinalityErrorV2::BackendUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage11_owner_test_provider_is_constructible_only_in_its_owner_module() {
        let mut backend = Stage11PreStoreFinalitySeedV1::test_unavailable();
        assert!(matches!(
            super::super::durable_finality::prepare_pre_store_from_stage11_owner(&mut backend),
            Err(super::super::durable_finality::Stage11PreStoreFinalityErrorV1)
        ));
    }
}
