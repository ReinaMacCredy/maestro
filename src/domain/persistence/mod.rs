//! Canonical Maestro vNext Store foundation.
//!
//! Persistence owns physical durability and coherent same-Store publication.
//! It does not own domain policy, authority, lifecycle, Projection, or
//! cross-Store atomicity.

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the Persistence-owned consumer current-view lease before its Stage 9 consumer"
    )
)]
pub(in crate::domain) mod consumer_snapshot;
mod export;
mod generation;
mod idempotency;
mod metadata;
mod object;
mod protected_diagnostic;
mod protected_diagnostic_stage9_seed;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the protected-locator lease before its Stage 9 and Stage 11 production consumers"
    )
)]
pub(in crate::domain) mod protected_locator_lease;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the protected-locator Stage 9 seed before the production provider integrates"
    )
)]
mod protected_locator_stage9_seed;
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
#[allow(
    unused_imports,
    reason = "Stage 5 freezes the provider seal before the Stage 9 implementation"
)]
pub(crate) use protected_diagnostic::{
    ProtectedDiagnosticCurrentViewAnchorV1, ProtectedDiagnosticCurrentViewProviderV1,
    ProtectedDiagnosticObservedCurrentViewV1, ProtectedDiagnosticProviderCurrentnessV1,
};
#[cfg(test)]
pub(crate) use protected_diagnostic::{
    ProtectedDiagnosticTestAnchorMutationV1, ProtectedDiagnosticTestCurrentViewProviderV1,
};
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

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO(persistence-stage9): Remove after Stage 9 consumes the frozen pre-candidate locator facade"
    )
)]
pub(in crate::domain) mod protected_locator_v2 {
    use crate::domain::distribution::runtime::{
        CanonicalTargetIdentityV1, DistributionTransactionV1,
    };
    use crate::domain::execution::{
        ProtectedCeremonyCarrierAnchorV1, ProtectedCeremonyEffectStoreV1,
        ProtectedCeremonyOwnerAuthorityV1,
    };

    pub(in crate::domain) use super::protected_locator_lease::{
        ProtectedLocatorFinalityDispositionV2, ProtectedLocatorLeaseErrorV2,
        ProtectedLocatorLeaseV2,
    };
    pub(in crate::domain) type Stage9ProtectedLocatorProviderSeedV2<'locator> =
        super::protected_locator_stage9_seed::Stage9ProtectedLocatorBackendSeedV2<'locator>;

    pub(in crate::domain) fn capture_pre_candidate<'locator>(
        store: &'locator ProtectedCeremonyEffectStoreV1,
        anchor: &'locator ProtectedCeremonyCarrierAnchorV1,
        owner_authority: &'locator ProtectedCeremonyOwnerAuthorityV1,
        transaction: &DistributionTransactionV1,
        target: &CanonicalTargetIdentityV1,
    ) -> Result<Stage9ProtectedLocatorProviderSeedV2<'locator>, ProtectedLocatorLeaseErrorV2> {
        super::protected_locator_stage9_seed::acquire_stage9_backend_v2(
            store,
            anchor,
            owner_authority,
            transaction,
            target,
        )
    }

    pub(in crate::domain) fn acquire_pre_candidate<'lease, 'provider>(
        provider: &'lease mut Stage9ProtectedLocatorProviderSeedV2<'provider>,
    ) -> Result<ProtectedLocatorLeaseV2<'lease>, ProtectedLocatorLeaseErrorV2> {
        super::protected_locator_stage9_seed::acquire_protected_locator_lease_v2(provider)
    }
}

#[cfg(test)]
mod protected_locator_v2_compile_tests {
    #[test]
    fn stage9_owner_facade_exposes_only_captured_pre_candidate_acquisition() {
        let _ = super::protected_locator_v2::capture_pre_candidate;
        let _ = super::protected_locator_v2::acquire_pre_candidate;
    }
}
