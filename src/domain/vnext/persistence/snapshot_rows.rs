use std::cmp::Ordering;
use std::collections::BTreeSet;

use rusqlite::{Connection, Row};
use thiserror::Error;

use crate::domain::vnext::persistence::{SEALED_BACKUP_FORMAT_V1, StoreRoleV1};
use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

const SNAPSHOT_ROW_VERSION_V1: u64 = 1;
const DIGEST_BYTES: usize = 32;
pub(crate) const MAX_SNAPSHOT_ROWS_V1: usize = 65_536;
pub(crate) const MAX_SNAPSHOT_ROW_BYTES_V1: usize = 1024;
pub(crate) const MAX_SNAPSHOT_ROWS_BYTES_V1: usize = 7 * 1024 * 1024;
pub(crate) const MAX_SEALED_EXPORT_ROWS_V1: usize = 1024;
pub(crate) const MAX_REFERENCED_PRIOR_ROOTS_V1: usize = 1024;

#[derive(Clone, Copy)]
struct StoreSnapshotRowLimitsV1 {
    rows: usize,
    row_bytes: usize,
    aggregate_bytes: usize,
    sealed_exports: usize,
    referenced_prior_roots: usize,
}

impl StoreSnapshotRowLimitsV1 {
    const PRODUCTION: Self = Self {
        rows: MAX_SNAPSHOT_ROWS_V1,
        row_bytes: MAX_SNAPSHOT_ROW_BYTES_V1,
        aggregate_bytes: MAX_SNAPSHOT_ROWS_BYTES_V1,
        sealed_exports: MAX_SEALED_EXPORT_ROWS_V1,
        referenced_prior_roots: MAX_REFERENCED_PRIOR_ROOTS_V1,
    };
}

pub(crate) struct StoreSnapshotRowBoundsV1 {
    limits: StoreSnapshotRowLimitsV1,
    rows: usize,
    aggregate_bytes: usize,
    sealed_exports: usize,
    referenced_prior_roots: BTreeSet<[u8; DIGEST_BYTES]>,
}

impl StoreSnapshotRowBoundsV1 {
    pub(crate) fn new() -> Self {
        Self::with_limits(StoreSnapshotRowLimitsV1::PRODUCTION)
    }

    fn with_limits(limits: StoreSnapshotRowLimitsV1) -> Self {
        Self {
            limits,
            rows: 0,
            aggregate_bytes: 0,
            sealed_exports: 0,
            referenced_prior_roots: BTreeSet::new(),
        }
    }

    pub(crate) fn observe(
        &mut self,
        row: &StoreSnapshotRowV1,
    ) -> Result<(), StoreSnapshotRowError> {
        let encoded = row.encode_canonical()?;
        let prior_root_id = match row {
            StoreSnapshotRowV1::SealedExport {
                snapshot_root_id, ..
            } => Some(*snapshot_root_id.as_bytes()),
            _ => None,
        };
        self.observe_encoded(encoded.len(), prior_root_id)
    }

