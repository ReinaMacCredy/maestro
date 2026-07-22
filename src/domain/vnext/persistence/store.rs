use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::vnext::identity::{
    IdentityKindV1, ManifestIdentityV1, RestoreCandidateIdV1, RetentionPinIdV1, SchemaIdV1,
    SealedExportIdV1, StoreGenerationIdV1, StoreHeadIdV1, StoreObjectIdV1,
};
use crate::foundation::core::deterministic_cbor::{self, CborValue};
use crate::foundation::core::secure_fs::{
    CreateIfAbsent, SecureDirectoryEntryKind, SecureFsError, SecureRoot,
};

use super::snapshot_blocks::{MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2, StoreSnapshotBlockClosureV2};
use super::snapshot_rows::MAX_REFERENCED_PRIOR_ROOTS_V1;
use super::{
    AtomicGenerationPublicationV1, BackupReceiptV1, CollectionPlanV1, ExportError, GenerationError,
    LogicalTombstoneV1, ReachabilitySnapshotV1, RestoreCandidateV1, RetentionError, RetentionPinV1,
    RetentionRootKindV1, RetentionRootV1, SEALED_BACKUP_FORMAT_V1, SEALED_EXPORT_FORMAT_V2,
    STORE_OBJECT_STORAGE_CODEC_V1, SealedBackupV1, SealedExportEntryV1, SealedExportLineageV1,
    SealedExportV1, SnapshotError, StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1,
    StoreHeadV1, StoreIdempotencyProbeV1, StoreObjectError, StoreObjectV1,
    StorePublicationOutcomeV1, StoreRoleV1, StoreSnapshotRootV1, TombstonedObjectV1,
    metadata::{
        ConditionalTransactionError, MetadataError, MetadataStore, PublicationMutation,
        revoke_sealed_exports_for_security_erasure,
    },
};
#[cfg(test)]
use super::{
    ProtectedDiagnosticCurrentViewAnchorV1, ProtectedDiagnosticCurrentViewProviderV1,
    protected_diagnostic::ProtectedDiagnosticObservedCurrentViewV1,
};

const METADATA_FILE: &str = "store.sqlite3";
const OBJECTS_DIRECTORY: &str = "objects";
const EXPORTS_DIRECTORY: &str = "exports";
const SNAPSHOT_CLOSURES_DIRECTORY: &str = "exports/snapshot-closures";
const RECOVERY_DIRECTORY: &str = "recovery";
const CONTROLLED_COPY_ERASURE_BARRIERS_DIRECTORY: &str = "recovery/security-erasure-barriers";
const CACHE_DIRECTORY: &str = "cache";
const MAX_OBJECT_CLOSURE_ENTRIES: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreStateV1 {
    Inactive,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedCollectionAbsenceV1 {
    tombstone_id: crate::domain::vnext::identity::LogicalTombstoneIdV1,
    collection_plan_id: crate::domain::vnext::identity::CollectionPlanIdV1,
    destroyed_object_hash: [u8; 32],
    receipt_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ControlledCopyErasureObligationV1 {
    export_id: SealedExportIdV1,
    export_byte_length: u64,
    export_bytes_digest: [u8; 32],
    snapshot_root_id: [u8; 32],
    snapshot_closure: Option<(u64, [u8; 32])>,
    public_carrier_present: bool,
    pending_carrier_present: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ControlledCopyErasurePlanV1 {
    object_id: StoreObjectIdV1,
    barrier_identity: [u8; 32],
    basis_publication_clock: u64,
    basis_retention_revision: u64,
    obligations: Vec<ControlledCopyErasureObligationV1>,
    plan_id: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedControlledCopyAbsenceV1 {
    plan_id: [u8; 32],
    receipt_hash: [u8; 32],
}

impl ControlledCopyErasureObligationV1 {
    fn canonical_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::Bytes(self.export_id.as_bytes().to_vec()),
            CborValue::Unsigned(self.export_byte_length),
            CborValue::Bytes(self.export_bytes_digest.to_vec()),
            CborValue::Bytes(self.snapshot_root_id.to_vec()),
            CborValue::optional(self.snapshot_closure.map(|(length, digest)| {
                CborValue::Array(vec![
                    CborValue::Unsigned(length),
                    CborValue::Bytes(digest.to_vec()),
                ])
            })),
            CborValue::Unsigned(u64::from(self.public_carrier_present)),
            CborValue::Unsigned(u64::from(self.pending_carrier_present)),
        ])
    }

    fn from_canonical_value(value: &CborValue) -> Result<Self, StoreError> {
        let CborValue::Array(fields) = value else {
            return Err(StoreError::InvalidControlledCopyErasurePlan);
        };
        let [
            CborValue::Bytes(export_id),
            CborValue::Unsigned(export_byte_length),
            CborValue::Bytes(export_bytes_digest),
            CborValue::Bytes(snapshot_root_id),
            snapshot_closure,
            CborValue::Unsigned(public_carrier_present),
            CborValue::Unsigned(pending_carrier_present),
        ] = fields.as_slice()
        else {
            return Err(StoreError::InvalidControlledCopyErasurePlan);
        };
        let snapshot_closure = match snapshot_closure {
            CborValue::Array(values) if values.as_slice() == [CborValue::Unsigned(0)] => None,
            CborValue::Array(values)
                if values.len() == 2 && values[0] == CborValue::Unsigned(1) =>
            {
                let CborValue::Array(parts) = &values[1] else {
                    return Err(StoreError::InvalidControlledCopyErasurePlan);
                };
                let [CborValue::Unsigned(length), CborValue::Bytes(digest)] = parts.as_slice()
                else {
                    return Err(StoreError::InvalidControlledCopyErasurePlan);
                };
                Some((*length, exact_digest(digest.clone())?))
            }
            _ => return Err(StoreError::InvalidControlledCopyErasurePlan),
        };
        if *export_byte_length == 0
            || !matches!(public_carrier_present, 0 | 1)
            || !matches!(pending_carrier_present, 0 | 1)
            || snapshot_closure.is_some_and(|(length, _)| length == 0)
        {
            return Err(StoreError::InvalidControlledCopyErasurePlan);
        }
        Ok(Self {
            export_id: SealedExportIdV1::from_digest(exact_digest(export_id.clone())?),
            export_byte_length: *export_byte_length,
            export_bytes_digest: exact_digest(export_bytes_digest.clone())?,
            snapshot_root_id: exact_digest(snapshot_root_id.clone())?,
            snapshot_closure,
            public_carrier_present: *public_carrier_present == 1,
            pending_carrier_present: *pending_carrier_present == 1,
        })
    }
}

impl ControlledCopyErasurePlanV1 {
    fn new(
        object_id: StoreObjectIdV1,
        barrier_identity: [u8; 32],
        basis_publication_clock: u64,
        basis_retention_revision: u64,
        obligations: Vec<ControlledCopyErasureObligationV1>,
    ) -> Result<Self, StoreError> {
        if barrier_identity == [0; 32]
            || basis_retention_revision == 0
            || obligations
                .windows(2)
                .any(|pair| pair[0].export_id >= pair[1].export_id)
        {
            return Err(StoreError::InvalidControlledCopyErasurePlan);
        }
        let mut plan = Self {
            object_id,
            barrier_identity,
            basis_publication_clock,
            basis_retention_revision,
            obligations,
            plan_id: [0; 32],
        };
        plan.plan_id = sha256(&deterministic_cbor::encode(&plan.identity_value())?);
        if plan.plan_id == [0; 32] {
            return Err(StoreError::InvalidControlledCopyErasurePlan);
        }
        Ok(plan)
    }

    pub(crate) fn from_canonical_value(value: &CborValue) -> Result<Self, StoreError> {
        let CborValue::Array(fields) = value else {
            return Err(StoreError::InvalidControlledCopyErasurePlan);
        };
        let [
            CborValue::Bytes(object_id),
            CborValue::Bytes(barrier_identity),
            CborValue::Unsigned(basis_publication_clock),
            CborValue::Unsigned(basis_retention_revision),
            CborValue::Array(obligations),
            CborValue::Bytes(plan_id),
        ] = fields.as_slice()
        else {
            return Err(StoreError::InvalidControlledCopyErasurePlan);
        };
        let plan = Self::new(
            StoreObjectIdV1::from_digest(exact_digest(object_id.clone())?),
            exact_digest(barrier_identity.clone())?,
            *basis_publication_clock,
            *basis_retention_revision,
            obligations
                .iter()
                .map(ControlledCopyErasureObligationV1::from_canonical_value)
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        if plan.plan_id != exact_digest(plan_id.clone())? || plan.canonical_value() != *value {
            return Err(StoreError::InvalidControlledCopyErasurePlan);
        }
        Ok(plan)
    }

    fn identity_value(&self) -> CborValue {
        CborValue::Array(vec![
            CborValue::text("maestro.vnext.store.controlled-copy-erasure-plan.v1")
                .expect("invariant: static CBOR domain text"),
            CborValue::Bytes(self.object_id.as_bytes().to_vec()),
            CborValue::Bytes(self.barrier_identity.to_vec()),
            CborValue::Unsigned(self.basis_publication_clock),
            CborValue::Unsigned(self.basis_retention_revision),
            CborValue::Array(
                self.obligations
                    .iter()
                    .map(ControlledCopyErasureObligationV1::canonical_value)
                    .collect(),
            ),
        ])
    }

    pub(crate) fn canonical_value(&self) -> CborValue {
        let CborValue::Array(mut fields) = self.identity_value() else {
            unreachable!("invariant: controlled-copy plan identity is an array")
        };
        fields.remove(0);
        fields.push(CborValue::Bytes(self.plan_id.to_vec()));
        CborValue::Array(fields)
    }

    pub(crate) const fn object_id(&self) -> StoreObjectIdV1 {
        self.object_id
    }

    pub(crate) const fn barrier_identity(&self) -> [u8; 32] {
        self.barrier_identity
    }

    pub(crate) const fn plan_id(&self) -> [u8; 32] {
        self.plan_id
    }

    pub(crate) fn obligations(&self) -> &[ControlledCopyErasureObligationV1] {
        &self.obligations
    }

    #[cfg(test)]
    pub(crate) fn test_only(object_id: StoreObjectIdV1) -> Self {
        Self::new(object_id, [0x5a; 32], 1, 1, Vec::new()).expect("test-only controlled-copy plan")
    }
}

impl VerifiedControlledCopyAbsenceV1 {
    pub(crate) const fn plan_id(self) -> [u8; 32] {
        self.plan_id
    }

    pub(crate) const fn receipt_hash(self) -> [u8; 32] {
        self.receipt_hash
    }
}

impl VerifiedCollectionAbsenceV1 {
    pub(crate) const fn tombstone_id(self) -> crate::domain::vnext::identity::LogicalTombstoneIdV1 {
        self.tombstone_id
    }

    pub(crate) const fn collection_plan_id(
        self,
    ) -> crate::domain::vnext::identity::CollectionPlanIdV1 {
        self.collection_plan_id
    }

    pub(crate) const fn destroyed_object_hash(self) -> [u8; 32] {
        self.destroyed_object_hash
    }

    pub(crate) const fn receipt_hash(self) -> [u8; 32] {
        self.receipt_hash
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryActivationIntentV1 {
    expected_head_id: StoreHeadIdV1,
    expected_state_revision: u64,
    authority_locator_digest: [u8; 32],
}

impl RepositoryActivationIntentV1 {
    pub fn expected_head_id(&self) -> StoreHeadIdV1 {
        self.expected_head_id
    }

    pub fn expected_state_revision(&self) -> u64 {
        self.expected_state_revision
    }

    pub fn authority_locator_digest(&self) -> &[u8; 32] {
        &self.authority_locator_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallationActivationIntentV1 {
    expected_head_id: StoreHeadIdV1,
    expected_state_revision: u64,
    authority_locator_digest: [u8; 32],
}

impl InstallationActivationIntentV1 {
    pub fn expected_head_id(&self) -> StoreHeadIdV1 {
        self.expected_head_id
    }

    pub fn expected_state_revision(&self) -> u64 {
        self.expected_state_revision
    }

    pub fn authority_locator_digest(&self) -> &[u8; 32] {
        &self.authority_locator_digest
    }
}

pub struct StoreV1 {
    root: SecureRoot,
    metadata: MetadataStore,
    domain: StoreDomainV1,
}

pub(crate) struct StorePublicationViewV1<'a> {
    root: &'a SecureRoot,
    connection: &'a Connection,
    domain: &'a StoreDomainV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoreGenerationIdempotencyV1 {
    namespace: String,
    key_digest: [u8; 32],
    meaning_digest: [u8; 32],
    result_object_id: StoreObjectIdV1,
    head_id: StoreHeadIdV1,
}

impl StoreGenerationIdempotencyV1 {
    pub(crate) fn namespace(&self) -> &str {
        &self.namespace
    }

    pub(crate) const fn key_digest(&self) -> &[u8; 32] {
        &self.key_digest
    }

    pub(crate) const fn meaning_digest(&self) -> &[u8; 32] {
        &self.meaning_digest
    }

    pub(crate) const fn result_object_id(&self) -> StoreObjectIdV1 {
        self.result_object_id
    }

    pub(crate) const fn head_id(&self) -> StoreHeadIdV1 {
        self.head_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StorePublicationAllocationV1 {
    token_commitment: [u8; 32],
    allocation_commitment: [u8; 32],
    expected_predecessor: Option<[u8; 32]>,
    store_generation: u64,
    publication_clock: u64,
}

impl StorePublicationAllocationV1 {
    pub(crate) const fn token_commitment(&self) -> [u8; 32] {
        self.token_commitment
    }

    pub(crate) const fn allocation_commitment(&self) -> [u8; 32] {
        self.allocation_commitment
    }

    pub(crate) const fn expected_predecessor(&self) -> Option<[u8; 32]> {
        self.expected_predecessor
    }

    pub(crate) const fn store_generation(&self) -> u64 {
        self.store_generation
    }

    pub(crate) const fn publication_clock(&self) -> u64 {
        self.publication_clock
    }
}

impl StorePublicationViewV1<'_> {
    pub(crate) fn domain(&self) -> &StoreDomainV1 {
        self.domain
    }

    pub(crate) fn role(&self) -> StoreRoleV1 {
        self.domain.role()
    }

    pub(crate) fn active_head(&self) -> Result<Option<StoreHeadV1>, StoreError> {
        let Some((head_id, _)) = active_head_row(self.connection)? else {
            return Ok(None);
        };
        load_head(self.connection, head_id, self.domain).map(Some)
    }

    pub(crate) fn active_generation(&self) -> Result<Option<StoreGenerationV1>, StoreError> {
        let Some(head) = self.active_head()? else {
            return Ok(None);
        };
        load_generation(self.connection, head.generation_id(), self.domain).map(Some)
    }

    pub(crate) fn generation(
        &self,
        generation_id: StoreGenerationIdV1,
    ) -> Result<StoreGenerationV1, StoreError> {
        load_generation(self.connection, generation_id, self.domain)
    }

    pub(crate) fn generation_idempotency(
        &self,
        generation_id: StoreGenerationIdV1,
    ) -> Result<Vec<StoreGenerationIdempotencyV1>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT namespace, key_digest, meaning_digest, result_object_id, head_id
             FROM store_idempotency WHERE generation_id = ?1
             ORDER BY namespace, key_digest",
        )?;
        statement
            .query_map(params![generation_id.as_bytes()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                ))
            })?
            .map(|row| {
                let (namespace, key_digest, meaning_digest, result_object_id, head_id) = row?;
                Ok(StoreGenerationIdempotencyV1 {
                    namespace,
                    key_digest: exact_digest(key_digest)?,
                    meaning_digest: exact_digest(meaning_digest)?,
                    result_object_id: identity(result_object_id)?,
                    head_id: identity(head_id)?,
                })
            })
            .collect()
    }

    pub(crate) fn active_generation_objects(&self) -> Result<Vec<StoreObjectV1>, StoreError> {
        let generation = self
            .active_generation()?
            .ok_or(StoreError::MissingActiveHead)?;
        generation_objects(self.root, self.connection, &generation)
    }

    #[cfg(test)]
    pub(crate) fn protected_diagnostic_current_view_anchor<'view>(
        &'view self,
        provider: &'view mut dyn ProtectedDiagnosticCurrentViewProviderV1,
    ) -> Result<ProtectedDiagnosticCurrentViewAnchorV1<'view>, StoreError> {
        let (store_state, state_revision) = state(self.connection)?;
        if store_state != StoreStateV1::Active {
            return Err(StoreError::ProtectedDiagnosticCurrentnessRefused);
        }
        let (head_id, head_revision) = active_head_row(self.connection)?
            .ok_or(StoreError::ProtectedDiagnosticCurrentnessRefused)?;
        let head = load_head(self.connection, head_id, self.domain)?;
        let generation = load_generation(self.connection, head.generation_id(), self.domain)?;
        if head.revision() != head_revision
            || head.generation_id() != generation.id()
            || generation.domain() != self.domain
        {
            return Err(StoreError::ProtectedDiagnosticCurrentnessRefused);
        }
        let observed = ProtectedDiagnosticObservedCurrentViewV1 {
            root: self.root.path(),
            state_revision,
            domain: self.domain,
            role: self.role(),
            head: &head,
            head_revision,
            generation: &generation,
            publication_clock: publication_clock(self.connection)?,
        };
        ProtectedDiagnosticCurrentViewAnchorV1::bind(provider, &observed, self.root.path())
            .ok_or(StoreError::ProtectedDiagnosticCurrentnessRefused)
    }

    #[cfg(test)]
    pub(crate) fn consume_protected_diagnostic_current_view_anchor<'view>(
        &'view self,
        anchor: ProtectedDiagnosticCurrentViewAnchorV1<'view>,
    ) -> Result<(), StoreError> {
        let (store_state, state_revision) = state(self.connection)?;
        if store_state != StoreStateV1::Active {
            return Err(StoreError::ProtectedDiagnosticCurrentnessRefused);
        }
        let (head_id, head_revision) = active_head_row(self.connection)?
            .ok_or(StoreError::ProtectedDiagnosticCurrentnessRefused)?;
        let head = load_head(self.connection, head_id, self.domain)?;
        let generation = load_generation(self.connection, head.generation_id(), self.domain)?;
        if head.revision() != head_revision
            || head.generation_id() != generation.id()
            || generation.domain() != self.domain
        {
            return Err(StoreError::ProtectedDiagnosticCurrentnessRefused);
        }
        let observed = ProtectedDiagnosticObservedCurrentViewV1 {
            root: self.root.path(),
            state_revision,
            domain: self.domain,
            role: self.role(),
            head: &head,
            head_revision,
            generation: &generation,
            publication_clock: publication_clock(self.connection)?,
        };
        if !anchor.consume_final_recheck(&observed) {
            return Err(StoreError::ProtectedDiagnosticCurrentnessRefused);
        }
        Ok(())
    }

    pub(crate) fn coherent_generation_snapshot(
        &self,
        generation_id: StoreGenerationIdV1,
    ) -> Result<(StoreHeadV1, StoreGenerationV1, Vec<StoreObjectV1>), StoreError> {
        let generation = load_generation(self.connection, generation_id, self.domain)?;
        let head = load_unique_head_for_generation(self.connection, generation_id, self.domain)?;
        let objects = historical_generation_objects(self.root, self.connection, &generation)?;
        Ok((head, generation, objects))
    }

    pub(crate) fn allocate_continuity_state_token(
        &self,
        store_generation: u64,
        expected_predecessor: Option<[u8; 32]>,
        meaning_digest: [u8; 32],
    ) -> Result<StorePublicationAllocationV1, StoreError> {
        let active_head = self.active_head()?;
        let active_generation = active_head
            .as_ref()
            .map(|head| load_generation(self.connection, head.generation_id(), self.domain))
            .transpose()?;
        let expected_generation = active_generation
            .as_ref()
            .map_or(1, |generation| generation.ordinal().saturating_add(1));
        if store_generation == 0 || store_generation != expected_generation {
            return Err(StoreError::InvalidPublicationAllocation);
        }
        let next_publication_clock = publication_clock(self.connection)?
            .checked_add(1)
            .ok_or(StoreError::InvalidPublicationAllocation)?;
        let allocation_value = CborValue::Array(vec![
            CborValue::text("maestro.vnext.store-publication-allocation.v1")?,
            CborValue::Unsigned(self.domain.role().tag()),
            CborValue::Bytes(self.domain.id().as_bytes().to_vec()),
            CborValue::optional(
                active_head
                    .as_ref()
                    .map(|head| CborValue::Bytes(head.id().as_bytes().to_vec())),
            ),
            CborValue::optional(
                active_generation
                    .as_ref()
                    .map(|generation| CborValue::Bytes(generation.id().as_bytes().to_vec())),
            ),
            CborValue::Unsigned(store_generation),
            CborValue::Unsigned(next_publication_clock),
            CborValue::optional(expected_predecessor.map(|token| CborValue::Bytes(token.to_vec()))),
            CborValue::Bytes(meaning_digest.to_vec()),
        ]);
        let allocation_commitment: [u8; 32] =
            Sha256::digest(deterministic_cbor::encode(&allocation_value)?).into();
        let token_value = CborValue::Array(vec![
            CborValue::text("maestro.vnext.authority-continuity-state-token.v1")?,
            CborValue::Bytes(allocation_commitment.to_vec()),
        ]);
        let token_commitment: [u8; 32] =
            Sha256::digest(deterministic_cbor::encode(&token_value)?).into();
        if expected_predecessor == Some(token_commitment) {
            return Err(StoreError::InvalidPublicationAllocation);
        }
        Ok(StorePublicationAllocationV1 {
            token_commitment,
            allocation_commitment,
            expected_predecessor,
            store_generation,
            publication_clock: next_publication_clock,
        })
    }
}

#[derive(Debug)]
pub(crate) enum PreparedPublicationError<E> {
    Store(StoreError),
    Prepare(E),
}

impl<E> From<StoreError> for PreparedPublicationError<E> {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl StoreV1 {
    pub fn create(path: impl AsRef<Path>, domain: StoreDomainV1) -> Result<Self, StoreError> {
        let root = SecureRoot::open_or_create(path)?;
        root.create_dir_all(OBJECTS_DIRECTORY)?;
        root.create_dir_all(EXPORTS_DIRECTORY)?;
        root.create_dir_all(SNAPSHOT_CLOSURES_DIRECTORY)?;
        root.create_dir_all(RECOVERY_DIRECTORY)?;
        root.create_dir_all(CACHE_DIRECTORY)?;
        root.verify_path_binding()?;
        let metadata_path = root.path().join(METADATA_FILE);
        reject_hard_link(&metadata_path)?;
        let metadata = MetadataStore::open_or_create(&metadata_path, domain.role(), domain.id())?;
        root.verify_path_binding()?;
        reject_sqlite_hard_links(&metadata_path)?;
        Self::finish_open(root, metadata, domain)
    }

