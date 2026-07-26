use super::durable_finality::{
    ActiveStoreFinalityOwnerV1, ActiveStoreFinalityRequestV1, ActiveStoreOwnerOutcomeV1,
    DurableInstallationFinalityErrorV1, InstallationFinalityCurrentnessV1, owner_sealed,
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
