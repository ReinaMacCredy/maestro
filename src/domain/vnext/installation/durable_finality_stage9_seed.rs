use super::durable_finality::{
    ActiveStoreFinalityOwnerV1, ActiveStoreFinalityOwnerV2, ActiveStoreFinalityRequestV1,
    ActiveStoreFinalityRequestV2, ActiveStoreOwnerOutcomeV1, ActiveStoreOwnerOutcomeV2,
    DurableInstallationFinalityErrorV1, DurableInstallationFinalityErrorV2,
    InstallationFinalityCurrentnessV1, owner_sealed,
};

pub(in crate::domain::vnext) struct Stage9ActiveStoreFinalitySeedV1 {
    _private: (),
}

impl Stage9ActiveStoreFinalitySeedV1 {
    #[cfg(test)]
    pub(in crate::domain::vnext) fn test_unavailable() -> Self {
        Self { _private: () }
    }
}

impl ActiveStoreFinalityOwnerV1 for Stage9ActiveStoreFinalitySeedV1 {
    fn prepare_request(
        &mut self,
    ) -> Result<ActiveStoreFinalityRequestV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }

    fn commit_and_readback(
        &mut self,
        _expected: InstallationFinalityCurrentnessV1,
        _request: &ActiveStoreFinalityRequestV1,
    ) -> Result<ActiveStoreOwnerOutcomeV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }
}

impl owner_sealed::Sealed for Stage9ActiveStoreFinalitySeedV1 {}

pub(in crate::domain::vnext) struct Stage9ActiveStoreFinalitySeedV2 {
    _private: (),
}

impl Stage9ActiveStoreFinalitySeedV2 {
    #[cfg(test)]
    pub(in crate::domain::vnext) fn test_unavailable() -> Self {
        Self { _private: () }
    }
}

impl owner_sealed::Sealed for Stage9ActiveStoreFinalitySeedV2 {}

impl ActiveStoreFinalityOwnerV2 for Stage9ActiveStoreFinalitySeedV2 {
    fn capture_active_request(
        &mut self,
    ) -> Result<ActiveStoreFinalityRequestV2, DurableInstallationFinalityErrorV2> {
        Err(DurableInstallationFinalityErrorV2::BackendUnavailable)
    }

    fn commit_and_readback(
        &mut self,
        _request: &ActiveStoreFinalityRequestV2,
    ) -> Result<ActiveStoreOwnerOutcomeV2, DurableInstallationFinalityErrorV2> {
        Err(DurableInstallationFinalityErrorV2::BackendUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage9_owner_test_provider_is_constructible_only_in_its_owner_module() {
        let mut backend = Stage9ActiveStoreFinalitySeedV1::test_unavailable();
        assert!(matches!(
            super::super::durable_finality::prepare_active_from_stage9_owner(&mut backend),
            Err(super::super::durable_finality::Stage9ActiveStoreFinalityErrorV1)
        ));
    }
}