    pub fn open(path: impl AsRef<Path>, domain: StoreDomainV1) -> Result<Self, StoreError> {
        let root = SecureRoot::open(path)?;
        root.verify_path_binding()?;
        let metadata_path = root.path().join(METADATA_FILE);
        reject_sqlite_hard_links(&metadata_path)?;
        let metadata = MetadataStore::open_existing(&metadata_path, domain.role(), domain.id())?;
        root.verify_path_binding()?;
        reject_sqlite_hard_links(&metadata_path)?;
        root.open_dir(OBJECTS_DIRECTORY)?;
        root.open_dir(EXPORTS_DIRECTORY)?;
        root.open_dir(SNAPSHOT_CLOSURES_DIRECTORY)?;
        root.open_dir(RECOVERY_DIRECTORY)?;
        root.open_dir(CACHE_DIRECTORY)?;
        Self::finish_open(root, metadata, domain)
    }

    fn finish_open(
        root: SecureRoot,
        metadata: MetadataStore,
        domain: StoreDomainV1,
    ) -> Result<Self, StoreError> {
        let store = Self {
            root,
            metadata,
            domain,
        };
        let (store_state, _) = store.state()?;
        let active = store.with_verified_read(|transaction| active_head_row(transaction))?;
        match active {
            Some((head_id, _)) => {
                store.with_verified_read(|connection| {
                    load_lineage(connection, head_id, &store.domain).map(|_| ())
                })?;
                if store_state == StoreStateV1::Active {
                    store.verify_generation_closure(head_id)?;
                }
            }
            None if store_state == StoreStateV1::Active => {
                return Err(StoreError::MissingActiveHead);
            }
            None => {}
        }
        Ok(store)
    }

    pub fn domain(&self) -> &StoreDomainV1 {
        &self.domain
    }

    pub fn role(&self) -> StoreRoleV1 {
        self.domain.role()
    }

    pub fn state(&self) -> Result<(StoreStateV1, u64), StoreError> {
        self.with_verified_read(|transaction| state(transaction))
    }

    #[cfg(test)]
    pub(crate) fn publication_clock_for_test(&self) -> Result<u64, StoreError> {
        self.with_verified_read(|transaction| publication_clock(transaction))
    }

    pub fn active_head(&self) -> Result<Option<StoreHeadV1>, StoreError> {
        if self.state()?.0 == StoreStateV1::Inactive {
            return Ok(None);
        }
        let Some((head_id, _)) =
            self.with_verified_read(|transaction| active_head_row(transaction))?
        else {
            return Ok(None);
        };
        self.verify_generation_closure(head_id).map(Some)
    }

    pub(crate) fn publication_generation(
        &self,
        head_id: StoreHeadIdV1,
    ) -> Result<StoreGenerationV1, StoreError> {
        self.with_verified_read(|connection| {
            let head = load_head(connection, head_id, &self.domain)?;
            load_generation(connection, head.generation_id(), &self.domain)
        })
    }

    pub(crate) fn generation(
        &self,
        generation_id: StoreGenerationIdV1,
    ) -> Result<StoreGenerationV1, StoreError> {
        self.with_verified_read(|connection| {
            load_generation(connection, generation_id, &self.domain)
        })
    }

    pub(crate) fn coherent_publication_snapshot(
        &self,
    ) -> Result<
        (
            StoreStateV1,
            StoreHeadV1,
            StoreGenerationV1,
            Vec<StoreObjectV1>,
        ),
        StoreError,
    > {
        self.with_verified_read(|connection| {
            let (store_state, _) = state(connection)?;
            let view = StorePublicationViewV1 {
                root: &self.root,
                connection,
                domain: &self.domain,
            };
            let head = view.active_head()?.ok_or(StoreError::MissingActiveHead)?;
            let generation = view
                .active_generation()?
                .ok_or(StoreError::MissingActiveHead)?;
            let objects = view.active_generation_objects()?;
            Ok((store_state, head, generation, objects))
        })
    }

    pub(crate) fn coherent_generation_snapshot(
        &self,
        generation_id: StoreGenerationIdV1,
    ) -> Result<(StoreHeadV1, StoreGenerationV1, Vec<StoreObjectV1>), StoreError> {
        self.with_verified_read(|connection| {
            let generation = load_generation(connection, generation_id, &self.domain)?;
            let head = load_unique_head_for_generation(connection, generation_id, &self.domain)?;
            let objects = historical_generation_objects(&self.root, connection, &generation)?;
            Ok((head, generation, objects))
        })
    }

    #[cfg(test)]
    pub(crate) fn put_object(&mut self, object: &StoreObjectV1) -> Result<(), StoreError> {
        self.root.verify_path_binding()?;
        self.persist_objects(std::slice::from_ref(object))
    }

    pub fn read_object(&self, object_id: StoreObjectIdV1) -> Result<StoreObjectV1, StoreError> {
        self.with_verified_read(|connection| self.read_object_with(connection, object_id))
    }

    fn read_stored_object(&self, object_id: StoreObjectIdV1) -> Result<StoreObjectV1, StoreError> {
        self.with_verified_read(|connection| self.read_stored_object_with(connection, object_id))
    }

    fn read_object_with(
        &self,
        connection: &Connection,
        object_id: StoreObjectIdV1,
    ) -> Result<StoreObjectV1, StoreError> {
        read_object_with_root(&self.root, connection, object_id)
    }

    fn read_stored_object_with(
        &self,
        connection: &Connection,
        object_id: StoreObjectIdV1,
    ) -> Result<StoreObjectV1, StoreError> {
        read_stored_object_with_root(&self.root, connection, object_id)
    }

    fn verify_generation_closure(&self, head_id: StoreHeadIdV1) -> Result<StoreHeadV1, StoreError> {
        self.with_verified_read(|connection| {
            let head = load_head(connection, head_id, &self.domain)?;
            let generation = load_generation(connection, head.generation_id(), &self.domain)?;
            walk_object_closure_with_limit(
                generation.roots().iter().copied(),
                MAX_OBJECT_CLOSURE_ENTRIES,
                |object_id| {
                    Ok(self
                        .read_object_with(connection, object_id)?
                        .references()
                        .to_vec())
                },
            )?;
            Ok(head)
        })
    }

    fn with_verified_read<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        self.metadata.with_verified_read(&self.root, operation)
    }

