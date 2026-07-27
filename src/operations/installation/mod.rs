#![allow(
    dead_code,
    unused_imports,
    reason = "Stage 9 is an isolated candidate until its integration commit exposes this facade"
)]

//! Stage-9 installation and distribution operation seam.

mod active;
mod agent_resource_release;
mod effects;
#[cfg(test)]
mod prestore;

pub use active::{
    ActiveDistributionTransactionV1, ActiveDomainInstallationClosureV1, ActiveInstallationFacadeV1,
    ActivePublicationObjectsV1, InstallationOperationErrorV1,
};
pub(crate) use agent_resource_release::{
    ActiveAgentResourceReleaseV1, AgentResourceReleaseOperationErrorV1,
};
pub use effects::{DistributionEffectPortV1, Stage4EffectReservationBatchV1};
#[cfg(test)]
pub use prestore::{
    ProtectedLocatorCommitOutcomeV1, ProtectedLocatorCutoverPortV1, commit_prestore_cutover,
};
