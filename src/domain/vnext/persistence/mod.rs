//! Canonical Maestro vNext Store foundation.
//!
//! Persistence owns physical durability and coherent same-Store publication.
//! It does not own domain policy, authority, lifecycle, Projection, or
//! cross-Store atomicity.

mod export;
mod generation;
mod idempotency;
mod metadata;
mod object;
mod retention;
mod snapshot;
mod snapshot_blocks;
mod snapshot_export;
mod snapshot_restore;
mod snapshot_rows;
mod store;
mod types;

#[cfg(test)]
mod tests;

const STORE_OBJECT_STORAGE_CODEC_V1: &str = "canonical-cbor-v1";
const SEALED_EXPORT_FORMAT_V2: &str = "maestro-sealed-export-v2";

pub use export::{
    BACKUP_RECEIPT_FORMAT_V1, BackupReceiptV1, ExportError, MAX_SEALED_BACKUP_BYTES_V1,
    RestoreCandidateV1, SEALED_BACKUP_FORMAT_V1, SealedBackupV1, SealedExportEntryV1,
    SealedExportLineageV1, SealedExportV1, TombstonedObjectV1,
};
pub use generation::{GenerationError, StoreCompatibilityV1, StoreGenerationV1, StoreHeadV1};
pub use idempotency::{
    AtomicGenerationPublicationV1, AtomicPublicationError, StoreIdempotencyProbeV1,
    StoreIdempotencyV1, StorePublicationOutcomeV1,
};
pub use object::{StoreObjectError, StoreObjectV1};
pub use retention::{
    CollectionPlanV1, LogicalTombstoneV1, ReachabilitySnapshotV1, RetentionError, RetentionPinV1,
    RetentionRootKindV1, RetentionRootV1,
};
pub use snapshot::{SnapshotError, StoreSnapshotRootV1};
pub(crate) use store::{
    ControlledCopyErasurePlanV1, PreparedPublicationError, StorePublicationAllocationV1,
    StorePublicationViewV1, VerifiedCollectionAbsenceV1, VerifiedControlledCopyAbsenceV1,
};
pub use store::{
    InstallationActivationIntentV1, RepositoryActivationIntentV1, StoreError, StoreStateV1, StoreV1,
};
pub use types::{StoreDomainError, StoreDomainV1, StoreRoleV1};
