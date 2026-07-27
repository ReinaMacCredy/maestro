use std::marker::PhantomData;
use std::rc::Rc;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::integration::consumer_closure::{
    ConsumerClosureErrorV1, ConsumerClosureLeasePortV1, HostConsumerAdmissionGuardV1,
};
use crate::domain::migration::runtime::{ConsumerCensusEntryV1, MigrationDigestV1};
use crate::domain::persistence::consumer_snapshot::{
    ConsumerSnapshotCurrentFactsV1, ConsumerSnapshotCurrentViewLeasePortV1,
    ConsumerSnapshotCurrentViewLeaseV1, ConsumerSnapshotCurrentnessErrorV1,
};

mod stage_sealed {
    pub trait Sealed {}
}

pub(in crate::domain) trait ConsumerClosureStageV1: stage_sealed::Sealed {
    const TAG: u8;
}

pub(in crate::domain) struct PreCurrentnessConsumerStageV1;
pub(in crate::domain) struct ProtectedRetentionConsumerStageV1;
pub(in crate::domain) struct PhysicalPruningConsumerStageV1;

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

pub(in crate::domain) mod owner_operation_sealed {
    pub trait Sealed {}
}

pub(in crate::domain) trait ConsumerClosureOwnerOperationPortV1<K, S, H>:
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

pub(in crate::domain) trait ConsumerClosureStageProofIssuerV1<K>:
    owner_operation_sealed::Sealed
where
    K: ConsumerClosureStageV1,
{
}

