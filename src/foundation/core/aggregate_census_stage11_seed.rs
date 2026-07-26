use super::aggregate_census::{
    AggregateCensusBackendV1, AggregateCensusResultV1, AggregateComponentCensusV1,
    AggregateRootSetFactsV1, owner_sealed,
};
use super::secure_fs::{InventoryRowV1, SecureFsError, SecureFsResult};

pub(super) struct Stage11AggregateCensusBackendSeedV1 {
    _private: (),
}

pub(super) struct Stage11AggregateCensusOutputV1<'scan> {
    result: AggregateCensusResultV1<'scan>,
}

pub(super) struct Stage11AggregateCensusComponentV1 {
    resolved_identity: [u8; 32],
    inventory: [u8; 32],
    root_binding: [u8; 32],
    rows: Vec<InventoryRowV1>,
}

impl Stage11AggregateCensusOutputV1<'_> {
    pub(super) fn into_parts(self) -> ([u8; 32], u64, u64, Vec<Stage11AggregateCensusComponentV1>) {
        let (admitted_set, entries, bytes, roots) = self.result.into_stage11_parts();
        let roots = roots
            .into_iter()
            .map(|root| Stage11AggregateCensusComponentV1 {
                resolved_identity: root.resolved_identity,
                inventory: root.inventory,
                root_binding: root.root_binding,
                rows: root.rows,
            })
            .collect();
        (admitted_set, entries, bytes, roots)
    }
}

impl Stage11AggregateCensusComponentV1 {
    pub(super) fn into_parts(self) -> ([u8; 32], [u8; 32], [u8; 32], Vec<InventoryRowV1>) {
        (
            self.resolved_identity,
            self.inventory,
            self.root_binding,
            self.rows,
        )
    }
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

    fn consume_final_aggregate_fence(&mut self, _scan_invocation: [u8; 32]) -> SecureFsResult<()> {
        Err(SecureFsError::CensusRefused)
    }
}

pub(super) fn acquire() -> Stage11AggregateCensusBackendSeedV1 {
    Stage11AggregateCensusBackendSeedV1 { _private: () }
}

pub(super) fn census_from_stage11_owner<'scan>(
    backend: &'scan mut Stage11AggregateCensusBackendSeedV1,
) -> SecureFsResult<Stage11AggregateCensusOutputV1<'scan>> {
    super::aggregate_census::census_from_stage11_owner(backend)
        .map(|result| Stage11AggregateCensusOutputV1 { result })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_seed_is_explicitly_replaceable_and_fail_closed() {
        let mut backend = acquire();
        assert!(matches!(
            census_from_stage11_owner(&mut backend),
            Err(SecureFsError::CensusRefused)
        ));
    }
}
