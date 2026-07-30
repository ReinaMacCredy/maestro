//! Operations module root for multi-domain workflows.
//!
//! Concrete operation modules own orchestration that crosses domain aggregates,
//! while legacy operation-like roots stay re-exported during the migration.

pub mod card_migrate;
pub mod container_migrate;
pub mod feature_close;
pub mod feature_prepare;
pub mod harness;
pub mod init;
pub mod memory;
pub mod migrate;
pub mod sync;
pub mod update;

mod task_verify;

use std::fmt;

use anyhow::Result;

use crate::foundation::core::paths::MaestroPaths;

/// Result of applying a written Proof report back to Task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TaskVerifyApplication {
    Applied,
    Unapplied { reason: TaskVerifyUnappliedReason },
}

/// Typed reason a written Proof report could not be applied to Task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TaskVerifyUnappliedReason {
    Other(String),
}

impl TaskVerifyUnappliedReason {
    fn from_error(error: &anyhow::Error) -> Self {
        Self::Other(error.to_string())
    }
}

impl fmt::Display for TaskVerifyUnappliedReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskVerifyUnappliedReason::Other(reason) => formatter.write_str(reason),
        }
    }
}

/// Coordinate Task snapshot loading, Proof outcome evaluation, and Task outcome
/// application.
pub(crate) fn verify_task(
    paths: &MaestroPaths,
    task_id: &str,
    actor: &str,
) -> Result<task_verify::TaskVerifyResult> {
    task_verify::verify_task(paths, task_id, actor)
}
pub(crate) mod action;
pub(crate) mod adapters;
pub(crate) mod installation;
pub(crate) mod migration;
pub(crate) mod observation;
pub(crate) mod orchestration;
mod repository;

#[allow(
    unused_imports,
    reason = "crate-owned governed Operation entrypoints consume the concrete cutover facade"
)]
pub(crate) use repository::{
    CutoverGovernedOperationAssemblyV1, CutoverGovernedOperationPortV1,
    RepositoryBootstrapBackupPortV1, RepositoryBootstrapCeremonyV1,
    RepositoryBootstrapDescriptorPortV1,
};