pub(in crate::domain) struct ConsumerClosureDurableLinearizationV1<K> {
    owner_seed: super::consumer_snapshot_stage11_seed::Stage11ConsumerClosureDurableEffectSeedV1<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<K: ConsumerClosureStageV1> ConsumerClosureDurableLinearizationV1<K> {
    pub(super) fn from_stage11_owner_seed(
        owner_seed: super::consumer_snapshot_stage11_seed::Stage11ConsumerClosureDurableEffectSeedV1<
            K,
        >,
    ) -> Self {
        Self {
            owner_seed,
            _not_send_or_sync: PhantomData,
        }
    }

    fn commit(
        self,
        request: ConsumerClosureDurableLinearizationRequestV1<K>,
    ) -> Result<ConsumerClosureDurableLinearizationReceiptV1<K>, InstallationConsumerSnapshotErrorV1>
    {
        let applied = self.owner_seed.commit(&request)?;
        ConsumerClosureDurableLinearizationReceiptV1::from_applied_effect(request, applied)
    }
}

pub(in crate::domain) struct InstallationConsumerClosureDurableRootV1 {
    backend:
        crate::foundation::core::installation_consumer_closure_durability::DurableReceiptBackendV1,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl InstallationConsumerClosureDurableRootV1 {
    fn from_persistence_owner(
        backend: crate::foundation::core::installation_consumer_closure_durability::DurableReceiptBackendV1,
    ) -> Self {
        Self {
            backend,
            _not_send_or_sync: PhantomData,
        }
    }

    pub(super) fn open_from_installation_owner(
        path: impl AsRef<std::path::Path>,
    ) -> Result<Self, InstallationConsumerSnapshotErrorV1> {
        let backend =
            crate::foundation::core::installation_consumer_closure_durability::DurableReceiptBackendV1::open_or_create(
                path,
            )
            .map_err(|_| InstallationConsumerSnapshotErrorV1::FinalityMismatch)?;
        Ok(Self {
            backend,
            _not_send_or_sync: PhantomData,
        })
    }

    fn into_backend(
        self,
    ) -> crate::foundation::core::installation_consumer_closure_durability::DurableReceiptBackendV1
    {
        self.backend
    }
}

pub(in crate::domain) fn acquire_stage11_durable_linearization_from_store<
    K: ConsumerClosureStageV1,
>(
    store: &crate::domain::persistence::StoreV1,
) -> Result<ConsumerClosureDurableLinearizationV1<K>, InstallationConsumerSnapshotErrorV1> {
    let root = InstallationConsumerClosureDurableRootV1::from_persistence_owner(
        store
            .admit_consumer_closure_durable_root_v1()
            .map_err(|_| InstallationConsumerSnapshotErrorV1::FinalityMismatch)?,
    );
    acquire_stage11_durable_linearization(root)
}

pub(in crate::domain) fn acquire_stage11_durable_linearization<K: ConsumerClosureStageV1>(
    durable_root: InstallationConsumerClosureDurableRootV1,
) -> Result<ConsumerClosureDurableLinearizationV1<K>, InstallationConsumerSnapshotErrorV1> {
    Ok(super::consumer_snapshot_stage11_seed::acquire(
        durable_root.into_backend(),
    ))
}

#[cfg(test)]
pub(in crate::domain) fn stage11_test_durable_root(
    path: impl AsRef<std::path::Path>,
) -> Result<InstallationConsumerClosureDurableRootV1, InstallationConsumerSnapshotErrorV1> {
    InstallationConsumerClosureDurableRootV1::open_from_installation_owner(path)
}

#[cfg(test)]
pub(in crate::domain) fn stage11_test_successful_durable_linearization<
    K: ConsumerClosureStageV1,
>(
    effects: Rc<std::cell::Cell<u64>>,
) -> ConsumerClosureDurableLinearizationV1<K> {
    super::consumer_snapshot_stage11_seed::test_seed::successful(effects)
}

#[cfg(test)]
pub(in crate::domain) fn stage11_test_no_effect_durable_linearization<K: ConsumerClosureStageV1>(
    effects: Rc<std::cell::Cell<u64>>,
) -> ConsumerClosureDurableLinearizationV1<K> {
    super::consumer_snapshot_stage11_seed::test_seed::no_effect(effects)
}

pub(in crate::domain) struct ConsumerClosureDurableLinearizationRequestV1<K> {
    expected_old_cas: [u8; 32],
    consumer_set_id: [u8; 32],
    _stage: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<K> ConsumerClosureDurableLinearizationRequestV1<K> {
    pub(in crate::domain::installation) const fn expected_old_cas(&self) -> [u8; 32] {
        self.expected_old_cas
    }

    pub(in crate::domain::installation) const fn consumer_set_id(&self) -> [u8; 32] {
        self.consumer_set_id
    }
}

pub(in crate::domain) struct ConsumerClosureDurableLinearizationReceiptV1<K> {
    expected_old_cas: [u8; 32],
    consumer_set_id: [u8; 32],
    durable_effect_commitment: [u8; 32],
    _stage: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<K> ConsumerClosureDurableLinearizationReceiptV1<K> {
    fn from_applied_effect(
        request: ConsumerClosureDurableLinearizationRequestV1<K>,
        applied: super::consumer_snapshot_stage11_seed::Stage11ConsumerClosureAppliedEffectV1<K>,
    ) -> Result<Self, InstallationConsumerSnapshotErrorV1> {
        let (expected_old_cas, consumer_set_id, durable_effect_commitment) =
            applied.into_commitments();
        if expected_old_cas != request.expected_old_cas
            || consumer_set_id != request.consumer_set_id
            || durable_effect_commitment == [0; 32]
        {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        Ok(Self {
            expected_old_cas,
            consumer_set_id,
            durable_effect_commitment,
            _stage: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

#[derive(Clone, Copy)]
pub(in crate::domain) struct ConsumerClosureFinalityFactsV1 {
    owner_operation: [u8; 32],
    stage_proof_commitment: [u8; 32],
    expected_old_cas: [u8; 32],
    consumer_set_id: [u8; 32],
    association_gate_result: [u8; 32],
}

pub(in crate::domain) struct PreCurrentnessClosureProofV1 {
    active_consumer_zero: [u8; 32],
    canonical_consumer_rows: [u8; 32],
    association_write_set: [u8; 32],
}

pub(in crate::domain) struct ProtectedRetentionClosureProofV1 {
    admitted_typed_reader_set: [u8; 32],
    sealed_reader_closure: [u8; 32],
    retention_authority: [u8; 32],
}

pub(in crate::domain) struct PhysicalPruningClosureProofV1 {
    reader_zero: [u8; 32],
    hold_zero: [u8; 32],
    custody: [u8; 32],
    authority: [u8; 32],
    rollback_safety: [u8; 32],
    erasure_safety: [u8; 32],
    legacy_removal_authorization: [u8; 32],
}

impl PreCurrentnessClosureProofV1 {
    pub(in crate::domain) fn from_owner_proof<I>(
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
    pub(in crate::domain) fn from_owner_proof<I>(
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
    pub(in crate::domain) fn from_owner_proof<I>(
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

pub(in crate::domain) struct ConsumerClosureFinalityGuardV1<'view, 'connection, S, H, K> {
    owner_operation: [u8; 32],
    consumer_set_id: [u8; 32],
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
    fn expected_old_cas(&self) -> [u8; 32] {
        self.store.initial().expected_old_cas()
    }

    fn linearize(
        mut self,
        stage_proof_commitment: [u8; 32],
        expected_old_cas: [u8; 32],
        durable_effect: ConsumerClosureDurableLinearizationV1<K>,
    ) -> Result<[u8; 32], InstallationConsumerSnapshotErrorV1> {
        if stage_proof_commitment == [0; 32] || expected_old_cas == [0; 32] {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        self.host.recheck_current()?;
        let current = self.store.recheck_current()?;
        if current.owner_stage_tag() != K::TAG
            || current.owner_operation() != self.owner_operation
            || current.expected_old_cas() != expected_old_cas
        {
            return Err(InstallationConsumerSnapshotErrorV1::OwnerStageMismatch);
        }
        let durable_receipt =
            durable_effect.commit(ConsumerClosureDurableLinearizationRequestV1 {
                expected_old_cas,
                consumer_set_id: self.consumer_set_id,
                _stage: PhantomData,
                _not_send_or_sync: PhantomData,
            })?;
        self.host.recheck_current()?;
        let after = self.store.recheck_current()?;
        if after.expected_old_cas() != expected_old_cas
            || durable_receipt.expected_old_cas != expected_old_cas
            || durable_receipt.consumer_set_id != self.consumer_set_id
            || durable_receipt.durable_effect_commitment == [0; 32]
        {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        Ok(durable_receipt.consumer_set_id)
    }

    fn finality(
        owner_operation: [u8; 32],
        consumer_set_id: [u8; 32],
        stage_proof_commitment: [u8; 32],
        expected_old_cas: [u8; 32],
        association_gate_result: [u8; 32],
    ) -> ConsumerClosureFinalityFactsV1 {
        ConsumerClosureFinalityFactsV1 {
            owner_operation,
            stage_proof_commitment,
            expected_old_cas,
            consumer_set_id,
            association_gate_result,
        }
    }

    pub(in crate::domain) fn linearize_pre_currentness_association(
        self,
        proof: PreCurrentnessClosureProofV1,
        expected_old_cas: [u8; 32],
        durable_effect: ConsumerClosureDurableLinearizationV1<K>,
    ) -> Result<ConsumerClosureFinalityFactsV1, InstallationConsumerSnapshotErrorV1> {
        if K::TAG != PreCurrentnessConsumerStageV1::TAG {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        let commitment = proof.commitment();
        let owner_operation = self.owner_operation;
        let consumer_set_id = self.consumer_set_id;
        let association_gate_result =
            self.linearize(commitment, expected_old_cas, durable_effect)?;
        let finality = Self::finality(
            owner_operation,
            consumer_set_id,
            commitment,
            expected_old_cas,
            association_gate_result,
        );
        Ok(finality)
    }

    pub(in crate::domain) fn linearize_protected_retention(
        self,
        proof: ProtectedRetentionClosureProofV1,
        expected_old_cas: [u8; 32],
        durable_effect: ConsumerClosureDurableLinearizationV1<K>,
    ) -> Result<ConsumerClosureFinalityFactsV1, InstallationConsumerSnapshotErrorV1> {
        if K::TAG != ProtectedRetentionConsumerStageV1::TAG {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        let commitment = proof.commitment();
        let owner_operation = self.owner_operation;
        let consumer_set_id = self.consumer_set_id;
        let association_gate_result =
            self.linearize(commitment, expected_old_cas, durable_effect)?;
        let finality = Self::finality(
            owner_operation,
            consumer_set_id,
            commitment,
            expected_old_cas,
            association_gate_result,
        );
        Ok(finality)
    }

    pub(in crate::domain) fn linearize_physical_pruning(
        self,
        proof: PhysicalPruningClosureProofV1,
        expected_old_cas: [u8; 32],
        durable_effect: ConsumerClosureDurableLinearizationV1<K>,
    ) -> Result<ConsumerClosureFinalityFactsV1, InstallationConsumerSnapshotErrorV1> {
        if K::TAG != PhysicalPruningConsumerStageV1::TAG {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        let commitment = proof.commitment();
        let owner_operation = self.owner_operation;
        let consumer_set_id = self.consumer_set_id;
        let association_gate_result =
            self.linearize(commitment, expected_old_cas, durable_effect)?;
        let finality = Self::finality(
            owner_operation,
            consumer_set_id,
            commitment,
            expected_old_cas,
            association_gate_result,
        );
        Ok(finality)
    }
}

pub(in crate::domain) struct InstallationConsumerSnapshotV1<'view, 'connection, S, H, O, K> {
    store: ConsumerSnapshotCurrentViewLeaseV1<'view, S>,
    host: HostConsumerAdmissionGuardV1<'connection, H>,
    owner_operation: O,
    consumer_set_id: [u8; 32],
    census_rows: Vec<[u8; 32]>,
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
    pub(in crate::domain::installation) fn issue(
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
        let census_rows = current.census_rows().to_vec();
        Ok(Self {
            store,
            host,
            owner_operation,
            consumer_set_id,
            census_rows,
            _stage: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }

    pub(in crate::domain::installation) fn consume_finality(
        self,
    ) -> Result<ConsumerClosureReceiptV1<'view, 'connection, K>, InstallationConsumerSnapshotErrorV1>
    {
        let initial_owner_operation = self.store.initial().owner_operation();
        let finality = self
            .owner_operation
            .linearize(ConsumerClosureFinalityGuardV1 {
                owner_operation: initial_owner_operation,
                consumer_set_id: self.consumer_set_id,
                store: self.store,
                host: self.host,
                _stage: PhantomData,
                _not_send_or_sync: PhantomData,
            })?;
        Ok(ConsumerClosureReceiptV1 {
            owner_operation: initial_owner_operation,
            consumer_set_id: self.consumer_set_id,
            finality_commitment: finality_commitment::<K>(&finality),
            census_rows: self.census_rows,
            _view: PhantomData,
            _connection: PhantomData,
            _stage: PhantomData,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(in crate::domain) struct ConsumerClosureReceiptV1<'view, 'connection, K> {
    owner_operation: [u8; 32],
    consumer_set_id: [u8; 32],
    finality_commitment: [u8; 32],
    census_rows: Vec<[u8; 32]>,
    _view: PhantomData<&'view mut ()>,
    _connection: PhantomData<&'connection mut ()>,
    _stage: PhantomData<K>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

pub(crate) struct AgentResourceReleaseConsumerSealV1 {
    closure: [u8; 32],
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl ConsumerClosureReceiptV1<'_, '_, PreCurrentnessConsumerStageV1> {
    pub(crate) fn into_agent_resource_release_seal(
        self,
    ) -> Result<AgentResourceReleaseConsumerSealV1, InstallationConsumerSnapshotErrorV1> {
        if self.census_rows.is_empty()
            || self.finality_commitment == [0; 32]
            || self.consumer_set_id == [0; 32]
        {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"maestro.vnext.agent-resource-release-consumer-seal.v1\0");
        hasher.update(self.owner_operation);
        hasher.update(self.consumer_set_id);
        hasher.update(self.finality_commitment);
        for row in self.census_rows {
            hasher.update(row);
        }
        Ok(AgentResourceReleaseConsumerSealV1 {
            closure: hasher.finalize().into(),
            _not_send_or_sync: PhantomData,
        })
    }
}

impl AgentResourceReleaseConsumerSealV1 {
    pub(crate) const fn closure(&self) -> [u8; 32] {
        self.closure
    }

    #[cfg(test)]
    pub(crate) fn test_seal(closure: [u8; 32]) -> Self {
        assert_ne!(closure, [0; 32]);
        Self {
            closure,
            _not_send_or_sync: PhantomData,
        }
    }
}

impl<K> ConsumerClosureReceiptV1<'_, '_, K> {
    pub(in crate::domain) const fn finality_commitment(&self) -> [u8; 32] {
        self.finality_commitment
    }

    pub(in crate::domain) fn bind_migration_census(
        self,
        source_manifest_id: MigrationDigestV1,
        closure_attestation_id: MigrationDigestV1,
        entries: Vec<ConsumerCensusEntryV1>,
    ) -> Result<InstallationMigrationConsumerSnapshotV1, InstallationConsumerSnapshotErrorV1> {
        let expected_rows = self
            .census_rows
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let observed_rows = entries
            .iter()
            .map(|entry| *entry.source_row_id().as_bytes())
            .collect::<std::collections::BTreeSet<_>>();
        if expected_rows.is_empty()
            || expected_rows.len() != entries.len()
            || expected_rows != observed_rows
            || source_manifest_id.as_bytes() == &[0; 32]
            || closure_attestation_id.as_bytes() == &[0; 32]
        {
            return Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch);
        }
        let owner_snapshot_id = MigrationDigestV1::from_digest(self.finality_commitment)
            .map_err(|_| InstallationConsumerSnapshotErrorV1::FinalityMismatch)?;
        Ok(InstallationMigrationConsumerSnapshotV1 {
            expected_member_count: entries.len(),
            source_manifest_id,
            owner_snapshot_id,
            closure_attestation_id,
            entries,
            _not_send_or_sync: PhantomData,
        })
    }
}

pub(in crate::domain) struct InstallationMigrationConsumerSnapshotV1 {
    expected_member_count: usize,
    source_manifest_id: MigrationDigestV1,
    owner_snapshot_id: MigrationDigestV1,
    closure_attestation_id: MigrationDigestV1,
    entries: Vec<ConsumerCensusEntryV1>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl InstallationMigrationConsumerSnapshotV1 {
    pub(in crate::domain) fn into_parts(
        self,
    ) -> (
        usize,
        MigrationDigestV1,
        MigrationDigestV1,
        MigrationDigestV1,
        Vec<ConsumerCensusEntryV1>,
    ) {
        (
            self.expected_member_count,
            self.source_manifest_id,
            self.owner_snapshot_id,
            self.closure_attestation_id,
            self.entries,
        )
    }
}

#[derive(Debug, Error)]
pub(in crate::domain) enum InstallationConsumerSnapshotErrorV1 {
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
    hasher.update(finality.consumer_set_id);
    hasher.update(finality.association_gate_result);
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

    use crate::domain::integration::consumer_closure::test_seed as integration_seed;
    use crate::domain::migration::runtime::{
        ConsumerAccessV1, ConsumerCensusEntryV1, ConsumerGenerationV1, ConsumerRecordV1,
        ConsumerSubjectV1, MigrationDigestV1, NormalizedLocatorV1,
    };
    use crate::domain::persistence::consumer_snapshot::{
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
            let expected_old_cas = guard.expected_old_cas();
            match K::TAG {
                1 => guard.linearize_pre_currentness_association(
                    PreCurrentnessClosureProofV1::from_owner_proof(
                        &self, [81; 32], [82; 32], [83; 32],
                    )?,
                    expected_old_cas,
                    stage11_test_successful_durable_linearization(Rc::new(Cell::new(0))),
                ),
                2 => guard.linearize_protected_retention(
                    ProtectedRetentionClosureProofV1::from_owner_proof(
                        &self, [84; 32], [85; 32], [86; 32],
                    )?,
                    expected_old_cas,
                    stage11_test_successful_durable_linearization(Rc::new(Cell::new(0))),
                ),
                3 => guard.linearize_physical_pruning(
                    PhysicalPruningClosureProofV1::from_owner_proof(
                        &self, [87; 32], [88; 32], [89; 32], [91; 32], [92; 32], [93; 32], [94; 32],
                    )?,
                    expected_old_cas,
                    stage11_test_successful_durable_linearization(Rc::new(Cell::new(0))),
                ),
                _ => Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch),
            }
        }
    }

    struct CountingOwnerOperationV1 {
        effects: Rc<Cell<u64>>,
    }

    struct NoOpOwnerOperationV1 {
        effects: Rc<Cell<u64>>,
    }

    impl owner_operation_sealed::Sealed for NoOpOwnerOperationV1 {}
    impl ConsumerClosureStageProofIssuerV1<PreCurrentnessConsumerStageV1> for NoOpOwnerOperationV1 {}

    impl<S, H> ConsumerClosureOwnerOperationPortV1<PreCurrentnessConsumerStageV1, S, H>
        for NoOpOwnerOperationV1
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
            let expected_old_cas = guard.expected_old_cas();
            guard.linearize_pre_currentness_association(
                PreCurrentnessClosureProofV1::from_owner_proof(
                    &self, [81; 32], [82; 32], [83; 32],
                )?,
                expected_old_cas,
                stage11_test_no_effect_durable_linearization(self.effects),
            )
        }
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
            let expected_old_cas = guard.expected_old_cas();
            guard.linearize_pre_currentness_association(
                PreCurrentnessClosureProofV1::from_owner_proof(
                    &self, [81; 32], [82; 32], [83; 32],
                )?,
                expected_old_cas,
                stage11_test_successful_durable_linearization(self.effects),
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
    fn pre_currentness_receipt_is_the_only_agent_resource_release_consumer_seal() {
        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let seal = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host)
            .unwrap()
            .consume_finality()
            .unwrap()
            .into_agent_resource_release_seal()
            .unwrap();
        assert_ne!(seal.closure(), [0; 32]);
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
    fn no_op_owner_callback_cannot_mint_consumer_finality() {
        let effects = Rc::new(Cell::new(0));
        let mut store = persistence_seed::TestProviderV1::new(active_store_facts(
            PreCurrentnessConsumerStageV1::TAG,
        ));
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let snapshot = InstallationConsumerSnapshotV1::issue(
            persistence_seed::bind(&mut store).unwrap(),
            integration_seed::bind(&mut host).unwrap(),
            NoOpOwnerOperationV1 {
                effects: effects.clone(),
            },
        )
        .unwrap();
        assert!(matches!(
            snapshot.consume_finality(),
            Err(InstallationConsumerSnapshotErrorV1::FinalityMismatch)
        ));
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

    #[test]
    fn final_consumer_snapshot_binds_active_reader_and_hold_rows_for_migration() {
        let mut facts = active_store_facts(PreCurrentnessConsumerStageV1::TAG);
        if let ConsumerSnapshotCurrentFactsV1::ActiveStore { census_rows, .. } = &mut facts {
            *census_rows = vec![[16; 32], [17; 32], [18; 32]];
        }
        let mut store = persistence_seed::TestProviderV1::new(facts);
        let mut host = integration_seed::TestProviderV1::new(integration_seed::standard_facts());
        let receipt = issue::<PreCurrentnessConsumerStageV1>(&mut store, &mut host)
            .expect("snapshot")
            .consume_finality()
            .expect("finality");
        let entries = vec![
            ConsumerCensusEntryV1::observed(
                MigrationDigestV1::from_digest([16; 32]).expect("source"),
                ConsumerRecordV1::new(
                    NormalizedLocatorV1::new(b"/active".to_vec()).expect("locator"),
                    ConsumerSubjectV1::CurrentTarget,
                    ConsumerGenerationV1::CurrentVNext,
                    ConsumerAccessV1::ActiveRuntime,
                    false,
                    false,
                    None,
                )
                .expect("active consumer"),
            ),
            ConsumerCensusEntryV1::observed(
                MigrationDigestV1::from_digest([17; 32]).expect("source"),
                ConsumerRecordV1::new(
                    NormalizedLocatorV1::new(b"/reader".to_vec()).expect("locator"),
                    ConsumerSubjectV1::LegacySource,
                    ConsumerGenerationV1::LegacyV1,
                    ConsumerAccessV1::SealedAuditReader,
                    false,
                    false,
                    None,
                )
                .expect("sealed reader"),
            ),
            ConsumerCensusEntryV1::observed(
                MigrationDigestV1::from_digest([18; 32]).expect("source"),
                ConsumerRecordV1::new(
                    NormalizedLocatorV1::new(b"/hold".to_vec()).expect("locator"),
                    ConsumerSubjectV1::LegacySource,
                    ConsumerGenerationV1::LegacyV1,
                    ConsumerAccessV1::ProtectedRetentionHold,
                    false,
                    false,
                    None,
                )
                .expect("retention hold"),
            ),
        ];
        let migration = receipt
            .bind_migration_census(
                MigrationDigestV1::from_digest([31; 32]).expect("source manifest"),
                MigrationDigestV1::from_digest([32; 32]).expect("closure"),
                entries,
            )
            .expect("authoritative migration snapshot");
        let (count, _, _, _, rows) = migration.into_parts();
        assert_eq!(count, 3);
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn production_consumer_closure_backend_persists_and_rereads_exact_receipt() {
        let root = std::fs::canonicalize(std::env::temp_dir())
            .expect("canonical temp root")
            .join(format!(
                "maestro-consumer-closure-durable-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos()
            ));
        let durable = acquire_stage11_durable_linearization::<PreCurrentnessConsumerStageV1>(
            stage11_test_durable_root(&root).expect("durable root"),
        )
        .expect("durable backend");
        let receipt = durable
            .commit(ConsumerClosureDurableLinearizationRequestV1 {
                expected_old_cas: [41; 32],
                consumer_set_id: [42; 32],
                _stage: PhantomData,
                _not_send_or_sync: PhantomData,
            })
            .expect("durable receipt");
        assert_eq!(receipt.expected_old_cas, [41; 32]);
        assert_eq!(receipt.consumer_set_id, [42; 32]);
        assert_ne!(receipt.durable_effect_commitment, [0; 32]);
        let receipt_directory = root.join("consumer-closure");
        assert_eq!(
            std::fs::read_dir(&receipt_directory)
                .expect("receipt directory")
                .count(),
            1
        );
        let replay = acquire_stage11_durable_linearization::<PreCurrentnessConsumerStageV1>(
            stage11_test_durable_root(&root).expect("durable root"),
        )
        .expect("replay backend")
        .commit(ConsumerClosureDurableLinearizationRequestV1 {
            expected_old_cas: [41; 32],
            consumer_set_id: [42; 32],
            _stage: PhantomData,
            _not_send_or_sync: PhantomData,
        })
        .expect("exact replay");
        assert_eq!(
            replay.durable_effect_commitment,
            receipt.durable_effect_commitment
        );
        assert_eq!(
            std::fs::read_dir(&receipt_directory)
                .expect("receipt directory")
                .count(),
            1
        );
        std::fs::remove_dir_all(root).expect("remove durable test root");
    }
}
