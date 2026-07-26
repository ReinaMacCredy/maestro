use super::durable_finality::{
    DurableInstallationFinalityErrorV1, InstallationFinalityCurrentnessV1, PreStoreFinalityOwnerV1,
    PreStoreFinalityRequestV1, PreStoreOwnerValidationV1, owner_sealed,
};

pub(super) struct Stage11PreStoreFinalitySeedV1 {
    _private: (),
}

pub(super) fn acquire() -> Stage11PreStoreFinalitySeedV1 {
    Stage11PreStoreFinalitySeedV1 { _private: () }
}

impl Stage11PreStoreFinalitySeedV1 {
    pub(super) fn prepare_request(
        &mut self,
    ) -> Result<PreStoreFinalityRequestV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }
}

impl owner_sealed::Sealed for Stage11PreStoreFinalitySeedV1 {}

impl PreStoreFinalityOwnerV1 for Stage11PreStoreFinalitySeedV1 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage11_owner_seed_is_constructible_only_in_its_owner_module() {
        let mut backend = acquire();
        assert!(matches!(
            super::super::durable_finality::prepare_pre_store_from_stage11_owner(&mut backend),
            Err(super::super::durable_finality::Stage11PreStoreFinalityErrorV1)
        ));
    }
}
