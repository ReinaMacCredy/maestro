use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::identity::{
    BackupReceiptIdV1, ManifestIdentityV1, ReachabilitySnapshotIdV1, RestoreCandidateIdV1,
    SchemaIdV1, SealedExportIdV1, StoreDomainIdV1, StoreGenerationIdV1, StoreHeadIdV1,
    StoreObjectIdV1, StoreSchemaManifestIdV1, StoreSnapshotRootIdV1, derive_identity,
};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::snapshot_blocks::{StoreSnapshotBlockClosureV2, StoreSnapshotBlockError};
use super::snapshot_rows::{ReachabilityStatusV1, StoreSnapshotRowV1, StoreStateV1};
use super::{
    GenerationError, LogicalTombstoneV1, ReachabilitySnapshotV1, RetentionError, RetentionPinV1,
    SEALED_EXPORT_FORMAT_V2, STORE_OBJECT_STORAGE_CODEC_V1, StoreGenerationV1, StoreHeadV1,
    StoreObjectError, StoreObjectV1, StoreRoleV1, StoreSnapshotRootV1,
};

pub const SEALED_EXPORT_VERSION_V1: u64 = 1;
pub const SEALED_EXPORT_VERSION_V2: u64 = 2;
pub const RESTORE_CANDIDATE_VERSION_V1: u64 = 1;
pub const BACKUP_RECEIPT_VERSION_V1: u64 = 1;
pub const SEALED_BACKUP_VERSION_V1: u64 = 1;
pub const BACKUP_RECEIPT_FORMAT_V1: &str = "maestro-backup-receipt-v1";
pub const SEALED_BACKUP_FORMAT_V1: &str = "maestro-sealed-backup-v1";
pub const MAX_SEALED_BACKUP_BYTES_V1: usize = 31 * 1024 * 1024;
pub const MAX_EXPORTED_OBJECTS: usize = 65_536;
pub const MAX_EXPORTED_LINEAGE: usize = 65_536;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedExportLineageV1 {
    generation: StoreGenerationV1,
    head: StoreHeadV1,
}

impl SealedExportLineageV1 {
    pub fn new(generation: StoreGenerationV1, head: StoreHeadV1) -> Result<Self, ExportError> {
        if head.generation_id() != generation.id()
            || head.generation_ordinal() != generation.ordinal()
            || head.revision() != generation.ordinal()
            || head.domain() != generation.domain()
        {
            return Err(ExportError::LineageMemberMismatch);
        }
        Ok(Self { generation, head })
    }

    pub fn generation(&self) -> &StoreGenerationV1 {
        &self.generation
    }

    pub fn head(&self) -> &StoreHeadV1 {
        &self.head
    }

    fn canonical_value(&self) -> Result<CborValue, ExportError> {
        Ok(CborValue::Array(vec![
            CborValue::Bytes(self.generation.canonical_bytes()?),
            CborValue::Bytes(self.head.canonical_bytes()?),
        ]))
    }

