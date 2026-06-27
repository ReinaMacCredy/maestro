//! Task aggregate facade.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card::schema::CardType;
use crate::domain::card::store as card_store;
use crate::foundation::core::fs::append_text_file;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::{parse_utc_timestamp, utc_now_timestamp};

pub(crate) mod blockers;
pub(crate) mod cards;
pub(crate) mod display;
pub(crate) mod doctor;
pub(crate) mod lifecycle;
pub(crate) mod lookup;
pub mod progress;
pub(crate) mod template;

pub use blockers::has_unresolved_blockers;
pub use display::{render_task, render_task_list, render_task_list_with_missing_checks};
pub use doctor::{
    TaskDoctorReport, TaskEntry, check_blocker_graph, check_blocker_graph_in_cards,
    load_archived_task_entries, load_progress_task_entries, load_task_entries, load_task_records,
    render_report,
};
pub use lifecycle::TransitionDetails;
pub use progress::PROGRESS_FILE;
pub use template::{
    AcceptanceFile, Blocker, BlockerKind, BlockerRef, BlockerSource, ClaimCheckReceipt,
    ProofSourceReceipt, TaskRecord, TaskState, VerificationBinding, VerificationCommandReceipt,
    VerificationStatus, task_markdown,
};

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
    pub project: Option<String>,
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
    /// is a manual/human block. A `--by` naming a feature or idea card errors:
    /// silently filing it as External would hide a graph edge the card store
    /// can track.
    pub fn from_ref(paths: &MaestroPaths, by: Option<String>) -> Result<Self> {
        let Some(by) = by else {
            return Ok(Self::Human);
        };
        // A ref that cannot be a card id (a URL, a path) is free-form external;
        // only id-shaped refs consult the store, where a failure is a real
        // unreadable-card error, not absence -- swallowing it would misfile a
        // tracked card ref as External.
        if card_store::validate_card_id(&by).is_err() {
            return Ok(Self::from_prefix(by));
        }
        let card = card_store::resolve(paths, &by)?.map(|resolved| resolved.card);
        match card.map(|card| card.card_type) {
            Some(CardType::Task | CardType::Bug | CardType::Chore) => Ok(Self::Task(by)),
            Some(CardType::Decision) => Ok(Self::Decision(by)),
            Some(
                kind @ (CardType::Feature | CardType::Custom | CardType::Progress | CardType::Idea),
            ) => bail!(
                "cannot block on {by}: it is a {} card, not a task or decision\n  record the dependency as a card edge instead: maestro card dep add <task> {by}",
                kind.as_str()
            ),
            None => Ok(Self::from_prefix(by)),
        }
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
    // Mint a typed slug id `task-<slug>-<hex4>` from the title plus a process
    // nonce (SPEC O3'); the create-time CAS (D1) is the collision belt, so two
    // creators racing on the same hash both attempt the write and the loser
    // fails loud rather than silently bumping.
    let paths = lookup::paths_for_tasks_dir(tasks_dir)
        .context("cannot resolve maestro paths from tasks dir")?;
    let id = card_store::mint_card_id(&paths, CardType::Task, title);
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
    cards::create(&paths, &task, options.project)?;
    Ok(task)
}

/// Create a standalone low-ceremony task that is immediately ready to start.
///
/// This is the `task add` path: no feature/card gate, no acceptance checks, and
/// no separate lifecycle. The record still uses the normal Task state machine
/// once work starts.
pub fn add_simple_task(
    tasks_dir: &Path,
    title: &str,
    card_id: Option<String>,
    project: Option<String>,
    created_at: String,
    actor: &str,
) -> Result<TaskRecord> {
    if title.trim().is_empty() {
        bail!("task title must not be empty");
    }
    let paths = lookup::paths_for_tasks_dir(tasks_dir)
        .context("cannot resolve maestro paths from tasks dir")?;
    if card_id.is_none() {
        return progress::add_simple_task(&paths, title, project, created_at, actor);
    }
    let id = card_store::mint_card_id(&paths, CardType::Task, title);
    let mut task = TaskRecord::draft(&id, title, &created_at);
    task.feature_id = card_id;
    task.state = TaskState::Ready;
    task.acceptance_locked = true;
    task.acceptance.locked_by = Some(actor.to_string());
    task.acceptance.locked_at = Some(created_at.clone());
    cards::create(&paths, &task, project)?;
    Ok(task)
}

