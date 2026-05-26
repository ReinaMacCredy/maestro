//! Task aggregate facade.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::foundation::core::safe_write::write_string_atomic;

pub(crate) mod blockers;
pub(crate) mod display;
pub(crate) mod doctor;
pub(crate) mod lifecycle;
pub(crate) mod lookup;
pub(crate) mod template;

pub use blockers::has_unresolved_blockers;
pub use display::{render_task, render_task_list};
pub use doctor::{
    check_blocker_graph, load_task_entries, load_task_records, render_report, TaskDoctorReport,
    TaskEntry,
};
pub use lifecycle::TransitionDetails;
pub use template::{
    AcceptanceFile, AppliedVerificationReceipt, Blocker, BlockerKind, BlockerRef, BlockerSource,
    TaskRecord, TaskState, VerificationBinding,
};
pub(crate) use template::{StateHistoryEntry, TaskSaveError};

/// Minimal Task projection for feature rollups.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct FeatureTaskProjection {
    pub feature_id: Option<String>,
    pub state: Option<TaskState>,
}

/// Task aggregate loaded with its Task-owned optimistic save context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TaskHandle {
    task: TaskRecord,
    task_dir: PathBuf,
    snapshot: template::TaskSnapshot,
}

impl TaskHandle {
    /// Loaded task record.
    pub(crate) fn task(&self) -> &TaskRecord {
        &self.task
    }

    /// Directory containing the task artifacts.
    pub(crate) fn task_dir(&self) -> &Path {
        &self.task_dir
    }
}

/// Typed blocker target accepted by the Task aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockerTarget {
    Task(String),
    Decision(String),
    External(String),
    Human,
}

/// Create a draft task and write its task artifacts.
pub fn create_task(
    tasks_dir: &Path,
    title: &str,
    feature: Option<String>,
    lane: Option<String>,
    risk: Option<String>,
    created_at: &str,
) -> Result<TaskRecord> {
    let id = next_task_id(tasks_dir)?;
    let mut task = TaskRecord::draft(&id, title, created_at);
    task.feature_id = feature;
    if let Some(lane) = lane {
        task.lane = Some(lane);
    }
    if let Some(risk) = risk {
        task.risk = Some(risk);
    }
    let acceptance = AcceptanceFile::new(&id, Vec::new());
    template::write_task_artifacts(tasks_dir, &task, &acceptance)?;
    Ok(task)
}

