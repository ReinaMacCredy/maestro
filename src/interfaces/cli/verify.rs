//! Verification command helpers for task proof flows.

use anyhow::{Result, bail};

use crate::domain::proof;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::task_id::resolve_optional_task_id;
use crate::operations::{self, TaskVerifyApplication};

/// Execute root `maestro verify`.
pub fn run(id: Option<String>) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    ensure_dir(paths.tasks_dir())?;
    let actor = super::actor();
    let id = resolve_optional_task_id(
        &paths,
        id,
        "task id is required or set MAESTRO_CURRENT_TASK",
    )?;
    run_for_task(&paths, &id, &actor)
}

pub(super) fn run_for_task(paths: &MaestroPaths, id: &str, actor: &str) -> Result<()> {
    let result = operations::verify_task(paths, id, actor)?;
    match result.application() {
        TaskVerifyApplication::Applied => {}
        TaskVerifyApplication::Unapplied { reason } => {
            for failure in &result.verification().failures {
                eprintln!("verification failure: {failure}");
            }
            bail!(
                "verification report was written but task outcome was not applied for {}: {}",
                result.verification().task_id,
                reason
            );
        }
    }
    render_applied_verification(result.verification())
}

fn render_applied_verification(verification: &proof::TaskVerification) -> Result<()> {
    match verification.status {
        proof::TaskVerificationStatus::Passed => {
            println!(
                "verification passed for {} ({} claim(s), {} proof source(s))",
                verification.task_id, verification.claim_count, verification.proof_source_count
            );
            Ok(())
        }
        proof::TaskVerificationStatus::Failed => {
            for failure in &verification.failures {
                eprintln!("verification failure: {failure}");
            }
            bail!("verification failed for {}", verification.task_id)
        }
    }
}
