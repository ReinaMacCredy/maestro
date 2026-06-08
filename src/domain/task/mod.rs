//! Task aggregate facade.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card::store as card_store;
use crate::domain::card::{StoreMode, store_mode};
use crate::foundation::core::fs::{
    ALLOC_MARKER_PREFIX, DirReservation, append_text_file, child_dirs, ensure_dir,
    try_reserve_marker_dir,
};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::time::{parse_utc_timestamp, utc_now_timestamp};

pub(crate) mod archive;
pub(crate) mod blockers;
pub(crate) mod cards;
pub(crate) mod display;
pub(crate) mod doctor;
pub(crate) mod lifecycle;
pub(crate) mod lookup;
pub(crate) mod template;

pub use archive::{archive_task, unarchive_task};
pub use blockers::has_unresolved_blockers;
pub use display::{render_task, render_task_list, render_task_list_with_missing_checks};
pub use doctor::{
    TaskDoctorReport, TaskEntry, check_blocker_graph, load_task_entries, load_task_records,
    render_report,
};
pub use lifecycle::TransitionDetails;
pub(crate) use lookup::task_roots;
pub(crate) use template::TaskSaveError;
pub use template::{
    AcceptanceFile, Blocker, BlockerKind, BlockerRef, BlockerSource, ClaimCheckReceipt,
    ProofSourceReceipt, TaskRecord, TaskState, VerificationBinding, VerificationCommandReceipt,
    VerificationStatus, task_markdown,
};

/// Minimal Task projection for feature rollups.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureTaskProjection {
    pub id: String,
    pub feature_id: Option<String>,
    pub state: Option<TaskState>,
}

/// Result of appending a task note.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteReport {
    /// Task id.
    pub id: String,
    /// Whether `notes.md` was created by this append.
    pub created: bool,
}

/// Inputs for creating a draft task.
pub struct CreateTaskOptions {
    pub feature: Option<String>,
    pub covers: Vec<String>,
    pub lane: Option<String>,
    pub risk: Option<String>,
    pub checks: Vec<String>,
    pub created_at: String,
}

/// Task aggregate loaded with its Task-owned optimistic save context.
///
/// `Eq` is intentionally absent: the card-mode snapshot carries a
/// [`serde_yaml::Mapping`] (via `CardSnapshot`), whose values may be floats, so
/// only `PartialEq` is derivable -- matching [`crate::domain::card::schema::Card`].
#[derive(Clone, Debug, PartialEq)]
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
    options: CreateTaskOptions,
) -> Result<TaskRecord> {
    if title.trim().is_empty() {
        bail!("task title must not be empty");
    }
    if options.checks.iter().any(|check| check.trim().is_empty()) {
        bail!("task check cannot be empty; pass an observable verify+ check");
    }
    if options.covers.iter().any(|cover| cover.trim().is_empty()) {
        bail!("task cover cannot be empty; pass an acceptance id such as ac-1");
    }
    // Both modes reserve a `.alloc-` marker that gives the id-allocation loop
    // liveness (concurrent creates bump past each other's held markers and mint
    // distinct numbers, SPEC D2) and hold it until the artifacts land. Card mode
    // allocates above the live task cards unioned with the still-legacy archive
    // (L6a: never reissue a freed id); the create-time CAS (D1) is the safety belt.
    let card_paths = lookup::paths_for_tasks_dir(tasks_dir)
        .filter(|paths| store_mode(paths) == StoreMode::Cards);
    let (id, _marker) = match card_paths.as_ref() {
        Some(paths) => {
            let reserved = reserve_next_card_task_id(paths, tasks_dir)?;
            (reserved.id, reserved._marker)
        }
        None => {
            let reserved = reserve_next_task_id(tasks_dir)?;
            (reserved.id, reserved._marker)
        }
    };
    let mut task = TaskRecord::draft(&id, title, &options.created_at);
    task.feature_id = options.feature;
    task.covers = options.covers;
    if let Some(lane) = options.lane {
        task.lane = Some(lane);
    }
    if let Some(risk) = options.risk {
        task.risk = Some(risk);
    }
    let acceptance = AcceptanceFile::new(&id, options.checks);
    task.acceptance = acceptance.clone();
    match card_paths.as_ref() {
        Some(paths) => cards::create(paths, &task)?,
        None => {
            let task_root = task_root_for_feature(tasks_dir, task.feature_id.as_deref())?;
            template::write_task_artifacts(&task_root, &task, &acceptance)?;
        }
    }
    drop(_marker);
    Ok(task)
}

