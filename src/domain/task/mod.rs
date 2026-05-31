//! Task aggregate facade.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::time::parse_utc_timestamp;

pub(crate) mod archive;
pub(crate) mod blockers;
pub(crate) mod display;
pub(crate) mod doctor;
pub(crate) mod lifecycle;
pub(crate) mod lookup;
pub(crate) mod template;

pub use archive::{archive_task, unarchive_task};
pub(crate) use archive::live_task_referrer;
pub use blockers::has_unresolved_blockers;
pub use display::{render_task, render_task_list};
pub use doctor::{
    check_blocker_graph, load_task_entries, load_task_records, render_report, TaskDoctorReport,
    TaskEntry,
};
pub use lifecycle::TransitionDetails;
pub use template::{
    task_markdown, AcceptanceFile, AppliedVerificationReceipt, Blocker, BlockerKind, BlockerRef,
    BlockerSource, TaskRecord, TaskState, VerificationBinding,
};
pub(crate) use template::{StateHistoryEntry, TaskSaveError};

/// Minimal Task projection for feature rollups.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct FeatureTaskProjection {
    pub id: String,
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

/// Filters applied to a task listing by the CLI and MCP surfaces.
#[derive(Default)]
pub struct TaskFilter {
    pub ready: bool,
    pub blocked: bool,
    pub blocked_by: Option<String>,
    pub blocks: Option<String>,
    pub feature_id: Option<String>,
    pub claimed_by: Option<String>,
    /// When false (the default), terminal/done tasks are hidden from the listing
    /// so the active set stays readable; `--all` flips it on.
    pub include_terminal: bool,
}