/// Load one Task record by id or id prefix.
pub fn load_task_record(tasks_dir: &Path, id: &str) -> Result<TaskRecord> {
    let (task, _, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    Ok(task)
}

/// [`load_task_record`] with true absence as `Ok(None)` instead of an error,
/// so fallbacks (archive probes) never swallow a real read failure.
pub fn try_load_task_record(tasks_dir: &Path, id: &str) -> Result<Option<TaskRecord>> {
    lookup::try_load_task_record(tasks_dir, id)
}

/// Read an archived task card (`archive/cards/<id>/card.yaml`) with its card
/// directory. Read-only: archived tasks stay immutable, so no save snapshot is
/// exposed. `None` when no Task-typed card holds the id in the archive.
pub fn load_archived_task_record(
    paths: &MaestroPaths,
    id: &str,
) -> Result<Option<(TaskRecord, PathBuf)>> {
    cards::load_one_archived(paths, id)
}

/// Resolve a task's on-disk record path by canonical id, probing every home
/// the resolver covers (a `tasks/` pool dir or a pre-migration flat dir).
/// Bails a clean not-found when no home holds the id. Callers take
/// `.parent()` to reach the card directory.
pub fn task_yaml_path(tasks_dir: &Path, id: &str) -> Result<PathBuf> {
    let paths = lookup::paths_for_tasks_dir(tasks_dir)
        .context("cannot resolve maestro paths from tasks dir")?;
    let Some(home) = card_store::locate(&paths, id)? else {
        if let Some((_, snapshot, _)) = progress::load_task_with_snapshot(&paths, id)? {
            return Ok(snapshot.path);
        }
        bail!("task not found: {id}");
    };
    Ok(home.path().to_path_buf())
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
) -> Result<BTreeSet<String>> {
    let mut missing = BTreeSet::new();
    for task in tasks {
        if task.feature_id.is_some()
            || !matches!(task.state, TaskState::Draft | TaskState::Exploring)
        {
            continue;
        }
        if load_task_checks(&paths.tasks_dir(), task)?.is_empty() {
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

    tasks.sort_by(|left, right| {
        task_list_priority(left)
            .cmp(&task_list_priority(right))
            .then_with(|| left.id.cmp(&right.id))
    });
    tasks
}

fn task_list_priority(task: &TaskRecord) -> u8 {
    match task.state {
        TaskState::InProgress => 0,
        TaskState::NeedsVerification => 1,
        TaskState::Ready => 2,
        TaskState::Draft | TaskState::Exploring => 3,
        TaskState::Verified
        | TaskState::Rejected
        | TaskState::Abandoned
        | TaskState::Superseded => 4,
    }
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

/// Complete a low-ceremony standalone task without bypassing explicit gates.
///
/// The shortcut is intentionally narrow: a task with a card/feature owner,
/// checks, blockers, or an explicit verify command must use the normal
/// `complete` -> `verify` path.
pub fn complete_simple_task(
    tasks_dir: &Path,
    id: &str,
    actor: &str,
    completed_at: &str,
    summary: Option<String>,
) -> Result<TaskRecord> {
    let (mut task, snapshot, _) = lookup::load_task_with_snapshot(tasks_dir, id)?;
    let paths = lookup::paths_for_tasks_dir(tasks_dir)
        .context("cannot resolve maestro paths from tasks dir")?;
    guard_simple_done_allowed(&paths, &task)?;
    let summary = summary.unwrap_or_else(|| "marked done".to_string());
    let claim = format!("simple completion: {summary}");
    lifecycle::transition(
        &mut task,
        TaskState::NeedsVerification,
        actor,
        completed_at,
        TransitionDetails {
            summary: Some(summary.clone()),
            claims: vec![claim.clone()],
            ..TransitionDetails::default()
        },
    )?;
    apply_verification_outcome(
        &mut task,
        VerificationOutcome::Passed(VerificationPassed {
            binding: VerificationBinding {
                status: Some(VerificationStatus::Passed),
                verified_at: Some(completed_at.to_string()),
                claim_checks: vec![ClaimCheckReceipt {
                    claim,
                    matched: true,
                    source: Some("task done".to_string()),
                }],
                claims_only: true,
                ..VerificationBinding::default()
            },
            summary: "simple task done: no explicit verification gate".to_string(),
        }),
        actor,
        completed_at,
    );
    template::save_task_with_snapshot(&task, &snapshot)?;
    Ok(task)
}

fn guard_simple_done_allowed(paths: &MaestroPaths, task: &TaskRecord) -> Result<()> {
    if task.state == TaskState::Verified {
        bail!("task {} is already done", task.id);
    }
    if task.state != TaskState::InProgress {
        bail!(
            "task {} is {}; run `maestro task start {}` before `maestro task done`",
            task.id,
            task.state.as_str(),
            task.id
        );
    }
    if let Some(card_id) = task.feature_id.as_deref() {
        let Some(card) = card_store::resolve(paths, card_id)?.map(|resolved| resolved.card) else {
            bail!(
                "task {} belongs to missing card {}; repair the card link before marking it done",
                task.id,
                card_id
            );
        };
        if card.card_type != CardType::Chore {
            bail!(
                "task {} belongs to a {} card; use `maestro task complete {} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\"`",
                task.id,
                card.card_type.as_str(),
                task.id
            );
        }
    }
    if !task.acceptance.checks.is_empty() || task.verify_command.is_some() {
        bail!(
            "task {} has an explicit verification gate; use `maestro task complete {} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\"`",
            task.id,
            task.id
        );
    }
    if has_unresolved_blockers(task) {
        bail!(
            "task {} has unresolved blockers; run `maestro task show {}`",
            task.id,
            task.id
        );
    }
    Ok(())
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
        let feature = handle
            .task()
            .feature_id
            .as_deref()
            .unwrap_or("<feature-id>");
        bail!(
            "task {} acceptance is locked; covers links cannot be changed after accept; cover the item with feature evidence instead: `maestro feature verify {feature} --prove <ac-id> --evidence \"<proof>\"`",
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

/// Author (or clear) a task's optional per-task narrow falsifier command.
///
/// When set, `task verify` runs ONLY this command for the slice instead of the
/// repo-global `stack.verify`. The command joins the task contract hash, so it
/// is frozen once the task is verified or terminal: changing how a settled task
/// was verified would silently invalidate its recorded proof. `None` clears it.
/// A no-op (already equal) returns without writing.
pub fn set_verify_command(
    tasks_dir: &Path,
    id: &str,
    command: Option<String>,
) -> Result<TaskRecord> {
    let handle = load_task_for_update(tasks_dir, id)?;
    if lifecycle::is_terminal(&handle.task().state) || handle.task().state == TaskState::Verified {
        bail!(
            "task {} is {}; its verify command is settled history and cannot change",
            handle.task().id,
            handle.task().state.as_str()
        );
    }
    let command = match command {
        Some(command) => {
            let trimmed = command.trim();
            if trimmed.is_empty() {
                bail!(
                    "task {} verify command cannot be empty; pass a command, e.g. `maestro task set {} --verify-command \"cargo test --test foo\"`, or clear it with `--clear-verify-command`",
                    handle.task().id,
                    handle.task().id
                );
            }
            Some(trimmed.to_string())
        }
        None => None,
    };
    if handle.task().verify_command == command {
        return Ok(handle.task().clone());
    }
    let mut task = handle.task().clone();
    task.verify_command = command;
    template::save_task_with_snapshot(&task, &handle.snapshot)?;
    Ok(task)
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
    if matches!(snapshot, template::TaskSnapshot::Progress(_)) {
        if task.feature_id == feature {
            return Ok(task);
        }
        bail!(
            "task {} lives inside progress.yml; lift it into a card-backed task before attaching it to a feature or card",
            task.id
        );
    }
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
    if !task.state.is_live() {
        bail!(
            "cannot update task {} — done (state: {}); use `maestro task note {}` for historical context",
            task.id,
            task.state.as_str(),
            task.id
        );
    }
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
            BlockerTarget::from_ref(&paths, Some("task-001".to_string())).expect("task ref"),
            BlockerTarget::Task("task-001".to_string())
        );
        assert_eq!(
            BlockerTarget::from_ref(&paths, Some("decision-007".to_string()))
                .expect("decision ref"),
            BlockerTarget::Decision("decision-007".to_string())
        );
        assert_eq!(
            BlockerTarget::from_ref(&paths, Some("PR #42".to_string())).expect("external ref"),
            BlockerTarget::External("PR #42".to_string())
        );
        assert_eq!(
            BlockerTarget::from_ref(&paths, None).expect("human ref"),
            BlockerTarget::Human
        );
    }

    /// A `--by` naming a feature card must error toward `dep add`, not silently
    /// classify as an External blocker the card graph can never resolve.
    #[test]
    fn blocker_target_refuses_a_feature_card_ref() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "maestro-from-ref-feature-{}-{nanos}",
            std::process::id()
        ));
        let paths = MaestroPaths::new(&root);
        let path = crate::domain::card::store::card_path(&paths, "csv-export");
        let snapshot = crate::domain::card::store::load_with_snapshot(&path).expect("snapshot");
        let card = crate::domain::card::schema::Card::new(
            "csv-export",
            crate::domain::card::schema::CardType::Feature,
            "CSV export",
            "open",
            "2026-06-10T00:00:00Z",
        );
        crate::domain::card::store::save_with_snapshot(&path, &card, &snapshot).expect("save");

        let error = BlockerTarget::from_ref(&paths, Some("csv-export".to_string()))
            .expect_err("a feature ref is not a blocker target");
        let message = format!("{error:#}");
        assert!(
            message.contains("maestro card dep add") && message.contains("feature"),
            "error routes to the dep edge: {message}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A store read failure propagates instead of silently misfiling a tracked
    /// card ref as External; a ref that cannot be a card id (a URL) never
    /// consults the store and stays free-form external.
    #[test]
    fn blocker_target_propagates_store_errors_and_keeps_freeform_refs() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "maestro-from-ref-error-{}-{nanos}",
            std::process::id()
        ));
        let paths = MaestroPaths::new(&root);
        let path = crate::domain::card::store::card_path(&paths, "card-bad001");
        std::fs::create_dir_all(path.parent().expect("card path has a dir"))
            .expect("create the card dir");
        std::fs::write(&path, "title: [unclosed").expect("plant the corrupt card");

        let error = BlockerTarget::from_ref(&paths, Some("card-bad001".to_string()))
            .expect_err("an unreadable card must surface, not misfile as External");
        assert!(
            format!("{error:#}").contains("failed to parse"),
            "{error:#}"
        );

        assert_eq!(
            BlockerTarget::from_ref(&paths, Some("https://github.com/acme/pull/42".to_string()))
                .expect("free-form ref"),
            BlockerTarget::External("https://github.com/acme/pull/42".to_string())
        );
        let _ = std::fs::remove_dir_all(&root);
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

        let missing = missing_verify_contract_ids(&paths, &[unchecked, checked, linked, settled])
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
        let paths = card_mode_repo("detach");
        let tasks_dir = paths.tasks_dir();

        let feature_id = crate::domain::feature::create(&paths, "Csv export", None)
            .expect("create feature card");
        let task = create_task(
            &tasks_dir,
            "Add CSV export",
            CreateTaskOptions {
                feature: Some(feature_id.clone()),
                covers: Vec::new(),
                lane: None,
                risk: None,
                checks: Vec::new(),
                project: None,
                created_at: "2026-06-09T12:00:00Z".to_string(),
            },
        )
        .expect("create task card with a feature parent");
        assert!(
            task.id.starts_with("task-add-csv-export-"),
            "card-mode id: {}",
            task.id
        );

        let counts =
            crate::domain::feature::query::count_tasks_by_feature(&tasks_dir).expect("count");
        assert_eq!(counts.get(&feature_id).map(|c| c.total), Some(1));

        set_feature(
            &tasks_dir,
            &task.id,
            None,
            "maestro",
            "2026-06-09T12:00:00Z",
        )
        .expect("detach the feature");

        let card = crate::domain::card::store::resolve(&paths, &task.id)
            .expect("card resolvable")
            .expect("card present")
            .card;
        assert_eq!(card.parent, None, "the parent field is cleared in place");

        let counts =
            crate::domain::feature::query::count_tasks_by_feature(&tasks_dir).expect("recount");
        assert_eq!(
            counts.get(&feature_id),
            None,
            "the detached task no longer counts toward the feature"
        );

        let _ = std::fs::remove_dir_all(paths.maestro_dir());
    }
}