    #[cfg(test)]
    pub(crate) fn publish_generation(
        &mut self,
        generation: &StoreGenerationV1,
        expected_old: Option<StoreHeadIdV1>,
    ) -> Result<StoreHeadV1, StoreError> {
        self.root.verify_path_binding()?;
        if generation.domain() != &self.domain {
            return Err(StoreError::DomainMismatch);
        }
        if !generation.compatibility().is_stage0_successor() {
            return Err(StoreError::IncompatibleGeneration);
        }
        let generation_bytes = generation.canonical_bytes()?;
        if StoreGenerationV1::decode(&generation_bytes)? != *generation {
            return Err(StoreError::NonCanonicalCarrier);
        }
        let observed = self.with_verified_read(|transaction| active_head_row(transaction))?;
        let observed_retention_revision =
            self.with_verified_read(|transaction| retention_revision(transaction))?;
        let closure = walk_object_closure_with_limit(
            generation.roots().iter().copied(),
            MAX_OBJECT_CLOSURE_ENTRIES,
            |object_id| Ok(self.read_object(object_id)?.references().to_vec()),
        )?;

        if let Some((head_id, revision)) = observed {
            let current = self.verify_generation_closure(head_id)?;
            if current.generation_id() == generation.id()
                && current.revision() == generation.ordinal()
                && current.previous_head_id() == expected_old
                && self.with_verified_read(|connection| {
                    load_generation(connection, current.generation_id(), &self.domain)
                })? == *generation
            {
                return Ok(current);
            }
            if revision != current.revision() {
                return Err(StoreError::StoredHeadMismatch);
            }
        }
        let (expected_revision, previous_generation) = match observed {
            None => {
                if expected_old.is_some()
                    || generation.ordinal() != 1
                    || generation.previous().is_some()
                {
                    return Err(StoreError::HeadCasMismatch);
                }
                (0, None)
            }
            Some((head_id, revision)) => {
                if expected_old != Some(head_id) || generation.ordinal() != revision + 1 {
                    return Err(StoreError::HeadCasMismatch);
                }
                let prior = self.with_verified_read(|connection| {
                    load_head(connection, head_id, &self.domain)
                })?;
                if generation.previous() != Some(prior.generation_id()) {
                    return Err(StoreError::HeadCasMismatch);
                }
                (revision, Some(head_id))
            }
        };
        let head = StoreHeadV1::new(generation, generation.ordinal(), previous_generation)?;

        self.metadata.with_immediate_transaction(&self.root, |transaction| {
            expect_active_head(transaction, expected_old, expected_revision)?;
            expect_retention_revision(transaction, observed_retention_revision)?;
            for object_id in &closure {
                let invalid: bool = transaction.query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM store_logical_tombstones WHERE object_id = ?1
                         UNION ALL
                         SELECT 1 FROM store_gc_collection_occurrences WHERE object_id = ?1
                     )",
                    params![object_id.as_bytes()],
                    |row| row.get(0),
                )?;
                if invalid {
                    return Err(MetadataError::FacadeCasMismatch);
                }
            }
            insert_generation(transaction, generation)?;
            insert_head(transaction, &head)?;
            let changed = if expected_revision == 0 {
                transaction.execute(
                    "INSERT INTO store_active_head(singleton, head_id, head_revision) VALUES (1, ?1, ?2)",
                    params![head.id().as_bytes(), to_i64(head.revision())?],
                )?
            } else {
                transaction.execute(
                    "UPDATE store_active_head SET head_id = ?1, head_revision = ?2
                     WHERE singleton = 1 AND head_id = ?3 AND head_revision = ?4",
                    params![
                        head.id().as_bytes(),
                        to_i64(head.revision())?,
                        expected_old.expect("invariant: non-initial publication has an old Head").as_bytes(),
                        to_i64(expected_revision)?,
                    ],
                )?
            };
            if changed != 1 {
                return Err(MetadataError::FacadeCasMismatch);
            }
            advance_retention_revision(transaction, Some(observed_retention_revision))?;
            Ok(())
        })?;
        Ok(head)
    }

    #[cfg(test)]
    pub(crate) fn publish_generation_atomically(
        &mut self,
        publication: &AtomicGenerationPublicationV1,
    ) -> Result<StorePublicationOutcomeV1, StoreError> {
        let probe = StoreIdempotencyProbeV1::new(
            publication.idempotency().namespace(),
            *publication.idempotency().key_digest(),
            *publication.idempotency().meaning_digest(),
        )
        .map_err(StoreError::AtomicPublication)?;
        match self.publish_generation_atomically_with_prepare(&probe, |_| {
            Ok::<_, std::convert::Infallible>(publication.clone())
        }) {
            Ok(outcome) => Ok(outcome),
            Err(PreparedPublicationError::Store(error)) => Err(error),
            Err(PreparedPublicationError::Prepare(never)) => match never {},
        }
    }

    pub(crate) fn publish_generation_atomically_with_prepare<E>(
        &mut self,
        probe: &StoreIdempotencyProbeV1,
        prepare: impl FnOnce(&StorePublicationViewV1<'_>) -> Result<AtomicGenerationPublicationV1, E>,
    ) -> Result<StorePublicationOutcomeV1, PreparedPublicationError<E>> {
        self.root.verify_path_binding().map_err(StoreError::from)?;
        let root = &self.root;
        let domain = self.domain.clone();
        let mut prepare = Some(prepare);
        let mut staged_files = Vec::new();
        let transaction_outcome = self.metadata.with_prepared_transaction(
            root,
            |transaction| {
                if let Some(stored) = load_idempotency_metadata(
                    transaction,
                    probe.namespace(),
                    probe.key_digest(),
                )? {
                    if stored.meaning_digest != *probe.meaning_digest() {
                        return Err(MetadataError::IdempotencyMeaningConflict.into());
                    }
                    return Ok(PublicationMutation::NoChange(
                        AtomicMetadataPublication::Replayed(stored),
                    ));
                }

                let view = StorePublicationViewV1 {
                    root,
                    connection: transaction,
                    domain: &domain,
                };
                let publication = prepare
                    .take()
                    .expect("invariant: one prepared Store transaction invokes its callback once")(
                    &view,
                )
                .map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Prepare(
                        error,
                    ))
                })?;
                let idempotency = publication.idempotency();
                if idempotency.namespace() != probe.namespace()
                    || idempotency.key_digest() != probe.key_digest()
                    || idempotency.meaning_digest() != probe.meaning_digest()
                {
                    return Err(ConditionalTransactionError::Operation(
                        PreparedPublicationError::Store(
                            StoreError::PreparedIdempotencyMismatch,
                        ),
                    ));
                }
                let generation = publication.generation();
                if generation.domain() != &domain {
                    return Err(ConditionalTransactionError::Operation(
                        PreparedPublicationError::Store(StoreError::DomainMismatch),
                    ));
                }
                if !generation.compatibility().is_stage0_successor() {
                    return Err(ConditionalTransactionError::Operation(
                        PreparedPublicationError::Store(StoreError::IncompatibleGeneration),
                    ));
                }
                if StoreGenerationV1::decode(&generation.canonical_bytes().map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                        StoreError::CanonicalCbor(error),
                    ))
                })?)
                .map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                        StoreError::Generation(error),
                    ))
                })? != *generation
                {
                    return Err(ConditionalTransactionError::Operation(
                        PreparedPublicationError::Store(StoreError::NonCanonicalCarrier),
                    ));
                }
                let objects = publication.objects();
                let object_map = objects
                    .iter()
                    .map(|object| (object.id(), object))
                    .collect::<BTreeMap<_, _>>();
                let closure = walk_object_closure_with_limit(
                    generation.roots().iter().copied(),
                    MAX_OBJECT_CLOSURE_ENTRIES,
                    |object_id| {
                        if let Some(object) = object_map.get(&object_id) {
                            Ok(object.references().to_vec())
                        } else {
                            Ok(read_object_with_root(root, transaction, object_id)?
                                .references()
                                .to_vec())
                        }
                    },
                )
                .map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Store(error))
                })?;
                if !closure.contains(&idempotency.result_object_id()) {
                    return Err(ConditionalTransactionError::Operation(
                        PreparedPublicationError::Store(
                            StoreError::IdempotencyResultOutsideGeneration,
                        ),
                    ));
                }
                if object_map.keys().any(|object_id| !closure.contains(object_id)) {
                    return Err(ConditionalTransactionError::Operation(
                        PreparedPublicationError::Store(
                            StoreError::PublicationObjectOutsideGeneration,
                        ),
                    ));
                }
                ensure_no_controlled_copy_erasure_barriers(root, closure.iter().copied())
                    .map_err(|error| {
                        ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                            error,
                        ))
                    })?;
                let observed = active_head_row(transaction).map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Store(error))
                })?;
                let observed_retention_revision = retention_revision(transaction).map_err(
                    |error| {
                        ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                            error,
                        ))
                    },
                )?;
                let (expected_revision, previous_head_id) = publication_basis(
                    observed,
                    publication.expected_old(),
                    generation,
                    &domain,
                    |head_id| load_head(transaction, head_id, &domain),
                )
                .map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Store(error))
                })?;
                let head = StoreHeadV1::new(
                    generation,
                    generation.ordinal(),
                    previous_head_id,
                )
                .map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                        StoreError::Generation(error),
                    ))
                })?;
                expect_active_head(
                    transaction,
                    publication.expected_old(),
                    expected_revision,
                )?;
                expect_retention_revision(transaction, observed_retention_revision)?;
                for object in objects {
                    let invalid: bool = transaction.query_row(
                        "SELECT EXISTS(
                             SELECT 1 FROM store_logical_tombstones WHERE object_id = ?1
                             UNION ALL
                             SELECT 1 FROM store_gc_collection_occurrences WHERE object_id = ?1
                         )",
                        params![object.id().as_bytes()],
                        |row| row.get(0),
                    )?;
                    if invalid {
                        return Err(MetadataError::FacadeCasMismatch.into());
                    }
                }
                stage_object_files(root, objects, &mut staged_files).map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Store(error))
                })?;
                for object in objects {
                    insert_object_metadata(transaction, object)?;
                }
                for object in objects {
                    insert_or_verify_references(transaction, object)?;
                }
                insert_generation(transaction, generation)?;
                insert_head(transaction, &head)?;
                let changed = if expected_revision == 0 {
                    transaction.execute(
                        "INSERT INTO store_active_head(singleton, head_id, head_revision)
                         VALUES (1, ?1, ?2)",
                        params![head.id().as_bytes(), to_i64(head.revision())?],
                    )?
                } else {
                    transaction.execute(
                        "UPDATE store_active_head SET head_id = ?1, head_revision = ?2
                         WHERE singleton = 1 AND head_id = ?3 AND head_revision = ?4",
                        params![
                            head.id().as_bytes(),
                            to_i64(head.revision())?,
                            publication
                                .expected_old()
                                .expect("invariant: non-initial atomic publication has an old Head")
                                .as_bytes(),
                            to_i64(expected_revision)?,
                        ],
                    )?
                };
                if changed != 1 {
                    return Err(MetadataError::FacadeCasMismatch.into());
                }
                advance_retention_revision(transaction, Some(observed_retention_revision))?;
                transaction.execute(
                    "INSERT INTO store_idempotency
                     (namespace, key_digest, meaning_digest, result_object_id, generation_id, head_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        idempotency.namespace(),
                        idempotency.key_digest().as_slice(),
                        idempotency.meaning_digest().as_slice(),
                        idempotency.result_object_id().as_bytes(),
                        generation.id().as_bytes(),
                        head.id().as_bytes(),
                    ],
                )?;
                Ok(PublicationMutation::Commit(
                    AtomicMetadataPublication::Committed {
                        head,
                        result_object_id: idempotency.result_object_id(),
                    },
                ))
            },
        );

        let metadata_outcome = match transaction_outcome {
            Ok(outcome) => outcome,
            Err(ConditionalTransactionError::Metadata(error)) => {
                if !error.publication_may_have_committed() {
                    self.cleanup_failed_atomic_publication(&staged_files)?;
                }
                return Err(PreparedPublicationError::Store(error.into()));
            }
            Err(ConditionalTransactionError::Operation(error)) => {
                self.cleanup_failed_atomic_publication(&staged_files)?;
                return Err(error);
            }
        };

        match metadata_outcome {
            AtomicMetadataPublication::Committed {
                head,
                result_object_id,
            } => Ok(StorePublicationOutcomeV1::Committed {
                head,
                result: self.read_object(result_object_id)?,
            }),
            AtomicMetadataPublication::Replayed(stored) => self
                .resolve_idempotency_replay(stored, probe.meaning_digest())
                .map_err(PreparedPublicationError::Store),
        }
    }

    pub(crate) fn with_serialized_active_view<T, E>(
        &mut self,
        operation: impl FnOnce(&StorePublicationViewV1<'_>) -> Result<T, E>,
    ) -> Result<T, PreparedPublicationError<E>> {
        self.root.verify_path_binding().map_err(StoreError::from)?;
        let root = &self.root;
        let domain = self.domain.clone();
        self.metadata
            .with_prepared_transaction(root, |transaction| {
                let view = StorePublicationViewV1 {
                    root,
                    connection: transaction,
                    domain: &domain,
                };
                operation(&view)
                    .map(PublicationMutation::NoChange)
                    .map_err(|error| {
                        ConditionalTransactionError::Operation(PreparedPublicationError::Prepare(
                            error,
                        ))
                    })
            })
            .map_err(|error| match error {
                ConditionalTransactionError::Metadata(error) => {
                    PreparedPublicationError::Store(error.into())
                }
                ConditionalTransactionError::Operation(error) => error,
            })
    }

    pub(crate) fn prepare_controlled_copy_erasure_plan<E>(
        &mut self,
        object_id: StoreObjectIdV1,
        barrier_identity: [u8; 32],
        validate: impl FnOnce(&StorePublicationViewV1<'_>) -> Result<(), E>,
    ) -> Result<ControlledCopyErasurePlanV1, PreparedPublicationError<E>> {
        self.root.verify_path_binding().map_err(StoreError::from)?;
        let root = &self.root;
        let domain = self.domain.clone();
        let mut validate = Some(validate);
        self.metadata
            .with_prepared_transaction(root, |transaction| {
                let view = StorePublicationViewV1 {
                    root,
                    connection: transaction,
                    domain: &domain,
                };
                validate
                    .take()
                    .expect("invariant: one copy-plan transaction validates once")(
                    &view
                )
                .map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Prepare(error))
                })?;
                let known: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM store_objects WHERE object_id = ?1)",
                    params![object_id.as_bytes()],
                    |row| row.get(0),
                )?;
                if !known || barrier_identity == [0; 32] {
                    return Err(ConditionalTransactionError::Operation(
                        PreparedPublicationError::Store(
                            StoreError::InvalidControlledCopyErasurePlan,
                        ),
                    ));
                }
                let marker = controlled_copy_erasure_barrier_bytes(object_id, barrier_identity)
                    .map_err(|error| {
                        ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                            error,
                        ))
                    })?;
                let marker_path = controlled_copy_erasure_barrier_path(object_id);
                root.create_dir_all(Path::new(CONTROLLED_COPY_ERASURE_BARRIERS_DIRECTORY))
                    .map_err(|error| {
                        ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                            error.into(),
                        ))
                    })?;
                match root
                    .create_file_if_absent(&marker_path, &marker)
                    .map_err(|error| {
                        ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                            error.into(),
                        ))
                    })? {
                    CreateIfAbsent::Created => {}
                    CreateIfAbsent::AlreadyExists => {
                        root.read_exact(&marker_path, &marker).map_err(|error| {
                            ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                                error.into(),
                            ))
                        })?;
                    }
                }
                let obligations = controlled_copy_obligations(root, transaction, object_id)
                    .map_err(|error| {
                        ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                            error,
                        ))
                    })?;
                let plan = ControlledCopyErasurePlanV1::new(
                    object_id,
                    barrier_identity,
                    publication_clock(transaction).map_err(|error| {
                        ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                            error,
                        ))
                    })?,
                    retention_revision(transaction).map_err(|error| {
                        ConditionalTransactionError::Operation(PreparedPublicationError::Store(
                            error,
                        ))
                    })?,
                    obligations,
                )
                .map_err(|error| {
                    ConditionalTransactionError::Operation(PreparedPublicationError::Store(error))
                })?;
                Ok(PublicationMutation::NoChange(plan))
            })
            .map_err(|error| match error {
                ConditionalTransactionError::Metadata(error) => {
                    PreparedPublicationError::Store(error.into())
                }
                ConditionalTransactionError::Operation(error) => error,
            })
    }

    pub(crate) fn abort_controlled_copy_erasure_plan(
        &mut self,
        plan: &ControlledCopyErasurePlanV1,
    ) -> Result<(), StoreError> {
        let marker =
            controlled_copy_erasure_barrier_bytes(plan.object_id(), plan.barrier_identity())?;
        let root = &self.root;
        match self
            .metadata
            .with_prepared_transaction(root, |transaction| {
                let current = controlled_copy_obligations(root, transaction, plan.object_id())
                    .map_err(ConditionalTransactionError::Operation)?;
                if current != plan.obligations {
                    return Err(ConditionalTransactionError::Operation(
                        StoreError::ControlledCopyErasureCensusChanged,
                    ));
                }
                root.remove_file_if_matches(
                    controlled_copy_erasure_barrier_path(plan.object_id()),
                    &marker,
                )
                .map_err(|error| ConditionalTransactionError::Operation(error.into()))?;
                Ok(PublicationMutation::NoChange(()))
            }) {
            Ok(()) => Ok(()),
            Err(ConditionalTransactionError::Metadata(error)) => Err(error.into()),
            Err(ConditionalTransactionError::Operation(error)) => Err(error),
        }
    }

    fn cleanup_failed_atomic_publication(
        &mut self,
        staged: &[StagedObjectFile],
    ) -> Result<(), StoreError> {
        if staged.is_empty() {
            return Ok(());
        }
        run_before_failed_publication_cleanup_test_hook();
        let root = &self.root;
        match self
            .metadata
            .with_prepared_transaction(root, |transaction| {
                cleanup_staged_object_files_if_unreferenced(root, transaction, staged)
                    .map_err(ConditionalTransactionError::Operation)?;
                Ok(PublicationMutation::NoChange(()))
            }) {
            Ok(()) => Ok(()),
            Err(ConditionalTransactionError::Metadata(error)) => Err(error.into()),
            Err(ConditionalTransactionError::Operation(error)) => Err(error),
        }
    }

    pub fn replay_idempotency(
        &self,
        probe: &StoreIdempotencyProbeV1,
    ) -> Result<Option<StorePublicationOutcomeV1>, StoreError> {
        let stored = self.with_verified_read(|connection| {
            load_idempotency(connection, probe.namespace(), probe.key_digest())
        })?;
        stored
            .map(|stored| self.resolve_idempotency_replay(stored, probe.meaning_digest()))
            .transpose()
    }

    fn resolve_idempotency_replay(
        &self,
        stored: StoredIdempotencyV1,
        expected_meaning_digest: &[u8; 32],
    ) -> Result<StorePublicationOutcomeV1, StoreError> {
        if &stored.meaning_digest != expected_meaning_digest {
            return Err(StoreError::IdempotencyMeaningConflict);
        }
        let (head, result) = self.with_verified_read(|connection| {
            let head = load_head(connection, stored.head_id, &self.domain)?;
            if head.generation_id() != stored.generation_id {
                return Err(StoreError::StoredIdempotencyMismatch);
            }
            load_generation(connection, stored.generation_id, &self.domain)?;
            let result = self.read_object_with(connection, stored.result_object_id)?;
            Ok((head, result))
        })?;
        Ok(StorePublicationOutcomeV1::Replayed { head, result })
    }

    pub fn add_retention_pin(
        &mut self,
        pin: &RetentionPinV1,
        expected_retention_revision: u64,
    ) -> Result<u64, StoreError> {
        self.root.verify_path_binding()?;
        let active = self
            .with_verified_read(|transaction| active_head_row(transaction))?
            .ok_or(StoreError::MissingActiveHead)?;
        if pin.basis_head_id() != active.0 {
            return Err(StoreError::SnapshotBasisMismatch);
        }
        self.read_object(pin.root().object_id())?;
        let bytes = pin.canonical_bytes()?;
        if RetentionPinV1::decode(&bytes)? != *pin {
            return Err(StoreError::NonCanonicalCarrier);
        }
        self.metadata
            .with_immediate_transaction(&self.root, |transaction| {
                expect_active_head(transaction, Some(active.0), active.1)?;
                expect_retention_revision(transaction, expected_retention_revision)?;
                transaction.execute(
                    "INSERT INTO store_retention_pins
                 (pin_id, basis_head_id, root_kind, root_object_id, reason_digest)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        pin.id().as_bytes(),
                        pin.basis_head_id().as_bytes(),
                        to_i64(pin.root().kind().tag())?,
                        pin.root().object_id().as_bytes(),
                        pin.reason_digest().as_slice(),
                    ],
                )?;
                advance_retention_revision(transaction, Some(expected_retention_revision))
            })
            .map_err(StoreError::from)
    }

    pub fn release_retention_pin(
        &mut self,
        pin_id: RetentionPinIdV1,
        reason_digest: [u8; 32],
        expected_retention_revision: u64,
    ) -> Result<u64, StoreError> {
        self.root.verify_path_binding()?;
        let active = self
            .with_verified_read(|transaction| active_head_row(transaction))?
            .ok_or(StoreError::MissingActiveHead)?;
        self.metadata
            .with_immediate_transaction(&self.root, |transaction| {
            expect_active_head(transaction, Some(active.0), active.1)?;
            expect_retention_revision(transaction, expected_retention_revision)?;
            let changed = transaction.execute(
                "INSERT INTO store_retention_pin_releases(pin_id, released_at_head_id, reason_digest)
                 SELECT ?1, ?2, ?3 FROM store_retention_pins
                 WHERE pin_id = ?1
                   AND NOT EXISTS (SELECT 1 FROM store_retention_pin_releases WHERE pin_id = ?1)",
                params![pin_id.as_bytes(), active.0.as_bytes(), reason_digest.as_slice()],
            )?;
            if changed != 1 {
                return Err(MetadataError::FacadeCasMismatch);
            }
            advance_retention_revision(transaction, Some(expected_retention_revision))
            })
            .map_err(StoreError::from)
    }

    pub fn tombstone(
        &mut self,
        tombstone: &LogicalTombstoneV1,
        expected_retention_revision: u64,
    ) -> Result<u64, StoreError> {
        self.root.verify_path_binding()?;
        let active = self
            .with_verified_read(|transaction| active_head_row(transaction))?
            .ok_or(StoreError::MissingActiveHead)?;
        if tombstone.basis_head_id() != active.0 {
            return Err(StoreError::SnapshotBasisMismatch);
        }
        self.read_object(tombstone.object_id())?;
        let (_, reachable) = self.reachability_roots_and_objects(active.0)?;
        if reachable.contains(&tombstone.object_id()) {
            return Err(StoreError::ObjectStillReachable(tombstone.object_id()));
        }
        let bytes = tombstone.canonical_bytes()?;
        if LogicalTombstoneV1::decode(&bytes)? != *tombstone {
            return Err(StoreError::NonCanonicalCarrier);
        }
        self.metadata
            .with_immediate_transaction(&self.root, |transaction| {
                expect_active_head(transaction, Some(active.0), active.1)?;
                expect_retention_revision(transaction, expected_retention_revision)?;
                let already_tombstoned: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM store_logical_tombstones WHERE object_id = ?1)",
                    params![tombstone.object_id().as_bytes()],
                    |row| row.get(0),
                )?;
                if already_tombstoned {
                    return Err(MetadataError::FacadeCasMismatch);
                }
                transaction.execute(
                    "INSERT INTO store_logical_tombstones
                 (tombstone_id, basis_head_id, object_id, reason_digest, invalidation_digest)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        tombstone.id().as_bytes(),
                        tombstone.basis_head_id().as_bytes(),
                        tombstone.object_id().as_bytes(),
                        tombstone.reason_digest().as_slice(),
                        tombstone.invalidation_digest().as_slice(),
                    ],
                )?;
                advance_retention_revision(transaction, Some(expected_retention_revision))
            })
            .map_err(StoreError::from)
    }

    pub(crate) fn ensure_tombstone(
        &mut self,
        tombstone: &LogicalTombstoneV1,
    ) -> Result<u64, StoreError> {
        self.root.verify_path_binding()?;
        let current_revision =
            self.with_verified_read(|connection| retention_revision(connection))?;
        let stored = self.with_verified_read(|connection| {
            connection
                .query_row(
                    "SELECT basis_head_id, tombstone_id, reason_digest, invalidation_digest
                     FROM store_logical_tombstones WHERE object_id = ?1",
                    params![tombstone.object_id().as_bytes()],
                    |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                        ))
                    },
                )
                .optional()
                .map_err(StoreError::from)
        })?;
        if let Some((basis, id, reason, invalidation)) = stored {
            if basis.as_slice() != tombstone.basis_head_id().as_bytes()
                || id.as_slice() != tombstone.id().as_bytes()
                || reason.as_slice() != tombstone.reason_digest()
                || invalidation.as_slice() != tombstone.invalidation_digest()
            {
                return Err(StoreError::StoredTombstoneMismatch);
            }
            return Ok(current_revision);
        }
        self.tombstone(tombstone, current_revision)
    }

    pub(crate) fn load_tombstone(
        &self,
        object_id: StoreObjectIdV1,
    ) -> Result<Option<LogicalTombstoneV1>, StoreError> {
        self.root.verify_path_binding()?;
        self.with_verified_read(|connection| {
            let row = connection
                .query_row(
                    "SELECT tombstone_id, basis_head_id, reason_digest, invalidation_digest
                     FROM store_logical_tombstones WHERE object_id = ?1",
                    params![object_id.as_bytes()],
                    |row| {
                        Ok((
                            row.get::<_, Vec<u8>>(0)?,
                            row.get::<_, Vec<u8>>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                        ))
                    },
                )
                .optional()?;
            let Some((stored_id, basis, reason, invalidation)) = row else {
                return Ok(None);
            };
            let tombstone = LogicalTombstoneV1::new(
                StoreHeadIdV1::from_digest(exact_digest(basis)?),
                object_id,
                exact_digest(reason)?,
                exact_digest(invalidation)?,
            )?;
            if tombstone.id().as_bytes() != stored_id.as_slice() {
                return Err(StoreError::StoredTombstoneMismatch);
            }
            Ok(Some(tombstone))
        })
    }

    pub fn snapshot_reachability(&mut self) -> Result<ReachabilitySnapshotV1, StoreError> {
        self.root.verify_path_binding()?;
        let (head_id, head_revision) = self
            .with_verified_read(|transaction| active_head_row(transaction))?
            .ok_or(StoreError::MissingActiveHead)?;
        let retention_revision =
            self.with_verified_read(|transaction| retention_revision(transaction))?;
        if retention_revision == 0 {
            return Err(StoreError::InvalidRetentionRevision);
        }
        let (roots, reachable) = self.reachability_roots_and_objects(head_id)?;
        let tombstoned = self.with_verified_read(|transaction| tombstoned_objects(transaction))?;
        if let Some(object_id) = reachable.intersection(&tombstoned).next() {
            return Err(StoreError::ObjectStillReachable(*object_id));
        }
        let snapshot = ReachabilitySnapshotV1::new(
            head_id,
            retention_revision,
            roots,
            reachable.into_iter().collect(),
            tombstoned.into_iter().collect(),
        )?;
        persist_snapshot(
            &mut self.metadata,
            &self.root,
            &snapshot,
            head_revision,
            retention_revision,
        )?;
        Ok(snapshot)
    }

    pub fn plan_collection(
        &mut self,
        snapshot: &ReachabilitySnapshotV1,
    ) -> Result<CollectionPlanV1, StoreError> {
        self.root.verify_path_binding()?;
        let sealed_export_holds =
            self.with_verified_read(|transaction| available_sealed_export_objects(transaction))?;
        let candidates = snapshot
            .tombstoned()
            .iter()
            .filter(|object_id| !sealed_export_holds.contains(object_id))
            .copied()
            .collect();
        let plan = CollectionPlanV1::new(snapshot, candidates)?;
        let active = self
            .with_verified_read(|transaction| active_head_row(transaction))?
            .ok_or(StoreError::MissingActiveHead)?;
        if active.0 != snapshot.head_id()
            || self.with_verified_read(|transaction| retention_revision(transaction))?
                != snapshot.retention_revision()
        {
            return Err(StoreError::SnapshotBasisMismatch);
        }
        self.metadata
            .with_immediate_transaction(&self.root, |transaction| {
            expect_active_head(transaction, Some(active.0), active.1)?;
            expect_retention_revision(transaction, snapshot.retention_revision())?;
            let snapshot_exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM store_reachability_snapshots WHERE snapshot_id = ?1)",
                params![snapshot.id().as_bytes()],
                |row| row.get(0),
            )?;
            if !snapshot_exists {
                return Err(MetadataError::FacadeCasMismatch);
            }
            transaction.execute(
                "INSERT OR IGNORE INTO store_gc_plans(plan_id, snapshot_id, head_id, retention_revision)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    plan.id().as_bytes(),
                    plan.snapshot_id().as_bytes(),
                    plan.head_id().as_bytes(),
                    to_i64(plan.retention_revision())?,
                ],
            )?;
            for candidate in plan.candidates() {
                transaction.execute(
                    "INSERT OR IGNORE INTO store_gc_plan_objects(plan_id, object_id) VALUES (?1, ?2)",
                    params![plan.id().as_bytes(), candidate.as_bytes()],
                )?;
            }
            verify_plan(transaction, &plan)
            })?;
        Ok(plan)
    }

    pub(crate) fn plan_exact_collection(
        &mut self,
        snapshot: &ReachabilitySnapshotV1,
        candidate: StoreObjectIdV1,
    ) -> Result<CollectionPlanV1, StoreError> {
        self.root.verify_path_binding()?;
        let sealed_export_holds =
            self.with_verified_read(|transaction| available_sealed_export_objects(transaction))?;
        if sealed_export_holds.contains(&candidate) {
            return Err(StoreError::ObjectStillReachable(candidate));
        }
        let plan = CollectionPlanV1::new(snapshot, vec![candidate])?;
        let active = self
            .with_verified_read(|transaction| active_head_row(transaction))?
            .ok_or(StoreError::MissingActiveHead)?;
        if active.0 != snapshot.head_id()
            || self.with_verified_read(|transaction| retention_revision(transaction))?
                != snapshot.retention_revision()
        {
            return Err(StoreError::SnapshotBasisMismatch);
        }
        self.metadata
            .with_immediate_transaction(&self.root, |transaction| {
                expect_active_head(transaction, Some(active.0), active.1)?;
                expect_retention_revision(transaction, snapshot.retention_revision())?;
                let snapshot_exists: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM store_reachability_snapshots WHERE snapshot_id = ?1)",
                    params![snapshot.id().as_bytes()],
                    |row| row.get(0),
                )?;
                if !snapshot_exists {
                    return Err(MetadataError::FacadeCasMismatch);
                }
                transaction.execute(
                    "INSERT OR IGNORE INTO store_gc_plans(plan_id, snapshot_id, head_id, retention_revision)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        plan.id().as_bytes(),
                        plan.snapshot_id().as_bytes(),
                        plan.head_id().as_bytes(),
                        to_i64(plan.retention_revision())?,
                    ],
                )?;
                transaction.execute(
                    "INSERT OR IGNORE INTO store_gc_plan_objects(plan_id, object_id) VALUES (?1, ?2)",
                    params![plan.id().as_bytes(), candidate.as_bytes()],
                )?;
                verify_plan(transaction, &plan)
            })?;
        Ok(plan)
    }

    pub(crate) fn erase_controlled_copies(
        &mut self,
        plan: &ControlledCopyErasurePlanV1,
    ) -> Result<VerifiedControlledCopyAbsenceV1, StoreError> {
        self.root.verify_path_binding()?;
        let marker =
            controlled_copy_erasure_barrier_bytes(plan.object_id(), plan.barrier_identity())?;
        self.root.read_exact(
            controlled_copy_erasure_barrier_path(plan.object_id()),
            &marker,
        )?;
        let root = &self.root;
        let revocation = self
            .metadata
            .with_prepared_transaction(root, |transaction| {
                let current = controlled_copy_obligations(root, transaction, plan.object_id())
                    .map_err(ConditionalTransactionError::Operation)?;
                ensure_monotonic_controlled_copy_erasure_census(plan.obligations(), &current)
                    .map_err(ConditionalTransactionError::Operation)?;
                let export_ids = plan
                    .obligations()
                    .iter()
                    .map(|obligation| {
                        let registered = transaction.query_row(
                            "SELECT EXISTS(SELECT 1 FROM store_sealed_exports WHERE export_id = ?1)",
                            params![obligation.export_id.as_bytes()],
                            |row| row.get::<_, bool>(0),
                        )?;
                        Ok(registered.then_some(*obligation.export_id.as_bytes()))
                    })
                    .collect::<Result<Vec<_>, rusqlite::Error>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                if !export_ids.is_empty() {
                    let expected_retention_revision = retention_revision(transaction)
                        .map_err(ConditionalTransactionError::Operation)?;
                    revoke_sealed_exports_for_security_erasure(
                        transaction,
                        plan.object_id().as_bytes(),
                        &export_ids,
                    )?;
                    advance_retention_revision(transaction, Some(expected_retention_revision))
                        .map_err(ConditionalTransactionError::Metadata)?;
                    Ok(PublicationMutation::Commit(()))
                } else {
                    Ok(PublicationMutation::NoChange(()))
                }
            });
        match revocation {
            Ok(()) => {}
            Err(ConditionalTransactionError::Metadata(error)) => return Err(error.into()),
            Err(ConditionalTransactionError::Operation(error)) => return Err(error),
        }

        let mut first_failure = None;
        let mut remaining = 0_usize;
        let mut removed_snapshot_roots = BTreeSet::new();
        for obligation in plan.obligations() {
            for path in [
                export_path(obligation.export_id),
                export_pending_path(obligation.export_id),
            ] {
                if let Err(error) = erase_secure_file_with_digest(
                    &self.root,
                    &path,
                    &obligation.export_bytes_digest,
                ) {
                    remaining += 1;
                    if first_failure.is_none() {
                        first_failure = Some(error);
                    }
                }
            }
            if let Some((_, digest)) = obligation.snapshot_closure
                && removed_snapshot_roots.insert(obligation.snapshot_root_id)
                && let Err(error) = erase_secure_file_with_digest(
                    &self.root,
                    snapshot_closure_path(&obligation.snapshot_root_id),
                    &digest,
                )
            {
                remaining += 1;
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
        if let Some(source) = first_failure {
            return Err(StoreError::ControlledCopyErasureSweepDebt { remaining, source });
        }
        self.verify_controlled_copy_erasure_absence(plan)
    }

    pub(crate) fn verify_controlled_copy_erasure_absence(
        &self,
        plan: &ControlledCopyErasurePlanV1,
    ) -> Result<VerifiedControlledCopyAbsenceV1, StoreError> {
        self.root.verify_path_binding()?;
        for obligation in plan.obligations() {
            for path in [
                export_path(obligation.export_id),
                export_pending_path(obligation.export_id),
            ] {
                self.root
                    .verify_file_removal_resolved(&path, &obligation.export_bytes_digest)?;
            }
            if let Some((_, digest)) = obligation.snapshot_closure {
                self.root.verify_file_removal_resolved(
                    snapshot_closure_path(&obligation.snapshot_root_id),
                    &digest,
                )?;
            }
        }
        let remaining = self.with_verified_read(|connection| {
            controlled_copy_obligations(&self.root, connection, plan.object_id())
        })?;
        if !remaining.is_empty() {
            return Err(StoreError::ControlledCopyErasureCensusChanged);
        }
        for obligation in plan.obligations() {
            for path in [
                export_path(obligation.export_id),
                export_pending_path(obligation.export_id),
            ] {
                verify_secure_path_absent(&self.root, &path)?;
            }
            if obligation.snapshot_closure.is_some() {
                verify_secure_path_absent(
                    &self.root,
                    &snapshot_closure_path(&obligation.snapshot_root_id),
                )?;
            }
        }
        let receipt_hash = sha256(&deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::text("maestro.vnext.store.controlled-copy-erasure-absence.v1")?,
            CborValue::Bytes(plan.plan_id().to_vec()),
            CborValue::Bytes(plan.object_id().as_bytes().to_vec()),
            CborValue::Array(
                plan.obligations()
                    .iter()
                    .map(ControlledCopyErasureObligationV1::canonical_value)
                    .collect(),
            ),
        ]))?);
        Ok(VerifiedControlledCopyAbsenceV1 {
            plan_id: plan.plan_id(),
            receipt_hash,
        })
    }

    pub(crate) fn finish_controlled_copy_erasure(
        &self,
        plan: &ControlledCopyErasurePlanV1,
        expected_receipt_hash: [u8; 32],
    ) -> Result<(), StoreError> {
        let verified = self.verify_controlled_copy_erasure_absence(plan)?;
        if verified.receipt_hash() != expected_receipt_hash {
            return Err(StoreError::ControlledCopyErasureReceiptMismatch);
        }
        let marker =
            controlled_copy_erasure_barrier_bytes(plan.object_id(), plan.barrier_identity())?;
        self.root.remove_file_if_matches(
            controlled_copy_erasure_barrier_path(plan.object_id()),
            &marker,
        )?;
        Ok(())
    }

    pub fn collect(&mut self, plan: &CollectionPlanV1) -> Result<usize, StoreError> {
        self.root.verify_path_binding()?;
        let active = self
            .with_verified_read(|transaction| active_head_row(transaction))?
            .ok_or(StoreError::MissingActiveHead)?;
        if active.0 != plan.head_id()
            || self.with_verified_read(|transaction| retention_revision(transaction))?
                != plan.retention_revision()
        {
            return Err(StoreError::RetentionCasMismatch);
        }
        let mut expected = Vec::with_capacity(plan.candidates().len());
        for candidate in plan.candidates() {
            let occurrence = self.with_verified_read(|connection| {
                collection_occurrence_digest(connection, *candidate)
            })?;
            match self.read_stored_object(*candidate) {
                Ok(object) => {
                    let bytes = object.canonical_bytes().to_vec();
                    let digest = sha256(&bytes);
                    if occurrence.is_some_and(|stored| stored != digest) {
                        return Err(StoreError::StoredObjectMismatch(*candidate));
                    }
                    expected.push((*candidate, digest, Some(bytes)));
                }
                Err(StoreError::SecureFs(SecureFsError::Io { source, .. }))
                    if source.kind() == std::io::ErrorKind::NotFound && occurrence.is_some() =>
                {
                    expected.push((
                        *candidate,
                        occurrence.expect("invariant: guarded as present"),
                        None,
                    ));
                }
                Err(error) => return Err(error),
            }
        }
        self.metadata
            .with_immediate_transaction(&self.root, |transaction| {
                expect_active_head(transaction, Some(active.0), active.1)?;
                expect_retention_revision(transaction, plan.retention_revision())?;
                verify_plan(transaction, plan)?;
                for (object_id, digest, _) in &expected {
                    transaction.execute(
                        "INSERT OR IGNORE INTO store_gc_collection_occurrences
                     (plan_id, object_id, stored_bytes_digest) VALUES (?1, ?2, ?3)",
                        params![
                            plan.id().as_bytes(),
                            object_id.as_bytes(),
                            digest.as_slice()
                        ],
                    )?;
                    let stored: Vec<u8> = transaction.query_row(
                        "SELECT stored_bytes_digest FROM store_gc_collection_occurrences
                     WHERE plan_id = ?1 AND object_id = ?2",
                        params![plan.id().as_bytes(), object_id.as_bytes()],
                        |row| row.get(0),
                    )?;
                    if stored.as_slice() != digest {
                        return Err(MetadataError::FacadeIntegrityMismatch);
                    }
                }
                Ok(())
            })?;

        let mut first_failure = None;
        let mut remaining = 0;
        for (object_id, digest, bytes) in &expected {
            let removal = match bytes {
                Some(bytes) => self
                    .root
                    .remove_file_if_matches(object_path(*object_id), bytes),
                None => self
                    .root
                    .finish_file_removal_if_digest_matches(object_path(*object_id), digest),
            }
            .and_then(|_| {
                self.root
                    .remove_crash_residual_temps_by_digest(Path::new(OBJECTS_DIRECTORY), digest)
                    .map(|_| ())
            });
            if let Err(error) = removal {
                remaining += 1;
                if first_failure.is_none() {
                    first_failure = Some(error);
                }
            }
        }
        if let Some(source) = first_failure {
            return Err(StoreError::CollectionSweepDebt {
                committed: expected.len(),
                remaining,
                source,
            });
        }
        Ok(expected.len())
    }

    pub(crate) fn verify_collected_object_absence(
        &self,
        tombstone: &LogicalTombstoneV1,
        plan: &CollectionPlanV1,
    ) -> Result<VerifiedCollectionAbsenceV1, StoreError> {
        self.root.verify_path_binding()?;
        if !plan.candidates().contains(&tombstone.object_id()) {
            return Err(StoreError::MissingCollectionOccurrence(
                tombstone.object_id(),
            ));
        }
        let destroyed_object_hash = self.with_verified_read(|connection| {
            let tombstone_matches: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM store_logical_tombstones
                    WHERE tombstone_id = ?1 AND basis_head_id = ?2 AND object_id = ?3
                      AND reason_digest = ?4 AND invalidation_digest = ?5
                )",
                params![
                    tombstone.id().as_bytes(),
                    tombstone.basis_head_id().as_bytes(),
                    tombstone.object_id().as_bytes(),
                    tombstone.reason_digest().as_slice(),
                    tombstone.invalidation_digest().as_slice(),
                ],
                |row| row.get(0),
            )?;
            if !tombstone_matches {
                return Err(StoreError::StoredTombstoneMismatch);
            }
            let digest = connection
                .query_row(
                    "SELECT stored_bytes_digest FROM store_gc_collection_occurrences
                     WHERE plan_id = ?1 AND object_id = ?2",
                    params![plan.id().as_bytes(), tombstone.object_id().as_bytes()],
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .optional()?
                .ok_or(StoreError::MissingCollectionOccurrence(
                    tombstone.object_id(),
                ))?;
            exact_digest(digest)
        })?;
        self.root.verify_file_removal_resolved(
            object_path(tombstone.object_id()),
            &destroyed_object_hash,
        )?;
        match self.root.read_immutable(object_path(tombstone.object_id())) {
            Err(SecureFsError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(StoreError::CollectedObjectStillPresent(
                    tombstone.object_id(),
                ));
            }
            Err(error) => return Err(StoreError::SecureFs(error)),
        }
        let receipt_hash = sha256(&deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Text("maestro.vnext.store-collection-physical-absence.v1".to_owned()),
            CborValue::Bytes(tombstone.id().as_bytes().to_vec()),
            CborValue::Bytes(plan.id().as_bytes().to_vec()),
            CborValue::Bytes(tombstone.object_id().as_bytes().to_vec()),
            CborValue::Bytes(destroyed_object_hash.to_vec()),
        ]))?);
        Ok(VerifiedCollectionAbsenceV1 {
            tombstone_id: tombstone.id(),
            collection_plan_id: plan.id(),
            destroyed_object_hash,
            receipt_hash,
        })
    }

    pub(crate) fn verify_recorded_collection_absence(
        &self,
        object_id: StoreObjectIdV1,
        tombstone_id: crate::domain::vnext::identity::LogicalTombstoneIdV1,
        collection_plan_id: crate::domain::vnext::identity::CollectionPlanIdV1,
        expected_destroyed_hash: [u8; 32],
        expected_receipt_hash: [u8; 32],
    ) -> Result<(), StoreError> {
        self.root.verify_path_binding()?;
        let destroyed_hash = self.with_verified_read(|connection| {
            let tombstone_matches: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM store_logical_tombstones
                    WHERE tombstone_id = ?1 AND object_id = ?2
                )",
                params![tombstone_id.as_bytes(), object_id.as_bytes()],
                |row| row.get(0),
            )?;
            if !tombstone_matches {
                return Err(StoreError::StoredTombstoneMismatch);
            }
            connection
                .query_row(
                    "SELECT stored_bytes_digest FROM store_gc_collection_occurrences
                     WHERE plan_id = ?1 AND object_id = ?2",
                    params![collection_plan_id.as_bytes(), object_id.as_bytes()],
                    |row| row.get::<_, Vec<u8>>(0),
                )
                .optional()?
                .map(exact_digest)
                .transpose()?
                .ok_or(StoreError::MissingCollectionOccurrence(object_id))
        })?;
        if destroyed_hash != expected_destroyed_hash {
            return Err(StoreError::StoredObjectMismatch(object_id));
        }
        self.root
            .verify_file_removal_resolved(object_path(object_id), &destroyed_hash)?;
        match self.root.read_immutable(object_path(object_id)) {
            Err(SecureFsError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => return Err(StoreError::CollectedObjectStillPresent(object_id)),
            Err(error) => return Err(StoreError::SecureFs(error)),
        }
        let receipt_hash = sha256(&deterministic_cbor::encode(&CborValue::Array(vec![
            CborValue::Text("maestro.vnext.store-collection-physical-absence.v1".to_owned()),
            CborValue::Bytes(tombstone_id.as_bytes().to_vec()),
            CborValue::Bytes(collection_plan_id.as_bytes().to_vec()),
            CborValue::Bytes(object_id.as_bytes().to_vec()),
            CborValue::Bytes(destroyed_hash.to_vec()),
        ]))?);
        if receipt_hash != expected_receipt_hash {
            return Err(StoreError::StoredObjectMismatch(object_id));
        }
        Ok(())
    }

    pub fn seal_export(&mut self) -> Result<SealedBackupV1, StoreError> {
        self.seal_export_with_before_receipt(|| {})
    }

    fn seal_export_with_before_receipt(
        &mut self,
        before_receipt: impl FnOnce(),
    ) -> Result<SealedBackupV1, StoreError> {
        self.root.verify_path_binding()?;
        let snapshot = self.snapshot_reachability()?;
        let (lineage, pins, object_rows, head_revision, cut_publication_clock, snapshot_root) =
            self.with_verified_read(|connection| {
                let active = active_head_row(connection)?.ok_or(StoreError::MissingActiveHead)?;
                if active.0 != snapshot.head_id()
                    || retention_revision(connection)? != snapshot.retention_revision()
                {
                    return Err(StoreError::SnapshotBasisMismatch);
                }
                let lineage = load_lineage(connection, snapshot.head_id(), &self.domain)?;
                let pins = active_pins(connection)?;
                let tombstones = tombstones_by_object(connection)?;
                let object_ids = authoritative_object_ids(connection)?;
                ensure_no_controlled_copy_erasure_barriers(&self.root, object_ids.iter().copied())?;
                let mut object_rows = Vec::with_capacity(object_ids.len());
                for object_id in object_ids {
                    object_rows.push(ExportObjectRow {
                        object_id,
                        metadata: object_metadata(connection, object_id)?
                            .ok_or(StoreError::UnknownObject(object_id))?,
                        references: object_references(connection, object_id)?,
                        tombstone: tombstones.get(&object_id).cloned(),
                    });
                }
                let snapshot_root =
                    StoreSnapshotRootV1::capture(connection, &self.domain, |object_id| {
                        self.root
                            .read_immutable(object_path(object_id))
                            .map_err(|error| SnapshotError::ObjectRead(error.to_string()))
                    })?;
                let cut_publication_clock = publication_clock(connection)?;
                Ok((
                    lineage,
                    pins,
                    object_rows,
                    active.1,
                    cut_publication_clock,
                    snapshot_root,
                ))
            })?;
        let mut entries = Vec::with_capacity(object_rows.len());
        for row in object_rows {
            if let Some(tombstone) = row.tombstone {
                entries.push(SealedExportEntryV1::Tombstoned(TombstonedObjectV1::new(
                    tombstone,
                    row.metadata.schema_id,
                    u64::try_from(row.metadata.logical_byte_length)
                        .map_err(|_| StoreError::InvalidMetadataInteger)?,
                    u64::try_from(row.metadata.stored_byte_length)
                        .map_err(|_| StoreError::InvalidMetadataInteger)?,
                    row.metadata.stored_bytes_digest,
                    row.metadata.storage_codec,
                    row.metadata.key_envelope,
                    row.references,
                )?));
            } else {
                entries.push(SealedExportEntryV1::Available(
                    self.read_export_object(&row)?,
                ));
            }
        }
        let prior_closures = self.prior_snapshot_closures(snapshot_root.prior_root_ids())?;
        let snapshot_blocks = StoreSnapshotBlockClosureV2::build(&snapshot_root, &prior_closures)
            .map_err(|error| StoreError::SnapshotBlocks(error.to_string()))?;
        let export =
            SealedExportV1::new_full_history(lineage, snapshot, pins, entries, snapshot_blocks)?;
        let committed_publication_clock = cut_publication_clock
            .checked_add(1)
            .ok_or(StoreError::InvalidMetadataInteger)?;
        let receipt = BackupReceiptV1::for_committed_export(&export, committed_publication_clock)?;
        let backup = SealedBackupV1::new(export, receipt)?;
        let pending_path = export_pending_path(backup.export().id());
        before_receipt();
        let receipt_result = self.metadata.with_publication_transaction(
            &self.root,
            Some(cut_publication_clock),
            |transaction| {
                let export = backup.export();
                let receipt = backup.receipt();
                let snapshot_root = export
                    .snapshot_root()
                    .ok_or(MetadataError::FacadeIntegrityMismatch)?;
                expect_active_head(transaction, Some(export.head().id()), head_revision)?;
                expect_retention_revision(
                    transaction,
                    export.reachability().retention_revision(),
                )?;
                verify_export_source_basis(transaction, export)?;
                ensure_no_controlled_copy_erasure_barriers(
                    &self.root,
                    export.entries().iter().map(SealedExportEntryV1::object_id),
                )
                .map_err(|_| MetadataError::FacadeCasMismatch)?;
                match self
                    .root
                    .create_file_if_absent(&pending_path, backup.canonical_bytes())?
                {
                    CreateIfAbsent::Created => {}
                    CreateIfAbsent::AlreadyExists => {
                        self.root
                            .read_exact(&pending_path, backup.canonical_bytes())?;
                    }
                }
                run_after_pending_sealed_export_test_hook();
                transaction.execute(
                    "INSERT OR IGNORE INTO store_sealed_exports
                   (export_id, head_id, generation_id, snapshot_id, schema_manifest_id,
                    family_manifest_set_digest, snapshot_root_id, source_publication_clock,
                    committed_publication_clock, payload_set_digest, backup_receipt_id,
                    export_byte_length, export_bytes_digest, export_format, carrier_format)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    params![
                        export.id().as_bytes(),
                        export.head().id().as_bytes(),
                        export.generation().id().as_bytes(),
                        export.reachability().id().as_bytes(),
                        snapshot_root.schema_manifest_id().as_bytes(),
                        snapshot_root.family_manifest_set_digest().as_slice(),
                        snapshot_root.id().as_bytes(),
                        to_i64(snapshot_root.publication_clock())?,
                        to_i64(receipt.committed_publication_clock())?,
                        snapshot_root.payload_set_digest().as_slice(),
                        receipt.id().as_bytes(),
                        to_i64(export.canonical_bytes().len() as u64)?,
                        sha256(export.canonical_bytes()).as_slice(),
                        SEALED_EXPORT_FORMAT_V2,
                        SEALED_BACKUP_FORMAT_V1,
                ],
            )?;
            for pin in export.retention_pins() {
                transaction.execute(
                    "INSERT OR IGNORE INTO store_sealed_export_pins(export_id, pin_id) VALUES (?1, ?2)",
                    params![export.id().as_bytes(), pin.id().as_bytes()],
                )?;
            }
            for entry in export.entries() {
                let kind = match entry {
                    SealedExportEntryV1::Available(_) => "available",
                    SealedExportEntryV1::Tombstoned(_) => "tombstoned",
                };
                transaction.execute(
                    "INSERT OR IGNORE INTO store_sealed_export_objects(export_id, object_id, entry_kind)
                     VALUES (?1, ?2, ?3)",
                    params![export.id().as_bytes(), entry.object_id().as_bytes(), kind],
                )?;
            }
                advance_retention_revision(
                    transaction,
                    Some(export.reachability().retention_revision()),
                )?;
                verify_export_metadata(transaction, &backup)
            },
        );
        if let Err(error) = receipt_result {
            if error.publication_may_have_committed() {
                return Err(StoreError::BackupPublicationRecoveryRequired {
                    export_id: backup.export().id(),
                    reason: error.to_string(),
                });
            }
            let receipt_error = error.into();
            self.cleanup_failed_sealed_export(&backup)?;
            return Err(receipt_error);
        }
        run_after_successful_sealed_export_receipt_test_hook();
        if let Err(error) = self.finish_backup_publication(&backup) {
            return Err(StoreError::BackupPublicationRecoveryRequired {
                export_id: backup.export().id(),
                reason: error.to_string(),
            });
        }
        Ok(backup)
    }

    fn cleanup_failed_sealed_export(&mut self, backup: &SealedBackupV1) -> Result<(), StoreError> {
        let export_id = backup.export().id();
        let pending_path = export_pending_path(export_id);
        let root = &self.root;
        match self
            .metadata
            .with_prepared_transaction(root, |transaction| {
                let committed: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM store_sealed_exports WHERE export_id = ?1)",
                    params![export_id.as_bytes()],
                    |row| row.get(0),
                )?;
                if !committed {
                    root.remove_file_if_matches(&pending_path, backup.canonical_bytes())
                        .map_err(ConditionalTransactionError::Operation)?;
                }
                Ok(PublicationMutation::NoChange(()))
            }) {
            Ok(()) => Ok(()),
            Err(ConditionalTransactionError::Metadata(error)) => Err(error.into()),
            Err(ConditionalTransactionError::Operation(error)) => Err(error.into()),
        }
    }

    fn finish_backup_publication(&mut self, backup: &SealedBackupV1) -> Result<(), StoreError> {
        let pending_path = export_pending_path(backup.export().id());
        let public_path = export_path(backup.export().id());
        let closure = backup
            .export()
            .snapshot_blocks()
            .ok_or(StoreError::LegacyPartialExport)?;
        let root = &self.root;
        match self
            .metadata
            .with_prepared_transaction(root, |transaction| {
                let committed: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM store_sealed_exports WHERE export_id = ?1)",
                    params![backup.export().id().as_bytes()],
                    |row| row.get(0),
                )?;
                if !committed {
                    return Err(ConditionalTransactionError::Operation(
                        StoreError::StoredControlledCopyMismatch,
                    ));
                }
                ensure_no_controlled_copy_erasure_barriers(
                    root,
                    backup
                        .export()
                        .entries()
                        .iter()
                        .map(SealedExportEntryV1::object_id),
                )
                .map_err(ConditionalTransactionError::Operation)?;
                persist_snapshot_closure_with_root(root, closure)
                    .map_err(ConditionalTransactionError::Operation)?;
                match root.rename_file_no_replace(&pending_path, &public_path) {
                    Ok(CreateIfAbsent::Created) => {}
                    Ok(CreateIfAbsent::AlreadyExists) => {
                        root.read_exact(&public_path, backup.canonical_bytes())
                            .map_err(|error| {
                                ConditionalTransactionError::Operation(error.into())
                            })?;
                        root.remove_file_if_matches(&pending_path, backup.canonical_bytes())
                            .map_err(|error| {
                                ConditionalTransactionError::Operation(error.into())
                            })?;
                    }
                    Err(error) if secure_fs_error_is_not_found(&error) => {
                        root.read_exact(&public_path, backup.canonical_bytes())
                            .map_err(|error| {
                                ConditionalTransactionError::Operation(error.into())
                            })?;
                    }
                    Err(error) => {
                        return Err(ConditionalTransactionError::Operation(error.into()));
                    }
                }
                Ok(PublicationMutation::NoChange(()))
            }) {
            Ok(()) => Ok(()),
            Err(ConditionalTransactionError::Metadata(error)) => Err(error.into()),
            Err(ConditionalTransactionError::Operation(error)) => Err(error),
        }
    }

    pub fn recover_sealed_export_publication(
        &mut self,
        export_id: SealedExportIdV1,
    ) -> Result<SealedBackupV1, StoreError> {
        let pending_path = export_pending_path(export_id);
        let public_path = export_path(export_id);
        let bytes = match self.root.read_immutable(&pending_path) {
            Ok(bytes) => bytes,
            Err(error) if secure_fs_error_is_not_found(&error) => {
                self.root.read_immutable(&public_path)?
            }
            Err(error) => return Err(error.into()),
        };
        let backup = SealedBackupV1::decode(&bytes)?;
        if backup.export().id() != export_id {
            return Err(StoreError::NonCanonicalCarrier);
        }
        self.with_verified_read(|connection| {
            verify_export_metadata(connection, &backup)?;
            Ok(())
        })?;
        self.finish_backup_publication(&backup)?;
        Ok(backup)
    }

    fn read_export_object(&self, row: &ExportObjectRow) -> Result<StoreObjectV1, StoreError> {
        if row.metadata.storage_codec != STORE_OBJECT_STORAGE_CODEC_V1 {
            return Err(StoreError::UnsupportedStorageCodec(
                row.metadata.storage_codec.clone(),
            ));
        }
        let bytes = self.root.read_immutable(object_path(row.object_id))?;
        if bytes.len() != row.metadata.stored_byte_length
            || bytes.len() != row.metadata.logical_byte_length
            || sha256(&bytes) != row.metadata.stored_bytes_digest
        {
            return Err(StoreError::StoredObjectMismatch(row.object_id));
        }
        let object = StoreObjectV1::decode(&bytes)?;
        if object.id() != row.object_id
            || object.schema_id() != row.metadata.schema_id
            || object.references() != row.references
        {
            return Err(StoreError::StoredObjectMismatch(row.object_id));
        }
        Ok(object)
    }

    pub fn import_inactive(&mut self, bytes: &[u8]) -> Result<RestoreCandidateV1, StoreError> {
        self.root.verify_path_binding()?;
        let (store_state, state_revision) = self.state()?;
        if store_state != StoreStateV1::Inactive {
            return Err(StoreError::ImportRequiresInactiveStore);
        }
        let backup = SealedBackupV1::decode(bytes)?;
        let export = backup.export();
        let resumed_publication_clock = to_i64(
            backup
                .receipt()
                .committed_publication_clock()
                .checked_add(1)
                .ok_or(StoreError::InvalidMetadataInteger)?,
        )?;
        if export.generation().domain() != &self.domain {
            return Err(StoreError::DomainMismatch);
        }
        if !export.generation().compatibility().is_stage0_successor() {
            return Err(StoreError::IncompatibleGeneration);
        }
        let snapshot_root = export
            .snapshot_root()
            .ok_or(StoreError::LegacyPartialExport)?;
        let snapshot_blocks = export
            .snapshot_blocks()
            .ok_or(StoreError::LegacyPartialExport)?;
        if snapshot_root.role() != self.role() || snapshot_root.domain_id() != self.domain.id() {
            return Err(StoreError::DomainMismatch);
        }
        let candidate = RestoreCandidateV1::for_verified_export(export, self.domain.id())?;
        let (has_heads, active) = self.with_verified_read(|connection| {
            let has_heads =
                connection.query_row("SELECT EXISTS(SELECT 1 FROM store_heads)", [], |row| {
                    row.get(0)
                })?;
            Ok((has_heads, active_head_row(connection)?))
        })?;
        if active.is_some() {
            return Err(StoreError::RestoreRequiresEmptyStore);
        }
        if has_heads {
            let existing = self.with_verified_read(|connection| {
                let candidate_exists: bool = connection.query_row(
                    "SELECT EXISTS(
                         SELECT 1 FROM store_restore_candidates WHERE candidate_id = ?1
                     )",
                    params![candidate.id().as_bytes()],
                    |row| row.get(0),
                )?;
                candidate_exists
                    .then(|| load_restore_candidate(connection, candidate.id()))
                    .transpose()
            })?;
            if let Some(existing) = existing {
                self.root.read_exact(export_path(export.id()), bytes)?;
                return Ok(existing);
            }
            return Err(StoreError::RestoreRequiresEmptyStore);
        }
        let available_objects = snapshot_root
            .object_blobs()
            .iter()
            .map(|(declared_id, bytes)| {
                let object = StoreObjectV1::decode(bytes)?;
                if object.id().as_bytes().as_slice() != declared_id {
                    return Err(StoreError::StoredObjectMismatch(object.id()));
                }
                Ok(object)
            })
            .collect::<Result<Vec<_>, StoreError>>()?;
        let export_path = export_path(export.id());
        match self.root.create_file_if_absent(&export_path, bytes)? {
            CreateIfAbsent::Created => {}
            CreateIfAbsent::AlreadyExists => {
                self.root.read_exact(&export_path, bytes)?;
            }
        }
        for prior_root_id in snapshot_root.prior_root_ids() {
            let prior_closure = snapshot_blocks
                .subclosure(prior_root_id)
                .map_err(|error| StoreError::SnapshotBlocks(error.to_string()))?;
            self.persist_snapshot_closure(&prior_closure)?;
        }
        self.persist_snapshot_closure(snapshot_blocks)?;
        self.persist_export_files(&available_objects)?;

        self.metadata.with_restore_transaction(
            &self.root,
            resumed_publication_clock,
            |transaction| {
                let (state, observed_revision): (String, i64) = transaction.query_row(
                    "SELECT state, state_revision FROM store_state WHERE singleton = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                if state != "inactive" || observed_revision != state_revision as i64 {
                    return Err(MetadataError::FacadeCasMismatch);
                }
                expect_active_head(transaction, None, 0)?;
                snapshot_root
                    .restore_history_inactive(
                        transaction,
                        u64::try_from(resumed_publication_clock)
                            .map_err(|_| MetadataError::FacadeIntegrityMismatch)?,
                    )
                    .map_err(|error| MetadataError::SnapshotRestore(error.to_string()))?;
                transaction.execute(
                    "INSERT INTO store_sealed_exports
                   (export_id, head_id, generation_id, snapshot_id, schema_manifest_id,
                    family_manifest_set_digest, snapshot_root_id, source_publication_clock,
                    committed_publication_clock, payload_set_digest, backup_receipt_id,
                    export_byte_length, export_bytes_digest, export_format, carrier_format)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    params![
                        export.id().as_bytes(),
                        export.head().id().as_bytes(),
                        export.generation().id().as_bytes(),
                        export.reachability().id().as_bytes(),
                        snapshot_root.schema_manifest_id().as_bytes(),
                        snapshot_root.family_manifest_set_digest().as_slice(),
                        snapshot_root.id().as_bytes(),
                        to_i64(snapshot_root.publication_clock())?,
                        to_i64(backup.receipt().committed_publication_clock())?,
                        snapshot_root.payload_set_digest().as_slice(),
                        backup.receipt().id().as_bytes(),
                        to_i64(export.canonical_bytes().len() as u64)?,
                        sha256(export.canonical_bytes()).as_slice(),
                        SEALED_EXPORT_FORMAT_V2,
                        SEALED_BACKUP_FORMAT_V1,
                    ],
                )?;
                for pin in export.retention_pins() {
                    transaction.execute(
                        "INSERT INTO store_sealed_export_pins(export_id, pin_id) VALUES (?1, ?2)",
                        params![export.id().as_bytes(), pin.id().as_bytes()],
                    )?;
                }
                for entry in export.entries() {
                    let kind = match entry {
                        SealedExportEntryV1::Available(_) => "available",
                        SealedExportEntryV1::Tombstoned(_) => "tombstoned",
                    };
                    transaction.execute(
                        "INSERT INTO store_sealed_export_objects(export_id, object_id, entry_kind)
                     VALUES (?1, ?2, ?3)",
                        params![export.id().as_bytes(), entry.object_id().as_bytes(), kind],
                    )?;
                }
                transaction.execute(
                    "INSERT INTO store_restore_candidates
                   (candidate_id, source_export_id, source_domain_id, source_export_bytes_digest,
                    source_schema_manifest_id, source_snapshot_root_id, destination_domain_id,
                    candidate_generation_id, candidate_head_id, candidate_snapshot_id,
                    verification_digest)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        candidate.id().as_bytes(),
                        candidate.source_export_id().as_bytes(),
                        candidate.source_domain_id().as_bytes(),
                        candidate.source_export_bytes_digest().as_slice(),
                        snapshot_root.schema_manifest_id().as_bytes(),
                        snapshot_root.id().as_bytes(),
                        candidate.destination_domain_id().as_bytes(),
                        candidate.candidate_generation_id().as_bytes(),
                        candidate.candidate_head_id().as_bytes(),
                        candidate.candidate_snapshot_id().as_bytes(),
                        candidate.verification_digest().as_slice(),
                    ],
                )?;
                for (position, root) in candidate.candidate_roots().iter().enumerate() {
                    transaction.execute(
                        "INSERT INTO store_restore_candidate_roots
                     (candidate_id, root_position, object_id) VALUES (?1, ?2, ?3)",
                        params![candidate.id().as_bytes(), position as i64, root.as_bytes()],
                    )?;
                }
                advance_retention_revision(transaction, None)?;
                Ok(())
            },
        )?;
        Ok(candidate)
    }

    pub fn restore_candidate(
        &self,
        candidate_id: RestoreCandidateIdV1,
    ) -> Result<RestoreCandidateV1, StoreError> {
        self.with_verified_read(|connection| load_restore_candidate(connection, candidate_id))
    }

    fn prior_snapshot_closures(
        &self,
        required_root_ids: &[[u8; 32]],
    ) -> Result<Vec<StoreSnapshotBlockClosureV2>, StoreError> {
        let mut unique_root_ids = BTreeSet::new();
        for root_id in required_root_ids {
            if !unique_root_ids.insert(*root_id) {
                return Err(StoreError::DuplicatePriorSnapshotClosureRoot);
            }
        }
        if unique_root_ids.len() > MAX_REFERENCED_PRIOR_ROOTS_V1 {
            return Err(StoreError::PriorSnapshotClosureCountLimitExceeded {
                limit: MAX_REFERENCED_PRIOR_ROOTS_V1,
            });
        }

        let mut aggregate_bytes = 0_usize;
        let mut closures = Vec::with_capacity(required_root_ids.len());
        for root_id in required_root_ids {
            let bytes = self.root.read_immutable(snapshot_closure_path(root_id))?;
            aggregate_bytes = aggregate_bytes.checked_add(bytes.len()).ok_or(
                StoreError::PriorSnapshotClosureBytesLimitExceeded {
                    limit: MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2,
                },
            )?;
            if aggregate_bytes > MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2 {
                return Err(StoreError::PriorSnapshotClosureBytesLimitExceeded {
                    limit: MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2,
                });
            }
            let closure = StoreSnapshotBlockClosureV2::decode(&bytes)
                .map_err(|error| StoreError::SnapshotBlocks(error.to_string()))?;
            if closure.current_root_id() != root_id {
                return Err(StoreError::SnapshotBlocks(
                    "snapshot closure path does not bind its declared root".to_owned(),
                ));
            }
            closures.push(
                closure
                    .subclosure(root_id)
                    .map_err(|error| StoreError::SnapshotBlocks(error.to_string()))?,
            );
        }
        Ok(closures)
    }

    fn persist_snapshot_closure(
        &self,
        closure: &StoreSnapshotBlockClosureV2,
    ) -> Result<(), StoreError> {
        persist_snapshot_closure_with_root(&self.root, closure)
    }

    fn persist_export_files(&mut self, objects: &[StoreObjectV1]) -> Result<(), StoreError> {
        let mut identities = BTreeSet::new();
        for object in objects {
            if !identities.insert(object.id()) {
                return Err(StoreError::DuplicateObject(object.id()));
            }
            if StoreObjectV1::decode(object.canonical_bytes())? != *object {
                return Err(StoreError::NonCanonicalCarrier);
            }
            let path = object_path(object.id());
            let parent = path
                .parent()
                .expect("invariant: canonical object path has a parent");
            self.root.create_dir_all(parent)?;
            match self
                .root
                .create_file_if_absent(&path, object.canonical_bytes())?
            {
                CreateIfAbsent::Created => {}
                CreateIfAbsent::AlreadyExists => {
                    self.root.read_exact(&path, object.canonical_bytes())?;
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn persist_objects(&mut self, objects: &[StoreObjectV1]) -> Result<(), StoreError> {
        let mut identities = BTreeSet::new();
        for object in objects {
            if !identities.insert(object.id()) {
                return Err(StoreError::DuplicateObject(object.id()));
            }
            if StoreObjectV1::decode(object.canonical_bytes())? != *object {
                return Err(StoreError::NonCanonicalCarrier);
            }
            let path = object_path(object.id());
            let parent = path
                .parent()
                .expect("invariant: canonical object path has a parent");
            self.root.create_dir_all(parent)?;
            match self
                .root
                .create_file_if_absent(&path, object.canonical_bytes())?
            {
                CreateIfAbsent::Created => {}
                CreateIfAbsent::AlreadyExists => {
                    self.root.read_exact(&path, object.canonical_bytes())?;
                }
            }
        }
        for object in objects {
            for reference in object.references() {
                if !identities.contains(reference) {
                    self.read_object(*reference)?;
                }
            }
        }

        self.metadata
            .with_immediate_transaction(&self.root, |transaction| {
                for object in objects {
                    insert_object_metadata(transaction, object)?;
                }
                for object in objects {
                    insert_or_verify_references(transaction, object)?;
                }
                Ok(())
            })?;
        Ok(())
    }

    fn reachability_roots_and_objects(
        &self,
        head_id: StoreHeadIdV1,
    ) -> Result<(Vec<RetentionRootV1>, BTreeSet<StoreObjectIdV1>), StoreError> {
        self.with_verified_read(|connection| {
            let head = load_head(connection, head_id, &self.domain)?;
            let generation = load_generation(connection, head.generation_id(), &self.domain)?;
            let mut roots = generation
                .roots()
                .iter()
                .map(|object_id| {
                    RetentionRootV1::new(RetentionRootKindV1::ActiveGeneration, *object_id)
                })
                .collect::<Vec<_>>();
            roots.extend(
                active_pins(connection)?
                    .into_iter()
                    .map(|pin| pin.root().clone()),
            );
            roots.extend(idempotency_replay_roots(connection)?);
            roots.sort();
            roots.dedup();

            let reachable = walk_object_closure_with_limit(
                roots.iter().map(RetentionRootV1::object_id),
                MAX_OBJECT_CLOSURE_ENTRIES,
                |object_id| {
                    Ok(self
                        .read_object_with(connection, object_id)?
                        .references()
                        .to_vec())
                },
            )?;
            Ok((roots, reachable))
        })
    }
}

fn walk_object_closure_with_limit(
    roots: impl IntoIterator<Item = StoreObjectIdV1>,
    limit: usize,
    mut references: impl FnMut(StoreObjectIdV1) -> Result<Vec<StoreObjectIdV1>, StoreError>,
) -> Result<BTreeSet<StoreObjectIdV1>, StoreError> {
    let mut discovered = BTreeSet::new();
    let mut frontier = Vec::new();
    for root in roots {
        if discovered.insert(root) {
            if discovered.len() > limit {
                return Err(StoreError::ObjectClosureLimitExceeded { limit });
            }
            frontier.push(root);
        }
    }
    while let Some(object_id) = frontier.pop() {
        for referenced in references(object_id)? {
            if discovered.insert(referenced) {
                if discovered.len() > limit {
                    return Err(StoreError::ObjectClosureLimitExceeded { limit });
                }
                frontier.push(referenced);
            }
        }
    }
    Ok(discovered)
}

fn state(connection: &Connection) -> Result<(StoreStateV1, u64), StoreError> {
    let (state, revision): (String, i64) = connection.query_row(
        "SELECT state, state_revision FROM store_state WHERE singleton = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let state = match state.as_str() {
        "inactive" => StoreStateV1::Inactive,
        "active" => StoreStateV1::Active,
        _ => return Err(StoreError::InvalidStoreState),
    };
    Ok((state, to_u64(revision)?))
}

fn active_head_row(connection: &Connection) -> Result<Option<(StoreHeadIdV1, u64)>, StoreError> {
    connection
        .query_row(
            "SELECT head_id, head_revision FROM store_active_head WHERE singleton = 1",
            [],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
        .map(|(id, revision)| Ok((identity(id)?, to_u64(revision)?)))
        .transpose()
}

fn expect_active_head(
    transaction: &Transaction<'_>,
    expected_id: Option<StoreHeadIdV1>,
    expected_revision: u64,
) -> Result<(), MetadataError> {
    let observed = transaction
        .query_row(
            "SELECT head_id, head_revision FROM store_active_head WHERE singleton = 1",
            [],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let matches = match (observed, expected_id) {
        (None, None) => expected_revision == 0,
        (Some((id, revision)), Some(expected)) => {
            id.as_slice() == expected.as_bytes() && revision == expected_revision as i64
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(MetadataError::FacadeCasMismatch)
    }
}

fn publication_basis(
    observed: Option<(StoreHeadIdV1, u64)>,
    expected_old: Option<StoreHeadIdV1>,
    generation: &StoreGenerationV1,
    domain: &StoreDomainV1,
    load_head_by_id: impl FnOnce(StoreHeadIdV1) -> Result<StoreHeadV1, StoreError>,
) -> Result<(u64, Option<StoreHeadIdV1>), StoreError> {
    match observed {
        None => {
            if expected_old.is_some()
                || generation.ordinal() != 1
                || generation.previous().is_some()
                || generation.domain() != domain
            {
                return Err(StoreError::HeadCasMismatch);
            }
            Ok((0, None))
        }
        Some((head_id, revision)) => {
            if expected_old != Some(head_id) || generation.ordinal() != revision + 1 {
                return Err(StoreError::HeadCasMismatch);
            }
            let prior = load_head_by_id(head_id)?;
            if generation.previous() != Some(prior.generation_id()) {
                return Err(StoreError::HeadCasMismatch);
            }
            Ok((revision, Some(head_id)))
        }
    }
}

fn read_object_with_root(
    root: &SecureRoot,
    connection: &Connection,
    object_id: StoreObjectIdV1,
) -> Result<StoreObjectV1, StoreError> {
    if object_is_tombstoned(connection, object_id)? {
        return Err(StoreError::TombstonedObject(object_id));
    }
    if collection_occurrence_digest(connection, object_id)?.is_some() {
        return Err(StoreError::CollectedObject(object_id));
    }
    read_stored_object_with_root(root, connection, object_id)
}

fn read_stored_object_with_root(
    root: &SecureRoot,
    connection: &Connection,
    object_id: StoreObjectIdV1,
) -> Result<StoreObjectV1, StoreError> {
    let metadata =
        object_metadata(connection, object_id)?.ok_or(StoreError::UnknownObject(object_id))?;
    if metadata.storage_codec != STORE_OBJECT_STORAGE_CODEC_V1 {
        return Err(StoreError::UnsupportedStorageCodec(metadata.storage_codec));
    }
    let bytes = root.read_immutable(object_path(object_id))?;
    if bytes.len() != metadata.stored_byte_length
        || bytes.len() != metadata.logical_byte_length
        || sha256(&bytes) != metadata.stored_bytes_digest
    {
        return Err(StoreError::StoredObjectMismatch(object_id));
    }
    let object = StoreObjectV1::decode(&bytes)?;
    if object.id() != object_id || object.schema_id() != metadata.schema_id {
        return Err(StoreError::StoredObjectMismatch(object_id));
    }
    let references = object_references(connection, object_id)?;
    if references != object.references() {
        return Err(StoreError::StoredReferenceMismatch(object_id));
    }
    Ok(object)
}

fn load_idempotency(
    connection: &Connection,
    namespace: &str,
    key_digest: &[u8; 32],
) -> Result<Option<StoredIdempotencyV1>, StoreError> {
    let row = connection
        .query_row(
            "SELECT meaning_digest, result_object_id, generation_id, head_id
             FROM store_idempotency WHERE namespace = ?1 AND key_digest = ?2",
            params![namespace, key_digest.as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            },
        )
        .optional()?;
    row.map(|(meaning, result, generation, head)| {
        Ok(StoredIdempotencyV1 {
            meaning_digest: exact_digest(meaning)?,
            result_object_id: identity(result)?,
            generation_id: identity(generation)?,
            head_id: identity(head)?,
        })
    })
    .transpose()
}

fn load_idempotency_metadata(
    connection: &Connection,
    namespace: &str,
    key_digest: &[u8; 32],
) -> Result<Option<StoredIdempotencyV1>, MetadataError> {
    let row = connection
        .query_row(
            "SELECT meaning_digest, result_object_id, generation_id, head_id
             FROM store_idempotency WHERE namespace = ?1 AND key_digest = ?2",
            params![namespace, key_digest.as_slice()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            },
        )
        .optional()?;
    row.map(|(meaning, result, generation, head)| {
        let meaning_digest: [u8; 32] = meaning
            .try_into()
            .map_err(|_| MetadataError::FacadeIntegrityMismatch)?;
        let result_object_id = ManifestIdentityV1::from_digest(
            result
                .try_into()
                .map_err(|_| MetadataError::FacadeIntegrityMismatch)?,
        );
        let generation_id = ManifestIdentityV1::from_digest(
            generation
                .try_into()
                .map_err(|_| MetadataError::FacadeIntegrityMismatch)?,
        );
        let head_id = ManifestIdentityV1::from_digest(
            head.try_into()
                .map_err(|_| MetadataError::FacadeIntegrityMismatch)?,
        );
        Ok(StoredIdempotencyV1 {
            meaning_digest,
            result_object_id,
            generation_id,
            head_id,
        })
    })
    .transpose()
}

fn retention_revision(connection: &Connection) -> Result<u64, StoreError> {
    let revision: i64 = connection.query_row(
        "SELECT retention_revision FROM store_retention_revision WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    to_u64(revision)
}

fn publication_clock(connection: &Connection) -> Result<u64, StoreError> {
    let clock: i64 = connection.query_row(
        "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    to_u64(clock)
}

fn expect_retention_revision(
    transaction: &Transaction<'_>,
    expected: u64,
) -> Result<(), MetadataError> {
    let actual: i64 = transaction.query_row(
        "SELECT retention_revision FROM store_retention_revision WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    if actual == expected as i64 {
        Ok(())
    } else {
        Err(MetadataError::FacadeCasMismatch)
    }
}

fn advance_retention_revision(
    transaction: &Transaction<'_>,
    expected: Option<u64>,
) -> Result<u64, MetadataError> {
    let actual: i64 = transaction.query_row(
        "SELECT retention_revision FROM store_retention_revision WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?;
    if expected.is_some_and(|expected| actual != expected as i64) {
        return Err(MetadataError::FacadeCasMismatch);
    }
    let next = actual
        .checked_add(1)
        .ok_or(MetadataError::FacadeCasMismatch)?;
    let changed = transaction.execute(
        "UPDATE store_retention_revision SET retention_revision = ?1
         WHERE singleton = 1 AND retention_revision = ?2",
        params![next, actual],
    )?;
    if changed != 1 {
        return Err(MetadataError::FacadeCasMismatch);
    }
    u64::try_from(next).map_err(|_| MetadataError::FacadeCasMismatch)
}

fn insert_object_metadata(
    transaction: &Transaction<'_>,
    object: &StoreObjectV1,
) -> Result<(), MetadataError> {
    let existing = transaction
        .query_row(
            "SELECT schema_id, logical_byte_length, stored_byte_length,
                    stored_bytes_digest, storage_codec
             FROM store_objects WHERE object_id = ?1",
            params![object.id().as_bytes()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;
    let length = object.canonical_bytes().len() as i64;
    let digest = sha256(object.canonical_bytes());
    if let Some((schema, logical, stored, existing_digest, codec)) = existing {
        if schema.as_slice() != object.schema_id().as_bytes()
            || logical != length
            || stored != length
            || existing_digest.as_slice() != digest
            || codec != STORE_OBJECT_STORAGE_CODEC_V1
        {
            return Err(MetadataError::FacadeIntegrityMismatch);
        }
        return Ok(());
    }
    transaction.execute(
        "INSERT INTO store_objects
         (object_id, schema_id, logical_byte_length, stored_byte_length,
          stored_bytes_digest, storage_codec, key_envelope_id, key_envelope_kind)
         VALUES (?1, ?2, ?3, ?3, ?4, ?5, NULL, NULL)",
        params![
            object.id().as_bytes(),
            object.schema_id().as_bytes(),
            length,
            digest.as_slice(),
            STORE_OBJECT_STORAGE_CODEC_V1,
        ],
    )?;
    Ok(())
}

fn insert_or_verify_references(
    transaction: &Transaction<'_>,
    object: &StoreObjectV1,
) -> Result<(), MetadataError> {
    let mut statement = transaction.prepare(
        "SELECT referenced_object_id FROM store_object_references
         WHERE object_id = ?1 ORDER BY reference_position",
    )?;
    let existing = statement
        .query_map(params![object.id().as_bytes()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if !existing.is_empty() {
        if existing.len() != object.references().len()
            || existing
                .iter()
                .zip(object.references())
                .any(|(stored, expected)| stored.as_slice() != expected.as_bytes())
        {
            return Err(MetadataError::FacadeIntegrityMismatch);
        }
        return Ok(());
    }
    for (position, reference) in object.references().iter().enumerate() {
        transaction.execute(
            "INSERT INTO store_object_references
             (object_id, reference_position, referenced_object_id) VALUES (?1, ?2, ?3)",
            params![
                object.id().as_bytes(),
                position as i64,
                reference.as_bytes()
            ],
        )?;
    }
    Ok(())
}

fn object_metadata(
    connection: &Connection,
    object_id: StoreObjectIdV1,
) -> Result<Option<ObjectMetadata>, StoreError> {
    connection
        .query_row(
            "SELECT schema_id, logical_byte_length, stored_byte_length,
                    stored_bytes_digest, storage_codec, key_envelope_id, key_envelope_kind
             FROM store_objects WHERE object_id = ?1",
            params![object_id.as_bytes()],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<Vec<u8>>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()?
        .map(
            |(schema, logical, stored, digest, codec, envelope_id, envelope_kind)| {
                let key_envelope = match (envelope_id, envelope_kind) {
                    (None, None) => None,
                    (Some(id), Some(kind)) => Some((exact_digest(id)?, kind)),
                    _ => return Err(StoreError::StoredObjectMismatch(object_id)),
                };
                Ok(ObjectMetadata {
                    schema_id: identity(schema)?,
                    logical_byte_length: usize::try_from(logical)
                        .map_err(|_| StoreError::InvalidMetadataInteger)?,
                    stored_byte_length: usize::try_from(stored)
                        .map_err(|_| StoreError::InvalidMetadataInteger)?,
                    stored_bytes_digest: exact_digest(digest)?,
                    storage_codec: codec,
                    key_envelope,
                })
            },
        )
        .transpose()
}

fn object_references(
    connection: &Connection,
    object_id: StoreObjectIdV1,
) -> Result<Vec<StoreObjectIdV1>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT referenced_object_id FROM store_object_references
         WHERE object_id = ?1 ORDER BY reference_position",
    )?;
    statement
        .query_map(params![object_id.as_bytes()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .map(|row| identity(row?))
        .collect()
}

fn authoritative_object_ids(connection: &Connection) -> Result<Vec<StoreObjectIdV1>, StoreError> {
    let mut statement =
        connection.prepare("SELECT object_id FROM store_objects ORDER BY object_id")?;
    statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
        .map(|row| identity(row?))
        .collect()
}

fn object_is_tombstoned(
    connection: &Connection,
    object_id: StoreObjectIdV1,
) -> Result<bool, StoreError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM store_logical_tombstones WHERE object_id = ?1)",
            params![object_id.as_bytes()],
            |row| row.get(0),
        )
        .map_err(StoreError::from)
}

fn collection_occurrence_digest(
    connection: &Connection,
    object_id: StoreObjectIdV1,
) -> Result<Option<[u8; 32]>, StoreError> {
    connection
        .query_row(
            "SELECT stored_bytes_digest FROM store_gc_collection_occurrences
             WHERE object_id = ?1 ORDER BY plan_id LIMIT 1",
            params![object_id.as_bytes()],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()?
        .map(exact_digest)
        .transpose()
}

fn insert_generation(
    transaction: &Transaction<'_>,
    generation: &StoreGenerationV1,
) -> Result<(), MetadataError> {
    let previous = generation.previous().map(ManifestIdentityV1::into_bytes);
    let compatibility = generation.compatibility();
    transaction.execute(
        "INSERT INTO store_generations
         (generation_id, generation_ordinal, previous_generation_id, contract_root_id,
          writer_compatibility_manifest_id, association_schema_id, finality_edge_manifest_id,
          schema_read_write_set_descriptor_id, writer_protocol_epoch_id, migration_epoch_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            generation.id().as_bytes(),
            to_i64_metadata(generation.ordinal())?,
            previous.as_ref().map(<[u8; 32]>::as_slice),
            generation.contract_root_id().as_bytes(),
            compatibility.writer_compatibility_manifest_id().as_bytes(),
            compatibility.association_schema_id().as_bytes(),
            compatibility.finality_edge_manifest_id().as_bytes(),
            compatibility
                .schema_read_write_set_descriptor_id()
                .as_bytes(),
            compatibility.writer_protocol_epoch_id().as_bytes(),
            compatibility.migration_epoch_id().as_bytes(),
        ],
    )?;
    for (position, root) in generation.roots().iter().enumerate() {
        transaction.execute(
            "INSERT INTO store_generation_roots(generation_id, root_position, object_id)
             VALUES (?1, ?2, ?3)",
            params![generation.id().as_bytes(), position as i64, root.as_bytes()],
        )?;
    }
    Ok(())
}

fn insert_head(transaction: &Transaction<'_>, head: &StoreHeadV1) -> Result<(), MetadataError> {
    let previous = head.previous_head_id().map(ManifestIdentityV1::into_bytes);
    transaction.execute(
        "INSERT INTO store_heads
         (head_id, generation_id, generation_ordinal, head_revision, previous_head_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            head.id().as_bytes(),
            head.generation_id().as_bytes(),
            to_i64_metadata(head.generation_ordinal())?,
            to_i64_metadata(head.revision())?,
            previous.as_ref().map(<[u8; 32]>::as_slice),
        ],
    )?;
    Ok(())
}

fn load_generation(
    connection: &Connection,
    generation_id: StoreGenerationIdV1,
    domain: &StoreDomainV1,
) -> Result<StoreGenerationV1, StoreError> {
    type GenerationRow = (
        i64,
        Option<Vec<u8>>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
    );
    let row: GenerationRow = connection.query_row(
        "SELECT generation_ordinal, previous_generation_id, contract_root_id,
                writer_compatibility_manifest_id, association_schema_id,
                finality_edge_manifest_id, schema_read_write_set_descriptor_id,
                writer_protocol_epoch_id, migration_epoch_id
         FROM store_generations WHERE generation_id = ?1",
        params![generation_id.as_bytes()],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        },
    )?;
    let mut statement = connection.prepare(
        "SELECT object_id FROM store_generation_roots
         WHERE generation_id = ?1 ORDER BY root_position",
    )?;
    let roots = statement
        .query_map(params![generation_id.as_bytes()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .map(|row| identity(row?))
        .collect::<Result<Vec<StoreObjectIdV1>, _>>()?;
    let generation = StoreGenerationV1::new(
        domain.clone(),
        to_u64(row.0)?,
        row.1.map(identity).transpose()?,
        identity(row.2)?,
        StoreCompatibilityV1::new(
            identity(row.3)?,
            identity(row.4)?,
            identity(row.5)?,
            identity(row.6)?,
            identity(row.7)?,
            identity(row.8)?,
        ),
        roots,
    )?;
    if !generation.compatibility().is_stage0_successor() {
        return Err(StoreError::IncompatibleGeneration);
    }
    Ok(generation)
}

fn load_unique_head_for_generation(
    connection: &Connection,
    generation_id: StoreGenerationIdV1,
    domain: &StoreDomainV1,
) -> Result<StoreHeadV1, StoreError> {
    let mut statement = connection
        .prepare("SELECT head_id FROM store_heads WHERE generation_id = ?1 ORDER BY head_id")?;
    let head_ids = statement
        .query_map(params![generation_id.as_bytes()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .map(|row| identity(row?))
        .collect::<Result<Vec<StoreHeadIdV1>, StoreError>>()?;
    let [head_id] = head_ids.as_slice() else {
        return Err(StoreError::StoredHeadMismatch);
    };
    load_head(connection, *head_id, domain)
}

fn generation_objects(
    root: &SecureRoot,
    connection: &Connection,
    generation: &StoreGenerationV1,
) -> Result<Vec<StoreObjectV1>, StoreError> {
    walk_object_closure_with_limit(
        generation.roots().iter().copied(),
        MAX_OBJECT_CLOSURE_ENTRIES,
        |object_id| {
            Ok(read_object_with_root(root, connection, object_id)?
                .references()
                .to_vec())
        },
    )?
    .into_iter()
    .map(|object_id| read_object_with_root(root, connection, object_id))
    .collect()
}

fn historical_generation_objects(
    root: &SecureRoot,
    connection: &Connection,
    generation: &StoreGenerationV1,
) -> Result<Vec<StoreObjectV1>, StoreError> {
    let object_ids = walk_object_closure_with_limit(
        generation.roots().iter().copied(),
        MAX_OBJECT_CLOSURE_ENTRIES,
        |object_id| match read_object_with_root(root, connection, object_id) {
            Ok(object) => Ok(object.references().to_vec()),
            Err(StoreError::TombstonedObject(_)) | Err(StoreError::CollectedObject(_)) => {
                object_references(connection, object_id)
            }
            Err(error) => Err(error),
        },
    )?;
    object_ids
        .into_iter()
        .filter_map(
            |object_id| match read_object_with_root(root, connection, object_id) {
                Ok(object) => Some(Ok(object)),
                Err(StoreError::TombstonedObject(_)) | Err(StoreError::CollectedObject(_)) => None,
                Err(error) => Some(Err(error)),
            },
        )
        .collect()
}

fn load_head(
    connection: &Connection,
    head_id: StoreHeadIdV1,
    domain: &StoreDomainV1,
) -> Result<StoreHeadV1, StoreError> {
    let (generation_bytes, revision, previous): (Vec<u8>, i64, Option<Vec<u8>>) = connection
        .query_row(
            "SELECT generation_id, head_revision, previous_head_id FROM store_heads WHERE head_id = ?1",
            params![head_id.as_bytes()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    let generation = load_generation(connection, identity(generation_bytes)?, domain)?;
    let head = StoreHeadV1::new(
        &generation,
        to_u64(revision)?,
        previous.map(identity).transpose()?,
    )?;
    if head.id() != head_id {
        return Err(StoreError::StoredHeadMismatch);
    }
    Ok(head)
}

fn load_lineage(
    connection: &Connection,
    current_head_id: StoreHeadIdV1,
    domain: &StoreDomainV1,
) -> Result<Vec<SealedExportLineageV1>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT head_id FROM store_heads WHERE head_revision <= (
             SELECT head_revision FROM store_heads WHERE head_id = ?1
         ) ORDER BY head_revision",
    )?;
    let head_ids = statement
        .query_map(params![current_head_id.as_bytes()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .map(|row| identity(row?))
        .collect::<Result<Vec<StoreHeadIdV1>, StoreError>>()?;
    if head_ids.last().copied() != Some(current_head_id) {
        return Err(StoreError::IncompleteRestoreLineage);
    }
    head_ids
        .into_iter()
        .map(|head_id| {
            let head = load_head(connection, head_id, domain)?;
            let generation = load_generation(connection, head.generation_id(), domain)?;
            SealedExportLineageV1::new(generation, head).map_err(StoreError::from)
        })
        .collect()
}

fn load_restore_candidate(
    connection: &Connection,
    candidate_id: RestoreCandidateIdV1,
) -> Result<RestoreCandidateV1, StoreError> {
    type CandidateRow = (
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
    );
    let row: CandidateRow = connection
        .query_row(
            "SELECT source_export_id, source_domain_id, source_export_bytes_digest,
                    destination_domain_id, candidate_generation_id, candidate_head_id,
                    candidate_snapshot_id, verification_digest
             FROM store_restore_candidates WHERE candidate_id = ?1",
            params![candidate_id.as_bytes()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .optional()?
        .ok_or(StoreError::UnknownRestoreCandidate(candidate_id))?;
    let mut statement = connection.prepare(
        "SELECT object_id FROM store_restore_candidate_roots
         WHERE candidate_id = ?1 ORDER BY root_position",
    )?;
    let roots = statement
        .query_map(params![candidate_id.as_bytes()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .map(|row| identity(row?))
        .collect::<Result<Vec<StoreObjectIdV1>, StoreError>>()?;
    let candidate = RestoreCandidateV1::new(
        identity(row.0)?,
        identity(row.1)?,
        exact_digest(row.2)?,
        identity(row.3)?,
        identity(row.4)?,
        identity(row.5)?,
        identity(row.6)?,
        roots,
        exact_digest(row.7)?,
    )?;
    if candidate.id() != candidate_id {
        return Err(StoreError::StoredRestoreCandidateMismatch);
    }
    Ok(candidate)
}

fn active_pins(connection: &Connection) -> Result<Vec<RetentionPinV1>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT pin.basis_head_id, pin.root_kind, pin.root_object_id, pin.reason_digest
         FROM store_retention_pins AS pin
         LEFT JOIN store_retention_pin_releases AS release ON release.pin_id = pin.pin_id
         WHERE release.pin_id IS NULL ORDER BY pin.pin_id",
    )?;
    statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })?
        .map(|row| {
            let (head, kind, object, reason) = row?;
            RetentionPinV1::new(
                identity(head)?,
                RetentionRootV1::new(retention_kind(kind)?, identity(object)?),
                exact_digest(reason)?,
            )
            .map_err(StoreError::from)
        })
        .collect()
}

fn idempotency_replay_roots(connection: &Connection) -> Result<Vec<RetentionRootV1>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT result_object_id FROM store_idempotency ORDER BY result_object_id",
    )?;
    statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
        .map(|row| {
            Ok(RetentionRootV1::new(
                RetentionRootKindV1::ReplayHorizon,
                identity(row?)?,
            ))
        })
        .collect()
}

fn tombstoned_objects(connection: &Connection) -> Result<BTreeSet<StoreObjectIdV1>, StoreError> {
    let mut statement = connection
        .prepare("SELECT DISTINCT object_id FROM store_logical_tombstones ORDER BY object_id")?;
    statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
        .map(|row| identity(row?))
        .collect()
}

fn available_sealed_export_objects(
    connection: &Connection,
) -> Result<BTreeSet<StoreObjectIdV1>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT object_id
         FROM store_sealed_export_objects
         WHERE entry_kind = 'available'
         ORDER BY object_id",
    )?;
    statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
        .map(|row| identity(row?))
        .collect()
}

fn tombstones_by_object(
    connection: &Connection,
) -> Result<BTreeMap<StoreObjectIdV1, LogicalTombstoneV1>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT tombstone_id, basis_head_id, object_id, reason_digest, invalidation_digest
         FROM store_logical_tombstones ORDER BY object_id",
    )?;
    let mut tombstones = BTreeMap::new();
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, Vec<u8>>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, Vec<u8>>(4)?,
        ))
    })?;
    for row in rows {
        let (declared, head, object, reason, invalidation) = row?;
        let tombstone = LogicalTombstoneV1::new(
            identity(head)?,
            identity(object)?,
            exact_digest(reason)?,
            exact_digest(invalidation)?,
        )?;
        if tombstone.id().as_bytes().as_slice() != declared {
            return Err(StoreError::StoredTombstoneMismatch);
        }
        tombstones.insert(tombstone.object_id(), tombstone);
    }
    Ok(tombstones)
}

fn persist_snapshot(
    metadata: &mut MetadataStore,
    root: &SecureRoot,
    snapshot: &ReachabilitySnapshotV1,
    head_revision: u64,
    retention_revision: u64,
) -> Result<(), StoreError> {
    metadata.with_immediate_transaction(root, |transaction| {
        expect_active_head(transaction, Some(snapshot.head_id()), head_revision)?;
        expect_retention_revision(transaction, retention_revision)?;
        transaction.execute(
            "INSERT OR IGNORE INTO store_reachability_snapshots(snapshot_id, head_id, retention_revision)
             VALUES (?1, ?2, ?3)",
            params![
                snapshot.id().as_bytes(),
                snapshot.head_id().as_bytes(),
                to_i64(snapshot.retention_revision())?,
            ],
        )?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM store_reachability_snapshots
                WHERE snapshot_id = ?1 AND head_id = ?2 AND retention_revision = ?3
             )",
            params![
                snapshot.id().as_bytes(),
                snapshot.head_id().as_bytes(),
                to_i64(snapshot.retention_revision())?,
            ],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(MetadataError::FacadeIntegrityMismatch);
        }
        for object in snapshot.reachable() {
            transaction.execute(
                "INSERT OR IGNORE INTO store_reachability_objects
                 (snapshot_id, object_id, reachability_status) VALUES (?1, ?2, 'reachable')",
                params![snapshot.id().as_bytes(), object.as_bytes()],
            )?;
        }
        for object in snapshot.tombstoned() {
            transaction.execute(
                "INSERT OR IGNORE INTO store_reachability_objects
                 (snapshot_id, object_id, reachability_status) VALUES (?1, ?2, 'tombstoned')",
                params![snapshot.id().as_bytes(), object.as_bytes()],
            )?;
        }
        for (position, root) in snapshot.roots().iter().enumerate() {
            transaction.execute(
                "INSERT OR IGNORE INTO store_reachability_roots
                 (snapshot_id, root_position, root_kind, object_id) VALUES (?1, ?2, ?3, ?4)",
                params![
                    snapshot.id().as_bytes(),
                    position as i64,
                    to_i64(root.kind().tag())?,
                    root.object_id().as_bytes(),
                ],
            )?;
        }
        verify_snapshot_metadata(transaction, snapshot)
    })?;
    Ok(())
}

fn verify_snapshot_metadata(
    transaction: &Transaction<'_>,
    snapshot: &ReachabilitySnapshotV1,
) -> Result<(), MetadataError> {
    let mut object_statement = transaction.prepare(
        "SELECT object_id, reachability_status FROM store_reachability_objects
         WHERE snapshot_id = ?1 ORDER BY object_id",
    )?;
    let stored_objects = object_statement
        .query_map(params![snapshot.id().as_bytes()], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let expected_objects = snapshot
        .reachable()
        .iter()
        .map(|object| (object.as_bytes().as_slice(), "reachable"))
        .chain(
            snapshot
                .tombstoned()
                .iter()
                .map(|object| (object.as_bytes().as_slice(), "tombstoned")),
        )
        .collect::<BTreeMap<_, _>>();
    if stored_objects.len() != expected_objects.len()
        || stored_objects.iter().any(|(stored_id, stored_status)| {
            expected_objects.get(stored_id.as_slice()).copied() != Some(stored_status.as_str())
        })
    {
        return Err(MetadataError::FacadeIntegrityMismatch);
    }

    let mut root_statement = transaction.prepare(
        "SELECT root_kind, object_id FROM store_reachability_roots
         WHERE snapshot_id = ?1 ORDER BY root_position",
    )?;
    let stored_roots = root_statement
        .query_map(params![snapshot.id().as_bytes()], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if stored_roots.len() != snapshot.roots().len()
        || stored_roots
            .iter()
            .zip(snapshot.roots())
            .any(|((kind, object), root)| {
                *kind != root.kind().tag() as i64
                    || object.as_slice() != root.object_id().as_bytes()
            })
    {
        return Err(MetadataError::FacadeIntegrityMismatch);
    }
    Ok(())
}

fn verify_plan(
    transaction: &Transaction<'_>,
    plan: &CollectionPlanV1,
) -> Result<(), MetadataError> {
    let plan_exists: bool = transaction.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM store_gc_plans
            WHERE plan_id = ?1 AND snapshot_id = ?2 AND head_id = ?3 AND retention_revision = ?4
         )",
        params![
            plan.id().as_bytes(),
            plan.snapshot_id().as_bytes(),
            plan.head_id().as_bytes(),
            to_i64_metadata(plan.retention_revision())?,
        ],
        |row| row.get(0),
    )?;
    if !plan_exists {
        return Err(MetadataError::FacadeCasMismatch);
    }
    let mut statement = transaction.prepare(
        "SELECT object_id FROM store_gc_plan_objects WHERE plan_id = ?1 ORDER BY object_id",
    )?;
    let stored = statement
        .query_map(params![plan.id().as_bytes()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if stored.len() != plan.candidates().len()
        || stored
            .iter()
            .zip(plan.candidates())
            .any(|(stored, expected)| stored.as_slice() != expected.as_bytes())
    {
        return Err(MetadataError::FacadeIntegrityMismatch);
    }
    Ok(())
}

fn verify_export_metadata(
    connection: &Connection,
    backup: &SealedBackupV1,
) -> Result<(), MetadataError> {
    let export = backup.export();
    let receipt = backup.receipt();
    let digest = sha256(export.canonical_bytes());
    let snapshot_root = export
        .snapshot_root()
        .ok_or(MetadataError::FacadeIntegrityMismatch)?;
    let header_matches: bool = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM store_sealed_exports
            WHERE export_id = ?1 AND head_id = ?2 AND generation_id = ?3 AND snapshot_id = ?4
              AND schema_manifest_id = ?5 AND family_manifest_set_digest = ?6
              AND snapshot_root_id = ?7 AND source_publication_clock = ?8
              AND committed_publication_clock = ?9 AND payload_set_digest = ?10
              AND backup_receipt_id = ?11 AND export_byte_length = ?12
              AND export_bytes_digest = ?13 AND export_format = ?14 AND carrier_format = ?15
         )",
        params![
            export.id().as_bytes(),
            export.head().id().as_bytes(),
            export.generation().id().as_bytes(),
            export.reachability().id().as_bytes(),
            snapshot_root.schema_manifest_id().as_bytes(),
            snapshot_root.family_manifest_set_digest().as_slice(),
            snapshot_root.id().as_bytes(),
            to_i64_metadata(snapshot_root.publication_clock())?,
            to_i64_metadata(receipt.committed_publication_clock())?,
            snapshot_root.payload_set_digest().as_slice(),
            receipt.id().as_bytes(),
            to_i64_metadata(export.canonical_bytes().len() as u64)?,
            digest.as_slice(),
            SEALED_EXPORT_FORMAT_V2,
            SEALED_BACKUP_FORMAT_V1,
        ],
        |row| row.get(0),
    )?;
    if !header_matches {
        return Err(MetadataError::FacadeIntegrityMismatch);
    }

    let mut pin_statement = connection.prepare(
        "SELECT pin_id FROM store_sealed_export_pins WHERE export_id = ?1 ORDER BY pin_id",
    )?;
    let stored_pins = pin_statement
        .query_map(params![export.id().as_bytes()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if stored_pins.len() != export.retention_pins().len()
        || stored_pins
            .iter()
            .zip(export.retention_pins())
            .any(|(stored, pin)| stored.as_slice() != pin.id().as_bytes())
    {
        return Err(MetadataError::FacadeIntegrityMismatch);
    }

    let mut entry_statement = connection.prepare(
        "SELECT object_id, entry_kind FROM store_sealed_export_objects
         WHERE export_id = ?1 ORDER BY object_id",
    )?;
    let stored_entries = entry_statement
        .query_map(params![export.id().as_bytes()], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if stored_entries.len() != export.entries().len()
        || stored_entries
            .iter()
            .zip(export.entries())
            .any(|((stored_id, stored_kind), entry)| {
                let expected_kind = match entry {
                    SealedExportEntryV1::Available(_) => "available",
                    SealedExportEntryV1::Tombstoned(_) => "tombstoned",
                };
                stored_id.as_slice() != entry.object_id().as_bytes() || stored_kind != expected_kind
            })
    {
        return Err(MetadataError::FacadeIntegrityMismatch);
    }
    Ok(())
}

fn verify_export_source_basis(
    transaction: &Transaction<'_>,
    export: &SealedExportV1,
) -> Result<(), MetadataError> {
    let mut object_statement =
        transaction.prepare("SELECT object_id FROM store_objects ORDER BY object_id")?;
    let stored_objects = object_statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if stored_objects.len() != export.object_inventory().len()
        || stored_objects
            .iter()
            .zip(export.object_inventory())
            .any(|(stored, expected)| stored.as_slice() != expected.as_bytes())
    {
        return Err(MetadataError::FacadeCasMismatch);
    }

    let mut pin_statement = transaction.prepare(
        "SELECT pin.pin_id FROM store_retention_pins AS pin
         LEFT JOIN store_retention_pin_releases AS release ON release.pin_id = pin.pin_id
         WHERE release.pin_id IS NULL ORDER BY pin.pin_id",
    )?;
    let stored_pins = pin_statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if stored_pins.len() != export.retention_pins().len()
        || stored_pins
            .iter()
            .zip(export.retention_pins())
            .any(|(stored, expected)| stored.as_slice() != expected.id().as_bytes())
    {
        return Err(MetadataError::FacadeCasMismatch);
    }

    let mut tombstone_statement =
        transaction.prepare("SELECT object_id FROM store_logical_tombstones ORDER BY object_id")?;
    let stored_tombstones = tombstone_statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let expected_tombstones = export
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            SealedExportEntryV1::Available(_) => None,
            SealedExportEntryV1::Tombstoned(object) => Some(object.object_id()),
        })
        .collect::<Vec<_>>();
    if stored_tombstones.len() != expected_tombstones.len()
        || stored_tombstones
            .iter()
            .zip(expected_tombstones)
            .any(|(stored, expected)| stored.as_slice() != expected.as_bytes())
    {
        return Err(MetadataError::FacadeCasMismatch);
    }
    Ok(())
}

fn object_path(object_id: StoreObjectIdV1) -> PathBuf {
    let hex = object_id.render();
    let hex = hex
        .strip_prefix("sha256:")
        .expect("invariant: identity rendering");
    Path::new(OBJECTS_DIRECTORY)
        .join(&hex[..2])
        .join(format!("{hex}.cbor"))
}

fn export_path(export_id: SealedExportIdV1) -> PathBuf {
    let hex = export_id.render();
    let hex = hex
        .strip_prefix("sha256:")
        .expect("invariant: identity rendering");
    Path::new(EXPORTS_DIRECTORY).join(format!("{hex}.cbor"))
}

fn export_pending_path(export_id: SealedExportIdV1) -> PathBuf {
    let hex = export_id
        .to_string()
        .strip_prefix("sha256:")
        .expect("invariant: identity rendering")
        .to_owned();
    Path::new(EXPORTS_DIRECTORY).join(format!(".maestro-export-{hex}.pending"))
}

fn controlled_copy_erasure_barrier_path(object_id: StoreObjectIdV1) -> PathBuf {
    let rendered = object_id.render();
    let hex = rendered
        .strip_prefix("sha256:")
        .expect("invariant: identity rendering");
    Path::new(CONTROLLED_COPY_ERASURE_BARRIERS_DIRECTORY).join(format!("{hex}.cbor"))
}

fn controlled_copy_erasure_barrier_bytes(
    object_id: StoreObjectIdV1,
    barrier_identity: [u8; 32],
) -> Result<Vec<u8>, StoreError> {
    if barrier_identity == [0; 32] {
        return Err(StoreError::InvalidControlledCopyErasurePlan);
    }
    Ok(deterministic_cbor::encode(&CborValue::Array(vec![
        CborValue::text("maestro.vnext.store.controlled-copy-erasure-barrier.v1")?,
        CborValue::Bytes(object_id.as_bytes().to_vec()),
        CborValue::Bytes(barrier_identity.to_vec()),
    ]))?)
}

fn controlled_copy_obligations(
    root: &SecureRoot,
    connection: &Connection,
    object_id: StoreObjectIdV1,
) -> Result<Vec<ControlledCopyErasureObligationV1>, StoreError> {
    let mut statement = connection.prepare(
        "SELECT export.export_id, export.export_byte_length, export.export_bytes_digest,
                export.snapshot_root_id
         FROM store_sealed_exports AS export
         JOIN store_sealed_export_objects AS object ON object.export_id = export.export_id
         WHERE object.object_id = ?1 AND object.entry_kind = 'available'
         ORDER BY export.export_id",
    )?;
    let rows = statement
        .query_map(params![object_id.as_bytes()], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut obligations = rows
        .into_iter()
        .map(
            |(export_id, export_byte_length, export_bytes_digest, snapshot_root_id)| {
                let export_id = SealedExportIdV1::from_digest(exact_digest(export_id)?);
                let export_byte_length = to_u64(export_byte_length)?;
                let export_bytes_digest = exact_digest(export_bytes_digest)?;
                if export_byte_length == 0 {
                    return Err(StoreError::InvalidControlledCopyErasurePlan);
                }
                let snapshot_root_id = exact_digest(snapshot_root_id)?;
                let public = secure_backup_binding(
                    root,
                    &export_path(export_id),
                    export_id,
                    export_byte_length,
                    export_bytes_digest,
                    snapshot_root_id,
                )?;
                let pending = secure_backup_binding(
                    root,
                    &export_pending_path(export_id),
                    export_id,
                    export_byte_length,
                    export_bytes_digest,
                    snapshot_root_id,
                )?;
                if public.is_some() && pending.is_some() && public != pending {
                    return Err(StoreError::StoredControlledCopyMismatch);
                }
                let (carrier_byte_length, carrier_bytes_digest) = public
                    .or(pending)
                    .ok_or(StoreError::StoredControlledCopyMismatch)?;
                let snapshot_closure =
                    secure_file_binding(root, &snapshot_closure_path(&snapshot_root_id))?;
                Ok(ControlledCopyErasureObligationV1 {
                    export_id,
                    export_byte_length: carrier_byte_length,
                    export_bytes_digest: carrier_bytes_digest,
                    snapshot_root_id,
                    snapshot_closure,
                    public_carrier_present: public.is_some(),
                    pending_carrier_present: pending.is_some(),
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let registered = obligations
        .iter()
        .map(|obligation| obligation.export_id)
        .collect::<BTreeSet<_>>();
    obligations.extend(orphan_controlled_copy_obligations(
        root,
        object_id,
        &registered,
    )?);
    obligations.sort_by_key(|obligation| obligation.export_id);
    if obligations
        .windows(2)
        .any(|pair| pair[0].export_id == pair[1].export_id)
    {
        return Err(StoreError::StoredControlledCopyMismatch);
    }
    Ok(obligations)
}

fn ensure_monotonic_controlled_copy_erasure_census(
    planned: &[ControlledCopyErasureObligationV1],
    current: &[ControlledCopyErasureObligationV1],
) -> Result<(), StoreError> {
    if current.len() > planned.len() {
        return Err(StoreError::ControlledCopyErasureCensusChanged);
    }
    for observed in current {
        let Some(expected) = planned
            .iter()
            .find(|candidate| candidate.export_id == observed.export_id)
        else {
            return Err(StoreError::ControlledCopyErasureCensusChanged);
        };
        if observed.export_byte_length != expected.export_byte_length
            || observed.export_bytes_digest != expected.export_bytes_digest
            || observed.snapshot_root_id != expected.snapshot_root_id
            || observed
                .snapshot_closure
                .is_some_and(|binding| expected.snapshot_closure != Some(binding))
            || (observed.public_carrier_present && !expected.public_carrier_present)
            || (observed.pending_carrier_present && !expected.pending_carrier_present)
        {
            return Err(StoreError::ControlledCopyErasureCensusChanged);
        }
    }
    Ok(())
}

fn orphan_controlled_copy_obligations(
    root: &SecureRoot,
    object_id: StoreObjectIdV1,
    registered: &BTreeSet<SealedExportIdV1>,
) -> Result<Vec<ControlledCopyErasureObligationV1>, StoreError> {
    let exports = root.open_dir(Path::new(EXPORTS_DIRECTORY))?;
    let mut carriers = BTreeMap::<SealedExportIdV1, (SealedBackupV1, bool, bool, [u8; 32])>::new();
    for entry in exports.read_dir_entries()? {
        let name = entry
            .name()
            .as_os_str()
            .to_str()
            .ok_or(StoreError::StoredControlledCopyMismatch)?
            .to_owned();
        if name == "snapshot-closures" {
            if entry.kind() != SecureDirectoryEntryKind::Directory {
                return Err(StoreError::StoredControlledCopyMismatch);
            }
            continue;
        }
        if entry.kind() != SecureDirectoryEntryKind::RegularFile {
            return Err(StoreError::StoredControlledCopyMismatch);
        }
        let (hex, public, pending) = if let Some(hex) = name.strip_suffix(".cbor") {
            (hex, true, false)
        } else if let Some(hex) = name
            .strip_prefix(".maestro-export-")
            .and_then(|value| value.strip_suffix(".pending"))
        {
            (hex, false, true)
        } else {
            return Err(StoreError::StoredControlledCopyMismatch);
        };
        if hex.len() != 64 || !hex.as_bytes().iter().all(u8::is_ascii_hexdigit) {
            return Err(StoreError::StoredControlledCopyMismatch);
        }
        let rendered = format!("sha256:{hex}");
        let export_id = SealedExportIdV1::parse(&rendered)
            .map_err(|_| StoreError::StoredControlledCopyMismatch)?;
        if registered.contains(&export_id) {
            continue;
        }
        let bytes = exports.read_immutable(Path::new(&name))?;
        let backup = SealedBackupV1::decode(&bytes)?;
        if backup.export().id() != export_id || backup.canonical_bytes() != bytes {
            return Err(StoreError::StoredControlledCopyMismatch);
        }
        let digest = sha256(&bytes);
        match carriers.entry(export_id) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((backup, public, pending, digest));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let (stored, stored_public, stored_pending, stored_digest) = entry.get_mut();
                if stored.canonical_bytes() != backup.canonical_bytes() || *stored_digest != digest
                {
                    return Err(StoreError::StoredControlledCopyMismatch);
                }
                *stored_public |= public;
                *stored_pending |= pending;
            }
        }
    }
    carriers
        .into_iter()
        .filter_map(|(export_id, (backup, public, pending, digest))| {
            backup
                .export()
                .entries()
                .iter()
                .any(|entry| {
                    matches!(entry, SealedExportEntryV1::Available(object)
                            if object.id() == object_id)
                })
                .then_some((export_id, backup, public, pending, digest))
        })
        .map(|(export_id, backup, public, pending, digest)| {
            let snapshot_root = backup
                .export()
                .snapshot_root()
                .ok_or(StoreError::StoredControlledCopyMismatch)?;
            Ok(ControlledCopyErasureObligationV1 {
                export_id,
                export_byte_length: u64::try_from(backup.canonical_bytes().len())
                    .map_err(|_| StoreError::InvalidMetadataInteger)?,
                export_bytes_digest: digest,
                snapshot_root_id: *snapshot_root.id().as_bytes(),
                snapshot_closure: secure_file_binding(
                    root,
                    &snapshot_closure_path(snapshot_root.id().as_bytes()),
                )?,
                public_carrier_present: public,
                pending_carrier_present: pending,
            })
        })
        .collect()
}

fn secure_backup_binding(
    root: &SecureRoot,
    path: &Path,
    export_id: SealedExportIdV1,
    export_byte_length: u64,
    export_bytes_digest: [u8; 32],
    snapshot_root_id: [u8; 32],
) -> Result<Option<(u64, [u8; 32])>, StoreError> {
    let bytes = match root.read_immutable(path) {
        Ok(bytes) => bytes,
        Err(error) if secure_fs_error_is_not_found(&error) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let backup = SealedBackupV1::decode(&bytes)?;
    let snapshot_root = backup
        .export()
        .snapshot_root()
        .ok_or(StoreError::StoredControlledCopyMismatch)?;
    if backup.export().id() != export_id
        || backup.export().canonical_bytes().len() as u64 != export_byte_length
        || sha256(backup.export().canonical_bytes()) != export_bytes_digest
        || snapshot_root.id().as_bytes() != &snapshot_root_id
    {
        return Err(StoreError::StoredControlledCopyMismatch);
    }
    Ok(Some((
        u64::try_from(bytes.len()).map_err(|_| StoreError::InvalidMetadataInteger)?,
        sha256(&bytes),
    )))
}

fn secure_file_binding(
    root: &SecureRoot,
    path: &Path,
) -> Result<Option<(u64, [u8; 32])>, StoreError> {
    match root.read_immutable(path) {
        Ok(bytes) => Ok(Some((
            u64::try_from(bytes.len()).map_err(|_| StoreError::InvalidMetadataInteger)?,
            sha256(&bytes),
        ))),
        Err(error) if secure_fs_error_is_not_found(&error) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn verify_secure_path_absent(root: &SecureRoot, path: &Path) -> Result<(), StoreError> {
    match root.read_immutable(path) {
        Err(error) if secure_fs_error_is_not_found(&error) => Ok(()),
        Ok(_) => Err(StoreError::ControlledCopyStillPresent(path.to_path_buf())),
        Err(error) => Err(error.into()),
    }
}

fn erase_secure_file_with_digest(
    root: &SecureRoot,
    path: impl AsRef<Path>,
    expected_digest: &[u8; 32],
) -> Result<bool, SecureFsError> {
    let path = path.as_ref();
    let removed = match root.read_immutable(path) {
        Ok(bytes) => {
            if &sha256(&bytes) != expected_digest {
                return Err(SecureFsError::ContentMismatch {
                    path: path.to_path_buf(),
                });
            }
            root.remove_file_if_matches(path, &bytes)
        }
        Err(error) if secure_fs_error_is_not_found(&error) => {
            root.finish_file_removal_if_digest_matches(path, expected_digest)
        }
        Err(error) => Err(error),
    }?;
    root.remove_crash_residual_temps_by_digest(
        path.parent()
            .expect("invariant: secure Store file path has a parent"),
        expected_digest,
    )?;
    Ok(removed)
}

fn ensure_no_controlled_copy_erasure_barriers(
    root: &SecureRoot,
    object_ids: impl IntoIterator<Item = StoreObjectIdV1>,
) -> Result<(), StoreError> {
    for object_id in object_ids {
        match root.read_immutable(controlled_copy_erasure_barrier_path(object_id)) {
            Err(error) if secure_fs_error_is_not_found(&error) => {}
            Ok(_) => return Err(StoreError::ControlledCopyErasureInProgress(object_id)),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn snapshot_closure_path(root_id: &[u8; 32]) -> PathBuf {
    let hex = root_id
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Path::new(SNAPSHOT_CLOSURES_DIRECTORY).join(format!("{hex}.cbor"))
}

fn persist_snapshot_closure_with_root(
    root: &SecureRoot,
    closure: &StoreSnapshotBlockClosureV2,
) -> Result<(), StoreError> {
    let path = snapshot_closure_path(closure.current_root_id());
    match root.create_file_if_absent(&path, closure.canonical_bytes())? {
        CreateIfAbsent::Created => Ok(()),
        CreateIfAbsent::AlreadyExists => {
            root.read_exact(&path, closure.canonical_bytes())?;
            Ok(())
        }
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn secure_fs_error_is_not_found(error: &SecureFsError) -> bool {
    matches!(
        error,
        SecureFsError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound
    )
}

fn identity<K: IdentityKindV1>(bytes: Vec<u8>) -> Result<ManifestIdentityV1<K>, StoreError> {
    let digest = exact_digest(bytes)?;
    Ok(ManifestIdentityV1::from_digest(digest))
}

fn exact_digest(bytes: Vec<u8>) -> Result<[u8; 32], StoreError> {
    bytes
        .try_into()
        .map_err(|_| StoreError::InvalidIdentityLength)
}

fn retention_kind(tag: i64) -> Result<RetentionRootKindV1, StoreError> {
    let tag = u64::try_from(tag).map_err(|_| StoreError::InvalidMetadataInteger)?;
    RetentionRootKindV1::ALL
        .into_iter()
        .find(|kind| kind.tag() == tag)
        .ok_or(StoreError::UnknownRetentionKind(tag))
}

fn to_i64(value: u64) -> Result<i64, MetadataError> {
    i64::try_from(value).map_err(|_| MetadataError::FacadeIntegrityMismatch)
}

fn to_i64_metadata(value: u64) -> Result<i64, MetadataError> {
    to_i64(value)
}

fn to_u64(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::InvalidMetadataInteger)
}

#[derive(Debug)]
struct StagedObjectFile {
    object_id: StoreObjectIdV1,
    path: PathBuf,
    bytes: Vec<u8>,
}

fn stage_object_files(
    root: &SecureRoot,
    objects: &[StoreObjectV1],
    staged: &mut Vec<StagedObjectFile>,
) -> Result<(), StoreError> {
    let mut identities = BTreeSet::new();
    for object in objects {
        if !identities.insert(object.id()) {
            return Err(StoreError::DuplicateObject(object.id()));
        }
        if StoreObjectV1::decode(object.canonical_bytes())? != *object {
            return Err(StoreError::NonCanonicalCarrier);
        }
        let path = object_path(object.id());
        let parent = path
            .parent()
            .expect("invariant: canonical object path has a parent");
        root.create_dir_all(parent)?;
        match root.create_file_if_absent(&path, object.canonical_bytes())? {
            CreateIfAbsent::Created => staged.push(StagedObjectFile {
                object_id: object.id(),
                path,
                bytes: object.canonical_bytes().to_vec(),
            }),
            CreateIfAbsent::AlreadyExists => {
                root.read_exact(&path, object.canonical_bytes())?;
            }
        }
    }
    Ok(())
}

fn cleanup_staged_object_files_if_unreferenced(
    root: &SecureRoot,
    transaction: &Transaction<'_>,
    staged: &[StagedObjectFile],
) -> Result<(), StoreError> {
    for file in staged.iter().rev() {
        let available: bool = transaction.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM store_objects AS object
                 WHERE object.object_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM store_logical_tombstones AS tombstone
                       WHERE tombstone.object_id = object.object_id
                   )
                   AND NOT EXISTS (
                       SELECT 1 FROM store_gc_collection_occurrences AS occurrence
                       WHERE occurrence.object_id = object.object_id
                   )
             )",
            params![file.object_id.as_bytes()],
            |row| row.get(0),
        )?;
        if !available {
            root.remove_file_if_matches(&file.path, &file.bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    static BEFORE_FAILED_PUBLICATION_CLEANUP_TEST_HOOK:
        std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
}

#[cfg(test)]
pub(super) fn install_before_failed_publication_cleanup_test_hook(hook: impl FnOnce() + 'static) {
    BEFORE_FAILED_PUBLICATION_CLEANUP_TEST_HOOK.with(|slot| {
        assert!(
            slot.borrow_mut().replace(Box::new(hook)).is_none(),
            "failed-publication cleanup hook must be exclusive"
        );
    });
}

#[cfg(test)]
fn run_before_failed_publication_cleanup_test_hook() {
    BEFORE_FAILED_PUBLICATION_CLEANUP_TEST_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
}

#[cfg(not(test))]
fn run_before_failed_publication_cleanup_test_hook() {}

#[cfg(test)]
thread_local! {
    static AFTER_SUCCESSFUL_SEALED_EXPORT_RECEIPT_TEST_HOOK:
        std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
}

#[cfg(test)]
fn install_after_successful_sealed_export_receipt_test_hook(hook: impl FnOnce() + 'static) {
    AFTER_SUCCESSFUL_SEALED_EXPORT_RECEIPT_TEST_HOOK.with(|slot| {
        assert!(
            slot.borrow_mut().replace(Box::new(hook)).is_none(),
            "sealed-export receipt hook must be exclusive"
        );
    });
}

#[cfg(test)]
fn run_after_successful_sealed_export_receipt_test_hook() {
    AFTER_SUCCESSFUL_SEALED_EXPORT_RECEIPT_TEST_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
}

#[cfg(not(test))]
fn run_after_successful_sealed_export_receipt_test_hook() {}

#[cfg(test)]
thread_local! {
    static AFTER_PENDING_SEALED_EXPORT_TEST_HOOK:
        std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
}

#[cfg(test)]
fn install_after_pending_sealed_export_test_hook(hook: impl FnOnce() + 'static) {
    AFTER_PENDING_SEALED_EXPORT_TEST_HOOK.with(|slot| {
        assert!(
            slot.borrow_mut().replace(Box::new(hook)).is_none(),
            "sealed-export pending-carrier hook must be exclusive"
        );
    });
}

#[cfg(test)]
fn run_after_pending_sealed_export_test_hook() {
    AFTER_PENDING_SEALED_EXPORT_TEST_HOOK.with(|hook| {
        if let Some(hook) = hook.borrow_mut().take() {
            hook();
        }
    });
}

#[cfg(not(test))]
fn run_after_pending_sealed_export_test_hook() {}

fn reject_sqlite_hard_links(path: &Path) -> Result<(), StoreError> {
    reject_hard_link(path)?;
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        reject_hard_link(Path::new(&sidecar))?;
    }
    Ok(())
}

#[cfg(unix)]
fn reject_hard_link(path: &Path) -> Result<(), StoreError> {
    use std::os::unix::fs::MetadataExt;

    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.nlink() != 1 => {
            Err(StoreError::HardLinkedMetadata(path.to_owned()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(StoreError::InspectMetadata {
            path: path.to_owned(),
            source,
        }),
    }
}

#[cfg(not(unix))]
fn reject_hard_link(_path: &Path) -> Result<(), StoreError> {
    Ok(())
}

struct ExportObjectRow {
    object_id: StoreObjectIdV1,
    metadata: ObjectMetadata,
    references: Vec<StoreObjectIdV1>,
    tombstone: Option<LogicalTombstoneV1>,
}

struct ObjectMetadata {
    schema_id: SchemaIdV1,
    logical_byte_length: usize,
    stored_byte_length: usize,
    stored_bytes_digest: [u8; 32],
    storage_codec: String,
    key_envelope: Option<([u8; 32], String)>,
}

#[derive(Clone, Debug)]
struct StoredIdempotencyV1 {
    meaning_digest: [u8; 32],
    result_object_id: StoreObjectIdV1,
    generation_id: StoreGenerationIdV1,
    head_id: StoreHeadIdV1,
}

enum AtomicMetadataPublication {
    Committed {
        head: StoreHeadV1,
        result_object_id: StoreObjectIdV1,
    },
    Replayed(StoredIdempotencyV1),
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Store role does not match the requested role-specific operation")]
    RoleMismatch,
    #[error("Store domain does not match the canonical carrier")]
    DomainMismatch,
    #[error("Store generation does not bind the exact Stage 0 successor compatibility set")]
    IncompatibleGeneration,
    #[error("Store Head changed since the exact expected-old value was read")]
    HeadCasMismatch,
    #[error("Store idempotency key already binds a different request meaning")]
    IdempotencyMeaningConflict,
    #[error("prepared Store publication does not bind the exact probed idempotency meaning")]
    PreparedIdempotencyMismatch,
    #[error("Store continuity token allocation does not bind the exact next publication")]
    InvalidPublicationAllocation,
    #[error(transparent)]
    AtomicPublication(#[from] super::AtomicPublicationError),
    #[error("Store idempotency result is not reachable from the published Generation")]
    IdempotencyResultOutsideGeneration,
    #[error("prepared Store publication supplied an Object outside the exact Generation closure")]
    PublicationObjectOutsideGeneration,
    #[error("Store idempotency metadata does not bind one exact result, Generation, and Head")]
    StoredIdempotencyMismatch,
    #[error("Store retention revision changed since it was read")]
    RetentionCasMismatch,
    #[error("Store snapshot does not bind the active Head and retention revision")]
    SnapshotBasisMismatch,
    #[error("Store has no active Head")]
    MissingActiveHead,
    #[error("protected diagnostic currentness could not be established")]
    ProtectedDiagnosticCurrentnessRefused,
    #[error("Store is already active")]
    StoreAlreadyActive,
    #[error("activation commitment must bind a nonzero external authority locator")]
    EmptyAuthorityLocator,
    #[error("sealed restore/import is permitted only while the Store is inactive")]
    ImportRequiresInactiveStore,
    #[error("legacy partial sealed exports are decode-only and cannot perform a full restore")]
    LegacyPartialExport,
    #[error("inactive sibling restore requires an empty Store with no published Head")]
    RestoreRequiresEmptyStore,
    #[error(
        "sealed export omits the predecessor Generation/Head lineage required for exact restore"
    )]
    IncompleteRestoreLineage,
    #[error("Store state row contains an unsupported value")]
    InvalidStoreState,
    #[error("Store retention revision must be positive after publication")]
    InvalidRetentionRevision,
    #[error("Store metadata contains an invalid signed integer")]
    InvalidMetadataInteger,
    #[error("Store identity bytes must contain exactly 32 bytes")]
    InvalidIdentityLength,
    #[error("Store carrier is not its exact canonical encoding")]
    NonCanonicalCarrier,
    #[error("Store Object closure exceeds its admission limit of {limit} entries")]
    ObjectClosureLimitExceeded { limit: usize },
    #[error("Store Object is not authoritative in SQLite: {0}")]
    UnknownObject(StoreObjectIdV1),
    #[error("Store Object is logically collected in committed metadata: {0}")]
    CollectedObject(StoreObjectIdV1),
    #[error("Store Object is logically tombstoned and unavailable to runtime readers: {0}")]
    TombstonedObject(StoreObjectIdV1),
    #[error("Store Object appears more than once in one persistence batch: {0}")]
    DuplicateObject(StoreObjectIdV1),
    #[error("Store Object bytes do not match authoritative metadata: {0}")]
    StoredObjectMismatch(StoreObjectIdV1),
    #[error("Store Object references do not match authoritative metadata: {0}")]
    StoredReferenceMismatch(StoreObjectIdV1),
    #[error("Store Head identity does not match authoritative metadata")]
    StoredHeadMismatch,
    #[error("Store tombstone identity does not match authoritative metadata")]
    StoredTombstoneMismatch,
    #[error("Store has no committed collection occurrence for object {0}")]
    MissingCollectionOccurrence(StoreObjectIdV1),
    #[error("collected Store Object bytes remain physically present: {0}")]
    CollectedObjectStillPresent(StoreObjectIdV1),
    #[error("Restore Candidate identity does not match authoritative metadata")]
    StoredRestoreCandidateMismatch,
    #[error("unknown Restore Candidate {0}")]
    UnknownRestoreCandidate(RestoreCandidateIdV1),
    #[error("Store Object remains reachable and cannot be tombstoned or collected: {0}")]
    ObjectStillReachable(StoreObjectIdV1),
    #[error("controlled-copy erasure plan is malformed or does not bind one exact census")]
    InvalidControlledCopyErasurePlan,
    #[error("controlled-copy erasure is already in progress for Store Object {0}")]
    ControlledCopyErasureInProgress(StoreObjectIdV1),
    #[error("controlled-copy erasure census changed after its durable barrier")]
    ControlledCopyErasureCensusChanged,
    #[error("controlled Store copy does not match its committed metadata")]
    StoredControlledCopyMismatch,
    #[error("controlled Store copy remains present at {0}")]
    ControlledCopyStillPresent(PathBuf),
    #[error("failed to inspect controlled-copy directory {path}")]
    InspectControlledCopyDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("controlled-copy physical-absence receipt does not match the exact erasure plan")]
    ControlledCopyErasureReceiptMismatch,
    #[error("Store is missing a logical tombstone for exported object {0}")]
    MissingTombstone(StoreObjectIdV1),
    #[error("unknown retention root kind tag {0}")]
    UnknownRetentionKind(u64),
    #[error("unsupported Store object storage codec {0}")]
    UnsupportedStorageCodec(String),
    #[error("Store metadata file has more than one hard link: {0}")]
    HardLinkedMetadata(PathBuf),
    #[error("failed to inspect Store metadata path {path}")]
    InspectMetadata {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "GC committed {committed} collection occurrence(s), but {remaining} raw object sweep(s) remain as cleanup debt"
    )]
    CollectionSweepDebt {
        committed: usize,
        remaining: usize,
        #[source]
        source: SecureFsError,
    },
    #[error(
        "controlled-copy erasure committed its metadata revocation, but {remaining} file sweep(s) remain as cleanup debt"
    )]
    ControlledCopyErasureSweepDebt {
        remaining: usize,
        #[source]
        source: SecureFsError,
    },
    #[error(
        "sealed backup {export_id} committed but public artifact recovery is required: {reason}"
    )]
    BackupPublicationRecoveryRequired {
        export_id: SealedExportIdV1,
        reason: String,
    },
    #[error("Store metadata publication may have committed and recovery is required: {reason}")]
    PublicationRecoveryRequired { reason: String },
    #[error("Store snapshot references more than {limit} prior snapshot closure roots")]
    PriorSnapshotClosureCountLimitExceeded { limit: usize },
    #[error("Store snapshot references the same prior snapshot closure root more than once")]
    DuplicatePriorSnapshotClosureRoot,
    #[error("prior Store snapshot closures exceed the aggregate byte limit of {limit}")]
    PriorSnapshotClosureBytesLimitExceeded { limit: usize },
    #[error("Store metadata operation failed: {0}")]
    Metadata(String),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    SecureFs(#[from] SecureFsError),
    #[error(transparent)]
    Object(#[from] StoreObjectError),
    #[error(transparent)]
    Generation(#[from] GenerationError),
    #[error(transparent)]
    Retention(#[from] RetentionError),
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
    #[error("Store snapshot block closure is invalid: {0}")]
    SnapshotBlocks(String),
    #[error(transparent)]
    CanonicalCbor(#[from] crate::foundation::core::deterministic_cbor::CborError),
}

impl From<MetadataError> for StoreError {
    fn from(error: MetadataError) -> Self {
        if matches!(error, MetadataError::IdempotencyMeaningConflict) {
            Self::IdempotencyMeaningConflict
        } else if error.publication_may_have_committed() {
            Self::PublicationRecoveryRequired {
                reason: error.to_string(),
            }
        } else {
            Self::Metadata(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Barrier};

    use crate::domain::vnext::identity::{ContractRootIdV1, SchemaIdV1};
    use crate::foundation::core::deterministic_cbor::CborValue;

    use super::*;

    static NEXT_STORE_TEST: AtomicU64 = AtomicU64::new(0);

    struct TestStorePath(PathBuf);

    impl TestStorePath {
        fn new() -> Self {
            let sequence = NEXT_STORE_TEST.fetch_add(1, Ordering::Relaxed);
            let root = fs::canonicalize(std::env::temp_dir()).expect("canonical temp root");
            Self(root.join(format!(
                "maestro-vnext-store-unit-{}-{sequence}",
                std::process::id()
            )))
        }
    }

    impl Drop for TestStorePath {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn rendered(byte: u8) -> String {
        format!("sha256:{}", format!("{byte:02x}").repeat(32))
    }

    fn object(seed: u64) -> StoreObjectV1 {
        StoreObjectV1::new(
            SchemaIdV1::parse(&rendered(1)).expect("Schema identity"),
            CborValue::Array(vec![CborValue::Unsigned(seed)]),
            vec![],
        )
        .expect("Store Object")
    }

    fn object_id(byte: u8) -> StoreObjectIdV1 {
        StoreObjectIdV1::parse(&rendered(byte)).expect("Store Object identity")
    }

    #[test]
    fn object_closure_walker_accepts_the_exact_entry_limit() {
        let graph = BTreeMap::from([
            (object_id(1), vec![object_id(2)]),
            (object_id(2), vec![object_id(3)]),
            (object_id(3), vec![]),
        ]);
        let closure = walk_object_closure_with_limit([object_id(1)], 3, |id| {
            Ok(graph.get(&id).cloned().unwrap_or_default())
        })
        .expect("exactly bounded closure");
        assert_eq!(closure.len(), 3);
    }

    #[test]
    fn object_closure_walker_rejects_before_loading_entry_over_the_limit() {
        let graph = BTreeMap::from([
            (object_id(1), vec![object_id(2)]),
            (object_id(2), vec![object_id(3)]),
            (object_id(3), vec![object_id(4)]),
        ]);
        let loaded = std::cell::Cell::new(0_usize);
        let error = walk_object_closure_with_limit([object_id(1)], 3, |id| {
            loaded.set(loaded.get() + 1);
            Ok(graph.get(&id).cloned().unwrap_or_default())
        })
        .expect_err("fourth discovered object must be rejected");
        assert!(matches!(
            error,
            StoreError::ObjectClosureLimitExceeded { limit: 3 }
        ));
        assert_eq!(loaded.get(), 3, "the fourth object must never be loaded");
    }

    #[test]
    fn prior_snapshot_closures_reject_excess_roots_before_any_carrier_read() {
        let path = TestStorePath::new();
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"prior-root-limit")
            .expect("Store domain");
        let store = StoreV1::create(&path.0, domain).expect("create Store");
        let roots = (0..=MAX_REFERENCED_PRIOR_ROOTS_V1)
            .map(|index| {
                let mut root = [0_u8; 32];
                root[..8].copy_from_slice(&(index as u64).to_be_bytes());
                root
            })
            .collect::<Vec<_>>();

        let error = store
            .prior_snapshot_closures(&roots)
            .expect_err("the first root beyond the bound must be rejected");
        assert!(matches!(
            error,
            StoreError::PriorSnapshotClosureCountLimitExceeded {
                limit: MAX_REFERENCED_PRIOR_ROOTS_V1
            }
        ));
    }

    #[test]
    fn prior_snapshot_closures_reject_aggregate_bytes_before_decoding_the_carrier() {
        let path = TestStorePath::new();
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"prior-byte-limit")
            .expect("Store domain");
        let store = StoreV1::create(&path.0, domain).expect("create Store");
        let roots = [[1_u8; 32]];
        let carrier_length = (MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2 + 1) as u64;
        for root in roots {
            let carrier = path.0.join(snapshot_closure_path(&root));
            File::create(carrier)
                .expect("create sparse prior carrier")
                .set_len(carrier_length)
                .expect("size sparse prior carrier");
        }

        let error = store
            .prior_snapshot_closures(&roots)
            .expect_err("aggregate bytes beyond the bound must be rejected");
        assert!(matches!(
            error,
            StoreError::PriorSnapshotClosureBytesLimitExceeded {
                limit: MAX_SNAPSHOT_BLOCK_CLOSURE_BYTES_V2
            }
        ));
    }

    #[test]
    fn authoritative_write_between_export_cut_and_receipt_rejects_the_stale_seal() {
        let path = TestStorePath::new();
        let domain =
            StoreDomainV1::derive(StoreRoleV1::Repository, b"seal-race").expect("Store domain");
        let mut store = StoreV1::create(&path.0, domain.clone()).expect("create Store");
        let root = object(1);
        store.put_object(&root).expect("persist root");
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            ContractRootIdV1::parse(&rendered(2)).expect("Contract Root identity"),
            StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
            vec![root.id()],
        )
        .expect("Generation");
        store
            .publish_generation(&generation, None)
            .expect("publish Generation");

        let start = Arc::new(Barrier::new(2));
        let finished = Arc::new(Barrier::new(2));
        let contender_path = path.0.clone();
        let contender_domain = domain.clone();
        let contender_start = Arc::clone(&start);
        let contender_finished = Arc::clone(&finished);
        let contender = std::thread::spawn(move || {
            let mut contender =
                StoreV1::open(contender_path, contender_domain).expect("open contender");
            contender_start.wait();
            contender
                .put_object(&object(3))
                .expect("publish concurrent authoritative object");
            contender_finished.wait();
        });

        let result = store.seal_export_with_before_receipt(|| {
            start.wait();
            finished.wait();
        });
        contender.join().expect("contender thread");
        assert!(
            result.is_err(),
            "stale export cut must not publish a receipt"
        );
        let receipt_count: i64 = store
            .metadata
            .connection()
            .expect("bound metadata")
            .query_row("SELECT COUNT(*) FROM store_sealed_exports", [], |row| {
                row.get(0)
            })
            .expect("receipt count");
        assert_eq!(receipt_count, 0);
        let residual_files = fs::read_dir(path.0.join(EXPORTS_DIRECTORY))
            .expect("exports directory")
            .map(|entry| entry.expect("export entry"))
            .filter(|entry| entry.file_type().expect("export file type").is_file())
            .map(|entry| entry.file_name())
            .collect::<Vec<_>>();
        assert!(
            residual_files.is_empty(),
            "a stale seal must leave no public or pending carrier: {residual_files:?}"
        );
    }

    #[test]
    fn controlled_copy_census_includes_an_orphan_pre_receipt_export() {
        let path = TestStorePath::new();
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"orphan-export-erasure")
            .expect("Store domain");
        let mut store = StoreV1::create(&path.0, domain.clone()).expect("create Store");
        let root = object(91);
        store.put_object(&root).expect("persist root");
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            ContractRootIdV1::parse(&rendered(92)).expect("Contract Root identity"),
            StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
            vec![root.id()],
        )
        .expect("Generation");
        store
            .publish_generation(&generation, None)
            .expect("publish Generation");

        install_after_pending_sealed_export_test_hook(|| {
            panic!("simulated crash after pending carrier publication")
        });
        let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = store.seal_export();
        }));
        assert!(crash.is_err());
        let committed: i64 = store
            .metadata
            .connection()
            .expect("bound metadata")
            .query_row("SELECT COUNT(*) FROM store_sealed_exports", [], |row| {
                row.get(0)
            })
            .expect("sealed export count");
        assert_eq!(committed, 0);

        let plan = store
            .prepare_controlled_copy_erasure_plan(root.id(), [93; 32], |_| Ok::<(), ()>(()))
            .expect("prepare copy-total erasure plan");
        assert_eq!(plan.obligations().len(), 1);
        assert!(plan.obligations()[0].pending_carrier_present);
        assert!(!plan.obligations()[0].public_carrier_present);
        let absence = store
            .erase_controlled_copies(&plan)
            .expect("erase orphan pending carrier");
        store
            .finish_controlled_copy_erasure(&plan, absence.receipt_hash())
            .expect("remove durable barrier after verified absence");
    }

    #[test]
    fn controlled_copy_census_fails_closed_on_a_renamed_export_carrier() {
        let path = TestStorePath::new();
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"renamed-export-erasure")
            .expect("Store domain");
        let mut store = StoreV1::create(&path.0, domain.clone()).expect("create Store");
        let root = object(94);
        store.put_object(&root).expect("persist root");
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            ContractRootIdV1::parse(&rendered(95)).expect("Contract Root identity"),
            StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
            vec![root.id()],
        )
        .expect("Generation");
        store
            .publish_generation(&generation, None)
            .expect("publish Generation");
        let backup = store.seal_export().expect("seal export");
        fs::write(
            path.0
                .join(EXPORTS_DIRECTORY)
                .join("renamed-valid-export.copy"),
            backup.canonical_bytes(),
        )
        .expect("copy valid carrier under an unowned name");

        let result =
            store.prepare_controlled_copy_erasure_plan(root.id(), [96; 32], |_| Ok::<(), ()>(()));
        assert!(matches!(
            result,
            Err(PreparedPublicationError::Store(
                StoreError::StoredControlledCopyMismatch
            ))
        ));
    }

    #[test]
    fn controlled_copy_erasure_recovery_accepts_only_monotonic_disappearance() {
        let path = TestStorePath::new();
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"partial-export-erasure")
            .expect("Store domain");
        let mut store = StoreV1::create(&path.0, domain.clone()).expect("create Store");
        let root = object(97);
        store.put_object(&root).expect("persist root");
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            ContractRootIdV1::parse(&rendered(98)).expect("Contract Root identity"),
            StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
            vec![root.id()],
        )
        .expect("Generation");
        store
            .publish_generation(&generation, None)
            .expect("publish Generation");
        let backup = store.seal_export().expect("seal export");
        let export_id = backup.export().id();
        fs::write(
            path.0.join(export_pending_path(export_id)),
            backup.canonical_bytes(),
        )
        .expect("recreate matching recoverable pending carrier");
        let plan = store
            .prepare_controlled_copy_erasure_plan(root.id(), [99; 32], |_| Ok::<(), ()>(()))
            .expect("prepare exact two-carrier erasure plan");
        assert!(plan.obligations()[0].public_carrier_present);
        assert!(plan.obligations()[0].pending_carrier_present);

        fs::remove_file(path.0.join(export_path(export_id)))
            .expect("simulate a completed first sweep before restart");
        let absence = store
            .erase_controlled_copies(&plan)
            .expect("resume from a strict subset of the original census");
        store
            .finish_controlled_copy_erasure(&plan, absence.receipt_hash())
            .expect("finish after exact absence verification");
    }

    #[test]
    fn hard_link_race_blocks_controlled_copy_absence_receipt_after_restart() {
        let path = TestStorePath::new();
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"copy-erasure-link-race")
            .expect("Store domain");
        let mut store = StoreV1::create(&path.0, domain.clone()).expect("create Store");
        let root = object(100);
        store.put_object(&root).expect("persist root");
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            ContractRootIdV1::parse(&rendered(101)).expect("Contract Root identity"),
            StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
            vec![root.id()],
        )
        .expect("Generation");
        store
            .publish_generation(&generation, None)
            .expect("publish Generation");
        store.seal_export().expect("seal controlled copy");
        let plan = store
            .prepare_controlled_copy_erasure_plan(root.id(), [102; 32], |_| Ok::<(), ()>(()))
            .expect("prepare copy-total erasure plan");
        let escaped_alias = path.0.join("escaped-controlled-copy-alias");
        let alias_for_hook = escaped_alias.clone();
        SecureRoot::install_before_removal_unlink_test_hook(move |quarantine| {
            fs::hard_link(quarantine, &alias_for_hook)
                .expect("create racing controlled-copy hard-link alias");
        });

        assert!(matches!(
            store.erase_controlled_copies(&plan),
            Err(StoreError::ControlledCopyErasureSweepDebt {
                source: SecureFsError::UnsafeObject {
                    reason: "removed file still has a hard-link alias",
                    ..
                },
                ..
            })
        ));
        assert!(escaped_alias.is_file());
        drop(store);

        let reopened =
            StoreV1::open(&path.0, domain).expect("reopen Store after interrupted sweep");
        assert!(matches!(
            reopened.verify_controlled_copy_erasure_absence(&plan),
            Err(StoreError::SecureFs(SecureFsError::UnsafeObject {
                reason: "file removal has unresolved hard-link or crash debt",
                ..
            }))
        ));
    }

    #[test]
    fn failed_sealer_cleanup_cannot_unlink_a_waiting_sealers_committed_carrier() {
        let path = TestStorePath::new();
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"seal-cleanup-race")
            .expect("Store domain");
        let mut setup = StoreV1::create(&path.0, domain.clone()).expect("create Store");
        let root = object(70);
        setup.put_object(&root).expect("persist root");
        let generation = StoreGenerationV1::new(
            domain.clone(),
            1,
            None,
            ContractRootIdV1::parse(&rendered(2)).expect("Contract Root identity"),
            StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
            vec![root.id()],
        )
        .expect("Generation");
        setup
            .publish_generation(&generation, None)
            .expect("publish Generation");
        drop(setup);

        let failed_ready = Arc::new(Barrier::new(2));
        let receipt_committed = Arc::new(Barrier::new(2));
        let finish_allowed = Arc::new(Barrier::new(2));
        let failed_path = path.0.clone();
        let failed_domain = domain.clone();
        let failed_ready_thread = Arc::clone(&failed_ready);
        let receipt_committed_failed = Arc::clone(&receipt_committed);
        let failed = std::thread::spawn(move || {
            let mut store = StoreV1::open(failed_path, failed_domain).expect("open failed sealer");
            store.seal_export_with_before_receipt(|| {
                failed_ready_thread.wait();
                receipt_committed_failed.wait();
            })
        });

        failed_ready.wait();
        let waiting_path = path.0.clone();
        let waiting_domain = domain.clone();
        let receipt_committed_waiting = Arc::clone(&receipt_committed);
        let finish_allowed_waiting = Arc::clone(&finish_allowed);
        let waiting = std::thread::spawn(move || {
            let mut store =
                StoreV1::open(waiting_path, waiting_domain).expect("open waiting sealer");
            install_after_successful_sealed_export_receipt_test_hook(move || {
                receipt_committed_waiting.wait();
                finish_allowed_waiting.wait();
            });
            store.seal_export()
        });

        let failed_result = failed.join().expect("failed sealer thread");
        assert!(
            failed_result.is_err(),
            "the stale sealer must not claim the waiting sealer's receipt"
        );
        let pending_files = fs::read_dir(path.0.join(EXPORTS_DIRECTORY))
            .expect("exports directory")
            .map(|entry| entry.expect("export entry"))
            .filter(|entry| entry.file_type().expect("export file type").is_file())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".pending"))
            .collect::<Vec<_>>();
        assert_eq!(
            pending_files.len(),
            1,
            "failed cleanup must preserve the carrier owned by committed export metadata"
        );

        finish_allowed.wait();
        let backup = waiting
            .join()
            .expect("waiting sealer thread")
            .expect("waiting sealer completes publication");
        assert!(
            !path
                .0
                .join(export_pending_path(backup.export().id()))
                .exists()
        );
        assert_eq!(
            fs::read(path.0.join(export_path(backup.export().id()))).expect("public sealed backup"),
            backup.canonical_bytes()
        );
        let current_clock: i64 = Connection::open(path.0.join(METADATA_FILE))
            .expect("metadata")
            .query_row(
                "SELECT publication_clock FROM store_publication_clock WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .expect("publication clock");
        assert_eq!(
            u64::try_from(current_clock).expect("non-negative publication clock"),
            backup.receipt().committed_publication_clock(),
            "guarded cleanup and filesystem finalization must not advance the publication clock"
        );
    }
}
