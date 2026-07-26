use super::durable_finality::{
    ActiveStoreFinalityReadbackV1, ActiveStoreFinalityV1, DurableInstallationFinalityErrorV1,
    DurableInstallationFinalityRequestV1, DurableInstallationOwnerEffectV1,
    InstallationFinalityCurrentnessV1, owner_sealed,
};

pub(super) struct Stage9ActiveStoreFinalitySeedV1 {
    _private: (),
}

pub(super) fn acquire() -> Stage9ActiveStoreFinalitySeedV1 {
    Stage9ActiveStoreFinalitySeedV1 { _private: () }
}

impl owner_sealed::Sealed for Stage9ActiveStoreFinalitySeedV1 {}

impl DurableInstallationOwnerEffectV1<ActiveStoreFinalityV1> for Stage9ActiveStoreFinalitySeedV1 {
    type Readback = ActiveStoreFinalityReadbackV1;

    fn linearize(
        &mut self,
        _expected: InstallationFinalityCurrentnessV1,
        _request: &DurableInstallationFinalityRequestV1<ActiveStoreFinalityV1>,
    ) -> Result<Self::Readback, DurableInstallationFinalityErrorV1> {
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
