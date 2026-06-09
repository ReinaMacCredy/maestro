//! Task aggregate facade.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card::schema::CardType;
use crate::domain::card::store as card_store;
use crate::foundation::core::fs::append_text_file;
use crate::foundation::core::paths::MaestroPaths;
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
    TaskDoctorReport, TaskEntry, check_blocker_graph, load_archived_task_entries,
    load_task_entries, load_task_records, render_report,
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

impl BlockerTarget {
    /// Classify an optional `--by` reference into a typed blocker target. A
    /// content-addressed `card-<hash>` id no longer encodes its type, so in card
    /// mode the referenced card's own `card_type` decides the routing; the
    /// id-prefix parse is the fallback for an absent card or legacy mode. `None`
    /// is a manual/human block.
    pub fn from_ref(paths: &MaestroPaths, by: Option<String>) -> Self {
        let Some(by) = by else {
            return Self::Human;
        };
        card_blocker_target(paths, &by).unwrap_or_else(|| Self::from_prefix(by))
    }

    fn from_prefix(by: String) -> Self {
        if by.starts_with("task-") {
            Self::Task(by)
        } else if by.starts_with("decision-") {
            Self::Decision(by)
        } else {
            Self::External(by)
        }
    }
}

/// Route a `--by` id to a typed target by the referenced card's type, or `None`
/// when no card resolves it (the caller falls back to the id-prefix parse). A
/// Feature/Idea card is not a Task/Decision blocker target, so it also yields
/// `None` and is left to the prefix parse.
fn card_blocker_target(paths: &MaestroPaths, by: &str) -> Option<BlockerTarget> {
    let card = card_store::load(&card_store::card_path(paths, by))
        .ok()
        .flatten()?;
    match card.card_type {
        CardType::Task | CardType::Bug | CardType::Chore => {
            Some(BlockerTarget::Task(by.to_string()))
        }
        CardType::Decision => Some(BlockerTarget::Decision(by.to_string())),
        CardType::Feature | CardType::Idea => None,
    }
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
    // Mint a content-addressed `card-<hash>` id from the title plus a process
    // nonce (SPEC O3'); the create-time CAS (D1) is the collision belt, so two
    // creators racing on the same hash both attempt the write and the loser
    // fails loud rather than silently bumping.
    let paths = lookup::paths_for_tasks_dir(tasks_dir)
        .context("cannot resolve maestro paths from tasks dir")?;
    let id = card_store::mint_card_id(&paths, title);
    let mut task = TaskRecord::draft(&id, title, &options.created_at);
    task.feature_id = options.feature;
    task.covers = options.covers;
    if let Some(lane) = options.lane {
        task.lane = Some(lane);
    }
    if let Some(risk) = options.risk {
        task.risk = Some(risk);
    }
    task.acceptance = AcceptanceFile::new(&id, options.checks);
    cards::create(&paths, &task)?;
    Ok(task)
}

