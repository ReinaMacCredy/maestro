//! Task verification operation.

use anyhow::Result;

use super::{TaskVerifyApplication, TaskVerifyUnappliedReason, feature_prepare};
use crate::domain::{proof, task};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::nanos_since_epoch_string;

/// Task verification result plus Task-application status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TaskVerifyResult {
    pub(crate) verification: proof::TaskVerification,
    pub(crate) application: TaskVerifyApplication,
    pub(crate) warnings: Vec<String>,
}

impl TaskVerifyResult {
    pub(crate) fn verification(&self) -> &proof::TaskVerification {
        &self.verification
    }

    pub(crate) fn application(&self) -> &TaskVerifyApplication {
        &self.application
    }

    pub(crate) fn warnings(&self) -> &[String] {
        &self.warnings
    }
}

struct TaskVerifyAttempt {
    report: proof::VerificationReport,
    application: Result<()>,
}

/// Evaluate Proof, then ask Task to embed the verification outcome against the
/// original snapshot.
pub(crate) fn verify_task(
    paths: &MaestroPaths,
    task_id: &str,
    actor: &str,
) -> Result<TaskVerifyResult> {
    let mut handle = task::load_task_for_update(&paths.tasks_dir(), task_id)?;
    let verified_at = nanos_since_epoch_string();
    let attempt = verify_loaded_task(paths, &mut handle, actor, &verified_at)?;
    let verification = proof::TaskVerification::from_report(&attempt.report);
    let application = match attempt.application {
        Ok(()) => TaskVerifyApplication::Applied,
        Err(error) => TaskVerifyApplication::Unapplied {
            reason: TaskVerifyUnappliedReason::from_error(&error),
        },
    };
    let mut warnings = Vec::new();
    if matches!(application, TaskVerifyApplication::Applied)
        && verification.status == proof::TaskVerificationStatus::Passed
        && let Err(error) =
            feature_prepare::resolve_after_dependency_blockers(paths, &verification.task_id, actor)
    {
        warnings.push(format!(
            "after-dependency cleanup incomplete for {}: {error}",
            verification.task_id
        ));
    }

    Ok(TaskVerifyResult {
        verification,
        application,
        warnings,
    })
}

fn verify_loaded_task(
    paths: &MaestroPaths,
    handle: &mut task::TaskHandle,
    actor: &str,
    verified_at: &str,
) -> Result<TaskVerifyAttempt> {
    let task_dir = handle.task_dir().to_path_buf();
    let report = proof::evaluate_task_report(paths, handle.task(), &task_dir, verified_at)?;

    let outcome = proof::verification_outcome_for_report(&report)?;
    let application =
        task::apply_verification_outcome_to_handle(handle, outcome, actor, verified_at);

    Ok(TaskVerifyAttempt {
        report,
        application,
    })
}
