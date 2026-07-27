//! Stage-11 offline Migration runtime.
//!
//! Migration owns source inventory, classification, identity association,
//! sealed quarantine, inactive import evidence, rollback assessment, and
//! legacy-consumer closure. It never owns Distribution effects, Store
//! currentness, Installation custody, or activation.

#[allow(
    dead_code,
    reason = "the verified association workflow awaits its Stage-9 owner call"
)]
mod association;
#[allow(
    dead_code,
    reason = "classification materialization awaits its Stage-4 owner publication call"
)]
mod classification;
#[allow(
    dead_code,
    reason = "consumer closure remains dormant pending Stage-9/10 owner receipts"
)]
mod consumer;
#[allow(
    dead_code,
    reason = "migration identity is reachable only through dormant Stage-11 modules"
)]
mod identity;
#[allow(
    dead_code,
    reason = "inactive import request binding is dormant pending consumer closure"
)]
mod import;
#[allow(
    dead_code,
    reason = "inventory orchestration awaits Stage-9/10 owner integration"
)]
mod inventory;
#[allow(
    dead_code,
    reason = "quarantine domain binding is dormant pending inventory closure"
)]
mod quarantine;
#[allow(
    dead_code,
    reason = "rollback assessment is dormant pending the typed Stage-9/10 host receipt"
)]
mod rollback;

#[allow(
    unused_imports,
    reason = "the move-only H3 member binding awaits the Stage-4/9 finality transaction"
)]
pub(in crate::domain::vnext) use association::H3NativeCancelledMigrationMemberV1;
#[allow(
    unused_imports,
    reason = "the association-meaning facade is reserved for ordered Stage-9 finality integration"
)]
pub use association::{MigrationAssociationErrorV1, MigrationAssociationMeaningV1};
#[allow(
    unused_imports,
    reason = "the association facade awaits Stage-9 owner integration"
)]
pub use association::{MigrationAssociationFinalityV1, MigrationAssociationV1};
#[cfg(test)]
pub use association::{Stage9CutoverAssociationAdapterV1, TestOnlyStage9CutoverFinalityV1};
#[allow(
    unused_imports,
    reason = "the cancellation-state facade is reserved for the typed Stage-4 H3 carrier"
)]
pub use classification::NativeCancellationStateV1;
#[allow(
    unused_imports,
    reason = "the classification facade is reserved for ordered Stage-4 H3 integration"
)]
pub use classification::{
    CancellationClassificationV1, ClassificationErrorV1, ClassificationSetV1,
    DeterministicIdentityMapV1, IdentityMapEntryV1, IdentityMappingBasisV1, MigrationDispositionV1,
    NativeCancellationCausalJoinV1, SourceClassificationV1,
};
#[cfg(test)]
pub use consumer::Stage9Stage10ConsumerCensusAdapterV1;
#[allow(
    unused_imports,
    reason = "authoritative census facades are reserved for Stage-9/10 owner receipts"
)]
pub use consumer::{AuthoritativeConsumerCensusV1, ConsumerCensusResolutionV1};
#[allow(
    unused_imports,
    reason = "the consumer facade is reserved for typed Stage-9/10 census receipts"
)]
pub use consumer::{
    ClientAdmissionV1, ClientRefusalReasonV1, ConsumerAccessV1, ConsumerCensusEntryV1,
    ConsumerClosureErrorV1, ConsumerClosureV1, ConsumerGateStageV1, ConsumerGenerationV1,
    ConsumerRecordV1, ConsumerSubjectV1, MigrationProtocolClosureV1, PrunePrerequisitesV1,
};
pub use identity::{MigrationDigestV1, MigrationIdentityErrorV1};
pub use import::{
    InactiveImportErrorV1, InactiveStoreImportReceiptV1, InactiveStoreImportRequestV1,
};
#[allow(
    unused_imports,
    reason = "the inventory facade is reserved for the foundation-owned census port"
)]
pub use inventory::{
    ByteTotalInventoryV1, DeclaredRootV1, InventoryDomainV1, InventoryErrorV1, InventoryNodeKindV1,
    InventoryPayloadV1, InventoryRowV1, NormalizedLocatorV1,
};
#[allow(
    unused_imports,
    reason = "the quarantine facade is reserved for ordered migration integration"
)]
pub use quarantine::{QuarantineEntryV1, QuarantineErrorV1, SealedQuarantineManifestV1};
#[cfg(test)]
pub use rollback::Stage9Stage10CutoverHostAdapterV1;
#[allow(
    unused_imports,
    reason = "the rollback facade is reserved for typed Stage-9/10 host receipts"
)]
pub use rollback::{
    CutoverAcceptanceV1, EffectCrossingV1, RollbackAssessmentErrorV1, RollbackAssessmentV1,
    RollbackDispositionV1,
};
