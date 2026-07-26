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
    OptionalAbsent,
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
    pub(super) locator_components: Vec<Vec<u8>>,
    pub(super) absence_fence: Option<[u8; 32]>,
}

#[derive(Eq, PartialEq)]
pub(super) struct AggregateRootSetFactsV1 {
    pub(super) admitted_set: [u8; 32],
    pub(super) namespace_epoch: u64,
    pub(super) roots: Vec<AggregateRootFactsV1>,
    pub(super) maximum_entries: u64,
    pub(super) maximum_bytes: u64,
    pub(super) maximum_roots: u64,
    pub(super) maximum_descriptors: u64,
    pub(super) maximum_depth: u64,
    pub(super) maximum_name_bytes: u64,
    pub(super) scan_invocation: [u8; 32],
    pub(super) root_set_currentness: [u8; 32],
    pub(super) revocation_revision: u64,
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

    fn aggregate_fence_is_live(&self) -> bool;

    fn consume_final_aggregate_fence(&mut self, scan_invocation: [u8; 32]) -> SecureFsResult<()>;
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

    fn consume(self) -> SecureFsResult<AggregateCensusResultV1<'scan>> {
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
        if !self.backend.aggregate_fence_is_live() {
            return Err(SecureFsError::CensusRefused);
        }
        let final_roots = self.backend.final_root_set_recheck()?;
        if final_roots != self.roots || !self.backend.aggregate_fence_is_live() {
            return Err(SecureFsError::CensusRefused);
        }
        validate_cross_root_aliases(&first)?;
        self.backend
            .consume_final_aggregate_fence(self.roots.scan_invocation)?;
        let (entries, bytes) = totals(&first)?;
        Ok(AggregateCensusResultV1 {
            admitted_set: self.roots.admitted_set,
            roots: first,
            entries,
            bytes,
            _scan: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(super) struct AggregateCensusResultV1<'scan> {
    admitted_set: [u8; 32],
    roots: Vec<AggregateComponentCensusV1>,
    entries: u64,
    bytes: u64,
    _scan: PhantomData<&'scan mut ()>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(super) struct AggregateCensusViewV1<'scan> {
    admitted_set: [u8; 32],
    entries: u64,
    bytes: u64,
    roots: &'scan [AggregateComponentCensusV1],
}

pub(super) trait AggregateCensusConsumerV1: owner_sealed::Sealed {
    fn consume(&mut self, view: AggregateCensusViewV1<'_>) -> SecureFsResult<()>;
}

impl AggregateCensusResultV1<'_> {
    pub(super) fn consume_by_stage11(
        self,
        consumer: &mut dyn AggregateCensusConsumerV1,
    ) -> SecureFsResult<()> {
        consumer.consume(AggregateCensusViewV1 {
            admitted_set: self.admitted_set,
            entries: self.entries,
            bytes: self.bytes,
            roots: &self.roots,
        })
    }

    pub(super) fn into_stage11_parts(
        self,
    ) -> ([u8; 32], u64, u64, Vec<AggregateComponentCensusV1>) {
        (self.admitted_set, self.entries, self.bytes, self.roots)
    }
}

fn validate_root_set(roots: &AggregateRootSetFactsV1) -> SecureFsResult<()> {
    if roots.admitted_set == [0; 32]
        || roots.namespace_epoch == 0
        || roots.roots.is_empty()
        || roots.maximum_entries == 0
        || roots.maximum_bytes == 0
        || roots.maximum_roots == 0
        || roots.maximum_descriptors == 0
        || roots.maximum_depth == 0
        || roots.maximum_name_bytes == 0
        || roots.scan_invocation == [0; 32]
        || roots.root_set_currentness == [0; 32]
        || roots.revocation_revision == 0
        || roots.roots.len() as u64 > roots.maximum_roots
    {
        return Err(SecureFsError::CensusRefused);
    }
    for (index, root) in roots.roots.iter().enumerate() {
        let present = matches!(
            root.role,
            AggregateRootRoleV1::Required | AggregateRootRoleV1::OptionalPresent
        );
        let commitments = [root.declared_locator];
        if commitments.contains(&[0; 32])
            || root.locator_components.is_empty()
            || root.locator_components.len() as u64 > roots.maximum_depth
            || root.locator_components.iter().any(|component| {
                component.is_empty() || component.len() as u64 > roots.maximum_name_bytes
            })
            || (present
                && [
                    root.resolved_identity,
                    root.mount_identity,
                    root.provider_identity,
                    root.anchor_identity,
                    root.fence_identity,
                    root.journal_position,
                ]
                .contains(&[0; 32]))
            || (present && root.absence_fence.is_some())
            || (!present
                && (root.absence_fence.is_none()
                    || root.resolved_identity != [0; 32]
                    || root.mount_identity != [0; 32]
                    || root.provider_identity != [0; 32]
                    || root.anchor_identity != [0; 32]
                    || root.fence_identity != [0; 32]
                    || root.journal_position != [0; 32]))
        {
            return Err(SecureFsError::CensusRefused);
        }
        if roots.roots[..index].iter().any(|prior| {
            prior.declared_locator == root.declared_locator
                || (present
                    && prior.role != AggregateRootRoleV1::OptionalAbsent
                    && (prior.resolved_identity == root.resolved_identity
                        || prior.anchor_identity == root.anchor_identity))
                || is_locator_prefix(&prior.locator_components, &root.locator_components)
                || is_locator_prefix(&root.locator_components, &prior.locator_components)
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
    let present_roots = roots
        .roots
        .iter()
        .filter(|root| root.role != AggregateRootRoleV1::OptionalAbsent)
        .collect::<Vec<_>>();
    if pass.len() != present_roots.len() {
        return Err(SecureFsError::CensusRefused);
    }
    for (root, census) in present_roots.into_iter().zip(pass) {
        if census.resolved_identity != root.resolved_identity
            || census.inventory == [0; 32]
            || census.root_binding == [0; 32]
            || census.entry_count != census.rows.len() as u64
            || census.byte_count != checked_row_bytes(&census.rows)?
        {
            return Err(SecureFsError::CensusRefused);
        }
    }
    let (entries, bytes) = totals(pass)?;
    if entries > roots.maximum_entries
        || entries > roots.maximum_descriptors
        || bytes > roots.maximum_bytes
    {
        return Err(SecureFsError::CensusRefused);
    }
    Ok(())
}

fn checked_row_bytes(rows: &[InventoryRowV1]) -> SecureFsResult<u64> {
    rows.iter().try_fold(0_u64, |total, row| {
        total
            .checked_add(row.logical_byte_length())
            .ok_or(SecureFsError::CensusRefused)
    })
}

fn is_locator_prefix(left: &[Vec<u8>], right: &[Vec<u8>]) -> bool {
    left.len() < right.len() && right.starts_with(left)
}

fn validate_cross_root_aliases(pass: &[AggregateComponentCensusV1]) -> SecureFsResult<()> {
    for (root_index, root) in pass.iter().enumerate() {
        for row in &root.rows {
            if pass[..root_index].iter().any(|prior| {
                prior
                    .rows
                    .iter()
                    .any(|candidate| candidate.object_identity() == row.object_identity())
            }) {
                return Err(SecureFsError::CensusRefused);
            }
        }
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

pub(super) fn census_from_stage11_owner<'scan>(
    backend: &'scan mut super::aggregate_census_stage11_seed::Stage11AggregateCensusBackendSeedV1,
) -> SecureFsResult<AggregateCensusResultV1<'scan>> {
    let result = AggregateCensusLeaseV1::acquire(backend)?.consume()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestBackendV1 {
        second: Vec<AggregateComponentCensusV1>,
        final_roots: AggregateRootSetFactsV1,
        passes: Vec<u8>,
        fence_live: bool,
        fence_consumed: bool,
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

        fn aggregate_fence_is_live(&self) -> bool {
            self.fence_live && !self.fence_consumed
        }

        fn consume_final_aggregate_fence(
            &mut self,
            scan_invocation: [u8; 32],
        ) -> SecureFsResult<()> {
            if !self.aggregate_fence_is_live() || scan_invocation != [25; 32] {
                return Err(SecureFsError::CensusRefused);
            }
            self.fence_consumed = true;
            Ok(())
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
                    locator_components: vec![b"global".to_vec()],
                    absence_fence: None,
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
                    locator_components: vec![b"project".to_vec()],
                    absence_fence: None,
                },
                AggregateRootFactsV1 {
                    role: AggregateRootRoleV1::OptionalAbsent,
                    declared_locator: [20; 32],
                    resolved_identity: [0; 32],
                    mount_identity: [0; 32],
                    provider_identity: [0; 32],
                    anchor_identity: [0; 32],
                    fence_identity: [0; 32],
                    journal_position: [0; 32],
                    locator_components: vec![b"legacy".to_vec()],
                    absence_fence: Some([21; 32]),
                },
            ],
            maximum_entries: 50,
            maximum_bytes: 500,
            maximum_roots: 3,
            maximum_descriptors: 100,
            maximum_depth: 4,
            maximum_name_bytes: 64,
            scan_invocation: [25; 32],
            root_set_currentness: [26; 32],
            revocation_revision: 27,
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
            fence_live: true,
            fence_consumed: false,
        }
    }

    struct TestConsumerV1 {
        observed: Option<([u8; 32], u64, u64, usize)>,
    }

    impl owner_sealed::Sealed for TestConsumerV1 {}

    impl AggregateCensusConsumerV1 for TestConsumerV1 {
        fn consume(&mut self, view: AggregateCensusViewV1<'_>) -> SecureFsResult<()> {
            self.observed = Some((
                view.admitted_set,
                view.entries,
                view.bytes,
                view.roots.len(),
            ));
            Ok(())
        }
    }

    #[test]
    fn owner_holds_the_complete_root_set_across_both_passes() {
        let mut backend = backend();
        let output = AggregateCensusLeaseV1::acquire(&mut backend)
            .and_then(AggregateCensusLeaseV1::consume)
            .unwrap();
        let mut consumer = TestConsumerV1 { observed: None };
        output.consume_by_stage11(&mut consumer).unwrap();
        assert_eq!(consumer.observed, Some(([1; 32], 0, 0, 2)));
        assert_eq!(backend.passes, [1, 2]);
        assert!(backend.fence_consumed);
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
    fn optional_absence_overlap_and_early_fence_release_refuse() {
        let roots = root_set();
        assert!(matches!(
            roots.roots[2].role,
            AggregateRootRoleV1::OptionalAbsent
        ));

        let mut overlap = root_set();
        overlap.roots[1].locator_components = vec![b"global".to_vec(), b"child".to_vec()];
        assert!(matches!(
            validate_root_set(&overlap),
            Err(SecureFsError::CensusRefused)
        ));

        let mut released = backend();
        released.fence_live = false;
        assert!(matches!(
            AggregateCensusLeaseV1::acquire(&mut released)
                .and_then(AggregateCensusLeaseV1::consume),
            Err(SecureFsError::CensusRefused)
        ));
    }

    #[test]
    fn production_stage11_seed_is_fail_closed_until_the_backend_integrates() {
        let mut backend = super::super::aggregate_census_stage11_seed::acquire();
        assert!(matches!(
            census_from_stage11_owner(&mut backend),
            Err(SecureFsError::CensusRefused)
        ));
    }
}