    fn observe_encoded(
        &mut self,
        encoded_bytes: usize,
        prior_root_id: Option<[u8; DIGEST_BYTES]>,
    ) -> Result<(), StoreSnapshotRowError> {
        if self.rows == self.limits.rows {
            return Err(StoreSnapshotRowError::RowCountLimitExceeded);
        }
        if encoded_bytes > self.limits.row_bytes {
            return Err(StoreSnapshotRowError::RowBytesLimitExceeded {
                actual: encoded_bytes,
            });
        }
        let aggregate_bytes = self
            .aggregate_bytes
            .checked_add(encoded_bytes)
            .ok_or(StoreSnapshotRowError::AggregateRowBytesLimitExceeded)?;
        if aggregate_bytes > self.limits.aggregate_bytes {
            return Err(StoreSnapshotRowError::AggregateRowBytesLimitExceeded);
        }
        if let Some(prior_root_id) = prior_root_id {
            if self.sealed_exports == self.limits.sealed_exports {
                return Err(StoreSnapshotRowError::SealedExportCountLimitExceeded);
            }
            if self.referenced_prior_roots.contains(&prior_root_id) {
                return Err(StoreSnapshotRowError::DuplicateReferencedPriorRoot(
                    prior_root_id,
                ));
            }
            if self.referenced_prior_roots.len() == self.limits.referenced_prior_roots {
                return Err(StoreSnapshotRowError::ReferencedPriorRootCountLimitExceeded);
            }
        }

        self.rows += 1;
        self.aggregate_bytes = aggregate_bytes;
        if let Some(prior_root_id) = prior_root_id {
            self.sealed_exports += 1;
            self.referenced_prior_roots.insert(prior_root_id);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum StoreSnapshotFamilyV1 {
    StoreIdentity,
    SourcePointers,
    ObjectHistory,
    GenerationHistory,
    ReachabilityRetentionHistory,
    GarbageCollectionHistory,
    ExportRestoreHistory,
    IdempotencyHistory,
}

impl StoreSnapshotFamilyV1 {
    pub(crate) const ALL: [Self; 8] = [
        Self::StoreIdentity,
        Self::SourcePointers,
        Self::ObjectHistory,
        Self::GenerationHistory,
        Self::ReachabilityRetentionHistory,
        Self::GarbageCollectionHistory,
        Self::ExportRestoreHistory,
        Self::IdempotencyHistory,
    ];

    pub(crate) const fn tag(self) -> u64 {
        match self {
            Self::StoreIdentity => 1,
            Self::SourcePointers => 2,
            Self::ObjectHistory => 3,
            Self::GenerationHistory => 4,
            Self::ReachabilityRetentionHistory => 5,
            Self::GarbageCollectionHistory => 6,
            Self::ExportRestoreHistory => 7,
            Self::IdempotencyHistory => 8,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::StoreIdentity => "store_identity",
            Self::SourcePointers => "source_pointers",
            Self::ObjectHistory => "object_history",
            Self::GenerationHistory => "generation_history",
            Self::ReachabilityRetentionHistory => "reachability_retention_history",
            Self::GarbageCollectionHistory => "garbage_collection_history",
            Self::ExportRestoreHistory => "export_restore_history",
            Self::IdempotencyHistory => "idempotency_history",
        }
    }

    fn from_tag(tag: u64) -> Result<Self, StoreSnapshotRowError> {
        Self::ALL
            .into_iter()
            .find(|family| family.tag() == tag)
            .ok_or(StoreSnapshotRowError::UnknownFamily(tag))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum StoreSnapshotTableV1 {
    Metadata,
    PublicationClock,
    State,
    RetentionRevision,
    ActiveHead,
    Objects,
    ObjectReferences,
    Generations,
    GenerationRoots,
    Heads,
    ReachabilitySnapshots,
    ReachabilityRoots,
    ReachabilityObjects,
    RetentionPins,
    RetentionPinReleases,
    LogicalTombstones,
    GcPlans,
    GcPlanObjects,
    GcCollectionOccurrences,
    SealedExports,
    SealedExportPins,
    SealedExportObjects,
    RestoreCandidates,
    RestoreCandidateRoots,
    Idempotency,
}

impl StoreSnapshotTableV1 {
    pub(crate) const ALL: [Self; 25] = [
        Self::Metadata,
        Self::PublicationClock,
        Self::State,
        Self::RetentionRevision,
        Self::ActiveHead,
        Self::Objects,
        Self::ObjectReferences,
        Self::Generations,
        Self::GenerationRoots,
        Self::Heads,
        Self::ReachabilitySnapshots,
        Self::ReachabilityRoots,
        Self::ReachabilityObjects,
        Self::RetentionPins,
        Self::RetentionPinReleases,
        Self::LogicalTombstones,
        Self::GcPlans,
        Self::GcPlanObjects,
        Self::GcCollectionOccurrences,
        Self::SealedExports,
        Self::SealedExportPins,
        Self::SealedExportObjects,
        Self::RestoreCandidates,
        Self::RestoreCandidateRoots,
        Self::Idempotency,
    ];

    pub(crate) const fn tag(self) -> u64 {
        self.order() as u64
    }

    pub(crate) const fn order(self) -> u8 {
        match self {
            Self::Metadata => 1,
            Self::PublicationClock => 2,
            Self::State => 3,
            Self::RetentionRevision => 4,
            Self::ActiveHead => 5,
            Self::Objects => 6,
            Self::ObjectReferences => 7,
            Self::Generations => 8,
            Self::GenerationRoots => 9,
            Self::Heads => 10,
            Self::ReachabilitySnapshots => 11,
            Self::ReachabilityRoots => 12,
            Self::ReachabilityObjects => 13,
            Self::RetentionPins => 14,
            Self::RetentionPinReleases => 15,
            Self::LogicalTombstones => 16,
            Self::GcPlans => 17,
            Self::GcPlanObjects => 18,
            Self::GcCollectionOccurrences => 19,
            Self::SealedExports => 20,
            Self::SealedExportPins => 21,
            Self::SealedExportObjects => 22,
            Self::RestoreCandidates => 23,
            Self::RestoreCandidateRoots => 24,
            Self::Idempotency => 25,
        }
    }

    pub(crate) const fn family(self) -> StoreSnapshotFamilyV1 {
        match self {
            Self::Metadata | Self::PublicationClock => StoreSnapshotFamilyV1::StoreIdentity,
            Self::State | Self::RetentionRevision | Self::ActiveHead => {
                StoreSnapshotFamilyV1::SourcePointers
            }
            Self::Objects | Self::ObjectReferences => StoreSnapshotFamilyV1::ObjectHistory,
            Self::Generations | Self::GenerationRoots | Self::Heads => {
                StoreSnapshotFamilyV1::GenerationHistory
            }
            Self::ReachabilitySnapshots
            | Self::ReachabilityRoots
            | Self::ReachabilityObjects
            | Self::RetentionPins
            | Self::RetentionPinReleases
            | Self::LogicalTombstones => StoreSnapshotFamilyV1::ReachabilityRetentionHistory,
            Self::GcPlans | Self::GcPlanObjects | Self::GcCollectionOccurrences => {
                StoreSnapshotFamilyV1::GarbageCollectionHistory
            }
            Self::SealedExports
            | Self::SealedExportPins
            | Self::SealedExportObjects
            | Self::RestoreCandidates
            | Self::RestoreCandidateRoots => StoreSnapshotFamilyV1::ExportRestoreHistory,
            Self::Idempotency => StoreSnapshotFamilyV1::IdempotencyHistory,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Metadata => "store_metadata",
            Self::PublicationClock => "store_publication_clock",
            Self::State => "store_state",
            Self::RetentionRevision => "store_retention_revision",
            Self::Objects => "store_objects",
            Self::ObjectReferences => "store_object_references",
            Self::Generations => "store_generations",
            Self::GenerationRoots => "store_generation_roots",
            Self::Heads => "store_heads",
            Self::ActiveHead => "store_active_head",
            Self::ReachabilitySnapshots => "store_reachability_snapshots",
            Self::ReachabilityRoots => "store_reachability_roots",
            Self::ReachabilityObjects => "store_reachability_objects",
            Self::RetentionPins => "store_retention_pins",
            Self::RetentionPinReleases => "store_retention_pin_releases",
            Self::LogicalTombstones => "store_logical_tombstones",
            Self::GcPlans => "store_gc_plans",
            Self::GcPlanObjects => "store_gc_plan_objects",
            Self::GcCollectionOccurrences => "store_gc_collection_occurrences",
            Self::SealedExports => "store_sealed_exports",
            Self::SealedExportPins => "store_sealed_export_pins",
            Self::SealedExportObjects => "store_sealed_export_objects",
            Self::RestoreCandidates => "store_restore_candidates",
            Self::RestoreCandidateRoots => "store_restore_candidate_roots",
            Self::Idempotency => "store_idempotency",
        }
    }

    fn from_tag(tag: u64) -> Result<Self, StoreSnapshotRowError> {
        Self::ALL
            .into_iter()
            .find(|table| table.tag() == tag)
            .ok_or(StoreSnapshotRowError::UnknownTable(tag))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StoreSnapshotTableManifestV1 {
    pub(crate) table: StoreSnapshotTableV1,
    pub(crate) family: StoreSnapshotFamilyV1,
    pub(crate) order: u8,
    pub(crate) columns: &'static [&'static str],
    pub(crate) primary_key: &'static [&'static str],
}

macro_rules! manifest {
    ($table:ident, [$($column:literal),+], [$($key:literal),+]) => {
        StoreSnapshotTableManifestV1 {
            table: StoreSnapshotTableV1::$table,
            family: StoreSnapshotTableV1::$table.family(),
            order: StoreSnapshotTableV1::$table.order(),
            columns: &[$($column),+],
            primary_key: &[$($key),+],
        }
    };
}

pub(crate) const STORE_SNAPSHOT_TABLE_MANIFEST_V1: [StoreSnapshotTableManifestV1; 25] = [
    manifest!(
        Metadata,
        ["singleton", "schema_version", "store_role", "domain_id"],
        ["singleton"]
    ),
    manifest!(
        PublicationClock,
        ["singleton", "publication_clock"],
        ["singleton"]
    ),
    manifest!(
        State,
        ["singleton", "state", "state_revision"],
        ["singleton"]
    ),
    manifest!(
        RetentionRevision,
        ["singleton", "retention_revision"],
        ["singleton"]
    ),
    manifest!(
        ActiveHead,
        ["singleton", "head_id", "head_revision"],
        ["singleton"]
    ),
    manifest!(
        Objects,
        [
            "object_id",
            "schema_id",
            "logical_byte_length",
            "stored_byte_length",
            "stored_bytes_digest",
            "storage_codec",
            "key_envelope_id",
            "key_envelope_kind"
        ],
        ["object_id"]
    ),
    manifest!(
        ObjectReferences,
        ["object_id", "reference_position", "referenced_object_id"],
        ["object_id", "reference_position"]
    ),
    manifest!(
        Generations,
        [
            "generation_id",
            "generation_ordinal",
            "previous_generation_id",
            "contract_root_id",
            "writer_compatibility_manifest_id",
            "association_schema_id",
            "finality_edge_manifest_id",
            "schema_read_write_set_descriptor_id",
            "writer_protocol_epoch_id",
            "migration_epoch_id"
        ],
        ["generation_id"]
    ),
    manifest!(
        GenerationRoots,
        ["generation_id", "root_position", "object_id"],
        ["generation_id", "root_position"]
    ),
    manifest!(
        Heads,
        [
            "head_id",
            "generation_id",
            "generation_ordinal",
            "head_revision",
            "previous_head_id"
        ],
        ["head_id"]
    ),
    manifest!(
        ReachabilitySnapshots,
        ["snapshot_id", "head_id", "retention_revision"],
        ["snapshot_id"]
    ),
    manifest!(
        ReachabilityRoots,
        ["snapshot_id", "root_position", "root_kind", "object_id"],
        ["snapshot_id", "root_position"]
    ),
    manifest!(
        ReachabilityObjects,
        ["snapshot_id", "object_id", "reachability_status"],
        ["snapshot_id", "object_id"]
    ),
    manifest!(
        RetentionPins,
        [
            "pin_id",
            "basis_head_id",
            "root_kind",
            "root_object_id",
            "reason_digest"
        ],
        ["pin_id"]
    ),
    manifest!(
        RetentionPinReleases,
        ["pin_id", "released_at_head_id", "reason_digest"],
        ["pin_id"]
    ),
    manifest!(
        LogicalTombstones,
        [
            "tombstone_id",
            "basis_head_id",
            "object_id",
            "reason_digest",
            "invalidation_digest"
        ],
        ["tombstone_id"]
    ),
    manifest!(
        GcPlans,
        ["plan_id", "snapshot_id", "head_id", "retention_revision"],
        ["plan_id"]
    ),
    manifest!(
        GcPlanObjects,
        ["plan_id", "object_id"],
        ["plan_id", "object_id"]
    ),
    manifest!(
        GcCollectionOccurrences,
        ["plan_id", "object_id", "stored_bytes_digest"],
        ["plan_id", "object_id"]
    ),
    manifest!(
        SealedExports,
        [
            "export_id",
            "head_id",
            "generation_id",
            "snapshot_id",
            "schema_manifest_id",
            "family_manifest_set_digest",
            "snapshot_root_id",
            "source_publication_clock",
            "committed_publication_clock",
            "payload_set_digest",
            "export_byte_length",
            "export_bytes_digest",
            "export_format",
            "backup_receipt_id",
            "carrier_format"
        ],
        ["export_id"]
    ),
    manifest!(
        SealedExportPins,
        ["export_id", "pin_id"],
        ["export_id", "pin_id"]
    ),
    manifest!(
        SealedExportObjects,
        ["export_id", "object_id", "entry_kind"],
        ["export_id", "object_id"]
    ),
    manifest!(
        RestoreCandidates,
        [
            "candidate_id",
            "source_export_id",
            "source_domain_id",
            "source_export_bytes_digest",
            "source_schema_manifest_id",
            "source_snapshot_root_id",
            "destination_domain_id",
            "candidate_generation_id",
            "candidate_head_id",
            "candidate_snapshot_id",
            "verification_digest"
        ],
        ["candidate_id"]
    ),
    manifest!(
        RestoreCandidateRoots,
        ["candidate_id", "root_position", "object_id"],
        ["candidate_id", "root_position"]
    ),
    manifest!(
        Idempotency,
        [
            "namespace",
            "key_digest",
            "meaning_digest",
            "result_object_id",
            "generation_id",
            "head_id"
        ],
        ["namespace", "key_digest"]
    ),
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct StoreSnapshotDigestV1([u8; DIGEST_BYTES]);

impl StoreSnapshotDigestV1 {
    pub(crate) fn as_bytes(&self) -> &[u8; DIGEST_BYTES] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct NonNegativeV1(u64);

impl NonNegativeV1 {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PositiveV1(u64);

impl PositiveV1 {
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum StoreStateV1 {
    Inactive,
    Active,
}

impl StoreStateV1 {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Active => "active",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum SnapshotRetentionRootKindV1 {
    ActiveGeneration,
    CandidateSeal,
    UnresolvedEffect,
    RecoveryBundle,
    RollbackBundle,
    LegalHold,
    ReplayHorizon,
    ProtocolClosure,
    BinaryClosure,
    ResourceClosure,
    SealedExport,
    MigrationAssociation,
    Quarantine,
    RecoveryCommitment,
}

impl SnapshotRetentionRootKindV1 {
    const ALL: [Self; 14] = [
        Self::ActiveGeneration,
        Self::CandidateSeal,
        Self::UnresolvedEffect,
        Self::RecoveryBundle,
        Self::RollbackBundle,
        Self::LegalHold,
        Self::ReplayHorizon,
        Self::ProtocolClosure,
        Self::BinaryClosure,
        Self::ResourceClosure,
        Self::SealedExport,
        Self::MigrationAssociation,
        Self::Quarantine,
        Self::RecoveryCommitment,
    ];

    pub(crate) const fn tag(self) -> u64 {
        match self {
            Self::ActiveGeneration => 1,
            Self::CandidateSeal => 2,
            Self::UnresolvedEffect => 3,
            Self::RecoveryBundle => 4,
            Self::RollbackBundle => 5,
            Self::LegalHold => 6,
            Self::ReplayHorizon => 7,
            Self::ProtocolClosure => 8,
            Self::BinaryClosure => 9,
            Self::ResourceClosure => 10,
            Self::SealedExport => 11,
            Self::MigrationAssociation => 12,
            Self::Quarantine => 13,
            Self::RecoveryCommitment => 14,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ReachabilityStatusV1 {
    Reachable,
    Tombstoned,
}

impl ReachabilityStatusV1 {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Reachable => "reachable",
            Self::Tombstoned => "tombstoned",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum SealedExportEntryKindV1 {
    Available,
    Tombstoned,
}

impl SealedExportEntryKindV1 {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Tombstoned => "tombstoned",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StoreSnapshotRowV1 {
    Metadata {
        singleton: u8,
        schema_version: u8,
        store_role: StoreRoleV1,
        domain_id: StoreSnapshotDigestV1,
    },
    PublicationClock {
        singleton: u8,
        publication_clock: NonNegativeV1,
    },
    State {
        singleton: u8,
        state: StoreStateV1,
        state_revision: NonNegativeV1,
    },
    RetentionRevision {
        singleton: u8,
        retention_revision: NonNegativeV1,
    },
    Object {
        object_id: StoreSnapshotDigestV1,
        schema_id: StoreSnapshotDigestV1,
        logical_byte_length: NonNegativeV1,
        stored_byte_length: PositiveV1,
        stored_bytes_digest: StoreSnapshotDigestV1,
        storage_codec: String,
        key_envelope_id: Option<StoreSnapshotDigestV1>,
        key_envelope_kind: Option<String>,
    },
    ObjectReference {
        object_id: StoreSnapshotDigestV1,
        reference_position: NonNegativeV1,
        referenced_object_id: StoreSnapshotDigestV1,
    },
    Generation {
        generation_id: StoreSnapshotDigestV1,
        generation_ordinal: PositiveV1,
        previous_generation_id: Option<StoreSnapshotDigestV1>,
        contract_root_id: StoreSnapshotDigestV1,
        writer_compatibility_manifest_id: StoreSnapshotDigestV1,
        association_schema_id: StoreSnapshotDigestV1,
        finality_edge_manifest_id: StoreSnapshotDigestV1,
        schema_read_write_set_descriptor_id: StoreSnapshotDigestV1,
        writer_protocol_epoch_id: StoreSnapshotDigestV1,
        migration_epoch_id: StoreSnapshotDigestV1,
    },
    GenerationRoot {
        generation_id: StoreSnapshotDigestV1,
        root_position: NonNegativeV1,
        object_id: StoreSnapshotDigestV1,
    },
    Head {
        head_id: StoreSnapshotDigestV1,
        generation_id: StoreSnapshotDigestV1,
        generation_ordinal: PositiveV1,
        head_revision: PositiveV1,
        previous_head_id: Option<StoreSnapshotDigestV1>,
    },
    ActiveHead {
        singleton: u8,
        head_id: StoreSnapshotDigestV1,
        head_revision: PositiveV1,
    },
    ReachabilitySnapshot {
        snapshot_id: StoreSnapshotDigestV1,
        head_id: StoreSnapshotDigestV1,
        retention_revision: PositiveV1,
    },
    ReachabilityRoot {
        snapshot_id: StoreSnapshotDigestV1,
        root_position: NonNegativeV1,
        root_kind: SnapshotRetentionRootKindV1,
        object_id: StoreSnapshotDigestV1,
    },
    ReachabilityObject {
        snapshot_id: StoreSnapshotDigestV1,
        object_id: StoreSnapshotDigestV1,
        reachability_status: ReachabilityStatusV1,
    },
    RetentionPin {
        pin_id: StoreSnapshotDigestV1,
        basis_head_id: StoreSnapshotDigestV1,
        root_kind: SnapshotRetentionRootKindV1,
        root_object_id: StoreSnapshotDigestV1,
        reason_digest: StoreSnapshotDigestV1,
    },
    RetentionPinRelease {
        pin_id: StoreSnapshotDigestV1,
        released_at_head_id: StoreSnapshotDigestV1,
        reason_digest: StoreSnapshotDigestV1,
    },
    LogicalTombstone {
        tombstone_id: StoreSnapshotDigestV1,
        basis_head_id: StoreSnapshotDigestV1,
        object_id: StoreSnapshotDigestV1,
        reason_digest: StoreSnapshotDigestV1,
        invalidation_digest: StoreSnapshotDigestV1,
    },
    GcPlan {
        plan_id: StoreSnapshotDigestV1,
        snapshot_id: StoreSnapshotDigestV1,
        head_id: StoreSnapshotDigestV1,
        retention_revision: PositiveV1,
    },
    GcPlanObject {
        plan_id: StoreSnapshotDigestV1,
        object_id: StoreSnapshotDigestV1,
    },
    GcCollectionOccurrence {
        plan_id: StoreSnapshotDigestV1,
        object_id: StoreSnapshotDigestV1,
        stored_bytes_digest: StoreSnapshotDigestV1,
    },
    SealedExport {
        export_id: StoreSnapshotDigestV1,
        head_id: StoreSnapshotDigestV1,
        generation_id: StoreSnapshotDigestV1,
        snapshot_id: StoreSnapshotDigestV1,
        schema_manifest_id: StoreSnapshotDigestV1,
        family_manifest_set_digest: StoreSnapshotDigestV1,
        snapshot_root_id: StoreSnapshotDigestV1,
        source_publication_clock: NonNegativeV1,
        committed_publication_clock: NonNegativeV1,
        payload_set_digest: StoreSnapshotDigestV1,
        export_byte_length: PositiveV1,
        export_bytes_digest: StoreSnapshotDigestV1,
        export_format: String,
        backup_receipt_id: StoreSnapshotDigestV1,
        carrier_format: String,
    },
    SealedExportPin {
        export_id: StoreSnapshotDigestV1,
        pin_id: StoreSnapshotDigestV1,
    },
    SealedExportObject {
        export_id: StoreSnapshotDigestV1,
        object_id: StoreSnapshotDigestV1,
        entry_kind: SealedExportEntryKindV1,
    },
    RestoreCandidate {
        candidate_id: StoreSnapshotDigestV1,
        source_export_id: StoreSnapshotDigestV1,
        source_domain_id: StoreSnapshotDigestV1,
        source_export_bytes_digest: StoreSnapshotDigestV1,
        source_schema_manifest_id: StoreSnapshotDigestV1,
        source_snapshot_root_id: StoreSnapshotDigestV1,
        destination_domain_id: StoreSnapshotDigestV1,
        candidate_generation_id: StoreSnapshotDigestV1,
        candidate_head_id: StoreSnapshotDigestV1,
        candidate_snapshot_id: StoreSnapshotDigestV1,
        verification_digest: StoreSnapshotDigestV1,
    },
    RestoreCandidateRoot {
        candidate_id: StoreSnapshotDigestV1,
        root_position: NonNegativeV1,
        object_id: StoreSnapshotDigestV1,
    },
    Idempotency {
        namespace: String,
        key_digest: StoreSnapshotDigestV1,
        meaning_digest: StoreSnapshotDigestV1,
        result_object_id: StoreSnapshotDigestV1,
        generation_id: StoreSnapshotDigestV1,
        head_id: StoreSnapshotDigestV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoreSnapshotSortKeyV1 {
    family: StoreSnapshotFamilyV1,
    table: StoreSnapshotTableV1,
    primary_key_cbor: Vec<u8>,
}

impl Ord for StoreSnapshotSortKeyV1 {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.family, self.table, &self.primary_key_cbor).cmp(&(
            other.family,
            other.table,
            &other.primary_key_cbor,
        ))
    }
}

impl PartialOrd for StoreSnapshotSortKeyV1 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl StoreSnapshotRowV1 {
    #[cfg(test)]
    pub(crate) fn rewrite_publication_clock(
        rows: &mut [Self],
        replacement_publication_clock: u64,
    ) -> bool {
        let mut matches = 0_usize;
        for row in rows {
            if let Self::PublicationClock {
                publication_clock, ..
            } = row
            {
                *publication_clock = NonNegativeV1(replacement_publication_clock);
                matches += 1;
            }
        }
        matches == 1
    }

    #[cfg(test)]
    pub(crate) fn rewrite_sealed_export_artifact(
        rows: &mut [Self],
        snapshot_root_id: [u8; 32],
        replacement_export_id: Option<[u8; 32]>,
        replacement_byte_length: Option<u64>,
        replacement_bytes_digest: Option<[u8; 32]>,
    ) -> bool {
        let mut original_export_id = None;
        for row in rows.iter_mut() {
            let Self::SealedExport {
                export_id,
                snapshot_root_id: candidate_root_id,
                export_byte_length,
                export_bytes_digest,
                ..
            } = row
            else {
                continue;
            };
            if candidate_root_id.as_bytes() != &snapshot_root_id {
                continue;
            }
            original_export_id = Some(*export_id.as_bytes());
            if let Some(replacement) = replacement_export_id {
                *export_id = StoreSnapshotDigestV1(replacement);
            }
            if let Some(replacement) = replacement_byte_length {
                if replacement == 0 {
                    return false;
                }
                *export_byte_length = PositiveV1(replacement);
            }
            if let Some(replacement) = replacement_bytes_digest {
                *export_bytes_digest = StoreSnapshotDigestV1(replacement);
            }
            break;
        }
        let Some(original_export_id) = original_export_id else {
            return false;
        };
        if let Some(replacement) = replacement_export_id {
            for row in rows {
                match row {
                    Self::SealedExportPin { export_id, .. }
                    | Self::SealedExportObject { export_id, .. }
                        if export_id.as_bytes() == &original_export_id =>
                    {
                        *export_id = StoreSnapshotDigestV1(replacement);
                    }
                    _ => {}
                }
            }
        }
        true
    }

    pub(crate) const fn family(&self) -> StoreSnapshotFamilyV1 {
        self.table().family()
    }

    pub(crate) const fn table(&self) -> StoreSnapshotTableV1 {
        match self {
            Self::Metadata { .. } => StoreSnapshotTableV1::Metadata,
            Self::PublicationClock { .. } => StoreSnapshotTableV1::PublicationClock,
            Self::State { .. } => StoreSnapshotTableV1::State,
            Self::RetentionRevision { .. } => StoreSnapshotTableV1::RetentionRevision,
            Self::Object { .. } => StoreSnapshotTableV1::Objects,
            Self::ObjectReference { .. } => StoreSnapshotTableV1::ObjectReferences,
            Self::Generation { .. } => StoreSnapshotTableV1::Generations,
            Self::GenerationRoot { .. } => StoreSnapshotTableV1::GenerationRoots,
            Self::Head { .. } => StoreSnapshotTableV1::Heads,
            Self::ActiveHead { .. } => StoreSnapshotTableV1::ActiveHead,
            Self::ReachabilitySnapshot { .. } => StoreSnapshotTableV1::ReachabilitySnapshots,
            Self::ReachabilityRoot { .. } => StoreSnapshotTableV1::ReachabilityRoots,
            Self::ReachabilityObject { .. } => StoreSnapshotTableV1::ReachabilityObjects,
            Self::RetentionPin { .. } => StoreSnapshotTableV1::RetentionPins,
            Self::RetentionPinRelease { .. } => StoreSnapshotTableV1::RetentionPinReleases,
            Self::LogicalTombstone { .. } => StoreSnapshotTableV1::LogicalTombstones,
            Self::GcPlan { .. } => StoreSnapshotTableV1::GcPlans,
            Self::GcPlanObject { .. } => StoreSnapshotTableV1::GcPlanObjects,
            Self::GcCollectionOccurrence { .. } => StoreSnapshotTableV1::GcCollectionOccurrences,
            Self::SealedExport { .. } => StoreSnapshotTableV1::SealedExports,
            Self::SealedExportPin { .. } => StoreSnapshotTableV1::SealedExportPins,
            Self::SealedExportObject { .. } => StoreSnapshotTableV1::SealedExportObjects,
            Self::RestoreCandidate { .. } => StoreSnapshotTableV1::RestoreCandidates,
            Self::RestoreCandidateRoot { .. } => StoreSnapshotTableV1::RestoreCandidateRoots,
            Self::Idempotency { .. } => StoreSnapshotTableV1::Idempotency,
        }
    }

    pub(crate) fn load_all(connection: &Connection) -> Result<Vec<Self>, StoreSnapshotRowError> {
        let mut output = Vec::new();
        let mut bounds = StoreSnapshotRowBoundsV1::new();
        Self::visit_connection_rows(connection, |row| {
            bounds.observe(&row)?;
            output.push(row);
            Ok(())
        })?;
        Ok(output)
    }

    pub(crate) fn validate_connection_bounds(
        connection: &Connection,
    ) -> Result<(), StoreSnapshotRowError> {
        let mut bounds = StoreSnapshotRowBoundsV1::new();
        Self::visit_connection_rows(connection, |row| bounds.observe(&row))
    }

    pub(crate) fn validate_bounds(rows: &[Self]) -> Result<(), StoreSnapshotRowError> {
        let mut bounds = StoreSnapshotRowBoundsV1::new();
        for row in rows {
            bounds.observe(row)?;
        }
        Ok(())
    }

    fn visit_connection_rows(
        connection: &Connection,
        mut visitor: impl FnMut(Self) -> Result<(), StoreSnapshotRowError>,
    ) -> Result<(), StoreSnapshotRowError> {
        for (table, query) in SNAPSHOT_SELECTS_V1 {
            let mut statement = connection.prepare(query)?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                visitor(Self::from_sql_row(*table, row)?)?;
            }
        }
        Ok(())
    }

    pub(crate) fn encode_canonical(&self) -> Result<Vec<u8>, StoreSnapshotRowError> {
        deterministic_cbor::encode(&self.to_canonical_value()).map_err(Into::into)
    }

    pub(crate) fn decode_canonical(bytes: &[u8]) -> Result<Self, StoreSnapshotRowError> {
        Self::from_canonical_value(deterministic_cbor::decode(bytes)?)
    }

    pub(crate) fn canonical_sort_key(
        &self,
    ) -> Result<StoreSnapshotSortKeyV1, StoreSnapshotRowError> {
        Ok(StoreSnapshotSortKeyV1 {
            family: self.family(),
            table: self.table(),
            primary_key_cbor: deterministic_cbor::encode(&CborValue::Array(
                self.primary_key_values(),
            ))?,
        })
    }

    pub(crate) fn to_canonical_value(&self) -> CborValue {
        let mut values = vec![
            CborValue::Unsigned(SNAPSHOT_ROW_VERSION_V1),
            CborValue::Unsigned(self.family().tag()),
            CborValue::Unsigned(self.table().tag()),
        ];
        match self {
            Self::Metadata {
                singleton,
                schema_version,
                store_role,
                domain_id,
            } => values.extend([
                unsigned(*singleton),
                unsigned(*schema_version),
                unsigned(store_role.tag()),
                digest(*domain_id),
            ]),
            Self::PublicationClock {
                singleton,
                publication_clock,
            } => values.extend([unsigned(*singleton), unsigned(publication_clock.get())]),
            Self::State {
                singleton,
                state,
                state_revision,
            } => values.extend([
                unsigned(*singleton),
                text(state.as_str()),
                unsigned(state_revision.get()),
            ]),
            Self::RetentionRevision {
                singleton,
                retention_revision,
            } => values.extend([unsigned(*singleton), unsigned(retention_revision.get())]),
            Self::Object {
                object_id,
                schema_id,
                logical_byte_length,
                stored_byte_length,
                stored_bytes_digest,
                storage_codec,
                key_envelope_id,
                key_envelope_kind,
            } => values.extend([
                digest(*object_id),
                digest(*schema_id),
                unsigned(logical_byte_length.get()),
                unsigned(stored_byte_length.get()),
                digest(*stored_bytes_digest),
                text(storage_codec),
                optional_digest(*key_envelope_id),
                optional_text(key_envelope_kind.as_deref()),
            ]),
            Self::ObjectReference {
                object_id,
                reference_position,
                referenced_object_id,
            } => values.extend([
                digest(*object_id),
                unsigned(reference_position.get()),
                digest(*referenced_object_id),
            ]),
            Self::Generation {
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
            } => values.extend([
                digest(*generation_id),
                unsigned(generation_ordinal.get()),
                optional_digest(*previous_generation_id),
                digest(*contract_root_id),
                digest(*writer_compatibility_manifest_id),
                digest(*association_schema_id),
                digest(*finality_edge_manifest_id),
                digest(*schema_read_write_set_descriptor_id),
                digest(*writer_protocol_epoch_id),
                digest(*migration_epoch_id),
            ]),
            Self::GenerationRoot {
                generation_id,
                root_position,
                object_id,
            } => values.extend([
                digest(*generation_id),
                unsigned(root_position.get()),
                digest(*object_id),
            ]),
            Self::Head {
                head_id,
                generation_id,
                generation_ordinal,
                head_revision,
                previous_head_id,
            } => values.extend([
                digest(*head_id),
                digest(*generation_id),
                unsigned(generation_ordinal.get()),
                unsigned(head_revision.get()),
                optional_digest(*previous_head_id),
            ]),
            Self::ActiveHead {
                singleton,
                head_id,
                head_revision,
            } => values.extend([
                unsigned(*singleton),
                digest(*head_id),
                unsigned(head_revision.get()),
            ]),
            Self::ReachabilitySnapshot {
                snapshot_id,
                head_id,
                retention_revision,
            } => values.extend([
                digest(*snapshot_id),
                digest(*head_id),
                unsigned(retention_revision.get()),
            ]),
            Self::ReachabilityRoot {
                snapshot_id,
                root_position,
                root_kind,
                object_id,
            } => values.extend([
                digest(*snapshot_id),
                unsigned(root_position.get()),
                unsigned(root_kind.tag()),
                digest(*object_id),
            ]),
            Self::ReachabilityObject {
                snapshot_id,
                object_id,
                reachability_status,
            } => values.extend([
                digest(*snapshot_id),
                digest(*object_id),
                text(reachability_status.as_str()),
            ]),
            Self::RetentionPin {
                pin_id,
                basis_head_id,
                root_kind,
                root_object_id,
                reason_digest,
            } => values.extend([
                digest(*pin_id),
                digest(*basis_head_id),
                unsigned(root_kind.tag()),
                digest(*root_object_id),
                digest(*reason_digest),
            ]),
            Self::RetentionPinRelease {
                pin_id,
                released_at_head_id,
                reason_digest,
            } => values.extend([
                digest(*pin_id),
                digest(*released_at_head_id),
                digest(*reason_digest),
            ]),
            Self::LogicalTombstone {
                tombstone_id,
                basis_head_id,
                object_id,
                reason_digest,
                invalidation_digest,
            } => values.extend([
                digest(*tombstone_id),
                digest(*basis_head_id),
                digest(*object_id),
                digest(*reason_digest),
                digest(*invalidation_digest),
            ]),
            Self::GcPlan {
                plan_id,
                snapshot_id,
                head_id,
                retention_revision,
            } => values.extend([
                digest(*plan_id),
                digest(*snapshot_id),
                digest(*head_id),
                unsigned(retention_revision.get()),
            ]),
            Self::GcPlanObject { plan_id, object_id } => {
                values.extend([digest(*plan_id), digest(*object_id)])
            }
            Self::GcCollectionOccurrence {
                plan_id,
                object_id,
                stored_bytes_digest,
            } => values.extend([
                digest(*plan_id),
                digest(*object_id),
                digest(*stored_bytes_digest),
            ]),
            Self::SealedExport {
                export_id,
                head_id,
                generation_id,
                snapshot_id,
                schema_manifest_id,
                family_manifest_set_digest,
                snapshot_root_id,
                source_publication_clock,
                committed_publication_clock,
                payload_set_digest,
                export_byte_length,
                export_bytes_digest,
                export_format,
                backup_receipt_id,
                carrier_format,
            } => values.extend([
                digest(*export_id),
                digest(*head_id),
                digest(*generation_id),
                digest(*snapshot_id),
                digest(*schema_manifest_id),
                digest(*family_manifest_set_digest),
                digest(*snapshot_root_id),
                unsigned(source_publication_clock.get()),
                unsigned(committed_publication_clock.get()),
                digest(*payload_set_digest),
                unsigned(export_byte_length.get()),
                digest(*export_bytes_digest),
                text(export_format),
                digest(*backup_receipt_id),
                text(carrier_format),
            ]),
            Self::SealedExportPin { export_id, pin_id } => {
                values.extend([digest(*export_id), digest(*pin_id)])
            }
            Self::SealedExportObject {
                export_id,
                object_id,
                entry_kind,
            } => values.extend([
                digest(*export_id),
                digest(*object_id),
                text(entry_kind.as_str()),
            ]),
            Self::RestoreCandidate {
                candidate_id,
                source_export_id,
                source_domain_id,
                source_export_bytes_digest,
                source_schema_manifest_id,
                source_snapshot_root_id,
                destination_domain_id,
                candidate_generation_id,
                candidate_head_id,
                candidate_snapshot_id,
                verification_digest,
            } => values.extend([
                digest(*candidate_id),
                digest(*source_export_id),
                digest(*source_domain_id),
                digest(*source_export_bytes_digest),
                digest(*source_schema_manifest_id),
                digest(*source_snapshot_root_id),
                digest(*destination_domain_id),
                digest(*candidate_generation_id),
                digest(*candidate_head_id),
                digest(*candidate_snapshot_id),
                digest(*verification_digest),
            ]),
            Self::RestoreCandidateRoot {
                candidate_id,
                root_position,
                object_id,
            } => values.extend([
                digest(*candidate_id),
                unsigned(root_position.get()),
                digest(*object_id),
            ]),
            Self::Idempotency {
                namespace,
                key_digest,
                meaning_digest,
                result_object_id,
                generation_id,
                head_id,
            } => values.extend([
                text(namespace),
                digest(*key_digest),
                digest(*meaning_digest),
                digest(*result_object_id),
                digest(*generation_id),
                digest(*head_id),
            ]),
        }
        CborValue::Array(values)
    }

    pub(crate) fn from_canonical_value(value: CborValue) -> Result<Self, StoreSnapshotRowError> {
        let CborValue::Array(values) = value else {
            return Err(StoreSnapshotRowError::RowNotArray);
        };
        if values.len() < 3 {
            return Err(StoreSnapshotRowError::ShortRow(values.len()));
        }
        let mut decoder = CanonicalRowDecoder::new(values);
        let version = decoder.unsigned("row_version")?;
        if version != SNAPSHOT_ROW_VERSION_V1 {
            return Err(StoreSnapshotRowError::UnsupportedRowVersion(version));
        }
        let family = StoreSnapshotFamilyV1::from_tag(decoder.unsigned("family")?)?;
        let table = StoreSnapshotTableV1::from_tag(decoder.unsigned("table")?)?;
        if table.family() != family {
            return Err(StoreSnapshotRowError::FamilyTableMismatch { family, table });
        }
        let row = Self::decode_columns(table, &mut decoder)?;
        decoder.finish(table)?;
        Ok(row)
    }

    fn primary_key_values(&self) -> Vec<CborValue> {
        match self {
            Self::Metadata { singleton, .. }
            | Self::PublicationClock { singleton, .. }
            | Self::State { singleton, .. }
            | Self::RetentionRevision { singleton, .. }
            | Self::ActiveHead { singleton, .. } => vec![unsigned(*singleton)],
            Self::Object { object_id, .. } => vec![digest(*object_id)],
            Self::ObjectReference {
                object_id,
                reference_position,
                ..
            } => vec![digest(*object_id), unsigned(reference_position.get())],
            Self::Generation { generation_id, .. } => vec![digest(*generation_id)],
            Self::GenerationRoot {
                generation_id,
                root_position,
                ..
            } => vec![digest(*generation_id), unsigned(root_position.get())],
            Self::Head { head_id, .. } => vec![digest(*head_id)],
            Self::ReachabilitySnapshot { snapshot_id, .. } => vec![digest(*snapshot_id)],
            Self::ReachabilityRoot {
                snapshot_id,
                root_position,
                ..
            } => vec![digest(*snapshot_id), unsigned(root_position.get())],
            Self::ReachabilityObject {
                snapshot_id,
                object_id,
                ..
            } => vec![digest(*snapshot_id), digest(*object_id)],
            Self::RetentionPin { pin_id, .. } | Self::RetentionPinRelease { pin_id, .. } => {
                vec![digest(*pin_id)]
            }
            Self::LogicalTombstone { tombstone_id, .. } => vec![digest(*tombstone_id)],
            Self::GcPlan { plan_id, .. } => vec![digest(*plan_id)],
            Self::GcPlanObject { plan_id, object_id }
            | Self::GcCollectionOccurrence {
                plan_id, object_id, ..
            } => vec![digest(*plan_id), digest(*object_id)],
            Self::SealedExport { export_id, .. } => vec![digest(*export_id)],
            Self::SealedExportPin { export_id, pin_id } => {
                vec![digest(*export_id), digest(*pin_id)]
            }
            Self::SealedExportObject {
                export_id,
                object_id,
                ..
            } => vec![digest(*export_id), digest(*object_id)],
            Self::RestoreCandidate { candidate_id, .. } => vec![digest(*candidate_id)],
            Self::RestoreCandidateRoot {
                candidate_id,
                root_position,
                ..
            } => vec![digest(*candidate_id), unsigned(root_position.get())],
            Self::Idempotency {
                namespace,
                key_digest,
                ..
            } => vec![text(namespace), digest(*key_digest)],
        }
    }
}

fn unsigned(value: impl Into<u64>) -> CborValue {
    CborValue::Unsigned(value.into())
}
fn text(value: impl Into<String>) -> CborValue {
    CborValue::Text(value.into())
}
fn digest(value: StoreSnapshotDigestV1) -> CborValue {
    CborValue::Bytes(value.0.to_vec())
}
fn optional_digest(value: Option<StoreSnapshotDigestV1>) -> CborValue {
    CborValue::optional(value.map(digest))
}
fn optional_text(value: Option<&str>) -> CborValue {
    CborValue::optional(value.map(text))
}

impl StoreSnapshotRowV1 {
    fn decode_columns(
        table: StoreSnapshotTableV1,
        decoder: &mut CanonicalRowDecoder,
    ) -> Result<Self, StoreSnapshotRowError> {
        Ok(match table {
            StoreSnapshotTableV1::Metadata => Self::Metadata {
                singleton: decoder.singleton()?,
                schema_version: decoder.schema_version()?,
                store_role: decoder.store_role()?,
                domain_id: decoder.digest("domain_id")?,
            },
            StoreSnapshotTableV1::PublicationClock => Self::PublicationClock {
                singleton: decoder.singleton()?,
                publication_clock: decoder.nonnegative("publication_clock")?,
            },
            StoreSnapshotTableV1::State => Self::State {
                singleton: decoder.singleton()?,
                state: decoder.store_state()?,
                state_revision: decoder.nonnegative("state_revision")?,
            },
            StoreSnapshotTableV1::RetentionRevision => Self::RetentionRevision {
                singleton: decoder.singleton()?,
                retention_revision: decoder.nonnegative("retention_revision")?,
            },
            StoreSnapshotTableV1::Objects => {
                let object_id = decoder.digest("object_id")?;
                let schema_id = decoder.digest("schema_id")?;
                let logical_byte_length = decoder.nonnegative("logical_byte_length")?;
                let stored_byte_length = decoder.positive("stored_byte_length")?;
                let stored_bytes_digest = decoder.digest("stored_bytes_digest")?;
                let storage_codec = decoder.bounded_text("storage_codec", 1, 64)?;
                let key_envelope_id = decoder.optional_digest("key_envelope_id")?;
                let key_envelope_kind =
                    decoder.optional_bounded_text("key_envelope_kind", 1, 64)?;
                if key_envelope_id.is_some() != key_envelope_kind.is_some() {
                    return Err(StoreSnapshotRowError::InvalidColumn {
                        table,
                        column: "key_envelope_id/key_envelope_kind",
                        reason: "both envelope fields must be null or both non-null",
                    });
                }
                Self::Object {
                    object_id,
                    schema_id,
                    logical_byte_length,
                    stored_byte_length,
                    stored_bytes_digest,
                    storage_codec,
                    key_envelope_id,
                    key_envelope_kind,
                }
            }
            StoreSnapshotTableV1::ObjectReferences => {
                let object_id = decoder.digest("object_id")?;
                let reference_position = decoder.nonnegative("reference_position")?;
                let referenced_object_id = decoder.digest("referenced_object_id")?;
                if object_id == referenced_object_id {
                    return Err(StoreSnapshotRowError::InvalidColumn {
                        table,
                        column: "referenced_object_id",
                        reason: "self references are forbidden",
                    });
                }
                Self::ObjectReference {
                    object_id,
                    reference_position,
                    referenced_object_id,
                }
            }
            StoreSnapshotTableV1::Generations => {
                let generation_id = decoder.digest("generation_id")?;
                let generation_ordinal = decoder.positive("generation_ordinal")?;
                let previous_generation_id = decoder.optional_digest("previous_generation_id")?;
                if (generation_ordinal.get() == 1) != previous_generation_id.is_none() {
                    return Err(StoreSnapshotRowError::InvalidColumn {
                        table,
                        column: "previous_generation_id",
                        reason: "only generation ordinal one omits its predecessor",
                    });
                }
                Self::Generation {
                    generation_id,
                    generation_ordinal,
                    previous_generation_id,
                    contract_root_id: decoder.digest("contract_root_id")?,
                    writer_compatibility_manifest_id: decoder
                        .digest("writer_compatibility_manifest_id")?,
                    association_schema_id: decoder.digest("association_schema_id")?,
                    finality_edge_manifest_id: decoder.digest("finality_edge_manifest_id")?,
                    schema_read_write_set_descriptor_id: decoder
                        .digest("schema_read_write_set_descriptor_id")?,
                    writer_protocol_epoch_id: decoder.digest("writer_protocol_epoch_id")?,
                    migration_epoch_id: decoder.digest("migration_epoch_id")?,
                }
            }
            StoreSnapshotTableV1::GenerationRoots => Self::GenerationRoot {
                generation_id: decoder.digest("generation_id")?,
                root_position: decoder.nonnegative("root_position")?,
                object_id: decoder.digest("object_id")?,
            },
            StoreSnapshotTableV1::Heads => {
                let head_id = decoder.digest("head_id")?;
                let generation_id = decoder.digest("generation_id")?;
                let generation_ordinal = decoder.positive("generation_ordinal")?;
                let head_revision = decoder.positive("head_revision")?;
                let previous_head_id = decoder.optional_digest("previous_head_id")?;
                if generation_ordinal != head_revision {
                    return Err(StoreSnapshotRowError::InvalidColumn {
                        table,
                        column: "generation_ordinal/head_revision",
                        reason: "generation ordinal must equal head revision",
                    });
                }
                if (head_revision.get() == 1) != previous_head_id.is_none() {
                    return Err(StoreSnapshotRowError::InvalidColumn {
                        table,
                        column: "previous_head_id",
                        reason: "only head revision one omits its predecessor",
                    });
                }
                Self::Head {
                    head_id,
                    generation_id,
                    generation_ordinal,
                    head_revision,
                    previous_head_id,
                }
            }
            StoreSnapshotTableV1::ActiveHead => Self::ActiveHead {
                singleton: decoder.singleton()?,
                head_id: decoder.digest("head_id")?,
                head_revision: decoder.positive("head_revision")?,
            },
            StoreSnapshotTableV1::ReachabilitySnapshots => Self::ReachabilitySnapshot {
                snapshot_id: decoder.digest("snapshot_id")?,
                head_id: decoder.digest("head_id")?,
                retention_revision: decoder.positive("retention_revision")?,
            },
            StoreSnapshotTableV1::ReachabilityRoots => Self::ReachabilityRoot {
                snapshot_id: decoder.digest("snapshot_id")?,
                root_position: decoder.nonnegative("root_position")?,
                root_kind: decoder.root_kind()?,
                object_id: decoder.digest("object_id")?,
            },
            StoreSnapshotTableV1::ReachabilityObjects => Self::ReachabilityObject {
                snapshot_id: decoder.digest("snapshot_id")?,
                object_id: decoder.digest("object_id")?,
                reachability_status: decoder.reachability_status()?,
            },
            StoreSnapshotTableV1::RetentionPins => Self::RetentionPin {
                pin_id: decoder.digest("pin_id")?,
                basis_head_id: decoder.digest("basis_head_id")?,
                root_kind: decoder.root_kind()?,
                root_object_id: decoder.digest("root_object_id")?,
                reason_digest: decoder.digest("reason_digest")?,
            },
            StoreSnapshotTableV1::RetentionPinReleases => Self::RetentionPinRelease {
                pin_id: decoder.digest("pin_id")?,
                released_at_head_id: decoder.digest("released_at_head_id")?,
                reason_digest: decoder.digest("reason_digest")?,
            },
            StoreSnapshotTableV1::LogicalTombstones => Self::LogicalTombstone {
                tombstone_id: decoder.digest("tombstone_id")?,
                basis_head_id: decoder.digest("basis_head_id")?,
                object_id: decoder.digest("object_id")?,
                reason_digest: decoder.digest("reason_digest")?,
                invalidation_digest: decoder.digest("invalidation_digest")?,
            },
            StoreSnapshotTableV1::GcPlans => Self::GcPlan {
                plan_id: decoder.digest("plan_id")?,
                snapshot_id: decoder.digest("snapshot_id")?,
                head_id: decoder.digest("head_id")?,
                retention_revision: decoder.positive("retention_revision")?,
            },
            StoreSnapshotTableV1::GcPlanObjects => Self::GcPlanObject {
                plan_id: decoder.digest("plan_id")?,
                object_id: decoder.digest("object_id")?,
            },
            StoreSnapshotTableV1::GcCollectionOccurrences => Self::GcCollectionOccurrence {
                plan_id: decoder.digest("plan_id")?,
                object_id: decoder.digest("object_id")?,
                stored_bytes_digest: decoder.digest("stored_bytes_digest")?,
            },
            StoreSnapshotTableV1::SealedExports => {
                let export_id = decoder.digest("export_id")?;
                let head_id = decoder.digest("head_id")?;
                let generation_id = decoder.digest("generation_id")?;
                let snapshot_id = decoder.digest("snapshot_id")?;
                let schema_manifest_id = decoder.digest("schema_manifest_id")?;
                let family_manifest_set_digest = decoder.digest("family_manifest_set_digest")?;
                let snapshot_root_id = decoder.digest("snapshot_root_id")?;
                let source_publication_clock = decoder.nonnegative("source_publication_clock")?;
                let committed_publication_clock =
                    decoder.nonnegative("committed_publication_clock")?;
                if source_publication_clock.get().checked_add(1)
                    != Some(committed_publication_clock.get())
                {
                    return Err(StoreSnapshotRowError::InvalidColumn {
                        table,
                        column: "committed_publication_clock",
                        reason: "must equal source_publication_clock + 1",
                    });
                }
                let payload_set_digest = decoder.digest("payload_set_digest")?;
                let export_byte_length = decoder.positive("export_byte_length")?;
                let export_bytes_digest = decoder.digest("export_bytes_digest")?;
                let export_format = decoder.bounded_text("export_format", 1, 64)?;
                let backup_receipt_id = decoder.digest("backup_receipt_id")?;
                let carrier_format = decoder.bounded_text("carrier_format", 1, 64)?;
                if carrier_format != SEALED_BACKUP_FORMAT_V1 {
                    return Err(StoreSnapshotRowError::InvalidColumn {
                        table,
                        column: "carrier_format",
                        reason: "must name the exact sealed-backup v1 carrier",
                    });
                }
                Self::SealedExport {
                    export_id,
                    head_id,
                    generation_id,
                    snapshot_id,
                    schema_manifest_id,
                    family_manifest_set_digest,
                    snapshot_root_id,
                    source_publication_clock,
                    committed_publication_clock,
                    payload_set_digest,
                    export_byte_length,
                    export_bytes_digest,
                    export_format,
                    backup_receipt_id,
                    carrier_format,
                }
            }
            StoreSnapshotTableV1::SealedExportPins => Self::SealedExportPin {
                export_id: decoder.digest("export_id")?,
                pin_id: decoder.digest("pin_id")?,
            },
            StoreSnapshotTableV1::SealedExportObjects => Self::SealedExportObject {
                export_id: decoder.digest("export_id")?,
                object_id: decoder.digest("object_id")?,
                entry_kind: decoder.export_entry_kind()?,
            },
            StoreSnapshotTableV1::RestoreCandidates => Self::RestoreCandidate {
                candidate_id: decoder.digest("candidate_id")?,
                source_export_id: decoder.digest("source_export_id")?,
                source_domain_id: decoder.digest("source_domain_id")?,
                source_export_bytes_digest: decoder.digest("source_export_bytes_digest")?,
                source_schema_manifest_id: decoder.digest("source_schema_manifest_id")?,
                source_snapshot_root_id: decoder.digest("source_snapshot_root_id")?,
                destination_domain_id: decoder.digest("destination_domain_id")?,
                candidate_generation_id: decoder.digest("candidate_generation_id")?,
                candidate_head_id: decoder.digest("candidate_head_id")?,
                candidate_snapshot_id: decoder.digest("candidate_snapshot_id")?,
                verification_digest: decoder.digest("verification_digest")?,
            },
            StoreSnapshotTableV1::RestoreCandidateRoots => Self::RestoreCandidateRoot {
                candidate_id: decoder.digest("candidate_id")?,
                root_position: decoder.nonnegative("root_position")?,
                object_id: decoder.digest("object_id")?,
            },
            StoreSnapshotTableV1::Idempotency => Self::Idempotency {
                namespace: decoder.bounded_text("namespace", 1, 128)?,
                key_digest: decoder.digest("key_digest")?,
                meaning_digest: decoder.digest("meaning_digest")?,
                result_object_id: decoder.digest("result_object_id")?,
                generation_id: decoder.digest("generation_id")?,
                head_id: decoder.digest("head_id")?,
            },
        })
    }
}

struct CanonicalRowDecoder {
    values: std::vec::IntoIter<CborValue>,
}

impl CanonicalRowDecoder {
    fn new(values: Vec<CborValue>) -> Self {
        Self {
            values: values.into_iter(),
        }
    }

    fn next(&mut self, column: &'static str) -> Result<CborValue, StoreSnapshotRowError> {
        self.values
            .next()
            .ok_or(StoreSnapshotRowError::MissingColumn(column))
    }

    fn unsigned(&mut self, column: &'static str) -> Result<u64, StoreSnapshotRowError> {
        match self.next(column)? {
            CborValue::Unsigned(value) => Ok(value),
            _ => Err(StoreSnapshotRowError::WrongCanonicalType {
                column,
                expected: "unsigned integer",
            }),
        }
    }

    fn singleton(&mut self) -> Result<u8, StoreSnapshotRowError> {
        let value = self.unsigned("singleton")?;
        if value == 1 {
            Ok(1)
        } else {
            Err(StoreSnapshotRowError::InvalidColumn {
                table: StoreSnapshotTableV1::Metadata,
                column: "singleton",
                reason: "singleton must equal one",
            })
        }
    }

    fn schema_version(&mut self) -> Result<u8, StoreSnapshotRowError> {
        let value = self.unsigned("schema_version")?;
        if value == 2 {
            Ok(2)
        } else {
            Err(StoreSnapshotRowError::InvalidColumn {
                table: StoreSnapshotTableV1::Metadata,
                column: "schema_version",
                reason: "schema version must equal two",
            })
        }
    }

    fn nonnegative(
        &mut self,
        column: &'static str,
    ) -> Result<NonNegativeV1, StoreSnapshotRowError> {
        Ok(NonNegativeV1(self.unsigned(column)?))
    }

    fn positive(&mut self, column: &'static str) -> Result<PositiveV1, StoreSnapshotRowError> {
        let value = self.unsigned(column)?;
        if value > 0 {
            Ok(PositiveV1(value))
        } else {
            Err(StoreSnapshotRowError::ZeroWherePositive(column))
        }
    }

    fn digest(
        &mut self,
        column: &'static str,
    ) -> Result<StoreSnapshotDigestV1, StoreSnapshotRowError> {
        let CborValue::Bytes(bytes) = self.next(column)? else {
            return Err(StoreSnapshotRowError::WrongCanonicalType {
                column,
                expected: "byte string",
            });
        };
        let actual = bytes.len();
        let bytes = bytes
            .try_into()
            .map_err(|_| StoreSnapshotRowError::InvalidDigestLength { column, actual })?;
        Ok(StoreSnapshotDigestV1(bytes))
    }

    fn bounded_text(
        &mut self,
        column: &'static str,
        min: usize,
        max: usize,
    ) -> Result<String, StoreSnapshotRowError> {
        let CborValue::Text(value) = self.next(column)? else {
            return Err(StoreSnapshotRowError::WrongCanonicalType {
                column,
                expected: "text string",
            });
        };
        if !value.is_ascii() {
            return Err(StoreSnapshotRowError::NonAsciiText(column));
        }
        if !(min..=max).contains(&value.len()) {
            return Err(StoreSnapshotRowError::InvalidTextLength {
                column,
                min,
                max,
                actual: value.len(),
            });
        }
        Ok(value)
    }

    fn optional_digest(
        &mut self,
        column: &'static str,
    ) -> Result<Option<StoreSnapshotDigestV1>, StoreSnapshotRowError> {
        let value = self.next(column)?;
        let mut nested = Self::optional_decoder(value, column)?;
        let present = nested.unsigned(column)?;
        let result = match present {
            0 => None,
            1 => Some(nested.digest(column)?),
            _ => {
                return Err(StoreSnapshotRowError::InvalidOptionalTag {
                    column,
                    tag: present,
                });
            }
        };
        nested.finish_optional(column)?;
        Ok(result)
    }

    fn optional_bounded_text(
        &mut self,
        column: &'static str,
        min: usize,
        max: usize,
    ) -> Result<Option<String>, StoreSnapshotRowError> {
        let value = self.next(column)?;
        let mut nested = Self::optional_decoder(value, column)?;
        let present = nested.unsigned(column)?;
        let result = match present {
            0 => None,
            1 => Some(nested.bounded_text(column, min, max)?),
            _ => {
                return Err(StoreSnapshotRowError::InvalidOptionalTag {
                    column,
                    tag: present,
                });
            }
        };
        nested.finish_optional(column)?;
        Ok(result)
    }

    fn optional_decoder(
        value: CborValue,
        column: &'static str,
    ) -> Result<Self, StoreSnapshotRowError> {
        match value {
            CborValue::Array(values) => Ok(Self::new(values)),
            _ => Err(StoreSnapshotRowError::WrongCanonicalType {
                column,
                expected: "optional array",
            }),
        }
    }

    fn store_role(&mut self) -> Result<StoreRoleV1, StoreSnapshotRowError> {
        let tag = self.unsigned("store_role")?;
        StoreRoleV1::from_tag(tag).map_err(|_| StoreSnapshotRowError::UnknownStoreRole(tag))
    }

    fn store_state(&mut self) -> Result<StoreStateV1, StoreSnapshotRowError> {
        match self.bounded_text("state", 6, 8)?.as_str() {
            "inactive" => Ok(StoreStateV1::Inactive),
            "active" => Ok(StoreStateV1::Active),
            _ => Err(StoreSnapshotRowError::UnknownTextEnum { column: "state" }),
        }
    }

    fn root_kind(&mut self) -> Result<SnapshotRetentionRootKindV1, StoreSnapshotRowError> {
        let tag = self.unsigned("root_kind")?;
        SnapshotRetentionRootKindV1::ALL
            .into_iter()
            .find(|kind| kind.tag() == tag)
            .ok_or(StoreSnapshotRowError::UnknownRootKind(tag))
    }

    fn reachability_status(&mut self) -> Result<ReachabilityStatusV1, StoreSnapshotRowError> {
        match self.bounded_text("reachability_status", 9, 10)?.as_str() {
            "reachable" => Ok(ReachabilityStatusV1::Reachable),
            "tombstoned" => Ok(ReachabilityStatusV1::Tombstoned),
            _ => Err(StoreSnapshotRowError::UnknownTextEnum {
                column: "reachability_status",
            }),
        }
    }

    fn export_entry_kind(&mut self) -> Result<SealedExportEntryKindV1, StoreSnapshotRowError> {
        match self.bounded_text("entry_kind", 9, 10)?.as_str() {
            "available" => Ok(SealedExportEntryKindV1::Available),
            "tombstoned" => Ok(SealedExportEntryKindV1::Tombstoned),
            _ => Err(StoreSnapshotRowError::UnknownTextEnum {
                column: "entry_kind",
            }),
        }
    }

    fn finish(mut self, table: StoreSnapshotTableV1) -> Result<(), StoreSnapshotRowError> {
        let remaining = self.values.by_ref().count();
        if remaining == 0 {
            Ok(())
        } else {
            Err(StoreSnapshotRowError::ExtraColumns {
                table,
                count: remaining,
            })
        }
    }

    fn finish_optional(mut self, column: &'static str) -> Result<(), StoreSnapshotRowError> {
        let remaining = self.values.by_ref().count();
        if remaining == 0 {
            Ok(())
        } else {
            Err(StoreSnapshotRowError::ExtraOptionalValues {
                column,
                count: remaining,
            })
        }
    }
}

const SNAPSHOT_SELECTS_V1: &[(StoreSnapshotTableV1, &str)] = &[
    (
        StoreSnapshotTableV1::Metadata,
        "SELECT singleton, schema_version, store_role, domain_id FROM store_metadata ORDER BY singleton",
    ),
    (
        StoreSnapshotTableV1::PublicationClock,
        "SELECT singleton, publication_clock FROM store_publication_clock ORDER BY singleton",
    ),
    (
        StoreSnapshotTableV1::State,
        "SELECT singleton, state, state_revision FROM store_state ORDER BY singleton",
    ),
    (
        StoreSnapshotTableV1::RetentionRevision,
        "SELECT singleton, retention_revision FROM store_retention_revision ORDER BY singleton",
    ),
    (
        StoreSnapshotTableV1::ActiveHead,
        "SELECT singleton, head_id, head_revision FROM store_active_head ORDER BY singleton",
    ),
    (
        StoreSnapshotTableV1::Objects,
        "SELECT object_id, schema_id, logical_byte_length, stored_byte_length, stored_bytes_digest, storage_codec, key_envelope_id, key_envelope_kind FROM store_objects ORDER BY object_id",
    ),
    (
        StoreSnapshotTableV1::ObjectReferences,
        "SELECT object_id, reference_position, referenced_object_id FROM store_object_references ORDER BY object_id, reference_position",
    ),
    (
        StoreSnapshotTableV1::Generations,
        "SELECT generation_id, generation_ordinal, previous_generation_id, contract_root_id, writer_compatibility_manifest_id, association_schema_id, finality_edge_manifest_id, schema_read_write_set_descriptor_id, writer_protocol_epoch_id, migration_epoch_id FROM store_generations ORDER BY generation_id",
    ),
    (
        StoreSnapshotTableV1::GenerationRoots,
        "SELECT generation_id, root_position, object_id FROM store_generation_roots ORDER BY generation_id, root_position",
    ),
    (
        StoreSnapshotTableV1::Heads,
        "SELECT head_id, generation_id, generation_ordinal, head_revision, previous_head_id FROM store_heads ORDER BY head_id",
    ),
    (
        StoreSnapshotTableV1::ReachabilitySnapshots,
        "SELECT snapshot_id, head_id, retention_revision FROM store_reachability_snapshots ORDER BY snapshot_id",
    ),
    (
        StoreSnapshotTableV1::ReachabilityRoots,
        "SELECT snapshot_id, root_position, root_kind, object_id FROM store_reachability_roots ORDER BY snapshot_id, root_position",
    ),
    (
        StoreSnapshotTableV1::ReachabilityObjects,
        "SELECT snapshot_id, object_id, reachability_status FROM store_reachability_objects ORDER BY snapshot_id, object_id",
    ),
    (
        StoreSnapshotTableV1::RetentionPins,
        "SELECT pin_id, basis_head_id, root_kind, root_object_id, reason_digest FROM store_retention_pins ORDER BY pin_id",
    ),
    (
        StoreSnapshotTableV1::RetentionPinReleases,
        "SELECT pin_id, released_at_head_id, reason_digest FROM store_retention_pin_releases ORDER BY pin_id",
    ),
    (
        StoreSnapshotTableV1::LogicalTombstones,
        "SELECT tombstone_id, basis_head_id, object_id, reason_digest, invalidation_digest FROM store_logical_tombstones ORDER BY tombstone_id",
    ),
    (
        StoreSnapshotTableV1::GcPlans,
        "SELECT plan_id, snapshot_id, head_id, retention_revision FROM store_gc_plans ORDER BY plan_id",
    ),
    (
        StoreSnapshotTableV1::GcPlanObjects,
        "SELECT plan_id, object_id FROM store_gc_plan_objects ORDER BY plan_id, object_id",
    ),
    (
        StoreSnapshotTableV1::GcCollectionOccurrences,
        "SELECT plan_id, object_id, stored_bytes_digest FROM store_gc_collection_occurrences ORDER BY plan_id, object_id",
    ),
    (
        StoreSnapshotTableV1::SealedExports,
        "SELECT export_id, head_id, generation_id, snapshot_id, schema_manifest_id, family_manifest_set_digest, snapshot_root_id, source_publication_clock, committed_publication_clock, payload_set_digest, export_byte_length, export_bytes_digest, export_format, backup_receipt_id, carrier_format FROM store_sealed_exports ORDER BY export_id",
    ),
    (
        StoreSnapshotTableV1::SealedExportPins,
        "SELECT export_id, pin_id FROM store_sealed_export_pins ORDER BY export_id, pin_id",
    ),
    (
        StoreSnapshotTableV1::SealedExportObjects,
        "SELECT export_id, object_id, entry_kind FROM store_sealed_export_objects ORDER BY export_id, object_id",
    ),
    (
        StoreSnapshotTableV1::RestoreCandidates,
        "SELECT candidate_id, source_export_id, source_domain_id, source_export_bytes_digest, source_schema_manifest_id, source_snapshot_root_id, destination_domain_id, candidate_generation_id, candidate_head_id, candidate_snapshot_id, verification_digest FROM store_restore_candidates ORDER BY candidate_id",
    ),
    (
        StoreSnapshotTableV1::RestoreCandidateRoots,
        "SELECT candidate_id, root_position, object_id FROM store_restore_candidate_roots ORDER BY candidate_id, root_position",
    ),
    (
        StoreSnapshotTableV1::Idempotency,
        "SELECT namespace, key_digest, meaning_digest, result_object_id, generation_id, head_id FROM store_idempotency ORDER BY namespace, key_digest",
    ),
];

impl StoreSnapshotRowV1 {
    fn from_sql_row(
        table: StoreSnapshotTableV1,
        row: &Row<'_>,
    ) -> Result<Self, StoreSnapshotRowError> {
        let columns = match table {
            StoreSnapshotTableV1::Metadata => vec![
                sql_unsigned(row, 0, "singleton")?,
                sql_unsigned(row, 1, "schema_version")?,
                sql_unsigned(row, 2, "store_role")?,
                sql_digest(row, 3, "domain_id")?,
            ],
            StoreSnapshotTableV1::PublicationClock => vec![
                sql_unsigned(row, 0, "singleton")?,
                sql_unsigned(row, 1, "publication_clock")?,
            ],
            StoreSnapshotTableV1::State => vec![
                sql_unsigned(row, 0, "singleton")?,
                sql_text(row, 1)?,
                sql_unsigned(row, 2, "state_revision")?,
            ],
            StoreSnapshotTableV1::RetentionRevision => vec![
                sql_unsigned(row, 0, "singleton")?,
                sql_unsigned(row, 1, "retention_revision")?,
            ],
            StoreSnapshotTableV1::Objects => vec![
                sql_digest(row, 0, "object_id")?,
                sql_digest(row, 1, "schema_id")?,
                sql_unsigned(row, 2, "logical_byte_length")?,
                sql_unsigned(row, 3, "stored_byte_length")?,
                sql_digest(row, 4, "stored_bytes_digest")?,
                sql_text(row, 5)?,
                sql_optional_digest(row, 6, "key_envelope_id")?,
                sql_optional_text(row, 7)?,
            ],
            StoreSnapshotTableV1::ObjectReferences => vec![
                sql_digest(row, 0, "object_id")?,
                sql_unsigned(row, 1, "reference_position")?,
                sql_digest(row, 2, "referenced_object_id")?,
            ],
            StoreSnapshotTableV1::Generations => vec![
                sql_digest(row, 0, "generation_id")?,
                sql_unsigned(row, 1, "generation_ordinal")?,
                sql_optional_digest(row, 2, "previous_generation_id")?,
                sql_digest(row, 3, "contract_root_id")?,
                sql_digest(row, 4, "writer_compatibility_manifest_id")?,
                sql_digest(row, 5, "association_schema_id")?,
                sql_digest(row, 6, "finality_edge_manifest_id")?,
                sql_digest(row, 7, "schema_read_write_set_descriptor_id")?,
                sql_digest(row, 8, "writer_protocol_epoch_id")?,
                sql_digest(row, 9, "migration_epoch_id")?,
            ],
            StoreSnapshotTableV1::GenerationRoots => vec![
                sql_digest(row, 0, "generation_id")?,
                sql_unsigned(row, 1, "root_position")?,
                sql_digest(row, 2, "object_id")?,
            ],
            StoreSnapshotTableV1::Heads => vec![
                sql_digest(row, 0, "head_id")?,
                sql_digest(row, 1, "generation_id")?,
                sql_unsigned(row, 2, "generation_ordinal")?,
                sql_unsigned(row, 3, "head_revision")?,
                sql_optional_digest(row, 4, "previous_head_id")?,
            ],
            StoreSnapshotTableV1::ActiveHead => vec![
                sql_unsigned(row, 0, "singleton")?,
                sql_digest(row, 1, "head_id")?,
                sql_unsigned(row, 2, "head_revision")?,
            ],
            StoreSnapshotTableV1::ReachabilitySnapshots => vec![
                sql_digest(row, 0, "snapshot_id")?,
                sql_digest(row, 1, "head_id")?,
                sql_unsigned(row, 2, "retention_revision")?,
            ],
            StoreSnapshotTableV1::ReachabilityRoots => vec![
                sql_digest(row, 0, "snapshot_id")?,
                sql_unsigned(row, 1, "root_position")?,
                sql_unsigned(row, 2, "root_kind")?,
                sql_digest(row, 3, "object_id")?,
            ],
            StoreSnapshotTableV1::ReachabilityObjects => vec![
                sql_digest(row, 0, "snapshot_id")?,
                sql_digest(row, 1, "object_id")?,
                sql_text(row, 2)?,
            ],
            StoreSnapshotTableV1::RetentionPins => vec![
                sql_digest(row, 0, "pin_id")?,
                sql_digest(row, 1, "basis_head_id")?,
                sql_unsigned(row, 2, "root_kind")?,
                sql_digest(row, 3, "root_object_id")?,
                sql_digest(row, 4, "reason_digest")?,
            ],
            StoreSnapshotTableV1::RetentionPinReleases => vec![
                sql_digest(row, 0, "pin_id")?,
                sql_digest(row, 1, "released_at_head_id")?,
                sql_digest(row, 2, "reason_digest")?,
            ],
            StoreSnapshotTableV1::LogicalTombstones => vec![
                sql_digest(row, 0, "tombstone_id")?,
                sql_digest(row, 1, "basis_head_id")?,
                sql_digest(row, 2, "object_id")?,
                sql_digest(row, 3, "reason_digest")?,
                sql_digest(row, 4, "invalidation_digest")?,
            ],
            StoreSnapshotTableV1::GcPlans => vec![
                sql_digest(row, 0, "plan_id")?,
                sql_digest(row, 1, "snapshot_id")?,
                sql_digest(row, 2, "head_id")?,
                sql_unsigned(row, 3, "retention_revision")?,
            ],
            StoreSnapshotTableV1::GcPlanObjects => vec![
                sql_digest(row, 0, "plan_id")?,
                sql_digest(row, 1, "object_id")?,
            ],
            StoreSnapshotTableV1::GcCollectionOccurrences => vec![
                sql_digest(row, 0, "plan_id")?,
                sql_digest(row, 1, "object_id")?,
                sql_digest(row, 2, "stored_bytes_digest")?,
            ],
            StoreSnapshotTableV1::SealedExports => vec![
                sql_digest(row, 0, "export_id")?,
                sql_digest(row, 1, "head_id")?,
                sql_digest(row, 2, "generation_id")?,
                sql_digest(row, 3, "snapshot_id")?,
                sql_digest(row, 4, "schema_manifest_id")?,
                sql_digest(row, 5, "family_manifest_set_digest")?,
                sql_digest(row, 6, "snapshot_root_id")?,
                sql_unsigned(row, 7, "source_publication_clock")?,
                sql_unsigned(row, 8, "committed_publication_clock")?,
                sql_digest(row, 9, "payload_set_digest")?,
                sql_unsigned(row, 10, "export_byte_length")?,
                sql_digest(row, 11, "export_bytes_digest")?,
                sql_text(row, 12)?,
                sql_digest(row, 13, "backup_receipt_id")?,
                sql_text(row, 14)?,
            ],
            StoreSnapshotTableV1::SealedExportPins => vec![
                sql_digest(row, 0, "export_id")?,
                sql_digest(row, 1, "pin_id")?,
            ],
            StoreSnapshotTableV1::SealedExportObjects => vec![
                sql_digest(row, 0, "export_id")?,
                sql_digest(row, 1, "object_id")?,
                sql_text(row, 2)?,
            ],
            StoreSnapshotTableV1::RestoreCandidates => vec![
                sql_digest(row, 0, "candidate_id")?,
                sql_digest(row, 1, "source_export_id")?,
                sql_digest(row, 2, "source_domain_id")?,
                sql_digest(row, 3, "source_export_bytes_digest")?,
                sql_digest(row, 4, "source_schema_manifest_id")?,
                sql_digest(row, 5, "source_snapshot_root_id")?,
                sql_digest(row, 6, "destination_domain_id")?,
                sql_digest(row, 7, "candidate_generation_id")?,
                sql_digest(row, 8, "candidate_head_id")?,
                sql_digest(row, 9, "candidate_snapshot_id")?,
                sql_digest(row, 10, "verification_digest")?,
            ],
            StoreSnapshotTableV1::RestoreCandidateRoots => vec![
                sql_digest(row, 0, "candidate_id")?,
                sql_unsigned(row, 1, "root_position")?,
                sql_digest(row, 2, "object_id")?,
            ],
            StoreSnapshotTableV1::Idempotency => vec![
                sql_text(row, 0)?,
                sql_digest(row, 1, "key_digest")?,
                sql_digest(row, 2, "meaning_digest")?,
                sql_digest(row, 3, "result_object_id")?,
                sql_digest(row, 4, "generation_id")?,
                sql_digest(row, 5, "head_id")?,
            ],
        };
        let mut value = vec![
            unsigned(SNAPSHOT_ROW_VERSION_V1),
            unsigned(table.family().tag()),
            unsigned(table.tag()),
        ];
        value.extend(columns);
        Self::from_canonical_value(CborValue::Array(value))
    }
}

fn sql_unsigned(
    row: &Row<'_>,
    index: usize,
    column: &'static str,
) -> Result<CborValue, StoreSnapshotRowError> {
    let value = row.get::<_, i64>(index)?;
    let value = u64::try_from(value)
        .map_err(|_| StoreSnapshotRowError::NegativeSqlInteger { column, value })?;
    Ok(unsigned(value))
}

fn sql_digest(
    row: &Row<'_>,
    index: usize,
    _column: &'static str,
) -> Result<CborValue, StoreSnapshotRowError> {
    Ok(CborValue::Bytes(row.get(index)?))
}
fn sql_text(row: &Row<'_>, index: usize) -> Result<CborValue, StoreSnapshotRowError> {
    Ok(CborValue::Text(row.get(index)?))
}
fn sql_optional_digest(
    row: &Row<'_>,
    index: usize,
    _column: &'static str,
) -> Result<CborValue, StoreSnapshotRowError> {
    Ok(CborValue::optional(
        row.get::<_, Option<Vec<u8>>>(index)?.map(CborValue::Bytes),
    ))
}
fn sql_optional_text(row: &Row<'_>, index: usize) -> Result<CborValue, StoreSnapshotRowError> {
    Ok(CborValue::optional(
        row.get::<_, Option<String>>(index)?.map(CborValue::Text),
    ))
}

#[derive(Debug, Error)]
pub(crate) enum StoreSnapshotRowError {
    #[error("Store snapshot contains more than 65,536 rows")]
    RowCountLimitExceeded,
    #[error("Store snapshot row has {actual} encoded bytes; maximum is 1,024")]
    RowBytesLimitExceeded { actual: usize },
    #[error("Store snapshot encoded rows exceed the 7 MiB aggregate limit")]
    AggregateRowBytesLimitExceeded,
    #[error("Store snapshot contains more than 1,024 sealed-export rows")]
    SealedExportCountLimitExceeded,
    #[error("Store snapshot sealed-export rows contain duplicate prior root {0:?}")]
    DuplicateReferencedPriorRoot([u8; DIGEST_BYTES]),
    #[error("Store snapshot references more than 1,024 prior roots")]
    ReferencedPriorRootCountLimitExceeded,
    #[error("Store snapshot row must be a canonical CBOR array")]
    RowNotArray,
    #[error("Store snapshot row has only {0} fields; header requires three")]
    ShortRow(usize),
    #[error("unsupported Store snapshot row version {0}")]
    UnsupportedRowVersion(u64),
    #[error("unknown Store snapshot family tag {0}")]
    UnknownFamily(u64),
    #[error("unknown Store snapshot table tag {0}")]
    UnknownTable(u64),
    #[error("Store snapshot family {family:?} does not own table {table:?}")]
    FamilyTableMismatch {
        family: StoreSnapshotFamilyV1,
        table: StoreSnapshotTableV1,
    },
    #[error("Store snapshot row is missing column {0}")]
    MissingColumn(&'static str),
    #[error("Store snapshot column {column} must be a canonical CBOR {expected}")]
    WrongCanonicalType {
        column: &'static str,
        expected: &'static str,
    },
    #[error("Store snapshot digest column {column} has {actual} bytes; expected 32")]
    InvalidDigestLength { column: &'static str, actual: usize },
    #[error("Store snapshot text column {0} must be ASCII")]
    NonAsciiText(&'static str),
    #[error("Store snapshot text column {column} has {actual} bytes; expected {min}..={max}")]
    InvalidTextLength {
        column: &'static str,
        min: usize,
        max: usize,
        actual: usize,
    },
    #[error("Store snapshot positive integer column {0} is zero")]
    ZeroWherePositive(&'static str),
    #[error("Store snapshot SQL integer column {column} is negative: {value}")]
    NegativeSqlInteger { column: &'static str, value: i64 },
    #[error("unknown Store role tag {0}")]
    UnknownStoreRole(u64),
    #[error("unknown retention root kind tag {0}")]
    UnknownRootKind(u64),
    #[error("unknown Store snapshot enum value for {column}")]
    UnknownTextEnum { column: &'static str },
    #[error("Store snapshot optional column {column} has invalid tag {tag}")]
    InvalidOptionalTag { column: &'static str, tag: u64 },
    #[error("Store snapshot optional column {column} has {count} extra values")]
    ExtraOptionalValues { column: &'static str, count: usize },
    #[error("Store snapshot table {table:?} row has {count} extra columns")]
    ExtraColumns {
        table: StoreSnapshotTableV1,
        count: usize,
    },
    #[error("invalid Store snapshot table {table:?} column {column}: {reason}")]
    InvalidColumn {
        table: StoreSnapshotTableV1,
        column: &'static str,
        reason: &'static str,
    },
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_limits() -> StoreSnapshotRowLimitsV1 {
        StoreSnapshotRowLimitsV1 {
            rows: 2,
            row_bytes: 4,
            aggregate_bytes: 6,
            sealed_exports: 1,
            referenced_prior_roots: 1,
        }
    }

    fn sealed_export_value(
        source_publication_clock: u64,
        committed_publication_clock: u64,
        carrier_format: &str,
    ) -> CborValue {
        let digest = || CborValue::Bytes(vec![9; DIGEST_BYTES]);
        CborValue::Array(vec![
            unsigned(SNAPSHOT_ROW_VERSION_V1),
            unsigned(StoreSnapshotFamilyV1::ExportRestoreHistory.tag()),
            unsigned(StoreSnapshotTableV1::SealedExports.tag()),
            digest(),
            digest(),
            digest(),
            digest(),
            digest(),
            digest(),
            digest(),
            unsigned(source_publication_clock),
            unsigned(committed_publication_clock),
            digest(),
            unsigned(1_u64),
            digest(),
            text("maestro-sealed-export-v2"),
            digest(),
            text(carrier_format),
        ])
    }

    #[test]
    fn snapshot_row_limits_are_frozen() {
        assert_eq!(MAX_SNAPSHOT_ROWS_V1, 65_536);
        assert_eq!(MAX_SNAPSHOT_ROW_BYTES_V1, 1024);
        assert_eq!(MAX_SNAPSHOT_ROWS_BYTES_V1, 7 * 1024 * 1024);
        assert_eq!(MAX_SEALED_EXPORT_ROWS_V1, 1024);
        assert_eq!(MAX_REFERENCED_PRIOR_ROOTS_V1, 1024);
    }

    #[test]
    fn injected_row_limits_reject_count_row_and_aggregate_overflow() {
        let mut bounds = StoreSnapshotRowBoundsV1::with_limits(small_limits());
        bounds.observe_encoded(3, None).unwrap();
        assert!(matches!(
            bounds.observe_encoded(5, None),
            Err(StoreSnapshotRowError::RowBytesLimitExceeded { actual: 5 })
        ));
        assert!(matches!(
            bounds.observe_encoded(4, None),
            Err(StoreSnapshotRowError::AggregateRowBytesLimitExceeded)
        ));
        bounds.observe_encoded(3, None).unwrap();
        assert!(matches!(
            bounds.observe_encoded(1, None),
            Err(StoreSnapshotRowError::RowCountLimitExceeded)
        ));
    }

    #[test]
    fn injected_row_limits_reject_duplicate_and_excess_prior_roots() {
        let root = [7; DIGEST_BYTES];
        let mut bounds = StoreSnapshotRowBoundsV1::with_limits(small_limits());
        bounds.observe_encoded(1, Some(root)).unwrap();
        assert!(matches!(
            bounds.observe_encoded(1, Some(root)),
            Err(StoreSnapshotRowError::SealedExportCountLimitExceeded)
        ));

        let mut duplicate_bounds =
            StoreSnapshotRowBoundsV1::with_limits(StoreSnapshotRowLimitsV1 {
                sealed_exports: 2,
                referenced_prior_roots: 2,
                ..small_limits()
            });
        duplicate_bounds.observe_encoded(1, Some(root)).unwrap();
        assert!(matches!(
            duplicate_bounds.observe_encoded(1, Some(root)),
            Err(StoreSnapshotRowError::DuplicateReferencedPriorRoot(candidate)) if candidate == root
        ));

        let mut root_bounds = StoreSnapshotRowBoundsV1::with_limits(StoreSnapshotRowLimitsV1 {
            sealed_exports: 2,
            ..small_limits()
        });
        root_bounds.observe_encoded(1, Some(root)).unwrap();
        assert!(matches!(
            root_bounds.observe_encoded(1, Some([8; DIGEST_BYTES])),
            Err(StoreSnapshotRowError::ReferencedPriorRootCountLimitExceeded)
        ));
    }

    #[test]
    fn sealed_export_receipt_fields_round_trip_and_enforce_exact_materialization() {
        let row = StoreSnapshotRowV1::from_canonical_value(sealed_export_value(
            7,
            8,
            SEALED_BACKUP_FORMAT_V1,
        ))
        .unwrap();
        let encoded = row.encode_canonical().unwrap();
        assert_eq!(StoreSnapshotRowV1::decode_canonical(&encoded).unwrap(), row);

        assert!(matches!(
            StoreSnapshotRowV1::from_canonical_value(sealed_export_value(
                7,
                9,
                SEALED_BACKUP_FORMAT_V1,
            )),
            Err(StoreSnapshotRowError::InvalidColumn {
                column: "committed_publication_clock",
                ..
            })
        ));
        assert!(matches!(
            StoreSnapshotRowV1::from_canonical_value(sealed_export_value(7, 8, "wrong")),
            Err(StoreSnapshotRowError::InvalidColumn {
                column: "carrier_format",
                ..
            })
        ));
    }
}
