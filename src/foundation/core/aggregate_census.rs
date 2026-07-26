use std::cell::Cell;
use std::marker::PhantomData;
use std::rc::Rc;

use super::secure_fs::{InventoryRowV1, SecureFsError, SecureFsResult};

pub(super) mod owner_sealed {
    pub trait Sealed {}
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum AggregateRootRoleV1 {
    Required,
    OptionalPresent,
}

#[derive(Eq, PartialEq)]
pub(super) struct AggregateRootFactsV1 {
    pub(super) role: AggregateRootRoleV1,
    pub(super) declared_locator: [u8; 32],
    pub(super) resolved_identity: [u8; 32],
    pub(super) mount_identity: [u8; 32],
    pub(super) provider_identity: [u8; 32],
    pub(super) anchor_identity: [u8; 32],
    pub(super) fence_identity: [u8; 32],
    pub(super) journal_position: [u8; 32],
}

#[derive(Eq, PartialEq)]
pub(super) struct AggregateRootSetFactsV1 {
    pub(super) admitted_set: [u8; 32],
    pub(super) namespace_epoch: u64,
    pub(super) roots: Vec<AggregateRootFactsV1>,
    pub(super) maximum_entries: u64,
    pub(super) maximum_bytes: u64,
}

#[derive(Eq, PartialEq)]
pub(super) struct AggregateComponentCensusV1 {
    pub(super) resolved_identity: [u8; 32],
    pub(super) inventory: [u8; 32],
    pub(super) root_binding: [u8; 32],
    pub(super) rows: Vec<InventoryRowV1>,
    pub(super) entry_count: u64,
    pub(super) byte_count: u64,
}

pub(super) trait AggregateCensusBackendV1: owner_sealed::Sealed {
    fn acquire_complete_root_set(&mut self) -> SecureFsResult<AggregateRootSetFactsV1>;

    fn census_pass(
        &mut self,
        roots: &AggregateRootSetFactsV1,
        pass: u8,
    ) -> SecureFsResult<Vec<AggregateComponentCensusV1>>;

