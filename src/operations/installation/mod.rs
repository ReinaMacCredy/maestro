//! Stage-9 installation and distribution operation seam.

mod active;
#[allow(
    dead_code,
    reason = "Stage 12 owner-fact methods remain frozen until the opaque coordinator is activated"
)]
mod agent_resource_release;
mod effects;
#[cfg(test)]
mod prestore;

#[allow(
    unused_imports,
    reason = "MainIntegration exposes the offline Stage 11 workflow without activating it"
)]
pub(crate) use crate::operations::migration::{
    Stage11PhysicalClosureV4, execute_offline_live_set_v4,
};
pub use active::{
    ActiveDistributionTransactionV1, ActiveInstallationFacadeV1, ActivePublicationObjectsV1,
    InstallationOperationErrorV1,
};
pub(crate) use agent_resource_release::{
    AgentResourceReleaseCeremonyV1, AgentResourceReleaseEffectAdapterV1,
};
pub use effects::{DistributionEffectPortV1, Stage4EffectReservationBatchV1};
#[cfg(test)]
#[allow(
    unused_imports,
    reason = "the frozen test-only PreStore adapter remains available to integration probes"
)]
pub use prestore::{
    ProtectedLocatorCommitOutcomeV1, ProtectedLocatorCutoverPortV1, commit_prestore_cutover,
};

#[allow(
    dead_code,
    reason = "MainIntegration freezes the sole opaque Stage 12 pruning coordinator without activating it"
)]
pub(crate) fn coordinate_stage12_product_pruning(
    coordinator: crate::domain::installation::Stage12ProductPruningCoordinatorV3<'_, '_>,
) -> Result<(), crate::domain::installation::AgentResourceCutoverErrorV1> {
    crate::domain::installation::coordinate_authority_admitted_stage12_pruning(coordinator)
}
