//! Operations module root for multi-domain workflows.
//!
//! Concrete operation modules own orchestration that crosses domain aggregates,
//! while legacy operation-like roots stay re-exported during the migration.

pub mod card_migrate;
pub mod container_migrate;
pub mod feature_prepare;
pub mod feature_close;
pub mod harness;
pub mod init;
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