/// Reserve the next `task-NNN` id in card mode through the shared D2 seam: the
/// floor is the max of live task cards and the still-legacy archive tasks (L6a --
/// never reissue an archived id), and the `.alloc-` marker is held by the caller
/// until the card lands.
fn reserve_next_card_task_id(
    paths: &MaestroPaths,
    tasks_dir: &Path,
) -> Result<card_store::ReservedCardId> {
    let mut floor = cards::max_task_number(paths)?;
    if let Some(archive_tasks_dir) = archive_tasks_sibling(tasks_dir) {
        floor = floor.max(max_task_number(&archive_tasks_dir)?);
    }
    card_store::reserve_next_numbered_id(paths, "task", floor)
}

/// Load one Task record by id or id prefix.
pub fn load_task_record(tasks_dir: &Path, id: &str) -> Result<TaskRecord> {
    let (task, _, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    Ok(task)
}

/// Resolve a task's current `task.yaml` path by id or id prefix.
pub fn task_yaml_path(tasks_dir: &Path, id: &str) -> Result<PathBuf> {
    lookup::resolve_task_yaml_path(tasks_dir, id)
}

/// Read a task's inline acceptance checks for display.
pub fn load_task_checks(tasks_dir: &Path, task: &TaskRecord) -> Result<Vec<String>> {
    let _ = tasks_dir;
    Ok(task.acceptance.checks.clone())
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
    Ok(load_task_entries(tasks_dir)?
        .into_iter()
        .map(|entry| FeatureTaskProjection {
            id: entry.task.id,
            feature_id: entry.task.feature_id,
            state: Some(entry.task.state),
        })
        .collect())
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

/// Append one dated line to a task's `notes.md`, creating it on first write.
pub fn note(tasks_dir: &Path, id: &str, text: &str) -> Result<NoteReport> {
    let (task, _, task_dir) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    if text.trim().is_empty() {
        bail!("task note text cannot be empty");
    }
    let path = task_dir.join("notes.md");
    let created = append_note_file(&path, &task.title, text)?;
    Ok(NoteReport {
        id: task.id,
        created,
    })
}

fn append_note_file(path: &Path, title: &str, text: &str) -> Result<bool> {
    let date = utc_now_timestamp()
        .split_once('T')
        .map(|(date, _)| date.to_string())
        .unwrap_or_else(|| "1970-01-01".to_string());
    append_text_file(
        path,
        &format!("# {title}\n\n"),
        &format!("{date}  {}\n", text.trim()),
    )
    .with_context(|| format!("failed to append task note {}", path.display()))
}

/// Lock acceptance criteria and move a task to ready.
pub fn accept_task(
    tasks_dir: &Path,
    id: &str,
    actor: &str,
    accepted_at: &str,
) -> Result<TaskRecord> {
    let (mut task, snapshot, task_dir) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    // Validate the state transition before the content gate: a terminal or
    // not-yet-explored task cannot become ready no matter how many checks it has,
    // so its add-check remedy would be a dead end. Letting `transition` speak
    // first surfaces the real blocker (terminal / explore-first). For a valid
    // exploring->ready accept the transition passes and the checks gate fires.
    task.acceptance_locked = true;
    task.acceptance.locked_by = Some(actor.to_string());
    task.acceptance.locked_at = Some(accepted_at.to_string());
    lifecycle::transition(
        &mut task,
        TaskState::Ready,
        actor,
        accepted_at,
        TransitionDetails::default(),
    )?;
    ensure_standalone_has_checks(&task, &task_dir)?;
    template::save_task_with_snapshot(&task, &snapshot)?;
    Ok(task)
}

/// Claim a ready task.
pub fn claim_task(tasks_dir: &Path, id: &str, actor: &str, claimed_at: &str) -> Result<TaskRecord> {
    let (mut task, snapshot, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    if task.state == TaskState::Draft {
        bail!(
            "task {} is draft; run `maestro task explore {}` before claiming",
            task.id,
            task.id
        );
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
///
/// Returns the updated task and the number of checks that were replaced, so the
/// caller can warn when a repeat call silently drops earlier checks.
///
/// # Errors
///
/// Refuses an empty or whitespace-only check: it would otherwise satisfy the
/// `ensure_standalone_has_checks` gate by list length while carrying no
/// contract, letting a standalone task be accepted with a meaningless
/// acceptance criterion. Refuses a terminal task first (settled history), then
/// an acceptance-locked one, so a previously-accepted but now-terminal task
/// reports the terminal reason rather than the misleading acceptance lock.
pub fn set_checks(tasks_dir: &Path, id: &str, checks: Vec<String>) -> Result<(TaskRecord, usize)> {
    let handle = load_task_for_update(tasks_dir, id)?;
    // Terminal first: a previously-accepted task that is now rejected/abandoned/
    // superseded is still `acceptance_locked`, so checking the lock first would
    // report "acceptance is locked ... after accept" for a task that is really
    // settled history. The terminal reason is the accurate, non-misleading one.
    if lifecycle::is_terminal(&handle.task().state) {
        bail!(
            "task {} is {}; its checks are settled history and cannot change",
            handle.task().id,
            handle.task().state.as_str()
        );
    }
    if handle.task().acceptance_locked {
        bail!(
            "task {} acceptance is locked; checks cannot be changed after accept",
            handle.task().id
        );
    }
    if checks.iter().any(|check| check.trim().is_empty()) {
        bail!(
            "task {} check cannot be empty; e.g. `maestro task set {} --check \"build passes\"`",
            handle.task().id,
            handle.task().id
        );
    }
    let mut task = handle.task().clone();
    // Re-check the freshly read acceptance file's own lock marker, not just the
    // task.yaml snapshot loaded above: a concurrent `accept`/claim can freeze the
    // contract between that load and this write. `lock_acceptance` records the
    // freeze as `locked_by`/`locked_at` in this file, so honoring it here closes
    // the race where set_checks would otherwise clobber an already-frozen contract.
    if task.acceptance.locked_by.is_some() {
        bail!(
            "task {} acceptance is locked; checks cannot be changed after accept",
            handle.task().id
        );
    }
    let replaced = task.acceptance.checks.len();
    task.acceptance.checks = checks;
    template::save_task_with_snapshot(&task, &handle.snapshot)?;
    Ok((task, replaced))
}

/// Author a task's feature acceptance coverage links, replacing the current list.
///
/// Coverage is part of the task contract and freezes with acceptance, just like
/// checks. A missed post-acceptance link should be handled by feature-level
/// explicit evidence instead of moving a verified task's goalposts.
pub fn set_covers(tasks_dir: &Path, id: &str, covers: Vec<String>) -> Result<(TaskRecord, usize)> {
    let handle = load_task_for_update(tasks_dir, id)?;
    if lifecycle::is_terminal(&handle.task().state) || handle.task().state == TaskState::Verified {
        bail!(
            "task {} is {}; its covers links are settled history and cannot change",
            handle.task().id,
            handle.task().state.as_str()
        );
    }
    if handle.task().acceptance_locked || handle.task().acceptance.locked_by.is_some() {
        bail!(
            "task {} acceptance is locked; covers links cannot be changed after accept",
            handle.task().id
        );
    }
    if covers.iter().any(|cover| cover.trim().is_empty()) {
        bail!(
            "task {} cover cannot be empty; e.g. `maestro task set {} --covers ac-1`",
            handle.task().id,
            handle.task().id
        );
    }
    let mut task = handle.task().clone();
    let replaced = task.covers.len();
    task.covers = covers;
    template::save_task_with_snapshot(&task, &handle.snapshot)?;
    Ok((task, replaced))
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
    let (loaded_task, snapshot, current_dir) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    let original_task = loaded_task.clone();
    let mut task = loaded_task;
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
    // Card mode: the parent is just a field, so there is no directory to move --
    // retarget, audit, and save. Legacy mode physically relocates the task dir
    // across feature subdirs (below), rolling the record back on a rename failure.
    if matches!(snapshot, template::TaskSnapshot::Card { .. }) {
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
        return Ok(task);
    }
    let target_root = task_root_for_feature(tasks_dir, feature.as_deref())?;
    let target_dir = target_root.join(task.directory_name());
    let should_move = target_dir != current_dir;
    if should_move && target_dir.exists() {
        bail!(
            "cannot move task {} — target already exists at {}",
            task.id,
            target_dir.display()
        );
    }
    if should_move {
        ensure_dir(&target_root)?;
    }

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
    if should_move && let Err(error) = fs::rename(&current_dir, &target_dir) {
        let rollback =
            serde_yaml::to_string(&original_task).context("failed to serialize task rollback")?;
        let current_yaml = current_dir.join("task.yaml");
        if let Err(rollback_error) = write_string_atomic(&current_yaml, &rollback) {
            return Err(error)
                .with_context(|| {
                    format!(
                        "failed to move {} to {}",
                        current_dir.display(),
                        target_dir.display()
                    )
                })
                .context(format!(
                    "failed to restore {} after move failure: {rollback_error}",
                    current_yaml.display()
                ));
        }
        return Err(error).with_context(|| {
            format!(
                "failed to move {} to {}",
                current_dir.display(),
                target_dir.display()
            )
        });
    }
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
    // Compare canonical ids, not the raw args: lookup resolves id-prefixes, so
    // `supersede task-001 --by task-1` could otherwise self-supersede unnoticed.
    let (target, _, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    if replacement.id == target.id {
        bail!(
            "cannot supersede {} by itself; `--by` must name a different task",
            target.id
        );
    }
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
    if !task.state.is_live() {
        bail!(
            "cannot block {id} — done (state: {}); a finished task cannot take a blocker",
            task.state.as_str()
        );
    }
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
///
/// # Errors
///
/// Returns an error when a `--claim` value is empty/whitespace: a claim is the
/// proof a later `task verify` checks against, so a blank one is meaningless.
pub fn update_task_history(
    tasks_dir: &Path,
    id: &str,
    actor: &str,
    updated_at: &str,
    details: TransitionDetails,
) -> Result<TaskRecord> {
    if details.claims.iter().any(|claim| claim.trim().is_empty()) {
        bail!(
            "`--claim` must not be empty; pass the proof to verify against, e.g. --claim \"cargo test passes\""
        );
    }
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
            task.verification = failed.binding;
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

pub(crate) fn apply_verification_outcome_to_handle(
    handle: &mut TaskHandle,
    outcome: VerificationOutcome,
    actor: &str,
    applied_at: &str,
) -> Result<()> {
    apply_verification_outcome(&mut handle.task, outcome, actor, applied_at);
    template::save_task_with_snapshot(&handle.task, &handle.snapshot)
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
    pub(crate) summary: String,
}

/// Failed verification data accepted by the Task aggregate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerificationFailed {
    pub(crate) binding: VerificationBinding,
    pub(crate) summary: String,
    pub(crate) failures: Vec<String>,
}

/// C4 lock-path gate: a standalone task (no `feature_id`) carries no inherited
/// product contract, so its `checks` are its only contract and must be
/// non-empty before it can be accepted or claim-auto-locked. Featured tasks
/// inherit the feature's frozen contract and are exempt.
fn ensure_standalone_has_checks(task: &TaskRecord, task_dir: &Path) -> Result<()> {
    let _ = task_dir;
    if task.feature_id.is_some() {
        return Ok(());
    }
    if task.acceptance.checks.is_empty() {
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

#[derive(Debug)]
struct ReservedTaskId {
    id: String,
    _marker: DirReservation,
}

fn reserve_next_task_id(tasks_dir: &Path) -> Result<ReservedTaskId> {
    // L6a: an archived `task-NNN` still owns its id, so allocate above the max
    // of the union of live + archived tasks — never reissue a freed id.
    let mut max = max_task_number(tasks_dir)?;
    if let Some(archive_tasks_dir) = archive_tasks_sibling(tasks_dir) {
        max = max.max(max_task_number(&archive_tasks_dir)?);
    }
    max = max.max(max_reserved_task_number(tasks_dir)?);
    let mut candidate = max + 1;
    loop {
        let marker_name = format!("{ALLOC_MARKER_PREFIX}task-{candidate:03}");
        let Some(marker) = try_reserve_marker_dir(tasks_dir, &marker_name)? else {
            candidate += 1;
            continue;
        };
        if task_number_exists(tasks_dir, candidate)? {
            drop(marker);
            candidate += 1;
            continue;
        }
        return Ok(ReservedTaskId {
            id: format!("task-{candidate:03}"),
            _marker: marker,
        });
    }
}

/// Highest `task-NNN` number among the directory's entries (0 if dir absent).
fn max_task_number(tasks_dir: &Path) -> Result<u32> {
    let mut max = 0_u32;
    for root in lookup::task_roots(tasks_dir)? {
        if !root.is_dir() {
            continue;
        }
        for entry in
            fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))?
        {
            let entry = entry.with_context(|| format!("failed to list {}", root.display()))?;
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
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

fn max_reserved_task_number(tasks_dir: &Path) -> Result<u32> {
    let mut max = 0_u32;
    for (path, _) in child_dirs(tasks_dir)? {
        if let Some(num) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(ALLOC_MARKER_PREFIX))
            .and_then(|rest| rest.strip_prefix("task-"))
            .and_then(|value| value.parse::<u32>().ok())
        {
            max = max.max(num);
        }
    }
    Ok(max)
}

fn task_number_exists(tasks_dir: &Path, number: u32) -> Result<bool> {
    if task_number_exists_in(tasks_dir, number)? {
        return Ok(true);
    }
    if let Some(archive_tasks_dir) = archive_tasks_sibling(tasks_dir)
        && task_number_exists_in(&archive_tasks_dir, number)?
    {
        return Ok(true);
    }
    Ok(false)
}

fn task_number_exists_in(tasks_dir: &Path, number: u32) -> Result<bool> {
    for root in lookup::task_roots(tasks_dir)? {
        for (path, _) in child_dirs(&root)? {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_prefix("task-"))
                .and_then(|rest| rest.split('-').next())
                .and_then(|value| value.parse::<u32>().ok())
                == Some(number)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn task_root_for_feature(tasks_dir: &Path, feature_id: Option<&str>) -> Result<PathBuf> {
    let Some(feature_id) = feature_id else {
        return Ok(tasks_dir.to_path_buf());
    };
    let maestro_dir = tasks_dir.parent().with_context(|| {
        format!(
            "cannot derive feature task root from {}",
            tasks_dir.display()
        )
    })?;
    Ok(maestro_dir.join("features").join(feature_id).join("tasks"))
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
        TaskRecord, TaskState, VerificationBinding, VerificationOutcome, VerificationPassed,
        VerificationStatus, apply_verification_outcome,
    };

    #[test]
    fn verification_outcome_pass_sets_binding_and_history() {
        let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        task.state = TaskState::NeedsVerification;

        apply_verification_outcome(
            &mut task,
            VerificationOutcome::Passed(VerificationPassed {
                binding: VerificationBinding {
                    status: Some(VerificationStatus::Passed),
                    verified_at: Some("t1".to_string()),
                    verified_commit: Some("abc123".to_string()),
                    verified_by_run: Some("runs/session".to_string()),
                    contract_hash: Some("task-hash".to_string()),
                    ..VerificationBinding::default()
                },
                summary: "verification passed: 1 claim(s), 1 proof source(s)".to_string(),
            }),
            "codex",
            "t1",
        );

        assert_eq!(task.state, TaskState::Verified);
        assert_eq!(task.verification.verified_at.as_deref(), Some("t1"));
        assert_eq!(task.verification.verified_commit.as_deref(), Some("abc123"));
        assert_eq!(task.verification.status, Some(VerificationStatus::Passed));
        assert_eq!(
            task.verification.contract_hash.as_deref(),
            Some("task-hash")
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
                binding: VerificationBinding {
                    status: Some(VerificationStatus::Failed),
                    verified_at: Some("t1".to_string()),
                    failures: vec!["missing evidence".to_string()],
                    ..VerificationBinding::default()
                },
                summary: "verification failed: missing evidence".to_string(),
                failures: vec!["missing evidence".to_string()],
            }),
            "codex",
            "t1",
        );

        assert_eq!(task.state, TaskState::NeedsVerification);
        assert_eq!(task.verification.verified_at.as_deref(), Some("t1"));
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
        assert_eq!(task.verification.status, Some(VerificationStatus::Failed));
        assert_eq!(task.verification.failures, vec!["missing evidence"]);
    }
}
