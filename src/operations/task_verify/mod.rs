//! Task verification operation.

use anyhow::Result;

use super::{TaskVerifyApplication, TaskVerifyUnappliedReason};
use crate::domain::{proof, task};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::nanos_since_epoch_string;

/// Task verification result plus Task-application status.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TaskVerifyResult {
    pub(crate) verification: proof::TaskVerification,
    pub(crate) application: TaskVerifyApplication,
}

impl TaskVerifyResult {
    pub(crate) fn verification(&self) -> &proof::TaskVerification {
        &self.verification
    }

    pub(crate) fn application(&self) -> &TaskVerifyApplication {
        &self.application
    }
}

struct TaskVerifyAttempt {
    report: proof::VerificationReport,
    application: Result<()>,
}

/// Evaluate Proof, persist a receipt-keyed attempt, then ask Task to promote
/// the canonical report and apply the outcome against the original snapshot.
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

    Ok(TaskVerifyResult {
        verification,
        application,
    })
}

fn verify_loaded_task(
    paths: &MaestroPaths,
    handle: &mut task::TaskHandle,
    actor: &str,
    verified_at: &str,
) -> Result<TaskVerifyAttempt> {
    let task_dir = handle.task_dir().to_path_buf();
    let report = proof::evaluate_and_write_task_report_attempt(
        paths,
        handle.task(),
        &task_dir,
        verified_at,
    )?;

    let outcome = proof::verification_outcome_for_report(&report)?;
    let application = task::apply_verification_outcome_to_handle_after(
        handle,
        outcome,
        actor,
        verified_at,
        || proof::replace_task_report_preserving_previous(&task_dir, &report),
    );

    Ok(TaskVerifyAttempt {
        report,
        application,
    })
}