/// Load one Task record by id or id prefix.
pub fn load_task_record(tasks_dir: &Path, id: &str) -> Result<TaskRecord> {
    let (task, _, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    Ok(task)
}

/// Resolve a task's on-disk record path (`cards/<id>/card.yaml`) by canonical id.
///
/// Card-routed: the legacy `.maestro/tasks` tree no longer exists, so this joins
/// the card store path and confirms the record is present, bailing a clean
/// not-found otherwise. Callers take `.parent()` to reach the card directory.
pub fn task_yaml_path(tasks_dir: &Path, id: &str) -> Result<PathBuf> {
    let paths = lookup::paths_for_tasks_dir(tasks_dir)
        .context("cannot resolve maestro paths from tasks dir")?;
    let path = card_store::card_path(&paths, id);
    if !path.is_file() {
        bail!("task not found: {id}");
    }
    Ok(path)
}

/// Read a task's inline acceptance checks for display.
pub fn load_task_checks(tasks_dir: &Path, task: &TaskRecord) -> Result<Vec<String>> {
    let _ = tasks_dir;
    Ok(task.acceptance.checks.clone())
}

/// Of `tasks`, the unlinked Draft/Exploring ids that carry no verify-contract
/// checks yet. The CLI and MCP list surfaces both annotate these, so the rule
/// lives here once instead of being duplicated across the two adapters.
pub fn missing_verify_contract_ids(
    paths: &MaestroPaths,
    tasks: &[TaskRecord],
    archived_ids: &BTreeSet<String>,
) -> Result<BTreeSet<String>> {
    let mut missing = BTreeSet::new();
    for task in tasks {
        if task.feature_id.is_some()
            || !matches!(task.state, TaskState::Draft | TaskState::Exploring)
        {
            continue;
        }
        let tasks_dir = if archived_ids.contains(&task.id) {
            paths.archive_tasks_dir()
        } else {
            paths.tasks_dir()
        };
        if load_task_checks(&tasks_dir, task)?.is_empty() {
            missing.insert(task.id.clone());
        }
    }
    Ok(missing)
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
    // The parent is just a card field -- retarget, audit, and save in place.
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
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::{
        BlockerTarget, CreateTaskOptions, MaestroPaths, TaskRecord, TaskState, VerificationBinding,
        VerificationOutcome, VerificationPassed, VerificationStatus, apply_verification_outcome,
        create_task, missing_verify_contract_ids, set_feature,
    };

    #[test]
    fn blocker_target_from_ref_falls_back_to_prefix_when_no_card() {
        // No card store resolves these ids, so classification falls through to
        // the legacy id-prefix parse -- the card-type lookup is exercised e2e.
        let paths = MaestroPaths::new(Path::new("/nonexistent-maestro-from-ref"));
        assert_eq!(
            BlockerTarget::from_ref(&paths, Some("task-001".to_string())),
            BlockerTarget::Task("task-001".to_string())
        );
        assert_eq!(
            BlockerTarget::from_ref(&paths, Some("decision-007".to_string())),
            BlockerTarget::Decision("decision-007".to_string())
        );
        assert_eq!(
            BlockerTarget::from_ref(&paths, Some("PR #42".to_string())),
            BlockerTarget::External("PR #42".to_string())
        );
        assert_eq!(BlockerTarget::from_ref(&paths, None), BlockerTarget::Human);
    }

    #[test]
    fn missing_verify_contract_ids_flags_only_unlinked_unchecked_drafts() {
        let paths = MaestroPaths::new(Path::new("/nonexistent-missing-verify-test"));

        let mut unchecked = TaskRecord::draft("task-001", "no checks yet", "t0");
        unchecked.state = TaskState::Exploring;

        let mut checked = TaskRecord::draft("task-002", "has a check", "t0");
        checked.acceptance.checks = vec!["ac-1".to_string()];

        let mut linked = TaskRecord::draft("task-003", "linked to a feature", "t0");
        linked.feature_id = Some("agent-cli-ux".to_string());

        let mut settled = TaskRecord::draft("task-004", "already ready", "t0");
        settled.state = TaskState::Ready;

        let missing = missing_verify_contract_ids(
            &paths,
            &[unchecked, checked, linked, settled],
            &BTreeSet::new(),
        )
        .expect("missing-verify scan should not error");

        assert_eq!(missing, BTreeSet::from(["task-001".to_string()]));
    }

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

    fn card_mode_repo(label: &str) -> MaestroPaths {
        use std::process;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "maestro-task-setfeat-{label}-{}-{nanos}",
            process::id()
        ));
        let paths = MaestroPaths::new(&root);
        crate::foundation::core::fs::ensure_dir(paths.cards_dir()).expect("create cards dir");
        paths
    }

    /// In card mode `set_feature` retargets `card.parent` in place -- no directory
    /// move (the legacy carrier of the feature link). Detaching to `None` clears
    /// the persisted parent and drops the task from the feature's on-demand count.
    /// (Ported from the deleted task_card_cutover: the legacy directory-move
    /// collision tests in task_commands cover a path that no longer exists.)
    #[test]
    fn card_mode_set_feature_retargets_parent_in_place_and_drops_the_count() {
        use crate::domain::card::schema::Card;
        use crate::domain::card::store::card_path;

        let paths = card_mode_repo("detach");
        let tasks_dir = paths.tasks_dir();

        let feature_id = crate::feature::create(&paths, "Csv export").expect("create feature card");
        let task = create_task(
            &tasks_dir,
            "Add CSV export",
            CreateTaskOptions {
                feature: Some(feature_id.clone()),
                covers: Vec::new(),
                lane: None,
                risk: None,
                checks: Vec::new(),
                created_at: "2026-06-09T12:00:00Z".to_string(),
            },
        )
        .expect("create task card with a feature parent");
        assert!(task.id.starts_with("card-"), "card-mode id: {}", task.id);

        let counts = crate::feature::query::count_tasks_by_feature(&tasks_dir).expect("count");
        assert_eq!(counts.get(&feature_id).map(|c| c.total), Some(1));

        set_feature(
            &tasks_dir,
            &task.id,
            None,
            "maestro",
            "2026-06-09T12:00:00Z",
        )
        .expect("detach the feature");

        let card: Card = serde_yaml::from_str(
            &std::fs::read_to_string(card_path(&paths, &task.id)).expect("card readable"),
        )
        .expect("card parses");
        assert_eq!(card.parent, None, "the parent field is cleared in place");

        let counts = crate::feature::query::count_tasks_by_feature(&tasks_dir).expect("recount");
        assert_eq!(
            counts.get(&feature_id),
            None,
            "the detached task no longer counts toward the feature"
        );

        let _ = std::fs::remove_dir_all(paths.maestro_dir());
    }
}
