use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::integration::consumer_closure::{
    ConsumerClosureErrorV1, ConsumerClosureLeasePortV1, HostConsumerAdmissionGuardV1,
};
use crate::domain::vnext::persistence::consumer_snapshot::{
    ConsumerSnapshotCurrentFactsV1, ConsumerSnapshotCurrentViewLeasePortV1,
    ConsumerSnapshotCurrentViewLeaseV1, ConsumerSnapshotCurrentnessErrorV1,
};

mod stage_sealed {
    pub trait Sealed {}
}

pub(in crate::domain::vnext) trait ConsumerClosureStageV1:
    stage_sealed::Sealed
{
    const TAG: u8;
}

pub(in crate::domain::vnext) struct PreCurrentnessConsumerStageV1;
pub(in crate::domain::vnext) struct ProtectedRetentionConsumerStageV1;
pub(in crate::domain::vnext) struct PhysicalPruningConsumerStageV1;

impl stage_sealed::Sealed for PreCurrentnessConsumerStageV1 {}
impl stage_sealed::Sealed for ProtectedRetentionConsumerStageV1 {}
impl stage_sealed::Sealed for PhysicalPruningConsumerStageV1 {}

impl ConsumerClosureStageV1 for PreCurrentnessConsumerStageV1 {
    const TAG: u8 = 1;
}
impl ConsumerClosureStageV1 for ProtectedRetentionConsumerStageV1 {
    const TAG: u8 = 2;
}
impl ConsumerClosureStageV1 for PhysicalPruningConsumerStageV1 {
    const TAG: u8 = 3;
}

pub(in crate::domain::vnext) mod owner_operation_sealed {
    pub trait Sealed {}
}

pub(in crate::domain::vnext) trait ConsumerClosureOwnerOperationPortV1<K, S, H>:
    owner_operation_sealed::Sealed
where
    K: ConsumerClosureStageV1,
    S: ConsumerSnapshotCurrentViewLeasePortV1,
    H: ConsumerClosureLeasePortV1,
{
    fn operation_identity(&self) -> [u8; 32];
    fn linearize(
        self,
        guard: ConsumerClosureFinalityGuardV1<'_, '_, S, H, K>,
    ) -> Result<ConsumerClosureFinalityFactsV1, InstallationConsumerSnapshotErrorV1>;
}

pub(in crate::domain::vnext) trait ConsumerClosureStageProofIssuerV1<K>:
    owner_operation_sealed::Sealed
where
    K: ConsumerClosureStageV1,
{
}

#[derive(Clone, Copy)]
pub(in crate::domain::vnext) struct ConsumerClosureFinalityFactsV1 {
    owner_operation: [u8; 32],
    stage_proof_commitment: [u8; 32],
    expected_old_cas: [u8; 32],
}

pub(in crate::domain::vnext) struct PreCurrentnessClosureProofV1 {
    active_consumer_zero: [u8; 32],
    canonical_consumer_rows: [u8; 32],
    association_write_set: [u8; 32],
}

pub(in crate::domain::vnext) struct ProtectedRetentionClosureProofV1 {
    admitted_typed_reader_set: [u8; 32],
    sealed_reader_closure: [u8; 32],
    retention_authority: [u8; 32],
}

pub(in crate::domain::vnext) struct PhysicalPruningClosureProofV1 {
    reader_zero: [u8; 32],
    hold_zero: [u8; 32],
    custody: [u8; 32],
    authority: [u8; 32],
    rollback_safety: [u8; 32],
    erasure_safety: [u8; 32],
    legacy_removal_authorization: [u8; 32],
}

impl PreCurrentnessClosureProofV1 {
    pub(in crate::domain::vnext) fn from_owner_proof<I>(
        _issuer: &I,
        active_consumer_zero: [u8; 32],
        canonical_consumer_rows: [u8; 32],
        association_write_set: [u8; 32],
    ) -> Result<Self, InstallationConsumerSnapshotErrorV1>
    where
        I: ConsumerClosureStageProofIssuerV1<PreCurrentnessConsumerStageV1>,
    {
        require_nonzero([
            active_consumer_zero,
            canonical_consumer_rows,
            association_write_set,
        ])?;
        Ok(Self {
            active_consumer_zero,
            canonical_consumer_rows,
            association_write_set,
        })
    }

