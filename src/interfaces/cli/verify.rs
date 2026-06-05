//! Verification command helpers for task proof flows.

use anyhow::{Result, bail};

use crate::domain::proof;
use crate::domain::{feature, task};
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
    render_applied_verification(paths, result.verification())?;
    for warning in result.warnings() {
        eprintln!("warning: {warning}");
        eprintln!("follow-up: run maestro task list --blocked");
    }
    Ok(())
}

fn render_applied_verification(
    paths: &MaestroPaths,
    verification: &proof::TaskVerification,
) -> Result<()> {
    match verification.status {
        proof::TaskVerificationStatus::Passed => {
            println!(
                "verification passed for {} ({} claim(s), {} proof source(s))",
                verification.task_id, verification.claim_count, verification.proof_source_count
            );
            render_verified_handoff(paths, &verification.task_id)
        }
        proof::TaskVerificationStatus::Failed => {
            for failure in &verification.failures {
                eprintln!("verification failure: {failure}");
            }
            bail!("verification failed for {}", verification.task_id)
        }
    }
}

pub(super) fn render_verified_handoff(paths: &MaestroPaths, task_id: &str) -> Result<()> {
    let task = task::load_task_record(&paths.tasks_dir(), task_id)?;
    println!("task verified: {}", task.id);
    if let Some(feature_id) = task.feature_id.as_deref() {
        println!("feature: {feature_id}");
        let features = feature::list(paths)?;
        if let Some(view) = features.iter().find(|view| view.id == feature_id) {
            if view.status == feature::FeatureStatus::InProgress
                && view.counts.total > 0
                && view.counts.total == view.counts.verified
            {
                println!("feature ready:");
                println!(
                    "  {feature_id} tasks: {}/{} verified",
                    view.counts.verified, view.counts.total
                );
                println!("template: maestro feature ship {feature_id} --outcome \"<outcome>\"");
                println!("required input:");
                println!("- outcome: shipping outcome text");
            } else if next_ready_task_for_feature(paths, feature_id)?.is_some() {
                println!("feature progress:");
                println!(
                    "  {feature_id} tasks: {}/{} verified",
                    view.counts.verified, view.counts.total
                );
                println!("next: maestro task claim --next");
            } else {
                println!("feature progress:");
                println!(
                    "  {feature_id} tasks: {}/{} verified",
                    view.counts.verified, view.counts.total
                );
                println!("next: maestro feature show {feature_id}");
            }
        } else {
            println!("next: maestro status");
        }
    } else {
        println!("next: maestro status");
        println!("inspect: maestro task show {}", task.id);
    }
    Ok(())
}

fn next_ready_task_for_feature(paths: &MaestroPaths, feature_id: &str) -> Result<Option<String>> {
    Ok(task::load_task_records(&paths.tasks_dir())?
        .into_iter()
        .find(|task| {
            task.feature_id.as_deref() == Some(feature_id)
                && task.state == task::TaskState::Ready
                && !task::has_unresolved_blockers(task)
        })
        .map(|task| task.id))
}
