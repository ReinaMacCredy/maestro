#![allow(
    dead_code,
    unused_imports,
    reason = "Stage 9 is an isolated candidate until its integration commit exposes this facade"
)]

//! Distribution and domain-local publication implementation seam.
//!
//! This module owns the Stage-9 transaction facts. It deliberately does not
//! originate Authority receipts, Effect intents, Resource identities, or
//! Migration associations; those arrive through their frozen owner seams.

mod catalog;
mod custody;
mod model;
mod owner_facts;
mod records;
mod transaction;

pub use catalog::{
    CatalogRotationV1, CleanupDebtV1, OrdinarySnapshotCatalogStateV1, OrdinarySnapshotStateV1,
    ProtectedSnapshotHoldV1, SnapshotCatalogErrorV1,
};
pub use custody::{
    CanonicalTargetIdentityV1, CustodyAssessmentV1, CustodyBasisV1, CustodyErrorV1,
    ManagedBlockBoundaryV1, ManagedTargetCustodyClassV1, TargetCustodyClassV1,
    TargetIdentityPartsV1, UnmanagedReasonV1,
};
pub use model::{
    DistributionDomainKindV1, DistributionDomainRefV1, DistributionModelErrorV1,
    DistributionRuntimeObjectKindV1, DistributionScopedObjectRefV1,
};
pub(crate) use owner_facts::CutoverPlanOwnerFactsV1;
pub use records::{
    DistributionCommitRecordV1, DistributionReceiptV1, DistributionRecordErrorV1,
    DistributionSnapshotTargetV1, DistributionSnapshotV1, InstalledResourceClaimSetV1,
    InstalledResourceClaimV1, OrdinarySnapshotCatalogV1, ReleaseMaterializationClosureV1,
    ReleaseMaterializationProofV1,
};
pub(crate) use transaction::AuthorizedDistributionPlanV1;
pub use transaction::{
    CapturedTargetPreimageV1, DistributionActionV1, DistributionMutationKindV1,
    DistributionPhaseAuthorizationV1, DistributionPlanTargetV1, DistributionPlanV1,
    DistributionRecoveryDirectiveV1, DistributionTransactionErrorV1,
    DistributionTransactionPhaseV1, DistributionTransactionV1, EffectCrossingDispositionV1,
    EffectCrossingObservationV1, TargetEffectKindV1, TargetPlanObservationV1,
    VerificationDispositionV1,
};

#[cfg(test)]
mod candidate_tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vnext_stage9_distribution.rs"
    ));
    stage9_distribution_candidate_tests!();
}
