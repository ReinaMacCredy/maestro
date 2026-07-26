use super::durable_finality::{
    ActiveStoreFinalityOwnerV1, ActiveStoreFinalityRequestV1, ActiveStoreOwnerOutcomeV1,
    DurableInstallationFinalityErrorV1, InstallationFinalityCurrentnessV1, owner_sealed,
};

pub(super) struct Stage9ActiveStoreFinalitySeedV1 {
    _private: (),
}

pub(super) fn acquire() -> Stage9ActiveStoreFinalitySeedV1 {
    Stage9ActiveStoreFinalitySeedV1 { _private: () }
}

impl owner_sealed::Sealed for Stage9ActiveStoreFinalitySeedV1 {}

impl ActiveStoreFinalityOwnerV1 for Stage9ActiveStoreFinalitySeedV1 {
    fn commit_and_readback(
        &mut self,
        _expected: InstallationFinalityCurrentnessV1,
        _request: &ActiveStoreFinalityRequestV1,
    ) -> Result<ActiveStoreOwnerOutcomeV1, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage9_owner_seed_is_constructible_only_in_its_owner_module() {
        let _ = acquire();
    }
}