/// Apply `filter` to `tasks` and return the matching records sorted by id.
///
/// The `blocks` target is resolved against the full input set before any
/// narrowing, and only `BlockerKind::Task` edges count toward the task graph.
pub fn filter_tasks(mut tasks: Vec<TaskRecord>, filter: &TaskFilter) -> Vec<TaskRecord> {
    let blocking_ids = filter.blocks.as_deref().map(|blocks| {
        tasks
            .iter()
            .find(|task| task.id == blocks)
            .map(|task| {
                task.blockers
                    .iter()
                    .filter(|blocker| blocker.resolved_at.is_none())
                    .filter_map(|blocker| blocker.blocked_ref.as_ref())
                    .filter(|blocked_ref| blocked_ref.kind == BlockerKind::Task)
                    .map(|blocked_ref| blocked_ref.id.clone())
                    .collect::<BTreeSet<String>>()
            })
            .unwrap_or_default()
    });

    if !filter.include_terminal {
        tasks.retain(|task| task.state.is_live());
    }
    if filter.ready {
        tasks.retain(|task| task.state == TaskState::Ready && !has_unresolved_blockers(task));
    }
    if filter.blocked {
        tasks.retain(has_unresolved_blockers);
    }
    if let Some(feature_id) = filter.feature_id.as_deref() {
        tasks.retain(|task| task.feature_id.as_deref() == Some(feature_id));
    }
    if let Some(claimed_by) = filter.claimed_by.as_deref() {
        tasks.retain(|task| task.claimed_by.as_deref() == Some(claimed_by));
    }
    if let Some(blocked_by) = filter.blocked_by.as_deref() {
        tasks.retain(|task| {
            task.blockers.iter().any(|blocker| {
                blocker.resolved_at.is_none()
                    && blocker
                        .blocked_ref
                        .as_ref()
                        .map(|blocked_ref| blocked_ref.id.as_str() == blocked_by)
                        .unwrap_or(false)
            })
        });
    }
    if let Some(blocking_ids) = blocking_ids.as_ref() {
        tasks.retain(|task| blocking_ids.contains(&task.id));
    }

    tasks.sort_by(|left, right| left.id.cmp(&right.id));
    tasks
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

/// Return per-task verification durations for loaded task entries.
pub fn task_verification_durations(entries: &[TaskEntry]) -> BTreeMap<String, u64> {
    let mut durations = BTreeMap::new();
    for entry in entries {
        if let Some(seconds) =
            verification_duration_seconds(&entry.task.created_at, &entry.task.verification)
        {
            durations.insert(entry.task.id.clone(), seconds);
        }
    }
    durations
}

/// Return verification duration in seconds for one task, when timestamps are complete.
pub fn verification_duration_seconds(
    created_at: &str,
    verification: &VerificationBinding,
) -> Option<u64> {
    let start = parse_timestamp_seconds(created_at)?;
    let end = parse_timestamp_seconds(verification.verified_at.as_deref()?)?;
    end.checked_sub(start)
}

fn parse_timestamp_seconds(value: &str) -> Option<u64> {
    if value.chars().all(|character| character.is_ascii_digit()) {
        return parse_numeric_timestamp_seconds(value);
    }
    let parsed = parse_utc_timestamp(value)?;
    if parsed.nanos_since_epoch < 0 {
        return None;
    }
    Some((parsed.nanos_since_epoch / 1_000_000_000) as u64)
}

fn parse_numeric_timestamp_seconds(value: &str) -> Option<u64> {
    let timestamp = value.parse::<u64>().ok()?;
    match value.len() {
        0..=10 => Some(timestamp),
        11..=13 => Some(timestamp / 1_000),
        14..=16 => Some(timestamp / 1_000_000),
        _ => Some(timestamp / 1_000_000_000),
    }
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
    ensure_standalone_has_checks(&task, &task_dir)?;
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
        ensure_standalone_has_checks(&task, &task_dir)?;
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
    if task.state == TaskState::Exploring {
        bail!(
            "task {} is exploring; run `maestro task accept {}` to make it ready before claiming",
            task.id,
            task.id
        );
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

/// Author a task's execution `checks` (C1), replacing the current list.
///
/// Refuses once acceptance is locked: the contract freezes at `accept`, so
/// re-authoring afterwards would silently move goalposts under a verified
/// binding. Replace-per-field semantics make a repeated call idempotent.
pub fn set_checks(tasks_dir: &Path, id: &str, checks: Vec<String>) -> Result<TaskRecord> {
    let handle = load_task_for_update(tasks_dir, id)?;
    if handle.task().acceptance_locked {
        bail!(
            "task {} acceptance is locked; checks cannot be changed after accept",
            handle.task().id
        );
    }
    let path = handle.task_dir().join("acceptance.yaml");
    let mut acceptance = read_acceptance_or_new(&path, &handle.task().id)?;
    acceptance.checks = checks;
    write_string_atomic(&path, &serde_yaml::to_string(&acceptance)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(handle.task().clone())
}

/// Attach, move, or detach a task's `feature_id` (Theme II Q-II-2/3).
///
/// Editing the link is working-state, not a contract edit, so it is allowed
/// only while the task is non-terminal (`feature_link_is_settled`); a settled
/// task keeps its link as history. Every change is audited in `state_history`.
/// A no-op (link already equals the target) returns without writing.
pub fn set_feature(
    tasks_dir: &Path,
    id: &str,
    feature: Option<String>,
    actor: &str,
    at: &str,
) -> Result<TaskRecord> {
    let (mut task, snapshot, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    if feature_link_is_settled(&task.state) {
        bail!(
            "task {} is {}; its feature link is settled history and cannot change",
            task.id,
            task.state.as_str()
        );
    }
    if task.feature_id == feature {
        return Ok(task);
    }
    let summary = match (&task.feature_id, &feature) {
        (Some(previous), Some(next)) => format!("feature link moved: {previous} -> {next}"),
        (None, Some(next)) => format!("feature link set: {next}"),
        (Some(previous), None) => format!("feature link cleared (was {previous})"),
        (None, None) => unreachable!("equal links are returned as a no-op above"),
    };
    task.feature_id = feature;
    lifecycle::append_history(
        &mut task,
        actor,
        at,
        TransitionDetails {
            summary: Some(summary),
            ..TransitionDetails::default()
        },
    );
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

/// Replace a task with another existing task, recording the superseded-by ref.
///
/// The replacement target is loaded first so `supersede --by` can never record
/// a dangling reference; the target's canonical id is the one stored.
pub fn supersede_task(
    tasks_dir: &Path,
    id: &str,
    by: &str,
    reason: &str,
    actor: &str,
    superseded_at: &str,
) -> Result<TaskRecord> {
    let (replacement, _, _) = lookup::load_task_with_snapshot(tasks_dir, by)
        .with_context(|| format!("supersede target `{by}` was not found"))?;
    transition_task(
        tasks_dir,
        id,
        TaskState::Superseded,
        actor,
        superseded_at,
        TransitionDetails {
            to: Some(replacement.id),
            summary: Some(reason.to_string()),
            ..TransitionDetails::default()
        },
    )
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
    let acceptance = read_acceptance_or_new(&path, task_id)?;
    let locked = AcceptanceFile {
        locked_by: Some(actor.to_string()),
        locked_at: Some(locked_at.to_string()),
        ..acceptance
    };
    write_string_atomic(&path, &serde_yaml::to_string(&locked)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

/// Read a task's `acceptance.yaml`, falling back to a fresh unlocked file when
/// it is absent (a freshly created task always has one, so this only guards a
/// hand-deleted artifact).
fn read_acceptance_or_new(path: &Path, task_id: &str) -> Result<AcceptanceFile> {
    if path.exists() {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_yaml::from_str::<AcceptanceFile>(&content)
            .with_context(|| format!("failed to parse {}", path.display()))
    } else {
        Ok(AcceptanceFile::new(task_id, Vec::new()))
    }
}

/// C4 lock-path gate: a standalone task (no `feature_id`) carries no inherited
/// product contract, so its `checks` are its only contract and must be
/// non-empty before it can be accepted or claim-auto-locked. Featured tasks
/// inherit the feature's frozen contract and are exempt.
fn ensure_standalone_has_checks(task: &TaskRecord, task_dir: &Path) -> Result<()> {
    if task.feature_id.is_some() {
        return Ok(());
    }
    let acceptance = read_acceptance_or_new(&task_dir.join("acceptance.yaml"), &task.id)?;
    if acceptance.checks.is_empty() {
        bail!(
            "standalone task {} has no checks; add at least one with `maestro task set {} --check \"...\"` before it can be accepted",
            task.id,
            task.id
        );
    }
    Ok(())
}

/// A task whose feature link is settled history and may no longer be re-pointed
/// (Theme II Q-II-3): terminal tasks plus `verified` keep their link as a record.
fn feature_link_is_settled(state: &TaskState) -> bool {
    matches!(
        state,
        TaskState::Verified | TaskState::Rejected | TaskState::Abandoned | TaskState::Superseded
    )
}

fn next_task_id(tasks_dir: &Path) -> Result<String> {
    // L6a: an archived `task-NNN` still owns its id, so allocate above the max
    // of the union of live + archived tasks — never reissue a freed id.
    let mut max = max_task_number(tasks_dir)?;
    if let Some(archive_tasks_dir) = archive_tasks_sibling(tasks_dir) {
        max = max.max(max_task_number(&archive_tasks_dir)?);
    }
    Ok(format!("task-{:03}", max + 1))
}

/// Highest `task-NNN` number among the directory's entries (0 if dir absent).
fn max_task_number(tasks_dir: &Path) -> Result<u32> {
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
    Ok(max)
}

/// `.maestro/tasks` → `.maestro/archive/tasks` (the archive sibling tree, §5.3).
/// Derived from `tasks_dir` to keep `create_task`'s signature stable (D-P4-5).
fn archive_tasks_sibling(tasks_dir: &Path) -> Option<PathBuf> {
    tasks_dir
        .parent()
        .map(|maestro_dir| maestro_dir.join("archive").join("tasks"))
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