/// Load one Task record by id or id prefix.
pub fn load_task_record(tasks_dir: &Path, id: &str) -> Result<TaskRecord> {
    let (task, _, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    Ok(task)
}

/// Load minimal task projections for feature read models without full record sorting.
pub fn load_feature_task_projections(tasks_dir: &Path) -> Result<Vec<FeatureTaskProjection>> {
    let mut projections = Vec::new();
    if !tasks_dir.is_dir() {
        return Ok(projections);
    }

    for entry in fs::read_dir(tasks_dir)
        .with_context(|| format!("failed to read tasks dir {}", tasks_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", tasks_dir.display()))?;
        let Some(task_path) = lookup::task_yaml_path_for_entry(&entry)? else {
            continue;
        };

        let contents = fs::read_to_string(&task_path)
            .with_context(|| format!("failed to read {}", task_path.display()))?;
        let projection: FeatureTaskProjection = serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", task_path.display()))?;
        projections.push(projection);
    }
    Ok(projections)
}

/// Load one Task aggregate by id or id prefix with its Task-owned save context.
pub(crate) fn load_task_for_update(tasks_dir: &Path, id: &str) -> Result<TaskHandle> {
    let (task, snapshot, task_dir) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    Ok(TaskHandle {
        task,
        task_dir,
        snapshot,
    })
}

/// Lock acceptance criteria and move a task to ready.
pub fn accept_task(
    tasks_dir: &Path,
    id: &str,
    actor: &str,
    accepted_at: &str,
) -> Result<TaskRecord> {
    let (mut task, snapshot, task_dir) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    task.acceptance_locked = true;
    lifecycle::transition(
        &mut task,
        TaskState::Ready,
        actor,
        accepted_at,
        TransitionDetails::default(),
    )?;
    template::save_task_with_snapshot(&task, &snapshot)?;
    lock_acceptance(
        task_dir.join("acceptance.yaml"),
        &task.id,
        actor,
        accepted_at,
    )?;
    Ok(task)
}

/// Claim a task, auto-accepting draft tasks using the existing CLI behavior.
pub fn claim_task(tasks_dir: &Path, id: &str, actor: &str, claimed_at: &str) -> Result<TaskRecord> {
    let (mut task, snapshot, task_dir) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    if task.state == TaskState::Draft {
        lifecycle::transition(
            &mut task,
            TaskState::Exploring,
            actor,
            claimed_at,
            TransitionDetails::default(),
        )?;
        task.acceptance_locked = true;
        lifecycle::transition(
            &mut task,
            TaskState::Ready,
            actor,
            claimed_at,
            TransitionDetails::default(),
        )?;
        lock_acceptance(
            task_dir.join("acceptance.yaml"),
            &task.id,
            actor,
            claimed_at,
        )?;
    }
    lifecycle::transition(
        &mut task,
        TaskState::InProgress,
        actor,
        claimed_at,
        TransitionDetails::default(),
    )?;
    template::save_task_with_snapshot(&task, &snapshot)?;
    Ok(task)
}

/// Transition a task through the Task lifecycle and save through optimistic concurrency.
pub fn transition_task(
    tasks_dir: &Path,
    id: &str,
    to: TaskState,
    actor: &str,
    transitioned_at: &str,
    details: TransitionDetails,
) -> Result<TaskRecord> {
    let (mut task, snapshot, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    lifecycle::transition(&mut task, to, actor, transitioned_at, details)?;
    template::save_task_with_snapshot(&task, &snapshot)?;
    Ok(task)
}

/// Add a Task-owned blocker and state-history entry.
pub fn block_task(
    tasks_dir: &Path,
    id: &str,
    reason: &str,
    target: BlockerTarget,
    actor: &str,
    blocked_at: &str,
) -> Result<(TaskRecord, String)> {
    let (mut task, snapshot, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    let blocker_id = next_blocker_id(&task);
    let (kind, blocked_ref, title) = blocker_descriptor(target);
    blockers::add_blocker(
        &mut task,
        blocker_id.clone(),
        kind,
        blocked_ref,
        title,
        reason.to_string(),
        blocked_at.to_string(),
    );
    lifecycle::append_history(
        &mut task,
        actor,
        blocked_at,
        TransitionDetails {
            summary: Some(format!("blocker added: {blocker_id}")),
            ..TransitionDetails::default()
        },
    );
    template::save_task_with_snapshot(&task, &snapshot)?;
    Ok((task, blocker_id))
}

/// Resolve a Task-owned blocker and state-history entry.
pub fn unblock_task(
    tasks_dir: &Path,
    id: &str,
    blocker_id: &str,
    actor: &str,
    resolved_at: &str,
) -> Result<TaskRecord> {
    let (mut task, snapshot, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    blockers::resolve_blocker(&mut task, blocker_id, resolved_at.to_string())?;
    lifecycle::append_history(
        &mut task,
        actor,
        resolved_at,
        TransitionDetails {
            summary: Some(format!("blocker resolved: {blocker_id}")),
            ..TransitionDetails::default()
        },
    );
    template::save_task_with_snapshot(&task, &snapshot)?;
    Ok(task)
}

/// Append a non-transition Task state-history update.
pub fn update_task_history(
    tasks_dir: &Path,
    id: &str,
    actor: &str,
    updated_at: &str,
    details: TransitionDetails,
) -> Result<TaskRecord> {
    let (mut task, snapshot, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    lifecycle::append_history(&mut task, actor, updated_at, details);
    template::save_task_with_snapshot(&task, &snapshot)?;
    Ok(task)
}

/// Apply a persisted verification outcome to the Task-owned lifecycle and binding fields.
fn apply_verification_outcome(
    task: &mut TaskRecord,
    outcome: VerificationOutcome,
    actor: &str,
    applied_at: &str,
) {
    match outcome {
        VerificationOutcome::Passed(passed) => {
            task.state = TaskState::Verified;
            task.verification = passed.binding;
            task.verification.applied_report = Some(passed.receipt);
            lifecycle::append_history(
                task,
                actor,
                applied_at,
                TransitionDetails {
                    summary: Some(passed.summary),
                    ..TransitionDetails::default()
                },
            );
        }
        VerificationOutcome::Failed(failed) => {
            if task.state == TaskState::Verified {
                task.state = TaskState::NeedsVerification;
            }
            task.verification = VerificationBinding {
                applied_report: Some(failed.receipt),
                ..VerificationBinding::default()
            };
            lifecycle::append_history(
                task,
                actor,
                applied_at,
                TransitionDetails {
                    summary: Some(failed.summary),
                    open_items: failed.failures,
                    ..TransitionDetails::default()
                },
            );
        }
    }
}

/// Apply and save a verification outcome after a caller-owned pre-save step.
pub(crate) fn apply_verification_outcome_to_handle_after<F, C>(
    handle: &mut TaskHandle,
    outcome: VerificationOutcome,
    actor: &str,
    applied_at: &str,
    before_save: F,
) -> Result<()>
where
    F: FnOnce() -> Result<C>,
    C: template::SaveTaskHook,
{
    apply_verification_outcome(&mut handle.task, outcome, actor, applied_at);
    template::save_task_with_snapshot_after(&handle.task, &handle.snapshot, before_save)
}

/// Verification outcome request accepted by the Task aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum VerificationOutcome {
    Passed(VerificationPassed),
    Failed(VerificationFailed),
}

/// Verification binding data accepted by the Task aggregate after proof succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerificationPassed {
    pub(crate) binding: VerificationBinding,
    pub(crate) receipt: AppliedVerificationReceipt,
    pub(crate) summary: String,
}

/// Failed verification data accepted by the Task aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerificationFailed {
    pub(crate) receipt: AppliedVerificationReceipt,
    pub(crate) summary: String,
    pub(crate) failures: Vec<String>,
}

fn lock_acceptance(path: PathBuf, task_id: &str, actor: &str, locked_at: &str) -> Result<()> {
    let acceptance = if path.exists() {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_yaml::from_str::<AcceptanceFile>(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?
    } else {
        AcceptanceFile::new(task_id, Vec::new())
    };

    let locked = AcceptanceFile {
        locked_by: Some(actor.to_string()),
        locked_at: Some(locked_at.to_string()),
        ..acceptance
    };
    write_string_atomic(&path, &serde_yaml::to_string(&locked)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn next_task_id(tasks_dir: &Path) -> Result<String> {
    let mut max = 0_u32;
    if tasks_dir.is_dir() {
        for entry in fs::read_dir(tasks_dir)
            .with_context(|| format!("failed to read {}", tasks_dir.display()))?
        {
            let entry = entry.with_context(|| format!("failed to list {}", tasks_dir.display()))?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if let Some(num) = name
                .strip_prefix("task-")
                .and_then(|rest| rest.split('-').next())
                .and_then(|value| value.parse::<u32>().ok())
            {
                max = max.max(num);
            }
        }
    }
    Ok(format!("task-{:03}", max + 1))
}

fn next_blocker_id(task: &TaskRecord) -> String {
    let max = task
        .blockers
        .iter()
        .filter_map(|blocker| blocker.id.strip_prefix("blk-"))
        .filter_map(|id| id.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("blk-{:03}", max + 1)
}

fn blocker_descriptor(target: BlockerTarget) -> (BlockerKind, Option<BlockerRef>, String) {
    match target {
        BlockerTarget::Task(id) => (
            BlockerKind::Task,
            Some(BlockerRef {
                kind: BlockerKind::Task,
                id: id.clone(),
            }),
            format!("Blocked by {id}"),
        ),
        BlockerTarget::Decision(id) => (
            BlockerKind::Decision,
            Some(BlockerRef {
                kind: BlockerKind::Decision,
                id: id.clone(),
            }),
            format!("Blocked by {id}"),
        ),
        BlockerTarget::External(id) => (BlockerKind::External, None, format!("Blocked by {id}")),
        BlockerTarget::Human => (BlockerKind::Human, None, "Manual block".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_verification_outcome, AppliedVerificationReceipt, TaskRecord, TaskState,
        VerificationBinding, VerificationOutcome, VerificationPassed,
    };

    #[test]
    fn verification_outcome_pass_sets_binding_and_history() {
        let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        task.state = TaskState::NeedsVerification;

        apply_verification_outcome(
            &mut task,
            VerificationOutcome::Passed(VerificationPassed {
                binding: VerificationBinding {
                    verified_at: Some("t1".to_string()),
                    verified_commit: Some("abc123".to_string()),
                    verified_by_run: Some("runs/session".to_string()),
                    task_contract_hash: Some("task-hash".to_string()),
                    acceptance_hash: Some("acceptance-hash".to_string()),
                    checks_hash: Some("checks-hash".to_string()),
                    ..VerificationBinding::default()
                },
                receipt: AppliedVerificationReceipt {
                    task_snapshot_updated_at: "t0".to_string(),
                    verified_at: "t1".to_string(),
                    attempt_id: Some("attempt-1".to_string()),
                },
                summary: "verification passed: 1 claim(s), 1 proof source(s)".to_string(),
            }),
            "codex",
            "t1",
        );

        assert_eq!(task.state, TaskState::Verified);
        assert_eq!(task.verification.verified_at.as_deref(), Some("t1"));
        assert_eq!(task.verification.verified_commit.as_deref(), Some("abc123"));
        assert_eq!(
            task.verification.applied_report,
            Some(AppliedVerificationReceipt {
                task_snapshot_updated_at: "t0".to_string(),
                verified_at: "t1".to_string(),
                attempt_id: Some("attempt-1".to_string())
            })
        );
        let latest = task
            .state_history
            .last()
            .expect("invariant: verification should append history");
        assert_eq!(latest.state, TaskState::Verified);
        assert_eq!(latest.by, "codex");
        assert_eq!(
            latest.summary.as_deref(),
            Some("verification passed: 1 claim(s), 1 proof source(s)")
        );
        assert!(latest.open_items.is_empty());
    }

    #[test]
    fn verification_outcome_failure_demotes_verified_task_and_records_failures() {
        let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        task.state = TaskState::Verified;

        apply_verification_outcome(
            &mut task,
            VerificationOutcome::Failed(super::VerificationFailed {
                receipt: AppliedVerificationReceipt {
                    task_snapshot_updated_at: "t0".to_string(),
                    verified_at: "t1".to_string(),
                    attempt_id: Some("attempt-1".to_string()),
                },
                summary: "verification failed: missing evidence".to_string(),
                failures: vec!["missing evidence".to_string()],
            }),
            "codex",
            "t1",
        );

        assert_eq!(task.state, TaskState::NeedsVerification);
        assert_eq!(task.verification.verified_at, None);
        assert_eq!(task.verification.verified_commit, None);
        let latest = task
            .state_history
            .last()
            .expect("invariant: verification should append history");
        assert_eq!(latest.state, TaskState::NeedsVerification);
        assert_eq!(
            latest.summary.as_deref(),
            Some("verification failed: missing evidence")
        );
        assert_eq!(latest.open_items, vec!["missing evidence"]);
        assert_eq!(
            task.verification.applied_report,
            Some(AppliedVerificationReceipt {
                task_snapshot_updated_at: "t0".to_string(),
                verified_at: "t1".to_string(),
                attempt_id: Some("attempt-1".to_string())
            })
        );
    }
}