    fn final_root_set_recheck(&mut self) -> SecureFsResult<AggregateRootSetFactsV1>;
}

struct AggregateCensusLeaseV1<'scan, B: AggregateCensusBackendV1> {
    backend: &'scan mut B,
    roots: AggregateRootSetFactsV1,
    consumed: Cell<bool>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'scan, B: AggregateCensusBackendV1> AggregateCensusLeaseV1<'scan, B> {
    fn acquire(backend: &'scan mut B) -> SecureFsResult<Self> {
        let roots = backend.acquire_complete_root_set()?;
        validate_root_set(&roots)?;
        Ok(Self {
            backend,
            roots,
            consumed: Cell::new(false),
            _not_send_or_sync: PhantomData,
        })
    }

    fn consume(self) -> SecureFsResult<AggregateCensusOutputV1> {
        if self.consumed.replace(true) {
            return Err(SecureFsError::CensusRefused);
        }
        let first = self.backend.census_pass(&self.roots, 1)?;
        validate_pass(&self.roots, &first)?;
        let second = self.backend.census_pass(&self.roots, 2)?;
        validate_pass(&self.roots, &second)?;
        if first != second {
            return Err(SecureFsError::CensusRefused);
        }
        let final_roots = self.backend.final_root_set_recheck()?;
        if final_roots != self.roots {
            return Err(SecureFsError::CensusRefused);
        }
        let (entries, bytes) = totals(&first)?;
        Ok(AggregateCensusOutputV1 {
            admitted_set: self.roots.admitted_set,
            roots: first,
            entries,
            bytes,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(crate) struct AggregateCensusOutputV1 {
    admitted_set: [u8; 32],
    roots: Vec<AggregateComponentCensusV1>,
    entries: u64,
    bytes: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(crate) struct AggregateCensusInventoriesV1 {
    pub(crate) admitted_set: [u8; 32],
    pub(crate) entries: u64,
    pub(crate) bytes: u64,
    pub(crate) roots: Vec<([u8; 32], Vec<InventoryRowV1>)>,
}

impl AggregateCensusOutputV1 {
    pub(crate) fn into_inventories(self) -> AggregateCensusInventoriesV1 {
        AggregateCensusInventoriesV1 {
            admitted_set: self.admitted_set,
            entries: self.entries,
            bytes: self.bytes,
            roots: self
                .roots
                .into_iter()
                .map(|root| (root.resolved_identity, root.rows))
                .collect(),
        }
    }
}

fn validate_root_set(roots: &AggregateRootSetFactsV1) -> SecureFsResult<()> {
    if roots.admitted_set == [0; 32]
        || roots.namespace_epoch == 0
        || roots.roots.is_empty()
        || roots.maximum_entries == 0
        || roots.maximum_bytes == 0
    {
        return Err(SecureFsError::CensusRefused);
    }
    for (index, root) in roots.roots.iter().enumerate() {
        match root.role {
            AggregateRootRoleV1::Required | AggregateRootRoleV1::OptionalPresent => {}
        }
        let commitments = [
            root.declared_locator,
            root.resolved_identity,
            root.mount_identity,
            root.provider_identity,
            root.anchor_identity,
            root.fence_identity,
            root.journal_position,
        ];
        if commitments.contains(&[0; 32]) {
            return Err(SecureFsError::CensusRefused);
        }
        if roots.roots[..index].iter().any(|prior| {
            prior.declared_locator == root.declared_locator
                || prior.resolved_identity == root.resolved_identity
                || prior.anchor_identity == root.anchor_identity
        }) {
            return Err(SecureFsError::CensusRefused);
        }
    }
    Ok(())
}

fn validate_pass(
    roots: &AggregateRootSetFactsV1,
    pass: &[AggregateComponentCensusV1],
) -> SecureFsResult<()> {
    if pass.len() != roots.roots.len() {
        return Err(SecureFsError::CensusRefused);
    }
    for (root, census) in roots.roots.iter().zip(pass) {
        if census.resolved_identity != root.resolved_identity
            || census.inventory == [0; 32]
            || census.root_binding == [0; 32]
            || census.entry_count != census.rows.len() as u64
            || census.byte_count
                != census
                    .rows
                    .iter()
                    .map(InventoryRowV1::logical_byte_length)
                    .sum::<u64>()
        {
            return Err(SecureFsError::CensusRefused);
        }
    }
    let (entries, bytes) = totals(pass)?;
    if entries > roots.maximum_entries || bytes > roots.maximum_bytes {
        return Err(SecureFsError::CensusRefused);
    }
    Ok(())
}

fn totals(pass: &[AggregateComponentCensusV1]) -> SecureFsResult<(u64, u64)> {
    pass.iter()
        .try_fold((0_u64, 0_u64), |(entries, bytes), row| {
            Ok((
                entries
                    .checked_add(row.entry_count)
                    .ok_or(SecureFsError::CensusRefused)?,
                bytes
                    .checked_add(row.byte_count)
                    .ok_or(SecureFsError::CensusRefused)?,
            ))
        })
}

pub(crate) fn census_from_stage11_owner() -> SecureFsResult<AggregateCensusOutputV1> {
    let mut backend = super::aggregate_census_stage11_seed::acquire()?;
    let result = AggregateCensusLeaseV1::acquire(&mut backend)?.consume()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBackendV1 {
        second: Vec<AggregateComponentCensusV1>,
        final_roots: AggregateRootSetFactsV1,
        passes: Vec<u8>,
    }

    impl owner_sealed::Sealed for TestBackendV1 {}

    impl AggregateCensusBackendV1 for TestBackendV1 {
        fn acquire_complete_root_set(&mut self) -> SecureFsResult<AggregateRootSetFactsV1> {
            Ok(root_set())
        }

        fn census_pass(
            &mut self,
            _roots: &AggregateRootSetFactsV1,
            pass: u8,
        ) -> SecureFsResult<Vec<AggregateComponentCensusV1>> {
            self.passes.push(pass);
            Ok(if pass == 1 {
                component_rows()
            } else {
                core::mem::take(&mut self.second)
            })
        }

        fn final_root_set_recheck(&mut self) -> SecureFsResult<AggregateRootSetFactsV1> {
            Ok(root_set_with_epoch(self.final_roots.namespace_epoch))
        }
    }

    fn root_set() -> AggregateRootSetFactsV1 {
        root_set_with_epoch(40)
    }

    fn root_set_with_epoch(namespace_epoch: u64) -> AggregateRootSetFactsV1 {
        AggregateRootSetFactsV1 {
            admitted_set: [1; 32],
            namespace_epoch,
            roots: vec![
                AggregateRootFactsV1 {
                    role: AggregateRootRoleV1::Required,
                    declared_locator: [2; 32],
                    resolved_identity: [3; 32],
                    mount_identity: [4; 32],
                    provider_identity: [5; 32],
                    anchor_identity: [6; 32],
                    fence_identity: [7; 32],
                    journal_position: [8; 32],
                },
                AggregateRootFactsV1 {
                    role: AggregateRootRoleV1::OptionalPresent,
                    declared_locator: [9; 32],
                    resolved_identity: [10; 32],
                    mount_identity: [11; 32],
                    provider_identity: [12; 32],
                    anchor_identity: [13; 32],
                    fence_identity: [14; 32],
                    journal_position: [15; 32],
                },
            ],
            maximum_entries: 50,
            maximum_bytes: 500,
        }
    }

    fn component_rows() -> Vec<AggregateComponentCensusV1> {
        vec![
            AggregateComponentCensusV1 {
                resolved_identity: [3; 32],
                inventory: [16; 32],
                root_binding: [17; 32],
                rows: Vec::new(),
                entry_count: 0,
                byte_count: 0,
            },
            AggregateComponentCensusV1 {
                resolved_identity: [10; 32],
                inventory: [18; 32],
                root_binding: [19; 32],
                rows: Vec::new(),
                entry_count: 0,
                byte_count: 0,
            },
        ]
    }

    fn backend() -> TestBackendV1 {
        TestBackendV1 {
            second: component_rows(),
            final_roots: root_set(),
            passes: Vec::new(),
        }
    }

    #[test]
    fn owner_holds_the_complete_root_set_across_both_passes() {
        let mut backend = backend();
        let output = AggregateCensusLeaseV1::acquire(&mut backend)
            .and_then(AggregateCensusLeaseV1::consume)
            .unwrap();
        let inventories = output.into_inventories();
        assert_eq!(
            (
                inventories.admitted_set,
                inventories.entries,
                inventories.bytes,
                inventories.roots.len(),
            ),
            ([1; 32], 0, 0, 2)
        );
        assert_eq!(backend.passes, [1, 2]);
    }

    #[test]
    fn partial_or_sequential_component_results_refuse() {
        let mut partial = backend();
        partial.second.pop();
        assert!(matches!(
            AggregateCensusLeaseV1::acquire(&mut partial).and_then(AggregateCensusLeaseV1::consume),
            Err(SecureFsError::CensusRefused)
        ));

        let mut changed = backend();
        changed.final_roots.namespace_epoch += 1;
        assert!(matches!(
            AggregateCensusLeaseV1::acquire(&mut changed).and_then(AggregateCensusLeaseV1::consume),
            Err(SecureFsError::CensusRefused)
        ));
    }

    #[test]
    fn aliases_and_aggregate_overflow_refuse() {
        let mut aliased = root_set();
        aliased.roots[1].resolved_identity = aliased.roots[0].resolved_identity;
        assert!(matches!(
            validate_root_set(&aliased),
            Err(SecureFsError::CensusRefused)
        ));

        let rows = vec![
            AggregateComponentCensusV1 {
                resolved_identity: [3; 32],
                inventory: [16; 32],
                root_binding: [17; 32],
                rows: Vec::new(),
                entry_count: u64::MAX,
                byte_count: 1,
            },
            AggregateComponentCensusV1 {
                resolved_identity: [10; 32],
                inventory: [18; 32],
                root_binding: [19; 32],
                rows: Vec::new(),
                entry_count: 1,
                byte_count: 1,
            },
        ];
        assert!(matches!(totals(&rows), Err(SecureFsError::CensusRefused)));
    }

    #[test]
    fn production_stage11_seed_is_fail_closed_until_the_backend_integrates() {
        assert!(matches!(
            census_from_stage11_owner(),
            Err(SecureFsError::CensusRefused)
        ));
    }
}
