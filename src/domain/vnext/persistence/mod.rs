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
pub(in crate::domain::vnext) mod consumer_snapshot;
mod export;
mod generation;
mod idempotency;
mod metadata;
mod object;
mod protected_diagnostic;
#[cfg(test)]
mod protected_diagnostic_stage9_seed;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Stage 5 freezes the protected-locator lease before its Stage 9 and Stage 11 production consumers"
    )
)]
pub(in crate::domain::vnext) mod protected_locator_lease;
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
pub(in crate::domain::vnext) mod protected_locator_v2 {
    use std::marker::PhantomData;
    use std::rc::Rc;

    pub(in crate::domain::vnext) use super::protected_locator_lease::{
        ProtectedLocatorFinalityDispositionV2, ProtectedLocatorLeaseErrorV2,
        ProtectedLocatorLeaseV2,
    };
    pub(in crate::domain::vnext) type Stage9ProtectedLocatorProviderSeedV2 =
        super::protected_locator_stage9_seed::Stage9ProtectedLocatorBackendSeedV2;

    pub(in crate::domain::vnext) trait Stage9ProtectedLocatorProviderV2:
        super::protected_locator_lease::ProtectedLocatorBackendV2
    {
    }

    impl<T> Stage9ProtectedLocatorProviderV2 for T where
        T: super::protected_locator_lease::ProtectedLocatorBackendV2 + ?Sized
    {
    }

    pub(in crate::domain::vnext) struct Stage9ProtectedLocatorProviderBindingV2<'locator> {
        inner: &'locator mut dyn super::protected_locator_lease::ProtectedLocatorBackendV2,
        _not_send_or_sync: PhantomData<Rc<()>>,
    }

    pub(in crate::domain::vnext) fn bind_stage9_owner_provider<P>(
        provider: &mut P,
    ) -> Stage9ProtectedLocatorProviderBindingV2<'_>
    where
        P: Stage9ProtectedLocatorProviderV2,
    {
        Stage9ProtectedLocatorProviderBindingV2 {
            inner: provider,
            _not_send_or_sync: PhantomData,
        }
    }

    pub(in crate::domain::vnext) fn acquire_pre_candidate(
        binding: Stage9ProtectedLocatorProviderBindingV2<'_>,
    ) -> Result<ProtectedLocatorLeaseV2<'_>, ProtectedLocatorLeaseErrorV2> {
        super::protected_locator_stage9_seed::acquire_protected_locator_lease_v2(binding.inner)
    }
}

#[cfg(test)]
mod protected_locator_v2_compile_tests {
    use super::protected_locator_v2::{
        Stage9ProtectedLocatorProviderSeedV2, acquire_pre_candidate, bind_stage9_owner_provider,
    };

    #[test]
    fn stage9_seed_reaches_the_v2_facade_without_a_zero_input_success_path() {
        let mut seed = Stage9ProtectedLocatorProviderSeedV2::test_unavailable();
        let binding = bind_stage9_owner_provider(&mut seed);
        assert!(acquire_pre_candidate(binding).is_err());
    }
}
