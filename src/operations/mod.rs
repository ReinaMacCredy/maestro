//! Operations module root for multi-domain workflows.
//!
//! Concrete operation modules own orchestration that crosses domain aggregates,
//! while legacy operation-like roots stay re-exported during the migration.

pub mod migrate;

mod task_verify;

use std::fmt;

use anyhow::Result;

use crate::domain::{proof, task};
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
    TaskSave(task::TaskSaveError),
    Other(String),
}

impl TaskVerifyUnappliedReason {
    fn from_error(error: &anyhow::Error) -> Self {
        match error.downcast_ref::<task::TaskSaveError>() {
            Some(error) => Self::TaskSave(error.clone()),
            None => Self::Other(error.to_string()),
        }
    }
}

impl fmt::Display for TaskVerifyUnappliedReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskVerifyUnappliedReason::TaskSave(error) => write!(formatter, "{error}"),
            TaskVerifyUnappliedReason::Other(reason) => formatter.write_str(reason),
        }
    }
}

/// Coordinate Task snapshot loading, Proof report writing, and Task outcome
/// application.
pub(crate) fn verify_task(
    paths: &MaestroPaths,
    task_id: &str,
    actor: &str,
) -> Result<task_verify::TaskVerifyResult> {
    task_verify::verify_task(paths, task_id, actor)
}

pub(crate) fn verify_task_report(
    paths: &MaestroPaths,
    task_id: &str,
    actor: &str,
) -> Result<proof::VerificationReport> {
    task_verify::verify_task_report(paths, task_id, actor)
}

pub use crate::improver;
pub use crate::metrics;
pub use crate::update;
