use super::aggregate_census::{
    AggregateCensusBackendV1, AggregateComponentCensusV1, AggregateRootSetFactsV1, owner_sealed,
};
use super::secure_fs::{SecureFsError, SecureFsResult};

pub(super) struct Stage11AggregateCensusBackendSeedV1 {
    _private: (),
}

impl owner_sealed::Sealed for Stage11AggregateCensusBackendSeedV1 {}

impl AggregateCensusBackendV1 for Stage11AggregateCensusBackendSeedV1 {
    fn acquire_complete_root_set(&mut self) -> SecureFsResult<AggregateRootSetFactsV1> {
        Err(SecureFsError::CensusRefused)
    }

    fn census_pass(
        &mut self,
        _roots: &AggregateRootSetFactsV1,
        _pass: u8,
    ) -> SecureFsResult<Vec<AggregateComponentCensusV1>> {
        Err(SecureFsError::CensusRefused)
    }

    fn final_root_set_recheck(&mut self) -> SecureFsResult<AggregateRootSetFactsV1> {
        Err(SecureFsError::CensusRefused)
    }

    fn aggregate_fence_is_live(&self) -> bool {
        false
    }

    fn consume_final_aggregate_fence(
        &mut self,
        _scan_invocation: [u8; 32],
    ) -> SecureFsResult<()> {
        Err(SecureFsError::CensusRefused)
    }
}

pub(super) fn acquire() -> Stage11AggregateCensusBackendSeedV1 {
    Stage11AggregateCensusBackendSeedV1 { _private: () }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_seed_is_explicitly_replaceable_and_fail_closed() {
        let mut backend = acquire();
        assert!(matches!(
            super::super::aggregate_census::census_from_stage11_owner(&mut backend),
            Err(SecureFsError::CensusRefused)
        ));
    }
}