    fn commitment(&self) -> [u8; 32] {
        canonical_stage_proof(
            1,
            &[
                self.active_consumer_zero,
                self.canonical_consumer_rows,
                self.association_write_set,
            ],
        )
    }
}

impl ProtectedRetentionClosureProofV1 {
    pub(in crate::domain::vnext) fn from_owner_proof<I>(
        _issuer: &I,
        admitted_typed_reader_set: [u8; 32],
        sealed_reader_closure: [u8; 32],
        retention_authority: [u8; 32],
    ) -> Result<Self, InstallationConsumerSnapshotErrorV1>
    where
        I: ConsumerClosureStageProofIssuerV1<ProtectedRetentionConsumerStageV1>,
    {
        require_nonzero([
            admitted_typed_reader_set,
            sealed_reader_closure,
            retention_authority,
        ])?;
        Ok(Self {
            admitted_typed_reader_set,
            sealed_reader_closure,
            retention_authority,
        })
    }

    fn commitment(&self) -> [u8; 32] {
        canonical_stage_proof(
            2,
            &[
                self.admitted_typed_reader_set,
                self.sealed_reader_closure,
                self.retention_authority,
            ],
        )
    }
}

impl PhysicalPruningClosureProofV1 {
    #[expect(
        clippy::too_many_arguments,
        reason = "physical pruning requires the complete locked safety conjunction"
    )]
    pub(in crate::domain::vnext) fn from_owner_proof<I>(
        _issuer: &I,
        reader_zero: [u8; 32],
        hold_zero: [u8; 32],
        custody: [u8; 32],
        authority: [u8; 32],
        rollback_safety: [u8; 32],
        erasure_safety: [u8; 32],
        legacy_removal_authorization: [u8; 32],
    ) -> Result<Self, InstallationConsumerSnapshotErrorV1>
    where
        I: ConsumerClosureStageProofIssuerV1<PhysicalPruningConsumerStageV1>,
    {
        require_nonzero([
            reader_zero,
            hold_zero,
            custody,
            authority,
            rollback_safety,
            erasure_safety,
            legacy_removal_authorization,
        ])?;
        Ok(Self {
            reader_zero,
            hold_zero,
            custody,
            authority,
            rollback_safety,
            erasure_safety,
            legacy_removal_authorization,
        })
    }

    fn commitment(&self) -> [u8; 32] {
        canonical_stage_proof(
            3,
            &[
                self.reader_zero,
                self.hold_zero,
                self.custody,
                self.authority,
                self.rollback_safety,
                self.erasure_safety,
                self.legacy_removal_authorization,
            ],
        )
    }
}