    fn decode(value: &CborValue) -> Result<Self, ExportError> {
        let CborValue::Array(fields) = value else {
            return Err(ExportError::InvalidShape);
        };
        let [CborValue::Bytes(generation), CborValue::Bytes(head)] = fields.as_slice() else {
            return Err(ExportError::InvalidShape);
        };
        let generation = StoreGenerationV1::decode(generation)?;
        let head = StoreHeadV1::decode_for_generation(head, &generation)?;
        Self::new(generation, head)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TombstonedObjectV1 {
    tombstone: LogicalTombstoneV1,
    schema_id: SchemaIdV1,
    logical_byte_length: u64,
    stored_byte_length: u64,
    stored_bytes_digest: [u8; 32],
    storage_codec: String,
    key_envelope: Option<([u8; 32], String)>,
    references: Vec<StoreObjectIdV1>,
}

impl TombstonedObjectV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "tombstoned metadata must preserve every immutable SQLite object field exactly"
    )]
    pub(crate) fn new(
        tombstone: LogicalTombstoneV1,
        schema_id: SchemaIdV1,
        logical_byte_length: u64,
        stored_byte_length: u64,
        stored_bytes_digest: [u8; 32],
        storage_codec: String,
        key_envelope: Option<([u8; 32], String)>,
        references: Vec<StoreObjectIdV1>,
    ) -> Result<Self, ExportError> {
        if stored_byte_length == 0 {
            return Err(ExportError::InvalidObjectMetadata);
        }
        validate_ascii_label(&storage_codec)?;
        if let Some((_, kind)) = &key_envelope {
            validate_ascii_label(kind)?;
        }
        if references.len() > super::object::MAX_STORE_OBJECT_REFERENCES
            || references.windows(2).any(|pair| pair[0] >= pair[1])
            || references.contains(&tombstone.object_id())
        {
            return Err(ExportError::InvalidObjectMetadata);
        }
        Ok(Self {
            tombstone,
            schema_id,
            logical_byte_length,
            stored_byte_length,
            stored_bytes_digest,
            storage_codec,
            key_envelope,
            references,
        })
    }

    pub fn tombstone(&self) -> &LogicalTombstoneV1 {
        &self.tombstone
    }

    pub fn object_id(&self) -> StoreObjectIdV1 {
        self.tombstone.object_id()
    }

    pub fn schema_id(&self) -> SchemaIdV1 {
        self.schema_id
    }

    pub fn logical_byte_length(&self) -> u64 {
        self.logical_byte_length
    }

    pub fn stored_byte_length(&self) -> u64 {
        self.stored_byte_length
    }

    pub fn stored_bytes_digest(&self) -> &[u8; 32] {
        &self.stored_bytes_digest
    }

    pub fn storage_codec(&self) -> &str {
        &self.storage_codec
    }

    pub fn key_envelope(&self) -> Option<(&[u8; 32], &str)> {
        self.key_envelope
            .as_ref()
            .map(|(id, kind)| (id, kind.as_str()))
    }

    pub fn references(&self) -> &[StoreObjectIdV1] {
        &self.references
    }

    fn canonical_value(&self) -> Result<CborValue, ExportError> {
        Ok(CborValue::Array(vec![
            CborValue::Bytes(self.tombstone.canonical_bytes()?),
            CborValue::Bytes(self.schema_id.as_bytes().to_vec()),
            CborValue::Unsigned(self.logical_byte_length),
            CborValue::Unsigned(self.stored_byte_length),
            CborValue::Bytes(self.stored_bytes_digest.to_vec()),
            CborValue::text(&self.storage_codec)?,
            optional_key_envelope(self.key_envelope.as_ref())?,
            identity_array(&self.references),
        ]))
    }

    fn decode(value: &CborValue) -> Result<Self, ExportError> {
        let CborValue::Array(fields) = value else {
            return Err(ExportError::InvalidShape);
        };
        let [
            CborValue::Bytes(tombstone),
            CborValue::Bytes(schema),
            CborValue::Unsigned(logical_byte_length),
            CborValue::Unsigned(stored_byte_length),
            CborValue::Bytes(stored_bytes_digest),
            CborValue::Text(storage_codec),
            key_envelope,
            CborValue::Array(references),
        ] = fields.as_slice()
        else {
            return Err(ExportError::InvalidShape);
        };
        Self::new(
            LogicalTombstoneV1::decode(tombstone)?,
            identity_from_bytes(schema)?,
            *logical_byte_length,
            *stored_byte_length,
            digest_from_bytes(stored_bytes_digest)?,
            storage_codec.clone(),
            decode_key_envelope(key_envelope)?,
            decode_identity_array(references)?,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SealedExportEntryV1 {
    Available(StoreObjectV1),
    Tombstoned(TombstonedObjectV1),
}

impl SealedExportEntryV1 {
    pub fn object_id(&self) -> StoreObjectIdV1 {
        match self {
            Self::Available(object) => object.id(),
            Self::Tombstoned(object) => object.object_id(),
        }
    }

    pub fn references(&self) -> &[StoreObjectIdV1] {
        match self {
            Self::Available(object) => object.references(),
            Self::Tombstoned(object) => object.references(),
        }
    }

    fn canonical_value(&self) -> Result<CborValue, ExportError> {
        match self {
            Self::Available(object) => Ok(CborValue::Array(vec![
                CborValue::Unsigned(1),
                CborValue::Bytes(object.id().as_bytes().to_vec()),
                CborValue::Bytes(object.canonical_bytes().to_vec()),
            ])),
            Self::Tombstoned(object) => Ok(CborValue::Array(vec![
                CborValue::Unsigned(2),
                CborValue::Bytes(object.object_id().as_bytes().to_vec()),
                object.canonical_value()?,
            ])),
        }
    }

    fn decode(value: &CborValue) -> Result<Self, ExportError> {
        let CborValue::Array(fields) = value else {
            return Err(ExportError::InvalidShape);
        };
        let [
            CborValue::Unsigned(tag),
            CborValue::Bytes(declared_id),
            payload,
        ] = fields.as_slice()
        else {
            return Err(ExportError::InvalidShape);
        };
        let entry = match (tag, payload) {
            (1, CborValue::Bytes(bytes)) => Self::Available(StoreObjectV1::decode(bytes)?),
            (2, value) => Self::Tombstoned(TombstonedObjectV1::decode(value)?),
            (1, _) => return Err(ExportError::InvalidShape),
            _ => return Err(ExportError::UnknownEntryTag(*tag)),
        };
        if entry.object_id().as_bytes().as_slice() != declared_id {
            return Err(ExportError::EntryIdentityMismatch);
        }
        Ok(entry)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedExportV1 {
    lineage: Vec<SealedExportLineageV1>,
    object_inventory: Vec<StoreObjectIdV1>,
    reachability: ReachabilitySnapshotV1,
    retention_pins: Vec<RetentionPinV1>,
    entries: Vec<SealedExportEntryV1>,
    source_publication_clock: Option<u64>,
    snapshot_root: Option<StoreSnapshotRootV1>,
    snapshot_blocks: Option<StoreSnapshotBlockClosureV2>,
    id: SealedExportIdV1,
    canonical_bytes: Vec<u8>,
}

impl SealedExportV1 {
    pub fn new(
        generation: StoreGenerationV1,
        head: StoreHeadV1,
        reachability: ReachabilitySnapshotV1,
        retention_pins: Vec<RetentionPinV1>,
        entries: Vec<SealedExportEntryV1>,
    ) -> Result<Self, ExportError> {
        Self::new_with_lineage(
            vec![SealedExportLineageV1::new(generation, head)?],
            reachability,
            retention_pins,
            entries,
        )
    }

    pub(crate) fn new_full_history(
        lineage: Vec<SealedExportLineageV1>,
        reachability: ReachabilitySnapshotV1,
        retention_pins: Vec<RetentionPinV1>,
        entries: Vec<SealedExportEntryV1>,
        snapshot_blocks: StoreSnapshotBlockClosureV2,
    ) -> Result<Self, ExportError> {
        let object_inventory = entries.iter().map(SealedExportEntryV1::object_id).collect();
        let snapshot_root = snapshot_blocks.current_snapshot()?;
        Self::new_verified(
            lineage,
            object_inventory,
            reachability,
            retention_pins,
            entries,
            Some((snapshot_root, snapshot_blocks)),
        )
    }

    pub fn new_with_lineage(
        lineage: Vec<SealedExportLineageV1>,
        reachability: ReachabilitySnapshotV1,
        retention_pins: Vec<RetentionPinV1>,
        entries: Vec<SealedExportEntryV1>,
    ) -> Result<Self, ExportError> {
        let object_inventory = entries.iter().map(SealedExportEntryV1::object_id).collect();
        Self::new_verified(
            lineage,
            object_inventory,
            reachability,
            retention_pins,
            entries,
            None,
        )
    }

    fn new_verified(
        lineage: Vec<SealedExportLineageV1>,
        object_inventory: Vec<StoreObjectIdV1>,
        reachability: ReachabilitySnapshotV1,
        retention_pins: Vec<RetentionPinV1>,
        entries: Vec<SealedExportEntryV1>,
        snapshot: Option<(StoreSnapshotRootV1, StoreSnapshotBlockClosureV2)>,
    ) -> Result<Self, ExportError> {
        if lineage.is_empty() || lineage.len() > MAX_EXPORTED_LINEAGE {
            return Err(ExportError::InvalidLineageLength);
        }
        if entries.len() > MAX_EXPORTED_OBJECTS {
            return Err(ExportError::TooManyEntries);
        }
        validate_lineage(&lineage)?;
        let current = lineage.last().expect("invariant: lineage is nonempty");
        if reachability.head_id() != current.head().id() {
            return Err(ExportError::SnapshotBasisMismatch);
        }
        if retention_pins
            .windows(2)
            .any(|pair| pair[0].id() >= pair[1].id())
        {
            return Err(ExportError::PinsNotStrictlySorted);
        }
        if entries
            .windows(2)
            .any(|pair| pair[0].object_id() >= pair[1].object_id())
        {
            return Err(ExportError::EntriesNotStrictlySorted);
        }
        let entry_ids = entries
            .iter()
            .map(SealedExportEntryV1::object_id)
            .collect::<Vec<_>>();
        if object_inventory != entry_ids
            || object_inventory.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ExportError::ObjectInventoryMismatch);
        }

        let available = entries
            .iter()
            .filter_map(|entry| match entry {
                SealedExportEntryV1::Available(object) => Some(object.id()),
                SealedExportEntryV1::Tombstoned(_) => None,
            })
            .collect::<Vec<_>>();
        let tombstoned = entries
            .iter()
            .filter_map(|entry| match entry {
                SealedExportEntryV1::Available(_) => None,
                SealedExportEntryV1::Tombstoned(object) => Some(object.object_id()),
            })
            .collect::<Vec<_>>();
        if reachability
            .reachable()
            .iter()
            .any(|object| available.binary_search(object).is_err())
            || tombstoned != reachability.tombstoned()
        {
            return Err(ExportError::ReachabilityMismatch);
        }
        if lineage.iter().any(|member| {
            member
                .generation()
                .roots()
                .iter()
                .any(|root| object_inventory.binary_search(root).is_err())
        }) {
            return Err(ExportError::GenerationRootMissing);
        }
        if entries.iter().any(|entry| {
            entry
                .references()
                .iter()
                .any(|reference| object_inventory.binary_search(reference).is_err())
        }) {
            return Err(ExportError::ReferenceClosureIncomplete);
        }
        let lineage_heads = lineage
            .iter()
            .map(|member| member.head().id())
            .collect::<BTreeSet<_>>();
        if entries.iter().any(|entry| match entry {
            SealedExportEntryV1::Available(_) => false,
            SealedExportEntryV1::Tombstoned(object) => {
                !lineage_heads.contains(&object.tombstone().basis_head_id())
            }
        }) {
            return Err(ExportError::TombstoneBasisMismatch);
        }
        if retention_pins.iter().any(|pin| {
            !lineage_heads.contains(&pin.basis_head_id())
                || reachability.roots().binary_search(pin.root()).is_err()
        }) {
            return Err(ExportError::RetentionPinMismatch);
        }
        if let Some((snapshot_root, _)) = &snapshot {
            validate_snapshot_export_basis(
                snapshot_root,
                &lineage,
                &reachability,
                &retention_pins,
                &entries,
            )?;
        }
        let source_publication_clock = snapshot
            .as_ref()
            .map(|(snapshot_root, _)| snapshot_root.publication_clock());
        let value = export_value(
            &lineage,
            &object_inventory,
            &reachability,
            &retention_pins,
            &entries,
            source_publication_clock,
            snapshot.as_ref().map(|(_, blocks)| blocks),
        )?;
        let canonical_bytes = deterministic_cbor::encode(&value)?;
        let id = derive_identity(&value)?;
        let (snapshot_root, snapshot_blocks) = match snapshot {
            Some((root, blocks)) => (Some(root), Some(blocks)),
            None => (None, None),
        };
        Ok(Self {
            lineage,
            object_inventory,
            reachability,
            retention_pins,
            entries,
            source_publication_clock,
            snapshot_root,
            snapshot_blocks,
            id,
            canonical_bytes,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ExportError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = value else {
            return Err(ExportError::InvalidShape);
        };
        let (
            lineage,
            object_inventory,
            reachability,
            pin_bytes,
            entry_values,
            source_publication_clock,
            snapshot_blocks,
        ) = match fields.as_slice() {
            [
                CborValue::Unsigned(version),
                CborValue::Array(lineage),
                CborValue::Array(object_inventory),
                CborValue::Bytes(reachability),
                CborValue::Array(pin_bytes),
                CborValue::Array(entry_values),
            ] if *version == SEALED_EXPORT_VERSION_V1 => (
                lineage,
                object_inventory,
                reachability,
                pin_bytes,
                entry_values,
                None,
                None,
            ),
            [
                CborValue::Unsigned(version),
                CborValue::Array(lineage),
                CborValue::Array(object_inventory),
                CborValue::Bytes(reachability),
                CborValue::Array(pin_bytes),
                CborValue::Array(entry_values),
                CborValue::Unsigned(source_publication_clock),
                CborValue::Bytes(snapshot_blocks),
            ] if *version == SEALED_EXPORT_VERSION_V2 => (
                lineage,
                object_inventory,
                reachability,
                pin_bytes,
                entry_values,
                Some(*source_publication_clock),
                Some(StoreSnapshotBlockClosureV2::decode(snapshot_blocks)?),
            ),
            [CborValue::Unsigned(version), ..] => {
                return Err(ExportError::UnknownVersion(*version));
            }
            _ => return Err(ExportError::InvalidShape),
        };
        let snapshot = match snapshot_blocks {
            Some(blocks) => {
                let snapshot_root = blocks.current_snapshot()?;
                if source_publication_clock != Some(snapshot_root.publication_clock()) {
                    return Err(ExportError::SnapshotClosureBasisMismatch(
                        "source publication clock",
                    ));
                }
                Some((snapshot_root, blocks))
            }
            None => None,
        };
        let export = Self::new_verified(
            lineage
                .iter()
                .map(SealedExportLineageV1::decode)
                .collect::<Result<Vec<_>, _>>()?,
            decode_identity_array(object_inventory)?,
            ReachabilitySnapshotV1::decode(reachability)?,
            pin_bytes
                .iter()
                .map(|value| {
                    let CborValue::Bytes(bytes) = value else {
                        return Err(ExportError::InvalidShape);
                    };
                    Ok(RetentionPinV1::decode(bytes)?)
                })
                .collect::<Result<Vec<_>, ExportError>>()?,
            entry_values
                .iter()
                .map(SealedExportEntryV1::decode)
                .collect::<Result<Vec<_>, _>>()?,
            snapshot,
        )?;
        if export.canonical_bytes != bytes {
            return Err(ExportError::NonCanonicalBytes);
        }
        Ok(export)
    }

    pub fn lineage(&self) -> &[SealedExportLineageV1] {
        &self.lineage
    }

    pub fn generation(&self) -> &StoreGenerationV1 {
        self.lineage
            .last()
            .expect("invariant: verified export lineage is nonempty")
            .generation()
    }

    pub fn head(&self) -> &StoreHeadV1 {
        self.lineage
            .last()
            .expect("invariant: verified export lineage is nonempty")
            .head()
    }

    pub fn object_inventory(&self) -> &[StoreObjectIdV1] {
        &self.object_inventory
    }

    pub fn reachability(&self) -> &ReachabilitySnapshotV1 {
        &self.reachability
    }

    pub fn retention_pins(&self) -> &[RetentionPinV1] {
        &self.retention_pins
    }

    pub fn entries(&self) -> &[SealedExportEntryV1] {
        &self.entries
    }

    pub fn source_publication_clock(&self) -> Option<u64> {
        self.source_publication_clock
    }

    pub fn snapshot_root(&self) -> Option<&StoreSnapshotRootV1> {
        self.snapshot_root.as_ref()
    }

    pub(crate) fn snapshot_blocks(&self) -> Option<&StoreSnapshotBlockClosureV2> {
        self.snapshot_blocks.as_ref()
    }

    pub fn id(&self) -> SealedExportIdV1 {
        self.id
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Immutable audit receipt for one committed full-history export.
///
/// The receipt binds provenance and bytes but grants no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupReceiptV1 {
    source_role: StoreRoleV1,
    source_domain_id: StoreDomainIdV1,
    export_id: SealedExportIdV1,
    head_id: StoreHeadIdV1,
    generation_id: StoreGenerationIdV1,
    reachability_snapshot_id: ReachabilitySnapshotIdV1,
    snapshot_root_id: StoreSnapshotRootIdV1,
    schema_manifest_id: StoreSchemaManifestIdV1,
    family_manifest_set_digest: [u8; 32],
    payload_set_digest: [u8; 32],
    source_publication_clock: u64,
    committed_publication_clock: u64,
    inner_export_byte_length: u64,
    inner_export_bytes_digest: [u8; 32],
    id: BackupReceiptIdV1,
    canonical_bytes: Vec<u8>,
}

impl BackupReceiptV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "the frozen BackupReceiptV1 tuple binds every provenance fact explicitly"
    )]
    pub fn new(
        source_role: StoreRoleV1,
        source_domain_id: StoreDomainIdV1,
        export_id: SealedExportIdV1,
        head_id: StoreHeadIdV1,
        generation_id: StoreGenerationIdV1,
        reachability_snapshot_id: ReachabilitySnapshotIdV1,
        snapshot_root_id: StoreSnapshotRootIdV1,
        schema_manifest_id: StoreSchemaManifestIdV1,
        family_manifest_set_digest: [u8; 32],
        payload_set_digest: [u8; 32],
        source_publication_clock: u64,
        committed_publication_clock: u64,
        inner_export_byte_length: u64,
        inner_export_bytes_digest: [u8; 32],
    ) -> Result<Self, ExportError> {
        let expected_committed_clock = source_publication_clock
            .checked_add(1)
            .ok_or(ExportError::BackupReceiptClockOverflow)?;
        if committed_publication_clock != expected_committed_clock {
            return Err(ExportError::BackupReceiptClockMismatch);
        }
        if inner_export_byte_length == 0 {
            return Err(ExportError::InvalidBackupReceiptExportLength);
        }
        let canonical_value = backup_receipt_value(
            source_role,
            source_domain_id,
            export_id,
            head_id,
            generation_id,
            reachability_snapshot_id,
            snapshot_root_id,
            schema_manifest_id,
            &family_manifest_set_digest,
            &payload_set_digest,
            source_publication_clock,
            committed_publication_clock,
            inner_export_byte_length,
            &inner_export_bytes_digest,
        )?;
        let id = derive_identity(&canonical_value)?;
        let canonical_bytes = deterministic_cbor::encode(&canonical_value)?;
        Ok(Self {
            source_role,
            source_domain_id,
            export_id,
            head_id,
            generation_id,
            reachability_snapshot_id,
            snapshot_root_id,
            schema_manifest_id,
            family_manifest_set_digest,
            payload_set_digest,
            source_publication_clock,
            committed_publication_clock,
            inner_export_byte_length,
            inner_export_bytes_digest,
            id,
            canonical_bytes,
        })
    }

    pub fn for_committed_export(
        export: &SealedExportV1,
        committed_publication_clock: u64,
    ) -> Result<Self, ExportError> {
        let snapshot_root = export
            .snapshot_root()
            .ok_or(ExportError::BackupReceiptRequiresFullHistoryExport)?;
        let source_publication_clock = export
            .source_publication_clock()
            .ok_or(ExportError::BackupReceiptRequiresFullHistoryExport)?;
        let inner_export_byte_length = u64::try_from(export.canonical_bytes().len())
            .map_err(|_| ExportError::InvalidBackupReceiptExportLength)?;
        Self::new(
            export.generation().domain().role(),
            export.generation().domain().id(),
            export.id(),
            export.head().id(),
            export.generation().id(),
            export.reachability().id(),
            snapshot_root.id(),
            snapshot_root.schema_manifest_id(),
            *snapshot_root.family_manifest_set_digest(),
            *snapshot_root.payload_set_digest(),
            source_publication_clock,
            committed_publication_clock,
            inner_export_byte_length,
            Sha256::digest(export.canonical_bytes()).into(),
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ExportError> {
        let value = deterministic_cbor::decode(bytes)?;
        let receipt = Self::decode_value(&value)?;
        if receipt.canonical_bytes != bytes {
            return Err(ExportError::NonCanonicalBackupReceipt);
        }
        Ok(receipt)
    }

    fn decode_value(value: &CborValue) -> Result<Self, ExportError> {
        let CborValue::Array(fields) = value else {
            return Err(ExportError::InvalidBackupReceiptShape);
        };
        let [
            CborValue::Unsigned(version),
            CborValue::Text(format),
            CborValue::Text(inner_export_format),
            CborValue::Unsigned(source_role),
            CborValue::Bytes(source_domain_id),
            CborValue::Bytes(export_id),
            CborValue::Bytes(head_id),
            CborValue::Bytes(generation_id),
            CborValue::Bytes(reachability_snapshot_id),
            CborValue::Bytes(snapshot_root_id),
            CborValue::Bytes(schema_manifest_id),
            CborValue::Bytes(family_manifest_set_digest),
            CborValue::Bytes(payload_set_digest),
            CborValue::Unsigned(source_publication_clock),
            CborValue::Unsigned(committed_publication_clock),
            CborValue::Unsigned(inner_export_byte_length),
            CborValue::Bytes(inner_export_bytes_digest),
        ] = fields.as_slice()
        else {
            return Err(ExportError::InvalidBackupReceiptShape);
        };
        if *version != BACKUP_RECEIPT_VERSION_V1 {
            return Err(ExportError::UnknownBackupReceiptVersion(*version));
        }
        if format != BACKUP_RECEIPT_FORMAT_V1 || inner_export_format != SEALED_EXPORT_FORMAT_V2 {
            return Err(ExportError::UnknownBackupReceiptFormat);
        }
        Self::new(
            StoreRoleV1::from_tag(*source_role)
                .map_err(|_| ExportError::UnknownBackupReceiptSourceRole(*source_role))?,
            identity_from_bytes(source_domain_id)?,
            identity_from_bytes(export_id)?,
            identity_from_bytes(head_id)?,
            identity_from_bytes(generation_id)?,
            identity_from_bytes(reachability_snapshot_id)?,
            identity_from_bytes(snapshot_root_id)?,
            identity_from_bytes(schema_manifest_id)?,
            digest_from_bytes(family_manifest_set_digest)?,
            digest_from_bytes(payload_set_digest)?,
            *source_publication_clock,
            *committed_publication_clock,
            *inner_export_byte_length,
            digest_from_bytes(inner_export_bytes_digest)?,
        )
    }

    fn canonical_value(&self) -> Result<CborValue, ExportError> {
        backup_receipt_value(
            self.source_role,
            self.source_domain_id,
            self.export_id,
            self.head_id,
            self.generation_id,
            self.reachability_snapshot_id,
            self.snapshot_root_id,
            self.schema_manifest_id,
            &self.family_manifest_set_digest,
            &self.payload_set_digest,
            self.source_publication_clock,
            self.committed_publication_clock,
            self.inner_export_byte_length,
            &self.inner_export_bytes_digest,
        )
    }

    pub fn source_role(&self) -> StoreRoleV1 {
        self.source_role
    }

    pub fn source_domain_id(&self) -> StoreDomainIdV1 {
        self.source_domain_id
    }

    pub fn export_id(&self) -> SealedExportIdV1 {
        self.export_id
    }

    pub fn head_id(&self) -> StoreHeadIdV1 {
        self.head_id
    }

    pub fn generation_id(&self) -> StoreGenerationIdV1 {
        self.generation_id
    }

    pub fn reachability_snapshot_id(&self) -> ReachabilitySnapshotIdV1 {
        self.reachability_snapshot_id
    }

    pub fn snapshot_root_id(&self) -> StoreSnapshotRootIdV1 {
        self.snapshot_root_id
    }

    pub fn schema_manifest_id(&self) -> StoreSchemaManifestIdV1 {
        self.schema_manifest_id
    }

    pub fn family_manifest_set_digest(&self) -> &[u8; 32] {
        &self.family_manifest_set_digest
    }

    pub fn payload_set_digest(&self) -> &[u8; 32] {
        &self.payload_set_digest
    }

    pub fn source_publication_clock(&self) -> u64 {
        self.source_publication_clock
    }

    pub fn committed_publication_clock(&self) -> u64 {
        self.committed_publication_clock
    }

    pub fn inner_export_byte_length(&self) -> u64 {
        self.inner_export_byte_length
    }

    pub fn inner_export_bytes_digest(&self) -> &[u8; 32] {
        &self.inner_export_bytes_digest
    }

    pub fn format(&self) -> &'static str {
        BACKUP_RECEIPT_FORMAT_V1
    }

    pub fn inner_export_format(&self) -> &'static str {
        SEALED_EXPORT_FORMAT_V2
    }

    pub fn id(&self) -> BackupReceiptIdV1 {
        self.id
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Canonical public backup carrier. The embedded export remains independently auditable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedBackupV1 {
    export: SealedExportV1,
    receipt: BackupReceiptV1,
    canonical_bytes: Vec<u8>,
}

impl SealedBackupV1 {
    pub fn new(export: SealedExportV1, receipt: BackupReceiptV1) -> Result<Self, ExportError> {
        validate_backup_receipt_parity(&export, &receipt)?;
        let export_value = deterministic_cbor::decode(export.canonical_bytes())?;
        let value = sealed_backup_value(export_value, receipt.canonical_value()?);
        let canonical_bytes = deterministic_cbor::encode(&value)?;
        if canonical_bytes.len() > MAX_SEALED_BACKUP_BYTES_V1 {
            return Err(ExportError::SealedBackupBytesLimitExceeded);
        }
        Ok(Self {
            export,
            receipt,
            canonical_bytes,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ExportError> {
        if bytes.len() > MAX_SEALED_BACKUP_BYTES_V1 {
            return Err(ExportError::SealedBackupBytesLimitExceeded);
        }
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = &value else {
            return Err(ExportError::InvalidSealedBackupShape);
        };
        let [
            CborValue::Unsigned(version),
            CborValue::Text(format),
            export_value,
            receipt_value,
        ] = fields.as_slice()
        else {
            return Err(ExportError::InvalidSealedBackupShape);
        };
        if *version != SEALED_BACKUP_VERSION_V1 {
            return Err(ExportError::UnknownSealedBackupVersion(*version));
        }
        if format != SEALED_BACKUP_FORMAT_V1 {
            return Err(ExportError::UnknownSealedBackupFormat);
        }
        let export = SealedExportV1::decode(&deterministic_cbor::encode(export_value)?)?;
        let receipt = BackupReceiptV1::decode_value(receipt_value)?;
        let backup = Self::new(export, receipt)?;
        if backup.canonical_bytes != bytes {
            return Err(ExportError::NonCanonicalSealedBackup);
        }
        Ok(backup)
    }

    pub fn export(&self) -> &SealedExportV1 {
        &self.export
    }

    pub fn receipt(&self) -> &BackupReceiptV1 {
        &self.receipt
    }

    pub fn format(&self) -> &'static str {
        SEALED_BACKUP_FORMAT_V1
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

impl std::ops::Deref for SealedBackupV1 {
    type Target = SealedExportV1;

    fn deref(&self) -> &Self::Target {
        self.export()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreCandidateV1 {
    source_export_id: SealedExportIdV1,
    source_domain_id: StoreDomainIdV1,
    source_export_bytes_digest: [u8; 32],
    destination_domain_id: StoreDomainIdV1,
    candidate_generation_id: StoreGenerationIdV1,
    candidate_head_id: StoreHeadIdV1,
    candidate_snapshot_id: ReachabilitySnapshotIdV1,
    candidate_roots: Vec<StoreObjectIdV1>,
    verification_digest: [u8; 32],
    id: RestoreCandidateIdV1,
    canonical_bytes: Vec<u8>,
}

impl RestoreCandidateV1 {
    #[allow(
        clippy::too_many_arguments,
        reason = "the frozen RestoreCandidateV1 tuple is intentionally explicit and positional"
    )]
    pub fn new(
        source_export_id: SealedExportIdV1,
        source_domain_id: StoreDomainIdV1,
        source_export_bytes_digest: [u8; 32],
        destination_domain_id: StoreDomainIdV1,
        candidate_generation_id: StoreGenerationIdV1,
        candidate_head_id: StoreHeadIdV1,
        candidate_snapshot_id: ReachabilitySnapshotIdV1,
        mut candidate_roots: Vec<StoreObjectIdV1>,
        verification_digest: [u8; 32],
    ) -> Result<Self, ExportError> {
        candidate_roots.sort();
        candidate_roots.dedup();
        if candidate_roots.is_empty() {
            return Err(ExportError::EmptyCandidateRoots);
        }
        let expected_verification_digest: [u8; 32] =
            Sha256::digest(deterministic_cbor::encode(&restore_verification_value(
                source_export_id,
                source_domain_id,
                &source_export_bytes_digest,
                destination_domain_id,
                candidate_generation_id,
                candidate_head_id,
                candidate_snapshot_id,
                &candidate_roots,
            ))?)
            .into();
        if verification_digest != expected_verification_digest {
            return Err(ExportError::RestoreVerificationDigestMismatch);
        }
        let value = restore_candidate_value(
            source_export_id,
            source_domain_id,
            &source_export_bytes_digest,
            destination_domain_id,
            candidate_generation_id,
            candidate_head_id,
            candidate_snapshot_id,
            &candidate_roots,
            &verification_digest,
        );
        let id = derive_identity(&value)?;
        let canonical_bytes = deterministic_cbor::encode(&value)?;
        Ok(Self {
            source_export_id,
            source_domain_id,
            source_export_bytes_digest,
            destination_domain_id,
            candidate_generation_id,
            candidate_head_id,
            candidate_snapshot_id,
            candidate_roots,
            verification_digest,
            id,
            canonical_bytes,
        })
    }

    pub fn for_verified_export(
        export: &SealedExportV1,
        destination_domain_id: StoreDomainIdV1,
    ) -> Result<Self, ExportError> {
        let source_export_bytes_digest: [u8; 32] = Sha256::digest(export.canonical_bytes()).into();
        let roots = export.generation().roots().to_vec();
        let verification_value = restore_verification_value(
            export.id(),
            export.generation().domain().id(),
            &source_export_bytes_digest,
            destination_domain_id,
            export.generation().id(),
            export.head().id(),
            export.reachability().id(),
            &roots,
        );
        let verification_digest: [u8; 32] =
            Sha256::digest(deterministic_cbor::encode(&verification_value)?).into();
        Self::new(
            export.id(),
            export.generation().domain().id(),
            source_export_bytes_digest,
            destination_domain_id,
            export.generation().id(),
            export.head().id(),
            export.reachability().id(),
            roots,
            verification_digest,
        )
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ExportError> {
        let value = deterministic_cbor::decode(bytes)?;
        let CborValue::Array(fields) = value else {
            return Err(ExportError::InvalidShape);
        };
        let [
            CborValue::Unsigned(version),
            CborValue::Bytes(source_export_id),
            CborValue::Bytes(source_domain_id),
            CborValue::Bytes(source_export_bytes_digest),
            CborValue::Bytes(destination_domain_id),
            CborValue::Bytes(candidate_generation_id),
            CborValue::Bytes(candidate_head_id),
            CborValue::Bytes(candidate_snapshot_id),
            CborValue::Array(candidate_roots),
            CborValue::Bytes(verification_digest),
        ] = fields.as_slice()
        else {
            return Err(ExportError::InvalidShape);
        };
        if *version != RESTORE_CANDIDATE_VERSION_V1 {
            return Err(ExportError::UnknownRestoreCandidateVersion(*version));
        }
        let candidate = Self::new(
            identity_from_bytes(source_export_id)?,
            identity_from_bytes(source_domain_id)?,
            digest_from_bytes(source_export_bytes_digest)?,
            identity_from_bytes(destination_domain_id)?,
            identity_from_bytes(candidate_generation_id)?,
            identity_from_bytes(candidate_head_id)?,
            identity_from_bytes(candidate_snapshot_id)?,
            decode_identity_array(candidate_roots)?,
            digest_from_bytes(verification_digest)?,
        )?;
        if candidate.canonical_bytes != bytes {
            return Err(ExportError::NonCanonicalBytes);
        }
        Ok(candidate)
    }

    pub fn source_export_id(&self) -> SealedExportIdV1 {
        self.source_export_id
    }

    pub fn source_domain_id(&self) -> StoreDomainIdV1 {
        self.source_domain_id
    }

    pub fn source_export_bytes_digest(&self) -> &[u8; 32] {
        &self.source_export_bytes_digest
    }

    pub fn destination_domain_id(&self) -> StoreDomainIdV1 {
        self.destination_domain_id
    }

    pub fn candidate_generation_id(&self) -> StoreGenerationIdV1 {
        self.candidate_generation_id
    }

    pub fn candidate_head_id(&self) -> StoreHeadIdV1 {
        self.candidate_head_id
    }

    pub fn candidate_snapshot_id(&self) -> ReachabilitySnapshotIdV1 {
        self.candidate_snapshot_id
    }

    pub fn candidate_roots(&self) -> &[StoreObjectIdV1] {
        &self.candidate_roots
    }

    pub fn verification_digest(&self) -> &[u8; 32] {
        &self.verification_digest
    }

    pub fn id(&self) -> RestoreCandidateIdV1 {
        self.id
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

fn validate_lineage(lineage: &[SealedExportLineageV1]) -> Result<(), ExportError> {
    let first = &lineage[0];
    if first.generation().ordinal() != 1
        || first.generation().previous().is_some()
        || first.head().revision() != 1
        || first.head().previous_head_id().is_some()
    {
        return Err(ExportError::IncompleteLineage);
    }
    for pair in lineage.windows(2) {
        let previous = &pair[0];
        let current = &pair[1];
        if current.generation().domain() != previous.generation().domain()
            || current.generation().ordinal() != previous.generation().ordinal() + 1
            || current.generation().previous() != Some(previous.generation().id())
            || current.head().revision() != previous.head().revision() + 1
            || current.head().previous_head_id() != Some(previous.head().id())
        {
            return Err(ExportError::IncompleteLineage);
        }
    }
    Ok(())
}

fn validate_snapshot_export_basis(
    snapshot: &StoreSnapshotRootV1,
    lineage: &[SealedExportLineageV1],
    reachability: &ReachabilitySnapshotV1,
    retention_pins: &[RetentionPinV1],
    entries: &[SealedExportEntryV1],
) -> Result<(), ExportError> {
    let current = lineage
        .last()
        .ok_or(ExportError::SnapshotClosureBasisMismatch("lineage"))?;
    if snapshot.role() != current.generation().domain().role()
        || snapshot.domain_id() != current.generation().domain().id()
    {
        return Err(ExportError::SnapshotClosureBasisMismatch("domain"));
    }
    validate_snapshot_lineage(snapshot.rows(), lineage)?;
    validate_snapshot_authority(snapshot.rows(), current, reachability)?;
    validate_snapshot_reachability(snapshot.rows(), reachability)?;
    validate_snapshot_pins(snapshot.rows(), retention_pins)?;
    validate_snapshot_entries(snapshot, entries)
}

fn validate_snapshot_lineage(
    rows: &[StoreSnapshotRowV1],
    lineage: &[SealedExportLineageV1],
) -> Result<(), ExportError> {
    let mut actual_generations = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::Generation {
                generation_id,
                generation_ordinal,
                previous_generation_id,
                contract_root_id,
                writer_compatibility_manifest_id,
                association_schema_id,
                finality_edge_manifest_id,
                schema_read_write_set_descriptor_id,
                writer_protocol_epoch_id,
                migration_epoch_id,
            } => Some((
                *generation_id.as_bytes(),
                generation_ordinal.get(),
                previous_generation_id.map(|id| *id.as_bytes()),
                *contract_root_id.as_bytes(),
                *writer_compatibility_manifest_id.as_bytes(),
                *association_schema_id.as_bytes(),
                *finality_edge_manifest_id.as_bytes(),
                *schema_read_write_set_descriptor_id.as_bytes(),
                *writer_protocol_epoch_id.as_bytes(),
                *migration_epoch_id.as_bytes(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected_generations = lineage
        .iter()
        .map(|member| {
            let generation = member.generation();
            let compatibility = generation.compatibility();
            (
                *generation.id().as_bytes(),
                generation.ordinal(),
                generation.previous().map(|id| *id.as_bytes()),
                *generation.contract_root_id().as_bytes(),
                *compatibility.writer_compatibility_manifest_id().as_bytes(),
                *compatibility.association_schema_id().as_bytes(),
                *compatibility.finality_edge_manifest_id().as_bytes(),
                *compatibility
                    .schema_read_write_set_descriptor_id()
                    .as_bytes(),
                *compatibility.writer_protocol_epoch_id().as_bytes(),
                *compatibility.migration_epoch_id().as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    actual_generations.sort();
    expected_generations.sort();
    if actual_generations != expected_generations {
        return Err(ExportError::SnapshotClosureBasisMismatch("generations"));
    }

    let mut actual_roots = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::GenerationRoot {
                generation_id,
                root_position,
                object_id,
            } => Some((
                *generation_id.as_bytes(),
                root_position.get(),
                *object_id.as_bytes(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected_roots = lineage
        .iter()
        .flat_map(|member| {
            member
                .generation()
                .roots()
                .iter()
                .enumerate()
                .map(move |(position, object_id)| {
                    (
                        *member.generation().id().as_bytes(),
                        position as u64,
                        *object_id.as_bytes(),
                    )
                })
        })
        .collect::<Vec<_>>();
    actual_roots.sort();
    expected_roots.sort();
    if actual_roots != expected_roots {
        return Err(ExportError::SnapshotClosureBasisMismatch(
            "generation roots",
        ));
    }

    let mut actual_heads = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::Head {
                head_id,
                generation_id,
                generation_ordinal,
                head_revision,
                previous_head_id,
            } => Some((
                *head_id.as_bytes(),
                *generation_id.as_bytes(),
                generation_ordinal.get(),
                head_revision.get(),
                previous_head_id.map(|id| *id.as_bytes()),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected_heads = lineage
        .iter()
        .map(|member| {
            let head = member.head();
            (
                *head.id().as_bytes(),
                *head.generation_id().as_bytes(),
                head.generation_ordinal(),
                head.revision(),
                head.previous_head_id().map(|id| *id.as_bytes()),
            )
        })
        .collect::<Vec<_>>();
    actual_heads.sort();
    expected_heads.sort();
    if actual_heads != expected_heads {
        return Err(ExportError::SnapshotClosureBasisMismatch("heads"));
    }
    Ok(())
}

fn validate_snapshot_authority(
    rows: &[StoreSnapshotRowV1],
    current: &SealedExportLineageV1,
    reachability: &ReachabilitySnapshotV1,
) -> Result<(), ExportError> {
    let states = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::State {
                state,
                state_revision,
                ..
            } => Some((*state, state_revision.get())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let active_heads = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::ActiveHead {
                head_id,
                head_revision,
                ..
            } => Some((*head_id.as_bytes(), head_revision.get())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let retention_revisions = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::RetentionRevision {
                retention_revision, ..
            } => Some(retention_revision.get()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if states.len() != 1 || retention_revisions != vec![reachability.retention_revision()] {
        return Err(ExportError::SnapshotClosureBasisMismatch(
            "source authority",
        ));
    }
    let pointer_matches_state = match states[0].0 {
        StoreStateV1::Active => {
            states[0].1 > 0
                && active_heads
                    == vec![(*current.head().id().as_bytes(), current.head().revision())]
        }
        StoreStateV1::Inactive => {
            active_heads == vec![(*current.head().id().as_bytes(), current.head().revision())]
        }
    };
    if !pointer_matches_state {
        return Err(ExportError::SnapshotClosureBasisMismatch(
            "source authority",
        ));
    }
    Ok(())
}

fn validate_snapshot_reachability(
    rows: &[StoreSnapshotRowV1],
    reachability: &ReachabilitySnapshotV1,
) -> Result<(), ExportError> {
    let basis = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::ReachabilitySnapshot {
                snapshot_id,
                head_id,
                retention_revision,
            } if snapshot_id.as_bytes() == reachability.id().as_bytes() => {
                Some((*head_id.as_bytes(), retention_revision.get()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if basis
        != vec![(
            *reachability.head_id().as_bytes(),
            reachability.retention_revision(),
        )]
    {
        return Err(ExportError::SnapshotClosureBasisMismatch("reachability"));
    }
    let actual_roots = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::ReachabilityRoot {
                snapshot_id,
                root_position,
                root_kind,
                object_id,
            } if snapshot_id.as_bytes() == reachability.id().as_bytes() => {
                Some((root_position.get(), root_kind.tag(), *object_id.as_bytes()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected_roots = reachability
        .roots()
        .iter()
        .enumerate()
        .map(|(position, root)| {
            (
                position as u64,
                root.kind().tag(),
                *root.object_id().as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    if actual_roots != expected_roots {
        return Err(ExportError::SnapshotClosureBasisMismatch(
            "reachability roots",
        ));
    }
    let mut actual_objects = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::ReachabilityObject {
                snapshot_id,
                object_id,
                reachability_status,
            } if snapshot_id.as_bytes() == reachability.id().as_bytes() => {
                Some((*object_id.as_bytes(), *reachability_status))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected_objects = reachability
        .reachable()
        .iter()
        .map(|object_id| (*object_id.as_bytes(), ReachabilityStatusV1::Reachable))
        .chain(
            reachability
                .tombstoned()
                .iter()
                .map(|object_id| (*object_id.as_bytes(), ReachabilityStatusV1::Tombstoned)),
        )
        .collect::<Vec<_>>();
    actual_objects.sort();
    expected_objects.sort();
    if actual_objects != expected_objects {
        return Err(ExportError::SnapshotClosureBasisMismatch(
            "reachability objects",
        ));
    }
    Ok(())
}

fn validate_snapshot_pins(
    rows: &[StoreSnapshotRowV1],
    retention_pins: &[RetentionPinV1],
) -> Result<(), ExportError> {
    let released = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::RetentionPinRelease { pin_id, .. } => Some(*pin_id.as_bytes()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut actual = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::RetentionPin {
                pin_id,
                basis_head_id,
                root_kind,
                root_object_id,
                reason_digest,
            } if !released.contains(pin_id.as_bytes()) => Some((
                *pin_id.as_bytes(),
                *basis_head_id.as_bytes(),
                root_kind.tag(),
                *root_object_id.as_bytes(),
                *reason_digest.as_bytes(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected = retention_pins
        .iter()
        .map(|pin| {
            (
                *pin.id().as_bytes(),
                *pin.basis_head_id().as_bytes(),
                pin.root().kind().tag(),
                *pin.root().object_id().as_bytes(),
                *pin.reason_digest(),
            )
        })
        .collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    if actual != expected {
        return Err(ExportError::SnapshotClosureBasisMismatch("retention pins"));
    }
    Ok(())
}

fn validate_snapshot_entries(
    snapshot: &StoreSnapshotRootV1,
    entries: &[SealedExportEntryV1],
) -> Result<(), ExportError> {
    let rows = snapshot.rows();
    let actual_object_ids = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::Object { object_id, .. } => Some(*object_id.as_bytes()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let expected_object_ids = entries
        .iter()
        .map(|entry| *entry.object_id().as_bytes())
        .collect::<Vec<_>>();
    if actual_object_ids != expected_object_ids {
        return Err(ExportError::SnapshotClosureBasisMismatch("entry set"));
    }

    let actual_references = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::ObjectReference {
                object_id,
                reference_position,
                referenced_object_id,
            } => Some((
                *object_id.as_bytes(),
                reference_position.get(),
                *referenced_object_id.as_bytes(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected_references = entries
        .iter()
        .flat_map(|entry| {
            entry
                .references()
                .iter()
                .enumerate()
                .map(move |(position, referenced)| {
                    (
                        *entry.object_id().as_bytes(),
                        position as u64,
                        *referenced.as_bytes(),
                    )
                })
        })
        .collect::<Vec<_>>();
    expected_references.sort();
    if actual_references != expected_references {
        return Err(ExportError::SnapshotClosureBasisMismatch(
            "entry references",
        ));
    }

    let mut actual_tombstoned = rows
        .iter()
        .filter_map(|row| match row {
            StoreSnapshotRowV1::LogicalTombstone { object_id, .. } => Some(*object_id.as_bytes()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected_tombstoned = entries
        .iter()
        .filter_map(|entry| match entry {
            SealedExportEntryV1::Available(_) => None,
            SealedExportEntryV1::Tombstoned(object) => Some(*object.object_id().as_bytes()),
        })
        .collect::<Vec<_>>();
    actual_tombstoned.sort();
    expected_tombstoned.sort();
    if actual_tombstoned != expected_tombstoned {
        return Err(ExportError::SnapshotClosureBasisMismatch("tombstone set"));
    }

    for entry in entries {
        let object_rows = rows
            .iter()
            .filter(|row| {
                matches!(
                    row,
                    StoreSnapshotRowV1::Object { object_id, .. }
                        if object_id.as_bytes() == entry.object_id().as_bytes()
                )
            })
            .collect::<Vec<_>>();
        let [object_row] = object_rows.as_slice() else {
            return Err(ExportError::SnapshotClosureBasisMismatch("entry metadata"));
        };
        match entry {
            SealedExportEntryV1::Available(object) => {
                let StoreSnapshotRowV1::Object {
                    schema_id,
                    logical_byte_length,
                    stored_byte_length,
                    stored_bytes_digest,
                    storage_codec,
                    key_envelope_id,
                    key_envelope_kind,
                    ..
                } = object_row
                else {
                    return Err(ExportError::SnapshotClosureBasisMismatch("available entry"));
                };
                let bytes = object.canonical_bytes();
                let blob_matches = snapshot.object_blobs().iter().any(|(id, blob)| {
                    id.as_slice() == object.id().as_bytes() && blob.as_slice() == bytes
                });
                if schema_id.as_bytes() != object.schema_id().as_bytes()
                    || logical_byte_length.get() != bytes.len() as u64
                    || stored_byte_length.get() != bytes.len() as u64
                    || stored_bytes_digest.as_bytes().as_slice() != Sha256::digest(bytes).as_slice()
                    || storage_codec != STORE_OBJECT_STORAGE_CODEC_V1
                    || key_envelope_id.is_some()
                    || key_envelope_kind.is_some()
                    || !blob_matches
                {
                    return Err(ExportError::SnapshotClosureBasisMismatch("available entry"));
                }
            }
            SealedExportEntryV1::Tombstoned(object) => {
                let StoreSnapshotRowV1::Object {
                    schema_id,
                    logical_byte_length,
                    stored_byte_length,
                    stored_bytes_digest,
                    storage_codec,
                    key_envelope_id,
                    key_envelope_kind,
                    ..
                } = object_row
                else {
                    return Err(ExportError::SnapshotClosureBasisMismatch(
                        "tombstoned entry",
                    ));
                };
                let expected_envelope = object
                    .key_envelope()
                    .map(|(id, kind)| (*id, kind.to_owned()));
                let actual_envelope = match (key_envelope_id, key_envelope_kind) {
                    (Some(id), Some(kind)) => Some((*id.as_bytes(), kind.clone())),
                    (None, None) => None,
                    _ => {
                        return Err(ExportError::SnapshotClosureBasisMismatch(
                            "tombstoned entry envelope",
                        ));
                    }
                };
                if schema_id.as_bytes() != object.schema_id().as_bytes()
                    || logical_byte_length.get() != object.logical_byte_length()
                    || stored_byte_length.get() != object.stored_byte_length()
                    || stored_bytes_digest.as_bytes() != object.stored_bytes_digest()
                    || storage_codec != object.storage_codec()
                    || actual_envelope != expected_envelope
                    || !rows.iter().any(|row| {
                        matches!(
                            row,
                            StoreSnapshotRowV1::LogicalTombstone {
                                tombstone_id,
                                basis_head_id,
                                object_id,
                                reason_digest,
                                invalidation_digest,
                            } if tombstone_id.as_bytes() == object.tombstone().id().as_bytes()
                                && basis_head_id.as_bytes()
                                    == object.tombstone().basis_head_id().as_bytes()
                                && object_id.as_bytes() == object.object_id().as_bytes()
                                && reason_digest.as_bytes() == object.tombstone().reason_digest()
                                && invalidation_digest.as_bytes()
                                    == object.tombstone().invalidation_digest()
                        )
                    })
                {
                    return Err(ExportError::SnapshotClosureBasisMismatch(
                        "tombstoned entry",
                    ));
                }
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the frozen BackupReceiptV1 tuple binds every provenance fact explicitly"
)]
fn backup_receipt_value(
    source_role: StoreRoleV1,
    source_domain_id: StoreDomainIdV1,
    export_id: SealedExportIdV1,
    head_id: StoreHeadIdV1,
    generation_id: StoreGenerationIdV1,
    reachability_snapshot_id: ReachabilitySnapshotIdV1,
    snapshot_root_id: StoreSnapshotRootIdV1,
    schema_manifest_id: StoreSchemaManifestIdV1,
    family_manifest_set_digest: &[u8; 32],
    payload_set_digest: &[u8; 32],
    source_publication_clock: u64,
    committed_publication_clock: u64,
    inner_export_byte_length: u64,
    inner_export_bytes_digest: &[u8; 32],
) -> Result<CborValue, ExportError> {
    Ok(CborValue::Array(vec![
        CborValue::Unsigned(BACKUP_RECEIPT_VERSION_V1),
        CborValue::text(BACKUP_RECEIPT_FORMAT_V1)?,
        CborValue::text(SEALED_EXPORT_FORMAT_V2)?,
        CborValue::Unsigned(source_role.tag()),
        CborValue::Bytes(source_domain_id.as_bytes().to_vec()),
        CborValue::Bytes(export_id.as_bytes().to_vec()),
        CborValue::Bytes(head_id.as_bytes().to_vec()),
        CborValue::Bytes(generation_id.as_bytes().to_vec()),
        CborValue::Bytes(reachability_snapshot_id.as_bytes().to_vec()),
        CborValue::Bytes(snapshot_root_id.as_bytes().to_vec()),
        CborValue::Bytes(schema_manifest_id.as_bytes().to_vec()),
        CborValue::Bytes(family_manifest_set_digest.to_vec()),
        CborValue::Bytes(payload_set_digest.to_vec()),
        CborValue::Unsigned(source_publication_clock),
        CborValue::Unsigned(committed_publication_clock),
        CborValue::Unsigned(inner_export_byte_length),
        CborValue::Bytes(inner_export_bytes_digest.to_vec()),
    ]))
}

fn sealed_backup_value(export: CborValue, receipt: CborValue) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(SEALED_BACKUP_VERSION_V1),
        CborValue::Text(SEALED_BACKUP_FORMAT_V1.to_owned()),
        export,
        receipt,
    ])
}

fn validate_backup_receipt_parity(
    export: &SealedExportV1,
    receipt: &BackupReceiptV1,
) -> Result<(), ExportError> {
    let snapshot_root = export
        .snapshot_root()
        .ok_or(ExportError::BackupReceiptRequiresFullHistoryExport)?;
    let source_publication_clock = export
        .source_publication_clock()
        .ok_or(ExportError::BackupReceiptRequiresFullHistoryExport)?;
    let inner_export_byte_length = u64::try_from(export.canonical_bytes().len())
        .map_err(|_| ExportError::InvalidBackupReceiptExportLength)?;
    let inner_export_bytes_digest: [u8; 32] = Sha256::digest(export.canonical_bytes()).into();

    let checks = [
        (
            receipt.source_role == export.generation().domain().role(),
            "source role",
        ),
        (
            receipt.source_domain_id == export.generation().domain().id(),
            "source domain identity",
        ),
        (receipt.export_id == export.id(), "export identity"),
        (receipt.head_id == export.head().id(), "Head identity"),
        (
            receipt.generation_id == export.generation().id(),
            "Generation identity",
        ),
        (
            receipt.reachability_snapshot_id == export.reachability().id(),
            "reachability snapshot identity",
        ),
        (
            receipt.snapshot_root_id == snapshot_root.id(),
            "snapshot root identity",
        ),
        (
            receipt.schema_manifest_id == snapshot_root.schema_manifest_id(),
            "schema manifest identity",
        ),
        (
            receipt.family_manifest_set_digest == *snapshot_root.family_manifest_set_digest(),
            "family manifest set digest",
        ),
        (
            receipt.payload_set_digest == *snapshot_root.payload_set_digest(),
            "payload set digest",
        ),
        (
            receipt.source_publication_clock == source_publication_clock,
            "source publication clock",
        ),
        (
            receipt.inner_export_byte_length == inner_export_byte_length,
            "inner export byte length",
        ),
        (
            receipt.inner_export_bytes_digest == inner_export_bytes_digest,
            "inner export bytes digest",
        ),
    ];
    if let Some((_, field)) = checks.into_iter().find(|(matches, _)| !matches) {
        return Err(ExportError::BackupReceiptParityMismatch(field));
    }
    Ok(())
}

fn export_value(
    lineage: &[SealedExportLineageV1],
    object_inventory: &[StoreObjectIdV1],
    reachability: &ReachabilitySnapshotV1,
    retention_pins: &[RetentionPinV1],
    entries: &[SealedExportEntryV1],
    source_publication_clock: Option<u64>,
    snapshot_blocks: Option<&StoreSnapshotBlockClosureV2>,
) -> Result<CborValue, ExportError> {
    let mut fields = vec![
        CborValue::Unsigned(if snapshot_blocks.is_some() {
            SEALED_EXPORT_VERSION_V2
        } else {
            SEALED_EXPORT_VERSION_V1
        }),
        CborValue::Array(
            lineage
                .iter()
                .map(SealedExportLineageV1::canonical_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        identity_array(object_inventory),
        CborValue::Bytes(reachability.canonical_bytes()?),
        CborValue::Array(
            retention_pins
                .iter()
                .map(|pin| pin.canonical_bytes().map(CborValue::Bytes))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CborValue::Array(
            entries
                .iter()
                .map(SealedExportEntryV1::canonical_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    ];
    if let Some(snapshot_blocks) = snapshot_blocks {
        fields.push(CborValue::Unsigned(source_publication_clock.ok_or(
            ExportError::SnapshotClosureBasisMismatch("source publication clock"),
        )?));
        fields.push(CborValue::Bytes(snapshot_blocks.canonical_bytes().to_vec()));
    } else if source_publication_clock.is_some() {
        return Err(ExportError::SnapshotClosureBasisMismatch(
            "source publication clock",
        ));
    }
    Ok(CborValue::Array(fields))
}

#[allow(
    clippy::too_many_arguments,
    reason = "the frozen RestoreCandidateV1 tuple is intentionally explicit and positional"
)]
fn restore_candidate_value(
    source_export_id: SealedExportIdV1,
    source_domain_id: StoreDomainIdV1,
    source_export_bytes_digest: &[u8; 32],
    destination_domain_id: StoreDomainIdV1,
    candidate_generation_id: StoreGenerationIdV1,
    candidate_head_id: StoreHeadIdV1,
    candidate_snapshot_id: ReachabilitySnapshotIdV1,
    candidate_roots: &[StoreObjectIdV1],
    verification_digest: &[u8; 32],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(RESTORE_CANDIDATE_VERSION_V1),
        CborValue::Bytes(source_export_id.as_bytes().to_vec()),
        CborValue::Bytes(source_domain_id.as_bytes().to_vec()),
        CborValue::Bytes(source_export_bytes_digest.to_vec()),
        CborValue::Bytes(destination_domain_id.as_bytes().to_vec()),
        CborValue::Bytes(candidate_generation_id.as_bytes().to_vec()),
        CborValue::Bytes(candidate_head_id.as_bytes().to_vec()),
        CborValue::Bytes(candidate_snapshot_id.as_bytes().to_vec()),
        identity_array(candidate_roots),
        CborValue::Bytes(verification_digest.to_vec()),
    ])
}

#[allow(
    clippy::too_many_arguments,
    reason = "the verification digest binds every frozen RestoreCandidateV1 basis field"
)]
fn restore_verification_value(
    source_export_id: SealedExportIdV1,
    source_domain_id: StoreDomainIdV1,
    source_export_bytes_digest: &[u8; 32],
    destination_domain_id: StoreDomainIdV1,
    candidate_generation_id: StoreGenerationIdV1,
    candidate_head_id: StoreHeadIdV1,
    candidate_snapshot_id: ReachabilitySnapshotIdV1,
    candidate_roots: &[StoreObjectIdV1],
) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(RESTORE_CANDIDATE_VERSION_V1),
        CborValue::Bytes(source_export_id.as_bytes().to_vec()),
        CborValue::Bytes(source_domain_id.as_bytes().to_vec()),
        CborValue::Bytes(source_export_bytes_digest.to_vec()),
        CborValue::Bytes(destination_domain_id.as_bytes().to_vec()),
        CborValue::Bytes(candidate_generation_id.as_bytes().to_vec()),
        CborValue::Bytes(candidate_head_id.as_bytes().to_vec()),
        CborValue::Bytes(candidate_snapshot_id.as_bytes().to_vec()),
        identity_array(candidate_roots),
    ])
}

fn identity_array(ids: &[StoreObjectIdV1]) -> CborValue {
    CborValue::Array(
        ids.iter()
            .map(|id| CborValue::Bytes(id.as_bytes().to_vec()))
            .collect(),
    )
}

fn decode_identity_array(values: &[CborValue]) -> Result<Vec<StoreObjectIdV1>, ExportError> {
    values
        .iter()
        .map(|value| {
            let CborValue::Bytes(bytes) = value else {
                return Err(ExportError::InvalidShape);
            };
            identity_from_bytes(bytes)
        })
        .collect()
}

fn optional_key_envelope(envelope: Option<&([u8; 32], String)>) -> Result<CborValue, ExportError> {
    match envelope {
        None => Ok(CborValue::Array(vec![CborValue::Unsigned(0)])),
        Some((id, kind)) => Ok(CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Bytes(id.to_vec()),
            CborValue::text(kind)?,
        ])),
    }
}

fn decode_key_envelope(value: &CborValue) -> Result<Option<([u8; 32], String)>, ExportError> {
    let CborValue::Array(fields) = value else {
        return Err(ExportError::InvalidShape);
    };
    match fields.as_slice() {
        [CborValue::Unsigned(0)] => Ok(None),
        [
            CborValue::Unsigned(1),
            CborValue::Bytes(id),
            CborValue::Text(kind),
        ] => Ok(Some((digest_from_bytes(id)?, kind.clone()))),
        _ => Err(ExportError::InvalidShape),
    }
}

fn validate_ascii_label(value: &str) -> Result<(), ExportError> {
    if value.is_empty() || value.len() > 64 || !value.is_ascii() {
        return Err(ExportError::InvalidObjectMetadata);
    }
    Ok(())
}

fn digest_from_bytes(bytes: &[u8]) -> Result<[u8; 32], ExportError> {
    bytes
        .try_into()
        .map_err(|_| ExportError::InvalidIdentityLength)
}

fn identity_from_bytes<K>(bytes: &[u8]) -> Result<ManifestIdentityV1<K>, ExportError>
where
    K: crate::domain::identity::IdentityKindV1,
{
    Ok(ManifestIdentityV1::from_digest(digest_from_bytes(bytes)?))
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ExportError {
    #[error("sealed export has an invalid canonical shape")]
    InvalidShape,
    #[error("sealed export identity or digest bytes must contain exactly 32 bytes")]
    InvalidIdentityLength,
    #[error("sealed export uses unsupported version {0}")]
    UnknownVersion(u64),
    #[error("restore candidate uses unsupported version {0}")]
    UnknownRestoreCandidateVersion(u64),
    #[error("Backup Receipt has an invalid canonical shape")]
    InvalidBackupReceiptShape,
    #[error("Backup Receipt uses unsupported version {0}")]
    UnknownBackupReceiptVersion(u64),
    #[error("Backup Receipt uses an unknown receipt or inner-export format")]
    UnknownBackupReceiptFormat,
    #[error("Backup Receipt uses unknown source role tag {0}")]
    UnknownBackupReceiptSourceRole(u64),
    #[error("Backup Receipt source publication clock cannot advance without overflow")]
    BackupReceiptClockOverflow,
    #[error("Backup Receipt committed publication clock must equal source clock plus one")]
    BackupReceiptClockMismatch,
    #[error("Backup Receipt requires a nonempty inner export")]
    InvalidBackupReceiptExportLength,
    #[error("Backup Receipt requires a full-history SealedExportV1")]
    BackupReceiptRequiresFullHistoryExport,
    #[error("Backup Receipt bytes are not the exact canonical encoding")]
    NonCanonicalBackupReceipt,
    #[error("Sealed Backup has an invalid canonical shape")]
    InvalidSealedBackupShape,
    #[error("Sealed Backup uses unsupported version {0}")]
    UnknownSealedBackupVersion(u64),
    #[error("Sealed Backup uses an unknown exact format")]
    UnknownSealedBackupFormat,
    #[error("Sealed Backup exceeds the finite 31 MiB v1 carrier limit")]
    SealedBackupBytesLimitExceeded,
    #[error("Sealed Backup receipt does not match inner export field {0}")]
    BackupReceiptParityMismatch(&'static str),
    #[error("Sealed Backup bytes are not the exact canonical encoding")]
    NonCanonicalSealedBackup,
    #[error("sealed export uses unknown entry tag {0}")]
    UnknownEntryTag(u64),
    #[error("sealed export entry identity does not match its verified payload")]
    EntryIdentityMismatch,
    #[error("sealed export lineage must be finite and nonempty")]
    InvalidLineageLength,
    #[error("sealed export lineage member does not bind its Generation exactly")]
    LineageMemberMismatch,
    #[error("sealed export lineage must begin at ordinal one and be complete and contiguous")]
    IncompleteLineage,
    #[error("sealed export exceeds the finite v1 object limit")]
    TooManyEntries,
    #[error("sealed export current Head and reachability basis do not match")]
    SnapshotBasisMismatch,
    #[error("sealed export retention pins must be strictly identity-sorted and unique")]
    PinsNotStrictlySorted,
    #[error("sealed export entries must be strictly object-identity sorted and unique")]
    EntriesNotStrictlySorted,
    #[error("sealed export object inventory must exactly equal its entry identities")]
    ObjectInventoryMismatch,
    #[error("sealed export entries do not contain the exact current reachability classification")]
    ReachabilityMismatch,
    #[error("sealed export omits an exported Generation root")]
    GenerationRootMissing,
    #[error("sealed export omits a referenced authoritative object")]
    ReferenceClosureIncomplete,
    #[error("sealed export tombstone does not bind a Head in the exported lineage")]
    TombstoneBasisMismatch,
    #[error("sealed export retention pin does not bind an exported Head and current root closure")]
    RetentionPinMismatch,
    #[error("sealed export V2 snapshot closure does not match top-level {0} facts")]
    SnapshotClosureBasisMismatch(&'static str),
    #[error("sealed export tombstoned object metadata is invalid")]
    InvalidObjectMetadata,
    #[error("restore candidate roots must be nonempty")]
    EmptyCandidateRoots,
    #[error("restore candidate verification digest does not bind its exact normalized basis")]
    RestoreVerificationDigestMismatch,
    #[error("sealed export bytes are not the exact canonical encoding")]
    NonCanonicalBytes,
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Identity(#[from] crate::domain::identity::IdentityError),
    #[error(transparent)]
    Generation(#[from] GenerationError),
    #[error(transparent)]
    Object(#[from] StoreObjectError),
    #[error(transparent)]
    Retention(#[from] RetentionError),
    #[error("Store snapshot validation failed: {0}")]
    Snapshot(String),
    #[error("Store snapshot block closure validation failed: {0}")]
    SnapshotBlocks(String),
}

impl From<StoreSnapshotBlockError> for ExportError {
    fn from(error: StoreSnapshotBlockError) -> Self {
        Self::SnapshotBlocks(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::domain::identity::{ContractRootIdV1, SchemaIdV1};

    use super::super::{
        StoreCompatibilityV1, StoreDomainV1, StoreGenerationV1, StoreObjectV1, StoreRoleV1, StoreV1,
    };
    use super::*;

    static NEXT_BACKUP_TEST: AtomicU64 = AtomicU64::new(0);

    struct TestStorePath(PathBuf);

    impl TestStorePath {
        fn new() -> Self {
            let sequence = NEXT_BACKUP_TEST.fetch_add(1, Ordering::Relaxed);
            let root = fs::canonicalize(std::env::temp_dir()).expect("canonical temp root");
            Self(root.join(format!(
                "maestro-vnext-sealed-backup-unit-{}-{sequence}",
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

    fn full_export() -> (TestStorePath, SealedExportV1) {
        let path = TestStorePath::new();
        let domain = StoreDomainV1::derive(StoreRoleV1::Repository, b"sealed-backup-tests")
            .expect("Store domain");
        let mut store = StoreV1::create(&path.0, domain.clone()).expect("create Store");
        let object = StoreObjectV1::new(
            SchemaIdV1::parse(&rendered(1)).expect("Schema identity"),
            CborValue::Array(vec![CborValue::Unsigned(1)]),
            vec![],
        )
        .expect("Store Object");
        store.put_object(&object).expect("persist object");
        let generation = StoreGenerationV1::new(
            domain,
            1,
            None,
            ContractRootIdV1::parse(&rendered(2)).expect("Contract Root identity"),
            StoreCompatibilityV1::stage0_successor().expect("Stage 0 compatibility"),
            vec![object.id()],
        )
        .expect("Generation");
        store
            .publish_generation(&generation, None)
            .expect("publish Generation");
        let backup = store.seal_export().expect("full-history backup");
        (path, backup.export().clone())
    }

    fn receipt_for(export: &SealedExportV1) -> BackupReceiptV1 {
        let committed_publication_clock = export
            .source_publication_clock()
            .expect("full-history source clock")
            + 1;
        BackupReceiptV1::for_committed_export(export, committed_publication_clock)
            .expect("Backup Receipt")
    }

    #[test]
    fn sealed_backup_round_trips_as_semantic_export_and_receipt_values() {
        let (_path, export) = full_export();
        let receipt = receipt_for(&export);
        let backup = SealedBackupV1::new(export.clone(), receipt.clone()).expect("Sealed Backup");

        let decoded = SealedBackupV1::decode(backup.canonical_bytes()).expect("decode backup");
        let decoded_receipt =
            BackupReceiptV1::decode(receipt.canonical_bytes()).expect("decode receipt");

        assert_eq!(decoded, backup);
        assert_eq!(decoded_receipt, receipt);
        assert_eq!(receipt.format(), BACKUP_RECEIPT_FORMAT_V1);
        assert_eq!(receipt.inner_export_format(), SEALED_EXPORT_FORMAT_V2);
        assert_eq!(receipt.id(), decoded.receipt().id());
        assert_eq!(backup.format(), SEALED_BACKUP_FORMAT_V1);
        assert_eq!(decoded.export(), &export);
        assert_eq!(decoded.receipt(), &receipt);
        let CborValue::Array(fields) =
            deterministic_cbor::decode(backup.canonical_bytes()).expect("decode carrier value")
        else {
            panic!("Sealed Backup must be an array");
        };
        assert!(matches!(fields.get(2), Some(CborValue::Array(_))));
        assert!(matches!(fields.get(3), Some(CborValue::Array(_))));
    }

    #[test]
    fn sealed_backup_rejects_missing_and_tampered_receipts() {
        let (_path, export) = full_export();
        let receipt = receipt_for(&export);
        let backup = SealedBackupV1::new(export, receipt).expect("Sealed Backup");
        let CborValue::Array(mut fields) =
            deterministic_cbor::decode(backup.canonical_bytes()).expect("decode carrier")
        else {
            panic!("Sealed Backup must be an array");
        };

        let mut missing = fields.clone();
        missing.pop();
        let missing =
            deterministic_cbor::encode(&CborValue::Array(missing)).expect("encode missing");
        assert!(matches!(
            SealedBackupV1::decode(&missing),
            Err(ExportError::InvalidSealedBackupShape)
        ));

        let CborValue::Array(receipt_fields) = fields.get_mut(3).expect("receipt value") else {
            panic!("Backup Receipt must be an array");
        };
        receipt_fields[11] = CborValue::Bytes([9_u8; 32].to_vec());
        let tampered =
            deterministic_cbor::encode(&CborValue::Array(fields)).expect("encode tamper");
        assert!(matches!(
            SealedBackupV1::decode(&tampered),
            Err(ExportError::BackupReceiptParityMismatch(
                "family manifest set digest"
            ))
        ));
    }

    #[test]
    fn sealed_backup_rejects_role_domain_clock_identity_and_format_mismatches() {
        let (_path, export) = full_export();
        let valid = receipt_for(&export);

        let wrong_role = BackupReceiptV1::new(
            StoreRoleV1::Installation,
            valid.source_domain_id(),
            valid.export_id(),
            valid.head_id(),
            valid.generation_id(),
            valid.reachability_snapshot_id(),
            valid.snapshot_root_id(),
            valid.schema_manifest_id(),
            *valid.family_manifest_set_digest(),
            *valid.payload_set_digest(),
            valid.source_publication_clock(),
            valid.committed_publication_clock(),
            valid.inner_export_byte_length(),
            *valid.inner_export_bytes_digest(),
        )
        .expect("internally valid mismatched receipt");
        assert!(matches!(
            SealedBackupV1::new(export.clone(), wrong_role),
            Err(ExportError::BackupReceiptParityMismatch("source role"))
        ));

        let wrong_domain = BackupReceiptV1::new(
            valid.source_role(),
            StoreDomainV1::derive(StoreRoleV1::Repository, b"different-backup-domain")
                .expect("different Store domain")
                .id(),
            valid.export_id(),
            valid.head_id(),
            valid.generation_id(),
            valid.reachability_snapshot_id(),
            valid.snapshot_root_id(),
            valid.schema_manifest_id(),
            *valid.family_manifest_set_digest(),
            *valid.payload_set_digest(),
            valid.source_publication_clock(),
            valid.committed_publication_clock(),
            valid.inner_export_byte_length(),
            *valid.inner_export_bytes_digest(),
        )
        .expect("internally valid mismatched receipt");
        assert!(matches!(
            SealedBackupV1::new(export.clone(), wrong_domain),
            Err(ExportError::BackupReceiptParityMismatch(
                "source domain identity"
            ))
        ));

        assert!(matches!(
            BackupReceiptV1::new(
                valid.source_role(),
                valid.source_domain_id(),
                valid.export_id(),
                valid.head_id(),
                valid.generation_id(),
                valid.reachability_snapshot_id(),
                valid.snapshot_root_id(),
                valid.schema_manifest_id(),
                *valid.family_manifest_set_digest(),
                *valid.payload_set_digest(),
                valid.source_publication_clock(),
                valid.committed_publication_clock() + 1,
                valid.inner_export_byte_length(),
                *valid.inner_export_bytes_digest(),
            ),
            Err(ExportError::BackupReceiptClockMismatch)
        ));

        let wrong_identity = BackupReceiptV1::new(
            valid.source_role(),
            valid.source_domain_id(),
            SealedExportIdV1::parse(&rendered(8)).expect("different Export identity"),
            valid.head_id(),
            valid.generation_id(),
            valid.reachability_snapshot_id(),
            valid.snapshot_root_id(),
            valid.schema_manifest_id(),
            *valid.family_manifest_set_digest(),
            *valid.payload_set_digest(),
            valid.source_publication_clock(),
            valid.committed_publication_clock(),
            valid.inner_export_byte_length(),
            *valid.inner_export_bytes_digest(),
        )
        .expect("internally valid mismatched receipt");
        assert!(matches!(
            SealedBackupV1::new(export.clone(), wrong_identity),
            Err(ExportError::BackupReceiptParityMismatch("export identity"))
        ));

        let backup = SealedBackupV1::new(export, valid).expect("Sealed Backup");
        let CborValue::Array(mut fields) =
            deterministic_cbor::decode(backup.canonical_bytes()).expect("decode carrier")
        else {
            panic!("Sealed Backup must be an array");
        };
        fields[1] = CborValue::text("wrong-format").expect("ASCII format");
        let wrong_format =
            deterministic_cbor::encode(&CborValue::Array(fields)).expect("encode format tamper");
        assert!(matches!(
            SealedBackupV1::decode(&wrong_format),
            Err(ExportError::UnknownSealedBackupFormat)
        ));

        let CborValue::Array(mut fields) =
            deterministic_cbor::decode(backup.canonical_bytes()).expect("decode carrier")
        else {
            panic!("Sealed Backup must be an array");
        };
        let CborValue::Array(receipt_fields) = fields.get_mut(3).expect("receipt value") else {
            panic!("Backup Receipt must be an array");
        };
        receipt_fields[1] = CborValue::text("wrong-receipt-format").expect("ASCII format");
        let wrong_receipt_format =
            deterministic_cbor::encode(&CborValue::Array(fields)).expect("encode format tamper");
        assert!(matches!(
            SealedBackupV1::decode(&wrong_receipt_format),
            Err(ExportError::UnknownBackupReceiptFormat)
        ));
    }
}
