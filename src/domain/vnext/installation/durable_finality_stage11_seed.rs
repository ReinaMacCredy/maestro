use super::durable_finality::{
    DurableInstallationFinalityErrorV1, DurableInstallationFinalityRequestV1,
    DurableInstallationOwnerEffectV1, InstallationFinalityCurrentnessV1,
    PreStoreFinalityReadbackV1, PreStoreFinalityV1, owner_sealed,
};

pub(super) struct Stage11PreStoreFinalitySeedV1 {
    _private: (),
}

pub(super) fn acquire() -> Stage11PreStoreFinalitySeedV1 {
    Stage11PreStoreFinalitySeedV1 { _private: () }
}

impl owner_sealed::Sealed for Stage11PreStoreFinalitySeedV1 {}

impl DurableInstallationOwnerEffectV1<PreStoreFinalityV1> for Stage11PreStoreFinalitySeedV1 {
    type Readback = PreStoreFinalityReadbackV1;

    fn linearize(
        &mut self,
        _expected: InstallationFinalityCurrentnessV1,
        _request: &DurableInstallationFinalityRequestV1<PreStoreFinalityV1>,
    ) -> Result<Self::Readback, DurableInstallationFinalityErrorV1> {
        Err(DurableInstallationFinalityErrorV1::BackendUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage11_owner_seed_is_constructible_only_in_its_owner_module() {
        let _ = acquire();
    }
}