pub(in crate::domain::vnext) struct ConsumerClosureFinalityGuardV1<'view, 'connection, S, H, K> {
    owner_operation: [u8; 32],
    store: ConsumerSnapshotCurrentViewLeaseV1<'view, S>,
    host: HostConsumerAdmissionGuardV1<'connection, H>,
    _stage: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<
    S: ConsumerSnapshotCurrentViewLeasePortV1,
    H: ConsumerClosureLeasePortV1,
    K: ConsumerClosureStageV1,
> ConsumerClosureFinalityGuardV1<'_, '_, S, H, K>
{
    fn linearize(
        mut self,
        stage_proof_commitment: [u8; 32],
        expected_old_cas: [u8; 32],
        durable_effect: impl FnOnce() -> Result<(), InstallationConsumerSnapshotErrorV1>,
    ) -> Result<(), InstallationConsumerSnapshotErrorV1> {
        if stage_proof_commitment == [0; 32] || expected_old_cas == [0; 32] {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        self.host.recheck_current()?;
        let current = self.store.recheck_current()?;
        if current.owner_stage_tag() != K::TAG || current.owner_operation() != self.owner_operation
        {
            return Err(InstallationConsumerSnapshotErrorV1::OwnerStageMismatch);
        }
        durable_effect()?;
        Ok(())
    }

    fn finality(
        &self,
        stage_proof_commitment: [u8; 32],
        expected_old_cas: [u8; 32],
    ) -> ConsumerClosureFinalityFactsV1 {
        ConsumerClosureFinalityFactsV1 {
            owner_operation: self.owner_operation,
            stage_proof_commitment,
            expected_old_cas,
        }
    }

    pub(in crate::domain::vnext) fn linearize_pre_currentness_association(
        self,
        proof: PreCurrentnessClosureProofV1,
        expected_old_cas: [u8; 32],
        durable_effect: impl FnOnce() -> Result<(), InstallationConsumerSnapshotErrorV1>,
    ) -> Result<ConsumerClosureFinalityFactsV1, InstallationConsumerSnapshotErrorV1> {
        if K::TAG != PreCurrentnessConsumerStageV1::TAG {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        let commitment = proof.commitment();
        let finality = self.finality(commitment, expected_old_cas);
        self.linearize(commitment, expected_old_cas, durable_effect)?;
        Ok(finality)
    }

    pub(in crate::domain::vnext) fn linearize_protected_retention(
        self,
        proof: ProtectedRetentionClosureProofV1,
        expected_old_cas: [u8; 32],
        durable_effect: impl FnOnce() -> Result<(), InstallationConsumerSnapshotErrorV1>,
    ) -> Result<ConsumerClosureFinalityFactsV1, InstallationConsumerSnapshotErrorV1> {
        if K::TAG != ProtectedRetentionConsumerStageV1::TAG {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        let commitment = proof.commitment();
        let finality = self.finality(commitment, expected_old_cas);
        self.linearize(commitment, expected_old_cas, durable_effect)?;
        Ok(finality)
    }

    pub(in crate::domain::vnext) fn linearize_physical_pruning(
        self,
        proof: PhysicalPruningClosureProofV1,
        expected_old_cas: [u8; 32],
        durable_effect: impl FnOnce() -> Result<(), InstallationConsumerSnapshotErrorV1>,
    ) -> Result<ConsumerClosureFinalityFactsV1, InstallationConsumerSnapshotErrorV1> {
        if K::TAG != PhysicalPruningConsumerStageV1::TAG {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        let commitment = proof.commitment();
        let finality = self.finality(commitment, expected_old_cas);
        self.linearize(commitment, expected_old_cas, durable_effect)?;
        Ok(finality)
    }
}

pub(in crate::domain::vnext) struct InstallationConsumerSnapshotV1<'view, 'connection, S, H, O, K> {
    store: ConsumerSnapshotCurrentViewLeaseV1<'view, S>,
    host: HostConsumerAdmissionGuardV1<'connection, H>,
    owner_operation: O,
    consumer_set_id: [u8; 32],
    _stage: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<
    'view,
    'connection,
    S: ConsumerSnapshotCurrentViewLeasePortV1,
    H: ConsumerClosureLeasePortV1,
    O: ConsumerClosureOwnerOperationPortV1<K, S, H>,
    K: ConsumerClosureStageV1,
> InstallationConsumerSnapshotV1<'view, 'connection, S, H, O, K>
{
    pub(in crate::domain::vnext::installation) fn issue(
        store: ConsumerSnapshotCurrentViewLeaseV1<'view, S>,
        host: HostConsumerAdmissionGuardV1<'connection, H>,
        owner_operation: O,
    ) -> Result<Self, InstallationConsumerSnapshotErrorV1> {
        let current = store.initial();
        if current.owner_stage_tag() != K::TAG
            || current.owner_operation() != owner_operation.operation_identity()
            || (matches!(current, ConsumerSnapshotCurrentFactsV1::PreStore { .. })
                && K::TAG != PreCurrentnessConsumerStageV1::TAG)
        {
            return Err(InstallationConsumerSnapshotErrorV1::OwnerStageMismatch);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"maestro.vnext.installation-consumer-set.v1\0");
        hasher.update([K::TAG]);
        for commitment in current.canonical_commitments() {
            hasher.update(commitment);
        }
        for scalar in current.canonical_scalars() {
            hasher.update(scalar.to_be_bytes());
        }
        hasher.update((current.census_rows().len() as u64).to_be_bytes());
        for commitment in host.closure_commitments() {
            hasher.update(commitment);
        }
        let consumer_set_id = hasher.finalize().into();
        Ok(Self {
            store,
            host,
            owner_operation,
            consumer_set_id,
            _stage: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain::vnext::installation) fn consume_finality(
        self,
    ) -> Result<ConsumerClosureReceiptV1<'view, 'connection, K>, InstallationConsumerSnapshotErrorV1>
    {
        let initial_owner_operation = self.store.initial().owner_operation();
        let finality = self
            .owner_operation
            .linearize(ConsumerClosureFinalityGuardV1 {
                owner_operation: initial_owner_operation,
                store: self.store,
                host: self.host,
                _stage: PhantomData,
                _not_send_or_sync: PhantomData,
            })?;
        Ok(ConsumerClosureReceiptV1 {
            owner_operation: initial_owner_operation,
            consumer_set_id: self.consumer_set_id,
            finality_commitment: finality_commitment::<K>(&finality),
            _view: PhantomData,
            _connection: PhantomData,
            _stage: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(in crate::domain::vnext) struct ConsumerClosureReceiptV1<'view, 'connection, K> {
    owner_operation: [u8; 32],
    consumer_set_id: [u8; 32],
    finality_commitment: [u8; 32],
    _view: PhantomData<&'view mut ()>,
    _connection: PhantomData<&'connection mut ()>,
    _stage: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<K> ConsumerClosureReceiptV1<'_, '_, K> {
    pub(in crate::domain::vnext) const fn finality_commitment(&self) -> [u8; 32] {
        self.finality_commitment
    }
}

#[derive(Debug, Error)]
pub(in crate::domain::vnext) enum InstallationConsumerSnapshotErrorV1 {
    #[error(transparent)]
    Store(#[from] ConsumerSnapshotCurrentnessErrorV1),
    #[error(transparent)]
    Host(#[from] ConsumerClosureErrorV1),
    #[error("consumer stage does not match the sealed owning operation")]
    OwnerStageMismatch,
    #[error("consumer closure does not bind the exact owning finality operation")]
    FinalityMismatch,
}

fn finality_commitment<K: ConsumerClosureStageV1>(
    finality: &ConsumerClosureFinalityFactsV1,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"maestro.vnext.consumer-closure-finality.v1\0");
    hasher.update([K::TAG]);
    hasher.update(finality.owner_operation);
    hasher.update(finality.stage_proof_commitment);
    hasher.update(finality.expected_old_cas);
    hasher.finalize().into()
}

fn require_nonzero<const N: usize>(
    values: [[u8; 32]; N],
) -> Result<(), InstallationConsumerSnapshotErrorV1> {
    if values.contains(&[0; 32]) {
        return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
    }
    Ok(())
}

fn canonical_stage_proof(tag: u8, commitments: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"maestro.vnext.consumer-closure-stage-proof.v1\0");
    hasher.update([tag]);
    for commitment in commitments {
        hasher.update(commitment);
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    use crate::domain::vnext::integration::consumer_closure::test_seed as integration_seed;
    use crate::domain::vnext::persistence::consumer_snapshot::{
        ConsumerSnapshotCurrentFactsV1, test_seed as persistence_seed,
    };

    struct TestOwnerOperationV1<K> {
        operation_identity: [u8; 32],
        _stage: PhantomData<K>,
    }

    impl<K> owner_operation_sealed::Sealed for TestOwnerOperationV1<K> {}
    impl<K> ConsumerClosureStageProofIssuerV1<PreCurrentnessConsumerStageV1>
        for TestOwnerOperationV1<K>
    {
    }
    impl<K> ConsumerClosureStageProofIssuerV1<ProtectedRetentionConsumerStageV1>
        for TestOwnerOperationV1<K>
    {
    }
    impl<K> ConsumerClosureStageProofIssuerV1<PhysicalPruningConsumerStageV1>
        for TestOwnerOperationV1<K>
    {
    }

    impl<K, S, H> ConsumerClosureOwnerOperationPortV1<K, S, H> for TestOwnerOperationV1<K>
    where
        K: ConsumerClosureStageV1,
        S: ConsumerSnapshotCurrentViewLeasePortV1,
        H: ConsumerClosureLeasePortV1,
    {
        fn operation_identity(&self) -> [u8; 32] {
            self.operation_identity
        }

        fn linearize(
            self,
            guard: ConsumerClosureFinalityGuardV1<'_, '_, S, H, K>,
        ) -> Result<ConsumerClosureFinalityFactsV1, InstallationConsumerSnapshotErrorV1> {
            match K::TAG {
                1 => guard.linearize_pre_currentness_association(
                    PreCurrentnessClosureProofV1::from_owner_proof(
                        &self, [81; 32], [82; 32], [83; 32],
                    )?,
                    [90; 32],
                    || Ok(()),
                ),
                2 => guard.linearize_protected_retention(
                    ProtectedRetentionClosureProofV1::from_owner_proof(
                        &self, [84; 32], [85; 32], [86; 32],
                    )?,
                    [90; 32],
                    || Ok(()),
                ),
                3 => guard.linearize_physical_pruning(
                    PhysicalPruningClosureProofV1::from_owner_proof(
                        &self, [87; 32], [88; 32], [89; 32], [91; 32], [92; 32], [93; 32], [94; 32],
                    )?,
                    [90; 32],
                    || Ok(()),
                ),
                _ => Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch),
            }
        }
    }

    struct CountingOwnerOperationV1 {
        effects: Rc<Cell<u64>>,
    }

    impl owner_operation_sealed::Sealed for CountingOwnerOperationV1 {}
    impl ConsumerClosureStageProofIssuerV1<PreCurrentnessConsumerStageV1> for CountingOwnerOperationV1 {}

    impl<S, H> ConsumerClosureOwnerOperationPortV1<PreCurrentnessConsumerStageV1, S, H>
        for CountingOwnerOperationV1
    where
        S: ConsumerSnapshotCurrentViewLeasePortV1,
        H: ConsumerClosureLeasePortV1,
    {
        fn operation_identity(&self) -> [u8; 32] {
            [1; 32]
        }

        fn linearize(
            self,
            guard: ConsumerClosureFinalityGuardV1<'_, '_, S, H, PreCurrentnessConsumerStageV1>,
        ) -> Result<ConsumerClosureFinalityFactsV1, InstallationConsumerSnapshotErrorV1> {
            guard.linearize_pre_currentness_association(
                PreCurrentnessClosureProofV1::from_owner_proof(
                    &self, [81; 32], [82; 32], [83; 32],
                )?,
                [90; 32],
                || {
                    self.effects.set(self.effects.get() + 1);
                    Ok(())
                },
            )
        }
    }

    fn active_store_facts(stage: u8) -> ConsumerSnapshotCurrentFactsV1 {
        ConsumerSnapshotCurrentFactsV1::ActiveStore {
            owner_operation: [1; 32],
            owner_stage_tag: stage,
            store_instance: [2; 32],
            active_state_revision: [3; 32],
            activation_incarnation: [4; 32],
            restore_incarnation: [5; 32],
            head_identity: [6; 32],
            head_revision: [7; 32],
            generation_identity: [8; 32],
            generation_ordinal: 9,
            publication_clock: [10; 32],
            currentness: [11; 32],
            installation_id: [12; 32],
            realm: [13; 32],
            domain: [14; 32],
            census_identity: [15; 32],
            census_rows: vec![[16; 32], [17; 32]],
            release_identity: [18; 32],
            writer_protocol_epoch: 19,
            schema_epoch: 20,
            migration_epoch: 21,
            declared_consumer_root_manifest: [22; 32],
            public_resource_closure: [23; 32],
            public_bundle_closure: [24; 32],
            public_release_closure: [25; 32],
            alias_roots: [26; 32],
            manager_roots: [27; 32],
            target_roots: [28; 32],
            claims_catalog_descriptors: [29; 32],
        }
    }

    fn pre_store_facts() -> ConsumerSnapshotCurrentFactsV1 {
        ConsumerSnapshotCurrentFactsV1::PreStore {
            owner_operation: [1; 32],
            inactive_candidate_state: [2; 32],
            pre_association_root_closure: [3; 32],
            installation_id: [4; 32],
            realm: [5; 32],
            domain: [6; 32],
            candidate_census_identity: [7; 32],
            candidate_census_rows: vec![[8; 32], [9; 32]],
            candidate_release_identity: [10; 32],
            writer_protocol_epoch: 11,
            schema_epoch: 12,
            migration_epoch: 13,
            declared_consumer_root_manifest: [14; 32],
            public_resource_closure: [15; 32],
            public_bundle_closure: [16; 32],
            public_release_closure: [17; 32],
            alias_roots: [18; 32],
            manager_roots: [19; 32],
            target_roots: [20; 32],
            ceremony_spec: [21; 32],
            attempt: [22; 32],
            protected_source_carrier: [23; 32],
            candidate_carrier: [24; 32],
            expected_old_locator_root: [25; 32],
            expected_old_cas: [26; 32],
        }
    }

    fn issue<'store, 'host, K: ConsumerClosureStageV1>(
        store: &'store mut persistence_seed::TestProviderV1,
        host: &'host mut integration_seed::TestProviderV1,
    ) -> Result<
        InstallationConsumerSnapshotV1<
            'store,
            'host,
            persistence_seed::TestLeaseV1<'store>,
            integration_seed::TestLeaseV1<'host>,
            TestOwnerOperationV1<K>,
            K,
        >,
        InstallationConsumerSnapshotErrorV1,
    > {
        InstallationConsumerSnapshotV1::issue(
            persistence_seed::bind(store).unwrap(),
            integration_seed::bind(host).unwrap(),
            TestOwnerOperationV1 {
                operation_identity: [1; 32],
                _stage: PhantomData,
            },
        )
    }

    #[test]
    fn owner_issued_snapshot_joins_store_and_host_until_both_final_rechecks() {
        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let snapshot = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host).unwrap();
        let expected = snapshot.consumer_set_id;
        let receipt = snapshot.consume_finality().unwrap();
        assert_eq!(receipt.owner_operation, [1; 32]);
        assert_eq!(receipt.consumer_set_id, expected);
        assert_ne!(receipt.finality_commitment(), [0; 32]);
    }

    #[test]
    fn caller_gate_stage_and_post_issue_currentness_substitution_refuse() {
        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        assert!(matches!(
            issue::<ProtectedRetentionConsumerStageV1>(&mut store, &mut host),
            Err(InstallationConsumerSnapshotErrorV1::OwnerStageMismatch)
        ));
        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PhysicalPruningConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        assert!(issue::<PhysicalPruningConsumerStageV1>(&mut store, &mut host).is_ok());

        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let host_control = host.control();
        let snapshot = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host).unwrap();
        integration_seed::change_admission_epoch(&host_control, [88; 32]);
        assert!(matches!(
            snapshot.consume_finality(),
            Err(InstallationConsumerSnapshotErrorV1::Host(
                ConsumerClosureErrorV1::Changed
            ))
        ));

        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let store_control = store.control();
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let snapshot = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host).unwrap();
        let mut changed = store_control.borrow().clone().unwrap();
        if let ConsumerSnapshotCurrentFactsV1::ActiveStore {
            restore_incarnation,
            ..
        } = &mut changed
        {
            *restore_incarnation = [77; 32];
        }
        *store_control.borrow_mut() = Some(changed);
        assert!(matches!(
            snapshot.consume_finality(),
            Err(InstallationConsumerSnapshotErrorV1::Store(
                ConsumerSnapshotCurrentnessErrorV1::Changed
            ))
        ));
    }

    #[test]
    fn changed_host_or_store_currentness_refuses_before_owner_effect() {
        let effects = Rc::new(Cell::new(0));
        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let host_control = host.control();
        let snapshot = InstallationConsumerSnapshotV1::issue(
            persistence_seed::bind(&mut store).unwrap(),
            integration_seed::bind(&mut host).unwrap(),
            CountingOwnerOperationV1 {
                effects: effects.clone(),
            },
        )
        .unwrap();
        integration_seed::change_admission_epoch(&host_control, [88; 32]);
        assert!(snapshot.consume_finality().is_err());
        assert_eq!(effects.get(), 0);

        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let store_control = store.control();
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let snapshot = InstallationConsumerSnapshotV1::issue(
            persistence_seed::bind(&mut store).unwrap(),
            integration_seed::bind(&mut host).unwrap(),
            CountingOwnerOperationV1 {
                effects: effects.clone(),
            },
        )
        .unwrap();
        let mut changed = store_control.borrow().clone().unwrap();
        if let ConsumerSnapshotCurrentFactsV1::ActiveStore {
            activation_incarnation,
            ..
        } = &mut changed
        {
            *activation_incarnation = [77; 32];
        }
        *store_control.borrow_mut() = Some(changed);
        assert!(snapshot.consume_finality().is_err());
        assert_eq!(effects.get(), 0);
    }

    #[test]
    fn pre_store_is_pre_currentness_only_and_has_no_final_root_or_candidate_seal() {
        let mut store = persistence_seed::TestProviderV1::new(pre_store_facts());
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let snapshot = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host).unwrap();
        snapshot.consume_finality().unwrap();

        let source = include_str!("../persistence/consumer_snapshot.rs");
        assert!(!source.contains("final_candidate_root"));
        assert!(!source.contains("candidate_seal"));
    }
}
