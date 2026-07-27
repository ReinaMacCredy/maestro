//! Stage-9 installation and distribution operation seam.

mod active;
mod agent_resource_release;
mod effects;
#[cfg(test)]
mod prestore;

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
