//! Feature store + guarded lifecycle operations.
//!
//! Each feature lives in its own directory `.maestro/features/<id>/` with a
//! `feature.yaml` record (the source of truth) and an append-only
//! `amend-log.yaml` audit trail. There is no flat registry index; reads scan
//! the features directory, faithfully mirroring the task store.
//!
//! Reads come in two flavours:
//!
//! - the **strict** scan ([`list`], [`show`], the verb ops, [`diagnose`]) errors
//!   on a malformed or schema-incompatible record, because those paths are
//!   authoritative,
//! - the **tolerant** scan ([`titles`]) skips bad records so a live display such
//!   as the TUI never hard-fails.
//!
//! The state machine is guarded: every state-changing verb routes through
//! [`legal_transition`] (the §3.2 transition table) and the gated verbs
//! (`accept`, `close`) layer their preconditions on top, emitting actionable
//! errors that name the gap and the fix command.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use crate::domain::card::archive_db;
use crate::domain::card::fold;
use crate::domain::card::query as card_query;
use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self as card_store, CardSnapshot};
use crate::domain::decisions;
use crate::domain::decisions::schema::{DecisionRecord, DecisionStatus};
use crate::domain::feature::query::{
    FeatureTaskCounts, count_tasks_by_feature, count_tasks_by_feature_in_entries,
    count_tasks_for_feature, count_tasks_for_feature_in_entries, live_child_task_ids,
    live_child_task_ids_in_entries,
};
use crate::domain::feature::schema::{
    AmendAdditions, AmendEntry, AmendLog, FeatureRecord, FeatureStatus, QaDeclaration,
    normalize_acceptance_id,
};
use crate::domain::feature::verification;
use crate::domain::feature::{qa, worktree};
use crate::domain::task::{self, TaskEntry, TaskState, TransitionDetails};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::{append_text_file, read_to_string_if_exists};
use crate::foundation::core::git;
use crate::foundation::core::hash::sha256_hex;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::FEATURE_SCHEMA_VERSION;
use crate::foundation::core::slug::slugify_ascii;
use crate::foundation::core::time::utc_now_timestamp;

const HANDOFF_FILE: &str = "handoff.md";
const HANDOFF_VERSION: &str = "1";
const HANDOFF_VERSION_MARKER: &str = "<!-- maestro:feature-handoff-version: ";
const HANDOFF_HASH_MARKER: &str = "<!-- maestro:feature-handoff-source-sha256: ";
const HANDOFF_GENERATED_MARKER: &str = "<!-- maestro:feature-handoff-generated-at: ";

/// A feature joined with its non-persisted task counts, ready for display.
///
/// The counts are computed on demand from `.maestro/tasks/**/task.yaml`,
/// preserving the invariant that task counts are not stored on the record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureView {
    /// Stable feature id.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Feature lifecycle status.
    pub status: FeatureStatus,
    /// Tasks that reference this feature, computed on read.
    pub counts: FeatureTaskCounts,
    /// Creation timestamp string.
    pub created_at: String,
    /// Last update timestamp string.
    pub updated_at: String,
    /// Optional feature description.
    pub description: Option<String>,
    /// Optional raw request that led to this feature.
    pub raw_request: Option<String>,
    /// Optional input type such as bug_report or refactor.
    pub input_type: Option<String>,
    /// Acceptance criteria (the product contract).
    pub acceptance: Vec<String>,
    /// Acceptance criteria joined with covering tasks, populated on show paths.
    pub acceptance_coverage: Option<Vec<verification::AcceptanceCoverage>>,
    /// Affected surfaces.
    pub affected_areas: Vec<String>,
    /// Explicit non-goals.
    pub non_goals: Vec<String>,
    /// Open questions (non-blocking).
    pub open_questions: Vec<String>,
    /// One-line closed outcome, set at `close --outcome`.
    pub outcome: Option<String>,
    /// Operator reason recorded at `cancel --reason`.
    pub cancel_reason: Option<String>,
    /// Reason for an explicit `qa: none` declaration.
    pub qa_none_reason: Option<String>,
    /// Design notes (`notes.md`), read on demand by `show`. None elsewhere.
    pub notes: Option<String>,
    /// Project/service scope carried on the underlying card base. Read-time
    /// projection: the folded `FeatureRecord` does not persist it, so the loader
    /// captures it from the card before folding.
    pub project: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeatureRosterEntry {
    Loaded(Box<FeatureView>),
    Unreadable {
        id: String,
        path: PathBuf,
        error: String,
        hint: Option<String>,
        typed_error: Option<MaestroError>,
    },
}

/// Found-vs-expected schema diagnostic for the feature store, reported as data
/// for `maestro doctor` rather than as an error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureDiagnostic {
    /// The schema version this binary expects for feature records.
    pub expected: &'static str,
    /// `Ok(feature_count)` when the features dir scans and every record is
    /// schema-compatible; `Err(message)` when the dir is absent or a record is
    /// unparseable / schema-incompatible.
    pub found: Result<usize, String>,
}

/// Declarative replace-per-field edits applied by `feature set` (Proposed only).
///
/// `Some` replaces that field; `None` leaves it untouched. List fields replace
/// the whole list; scalar fields replace the value.
#[derive(Clone, Debug, Default)]
pub struct ContractEdits {
    /// Replacement acceptance criteria.
    pub acceptance: Option<Vec<String>>,
    /// Replacement affected areas.
    pub affected_areas: Option<Vec<String>>,
    /// Replacement non-goals.
    pub non_goals: Option<Vec<String>>,
    /// Replacement open questions.
    pub open_questions: Option<Vec<String>>,
    /// Replacement description.
    pub description: Option<String>,
    /// Replacement raw request.
    pub raw_request: Option<String>,
    /// Replacement input type.
    pub input_type: Option<String>,
    /// Proposed-stage acceptance additions.
    pub add_acceptance: Vec<String>,
    /// Proposed-stage affected-area additions.
    pub add_affected_areas: Vec<String>,
    /// Proposed-stage non-goal additions.
    pub add_non_goals: Vec<String>,
    /// Proposed-stage open-question additions.
    pub add_open_questions: Vec<String>,
    /// Proposed-stage in-place acceptance text edits.
    pub edit_acceptance: Vec<AcceptanceTextEdit>,
}

impl ContractEdits {
    /// True when no field is set (a zero-flag `set`, which the CLI rejects).
    pub fn is_empty(&self) -> bool {
        self.acceptance.is_none()
            && self.affected_areas.is_none()
            && self.non_goals.is_none()
            && self.open_questions.is_none()
            && self.description.is_none()
            && self.raw_request.is_none()
            && self.input_type.is_none()
            && self.add_acceptance.is_empty()
            && self.add_affected_areas.is_empty()
            && self.add_non_goals.is_empty()
            && self.add_open_questions.is_empty()
            && self.edit_acceptance.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceTextEdit {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContractChangeCounts {
    pub acceptance: usize,
    pub affected_areas: usize,
    pub non_goals: usize,
    pub open_questions: usize,
    pub description: usize,
    pub raw_request: usize,
    pub input_type: usize,
}

impl ContractChangeCounts {
    pub fn is_empty(&self) -> bool {
        self.acceptance == 0
            && self.affected_areas == 0
            && self.non_goals == 0
            && self.open_questions == 0
            && self.description == 0
            && self.raw_request == 0
            && self.input_type == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetReport {
    pub view: FeatureView,
    pub replaced: ContractChangeCounts,
    pub added: ContractChangeCounts,
    pub edited_acceptance: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizeReport {
    pub id: String,
    pub path: PathBuf,
    pub fingerprint: String,
    pub generated_at: String,
    pub next_commands: Vec<String>,
}

/// Append-only additions applied by `feature amend` (Ready / InProgress).
#[derive(Clone, Debug, Default)]
pub struct ContractAdditions {
    /// Acceptance criteria to add.
    pub acceptance: Vec<String>,
    /// Affected areas to add.
    pub affected_areas: Vec<String>,
    /// Non-goals to add.
    pub non_goals: Vec<String>,
    /// Open questions to add.
    pub open_questions: Vec<String>,
}

impl ContractAdditions {
    /// True when no add-flag was supplied (which the CLI rejects).
    pub fn is_empty(&self) -> bool {
        self.acceptance.is_empty()
            && self.affected_areas.is_empty()
            && self.non_goals.is_empty()
            && self.open_questions.is_empty()
    }
}

/// Outcome of a state-changing verb, ready for the CLI to render.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionReport {
    /// Feature id.
    pub id: String,
    /// Resulting status (the current status when `changed` is false).
    pub status: FeatureStatus,
    /// Whether a write actually happened (false for no-ops and `--dry-run`).
    pub changed: bool,
    /// Human-readable summary line.
    pub note: String,
}

struct CloseGateReport {
    gaps: Vec<String>,
    qa_declared_none: bool,
    baseline: Option<qa::Baseline>,
}

/// Outcome of `feature cancel`, including which child tasks were abandoned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelReport {
    /// Feature id.
    pub id: String,
    /// Whether the feature was actually cancelled (false for an already-cancelled no-op).
    pub changed: bool,
    /// Ids of the child tasks abandoned by the cascade.
    pub abandoned: Vec<String>,
    /// Human-readable summary line.
    pub note: String,
}

/// Outcome of `feature amend`, including the genuinely-new values added.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmendReport {
    /// Feature id.
    pub id: String,
    /// Whether anything new was added (false when every value was already present).
    pub changed: bool,
    /// The post-dedup values added by this amend.
    pub added: AmendAdditions,
    /// Human-readable summary line.
    pub note: String,
}

/// Result of appending a feature note.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoteReport {
    /// Feature id.
    pub id: String,
    /// Whether `notes.md` was created by this append.
    pub created: bool,
    /// The exact dated line appended to `notes.md`.
    pub line: String,
}

/// Create a feature from a title, generating a slug id and persisting it.
///
/// # Errors
///
/// Errors when the title has no ASCII slug content, when a feature with the
/// generated id already exists, or when the record cannot be written.
pub fn create(paths: &MaestroPaths, title: &str, project: Option<String>) -> Result<String> {
    let id = slugify_ascii(title);
    if id.is_empty() {
        bail!("feature title must contain at least one ASCII letter or digit");
    }
    if live_record_exists(paths, &id) {
        bail!("feature {id} already exists");
    }
    // L6a: an archived feature still owns its slug — refuse to reissue it.
    if archived_card_path(paths, &id).is_file() || archive_db::contains_card_id(paths, &id)? {
        bail!(
            "feature {id} already exists in the archive; `maestro feature unarchive {id}` or choose a different title"
        );
    }
    let record = FeatureRecord::proposed(&id, title, &utc_now_timestamp());
    save_new_record(paths, &record, project)?;
    scaffold_spec_file(paths, &id, title)?;
    Ok(id)
}

/// Refuse a blank contract value so a vacuous `[""]` cannot satisfy the accept
/// gate (which checks list length only) while carrying no real contract,
/// matching the empty-value guards on the sibling task verbs.
fn ensure_no_blank_values(field: &str, values: &[String]) -> Result<()> {
    if values.iter().any(|value| value.trim().is_empty()) {
        bail!("feature {field} values must not be empty or whitespace");
    }
    Ok(())
}

/// Author a Proposed feature's contract (declarative replace-per-field).
///
/// # Errors
///
/// Errors when the feature is not found or its contract is frozen (status is
/// past `Proposed`).
pub fn set(paths: &MaestroPaths, id: &str, edits: ContractEdits) -> Result<FeatureView> {
    Ok(set_with_report(paths, id, edits)?.view)
}

pub fn set_with_report(paths: &MaestroPaths, id: &str, edits: ContractEdits) -> Result<SetReport> {
    let (mut record, write) = load_record_for_update(paths, id)?;
    match record.status {
        FeatureStatus::Proposed => {}
        // Recommend amend only where it actually works (Ready / InProgress);
        // on a terminal feature amend dead-ends too, so don't send the user
        // down a path with no exit.
        FeatureStatus::Ready | FeatureStatus::InProgress => bail!(
            "cannot edit {id} — contract frozen at accept (status: {}); grow it with `maestro feature amend {id} --add-acceptance \"…\" --reason \"…\"`",
            record.status.as_str()
        ),
        FeatureStatus::Closed | FeatureStatus::Cancelled => bail!(
            "cannot edit {id} — terminal (status: {})",
            record.status.as_str()
        ),
    }
    for (field, values) in [
        ("acceptance", edits.acceptance.as_deref()),
        ("affected_areas", edits.affected_areas.as_deref()),
        ("non_goals", edits.non_goals.as_deref()),
        ("open_questions", edits.open_questions.as_deref()),
        ("acceptance", Some(edits.add_acceptance.as_slice())),
        ("affected_areas", Some(edits.add_affected_areas.as_slice())),
        ("non_goals", Some(edits.add_non_goals.as_slice())),
        ("open_questions", Some(edits.add_open_questions.as_slice())),
    ] {
        if let Some(values) = values {
            ensure_no_blank_values(field, values)?;
        }
    }
    for edit in &edits.edit_acceptance {
        if edit.text.trim().is_empty() {
            bail!("feature acceptance values must not be empty or whitespace");
        }
    }
    let mut replaced = ContractChangeCounts::default();
    let mut added = ContractChangeCounts::default();
    if let Some(value) = edits.acceptance {
        replaced.acceptance = value.len();
        record.acceptance = value;
    }
    if let Some(value) = edits.affected_areas {
        replaced.affected_areas = value.len();
        record.affected_areas = value;
    }
    if let Some(value) = edits.non_goals {
        replaced.non_goals = value.len();
        record.non_goals = value;
    }
    if let Some(value) = edits.open_questions {
        replaced.open_questions = value.len();
        record.open_questions = value;
    }
    if let Some(value) = edits.description {
        replaced.description = 1;
        record.description = Some(value);
    }
    if let Some(value) = edits.raw_request {
        replaced.raw_request = 1;
        record.raw_request = Some(value);
    }
    if let Some(value) = edits.input_type {
        replaced.input_type = 1;
        record.input_type = Some(value);
    }
    let acceptance = dedup_new(&record.acceptance, &edits.add_acceptance);
    added.acceptance = acceptance.len();
    record.acceptance.extend(acceptance);
    let affected_areas = dedup_new(&record.affected_areas, &edits.add_affected_areas);
    added.affected_areas = affected_areas.len();
    record.affected_areas.extend(affected_areas);
    let non_goals = dedup_new(&record.non_goals, &edits.add_non_goals);
    added.non_goals = non_goals.len();
    record.non_goals.extend(non_goals);
    let open_questions = dedup_new(&record.open_questions, &edits.add_open_questions);
    added.open_questions = open_questions.len();
    record.open_questions.extend(open_questions);
    apply_acceptance_text_edits(id, &mut record.acceptance, &edits.edit_acceptance)?;
    record.updated_at = utc_now_timestamp();
    save_record(paths, &record, &write)?;
    let counts = count_tasks_for_feature(&paths.tasks_dir(), &record.id)?;
    Ok(SetReport {
        view: view_from_record(record, counts, None),
        replaced,
        added,
        edited_acceptance: edits.edit_acceptance.len(),
    })
}

pub fn finalize(paths: &MaestroPaths, id: &str) -> Result<FinalizeReport> {
    let record = load_record(paths, id)?;
    match record.status {
        FeatureStatus::Proposed | FeatureStatus::Ready | FeatureStatus::InProgress => {}
        FeatureStatus::Closed | FeatureStatus::Cancelled => bail!(
            "cannot finalize {id} — terminal (status: {})",
            record.status.as_str()
        ),
    }

    let sources = handoff_sources(paths, &record)?;
    let generated_at = utc_now_timestamp();
    let next_commands = handoff_next_commands(paths, &record)?;
    let worktree_statuses = worktree::lane_statuses(paths, &record.id)?;
    let contents = render_handoff(
        &record,
        &sources,
        &generated_at,
        &next_commands,
        &worktree_statuses,
    );
    let path = feature_sidecar_dir(paths, &record.id).join(HANDOFF_FILE);
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(FinalizeReport {
        id: record.id,
        path,
        fingerprint: sources.fingerprint,
        generated_at,
        next_commands,
    })
}

pub fn handoff_gap(paths: &MaestroPaths, id: &str) -> Result<Option<String>> {
    let record = load_record(paths, id)?;
    handoff_gap_for_record(paths, &record)
}

fn handoff_gap_for_record(paths: &MaestroPaths, record: &FeatureRecord) -> Result<Option<String>> {
    let sources = handoff_sources(paths, record)?;
    let path = feature_sidecar_dir(paths, &record.id).join(HANDOFF_FILE);
    let Some(contents) = read_to_string_if_exists(&path)? else {
        return Ok(Some(handoff_gate_gap(
            &record.id,
            "missing",
            ".maestro/cards/<id>/handoff.md does not exist",
        )));
    };
    let Some(found) = handoff_source_fingerprint_from_contents(&contents) else {
        return Ok(Some(handoff_gate_gap(
            &record.id,
            "stale",
            ".maestro/cards/<id>/handoff.md has no source fingerprint",
        )));
    };
    if found == sources.fingerprint {
        return Ok(None);
    }
    Ok(Some(handoff_gate_gap(
        &record.id,
        "stale",
        "design source fingerprint changed",
    )))
}

fn handoff_gate_gap(id: &str, state: &str, detail: &str) -> String {
    format!(
        "handoff (.maestro/cards/{id}/handoff.md {state}: {detail})\n    fix: maestro feature finalize {id}"
    )
}

struct HandoffSources {
    spec: Option<String>,
    notes: Option<String>,
    worktree_ledger: Option<String>,
    decisions: Vec<DecisionRecord>,
    fingerprint: String,
}

fn handoff_sources(paths: &MaestroPaths, record: &FeatureRecord) -> Result<HandoffSources> {
    let sidecar_dir = feature_sidecar_dir(paths, &record.id);
    let spec = read_to_string_if_exists(sidecar_dir.join("spec.md"))?;
    let notes = read_to_string_if_exists(sidecar_dir.join("notes.md"))?;
    let worktree_ledger = read_to_string_if_exists(worktree::ledger_path(paths, &record.id)?)?;
    let decisions = decisions::decisions_for_feature(paths, &record.id)?;
    let fingerprint = handoff_source_fingerprint(
        record,
        spec.as_deref(),
        notes.as_deref(),
        worktree_ledger.as_deref(),
        &decisions,
    );
    Ok(HandoffSources {
        spec,
        notes,
        worktree_ledger,
        decisions,
        fingerprint,
    })
}

fn handoff_source_fingerprint(
    record: &FeatureRecord,
    spec: Option<&str>,
    notes: Option<&str>,
    worktree_ledger: Option<&str>,
    decisions: &[DecisionRecord],
) -> String {
    let mut source = String::new();
    push_source_field(&mut source, "format", HANDOFF_VERSION);
    push_source_field(&mut source, "id", &record.id);
    push_source_field(&mut source, "title", &record.title);
    push_source_optional_field(&mut source, "description", record.description.as_deref());
    push_source_optional_field(&mut source, "raw_request", record.raw_request.as_deref());
    push_source_optional_field(&mut source, "input_type", record.input_type.as_deref());
    push_source_list(&mut source, "acceptance", &record.acceptance);
    push_source_list(&mut source, "affected_areas", &record.affected_areas);
    push_source_list(&mut source, "non_goals", &record.non_goals);
    push_source_list(&mut source, "open_questions", &record.open_questions);
    push_source_optional_field(&mut source, "spec.md", spec);
    push_source_optional_field(&mut source, "notes.md", notes);
    push_source_optional_field(&mut source, "worktree.yml", worktree_ledger);
    source.push_str("decisions\n");
    for decision in decisions {
        push_source_field(&mut source, "decision.id", &decision.id);
        push_source_field(&mut source, "decision.title", &decision.title);
        push_source_field(&mut source, "decision.status", decision.status.as_str());
        push_source_optional_field(&mut source, "decision.context", decision.context.as_deref());
        push_source_optional_field(
            &mut source,
            "decision.decision",
            decision.decision.as_deref(),
        );
        push_source_list(&mut source, "decision.rejected", &decision.rejected);
        push_source_optional_field(&mut source, "decision.preview", decision.preview.as_deref());
        push_source_list(&mut source, "decision.supersedes", &decision.supersedes);
        push_source_optional_field(
            &mut source,
            "decision.superseded_by",
            decision.superseded_by.as_deref(),
        );
        push_source_field(&mut source, "decision.created_at", &decision.created_at);
        push_source_optional_field(
            &mut source,
            "decision.locked_at",
            decision.locked_at.as_deref(),
        );
    }
    sha256_hex(source.as_bytes())
}

fn push_source_field(out: &mut String, name: &str, value: &str) {
    out.push_str(name);
    out.push('\0');
    out.push_str(&value.len().to_string());
    out.push('\0');
    out.push_str(value);
    out.push('\n');
}

fn push_source_optional_field(out: &mut String, name: &str, value: Option<&str>) {
    match value {
        Some(value) => push_source_field(out, name, value),
        None => push_source_field(out, name, "<missing>"),
    }
}

fn push_source_list(out: &mut String, name: &str, values: &[String]) {
    push_source_field(out, &format!("{name}.len"), &values.len().to_string());
    for value in values {
        push_source_field(out, name, value);
    }
}

fn handoff_source_fingerprint_from_contents(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let value = line.strip_prefix(HANDOFF_HASH_MARKER)?;
        value.strip_suffix(" -->").map(str::to_string)
    })
}

fn handoff_next_commands(paths: &MaestroPaths, record: &FeatureRecord) -> Result<Vec<String>> {
    let commands = match record.status {
        FeatureStatus::Proposed
            if qa::baseline_present(&feature_sidecar_dir(paths, &record.id))? =>
        {
            vec![format!("maestro feature accept {}", record.id)]
        }
        FeatureStatus::Proposed => vec![
            format!(
                "maestro qa baseline {} --observed \"<current behavior>\"",
                record.id
            ),
            format!("maestro feature accept {}", record.id),
        ],
        FeatureStatus::Ready => vec![format!("maestro feature prepare {} --draft", record.id)],
        FeatureStatus::InProgress => vec![format!("maestro feature verify {}", record.id)],
        FeatureStatus::Closed | FeatureStatus::Cancelled => {
            vec![format!("maestro card archive {}", record.id)]
        }
    };
    Ok(commands)
}

fn render_handoff(
    record: &FeatureRecord,
    sources: &HandoffSources,
    generated_at: &str,
    next_commands: &[String],
    worktree_statuses: &[worktree::WorktreeLaneStatus],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("{HANDOFF_VERSION_MARKER}{HANDOFF_VERSION} -->\n"));
    out.push_str(&format!(
        "{HANDOFF_HASH_MARKER}{} -->\n",
        sources.fingerprint
    ));
    out.push_str(&format!("{HANDOFF_GENERATED_MARKER}{generated_at} -->\n\n"));
    out.push_str(&format!("# {} Handoff\n\n", record.title));
    out.push_str("## Status\n\n");
    out.push_str(&format!("- Feature: `{}`\n", record.id));
    out.push_str(&format!("- Status: `{}`\n", record.status.as_str()));
    out.push_str(&format!("- Generated: `{generated_at}`\n\n"));

    out.push_str("## Continue\n\n");
    for command in next_commands {
        out.push_str(&format!("- `{command}`\n"));
    }
    out.push_str("\n## Locked Decisions\n\n");
    let locked = sources
        .decisions
        .iter()
        .filter(|decision| decision.status == DecisionStatus::Locked)
        .collect::<Vec<_>>();
    if locked.is_empty() {
        out.push_str("- None recorded.\n");
    } else {
        for decision in locked {
            if let Some(answer) = decision.decision.as_deref() {
                out.push_str(&format!(
                    "- `{}`: {} — {}\n",
                    decision.id, decision.title, answer
                ));
            } else {
                out.push_str(&format!("- `{}`: {}\n", decision.id, decision.title));
            }
        }
    }

    out.push_str("\n## Open Questions And Blockers\n\n");
    let mut wrote_blocker = false;
    for question in &record.open_questions {
        out.push_str(&format!("- Open question: {question}\n"));
        wrote_blocker = true;
    }
    for decision in sources
        .decisions
        .iter()
        .filter(|decision| decision.status == DecisionStatus::Open)
    {
        out.push_str(&format!(
            "- Open decision `{}`: {}\n",
            decision.id, decision.title
        ));
        wrote_blocker = true;
    }
    if !wrote_blocker {
        out.push_str("- None recorded.\n");
    }

    out.push_str("\n## Acceptance Criteria\n\n");
    if record.acceptance.is_empty() {
        out.push_str("- None recorded.\n");
    } else {
        for (index, item) in record.acceptance.iter().enumerate() {
            out.push_str(&format!(
                "- `{}`: {item}\n",
                verification::acceptance_id(index)
            ));
        }
    }

    out.push_str("\n## Affected Areas\n\n");
    push_handoff_list(&mut out, &record.affected_areas);

    out.push_str("\n## Non-Goals\n\n");
    push_handoff_list(&mut out, &record.non_goals);

    push_handoff_worktrees(&mut out, worktree_statuses);

    out.push_str("\n## Audit Trail\n\n");
    out.push_str(&format!("- Spec: `.maestro/cards/{}/spec.md`", record.id));
    if sources.spec.is_none() {
        out.push_str(" (missing)");
    }
    out.push('\n');
    out.push_str(&format!("- Notes: `.maestro/cards/{}/notes.md`", record.id));
    if sources.notes.is_none() {
        out.push_str(" (missing)");
    }
    out.push('\n');
    out.push_str(&format!(
        "- Worktree ledger: `.maestro/cards/{}/worktree.yml`",
        record.id
    ));
    if sources.worktree_ledger.is_none() {
        out.push_str(" (missing)");
    }
    out.push('\n');
    out.push_str(&format!(
        "- Decisions: `maestro decision list --feature {}`\n",
        record.id
    ));
    for decision in &sources.decisions {
        out.push_str(&format!(
            "  - `{}`: `.maestro/cards/{}/card.yaml`\n",
            decision.id, decision.id
        ));
    }
    out
}

fn push_handoff_worktrees(out: &mut String, lanes: &[worktree::WorktreeLaneStatus]) {
    if lanes.is_empty() {
        return;
    }
    out.push_str("\n## Worktree Ledger\n\n");
    for lane in lanes {
        out.push_str(&format!(
            "- Lane `{}`: `{}`\n",
            lane.slug,
            lane.state.as_str()
        ));
        out.push_str(&format!("  - Branch: `{}`\n", lane.intent.branch));
        out.push_str(&format!("  - Path: `{}`\n", lane.intent.path));
        out.push_str(&format!("  - Base: `{}`\n", lane.intent.base));
        if let Some(owner) = lane.intent.owner_checkout.as_deref() {
            out.push_str(&format!("  - Owner checkout: `{owner}`\n"));
        }
        if let Some(worker) = lane.intent.expected_worker_checkout.as_deref() {
            out.push_str(&format!("  - Expected worker checkout: `{worker}`\n"));
        }
        push_handoff_optional(
            out,
            "Branch reserved",
            lane.milestones.branch_reserved_at.as_deref(),
        );
        push_handoff_optional(
            out,
            "Lane created",
            lane.milestones.lane_created_at.as_deref(),
        );
        push_handoff_optional(
            out,
            "Merged back",
            lane.milestones.merged_back_at.as_deref(),
        );
        push_handoff_optional(
            out,
            "Merged back commit",
            lane.milestones.merged_back_commit.as_deref(),
        );
        push_handoff_optional(out, "Verified", lane.milestones.verified_at.as_deref());
        push_handoff_optional(
            out,
            "Verified commit",
            lane.milestones.verified_commit.as_deref(),
        );
        push_handoff_optional(
            out,
            "Cleanup due",
            lane.milestones.cleanup_due_at.as_deref(),
        );
        push_handoff_optional(
            out,
            "Cleanup completed",
            lane.milestones.cleanup_completed_at.as_deref(),
        );
        out.push_str(&format!(
            "  - Evidence: branch_exists={} path_exists={} worker_clean_or_absent={} active_owner={} open_conflict={}\n",
            lane.evidence.branch_exists,
            lane.evidence.path_exists,
            lane.evidence.worker_clean_or_absent,
            lane.evidence.active_owner,
            lane.evidence.open_conflict
        ));
        if !lane.cleanup_receipts.is_empty() {
            out.push_str("  - Cleanup receipts:\n");
            for receipt in &lane.cleanup_receipts {
                out.push_str(&format!(
                    "    - `{}` removed `{}` deleted `{}` pruned={} by `{}`\n",
                    receipt.recorded_at,
                    receipt.removed_path,
                    receipt.deleted_branch,
                    receipt.pruned_stale_metadata,
                    receipt.recorded_by
                ));
            }
        }
    }
}

fn push_handoff_optional(out: &mut String, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        out.push_str(&format!("  - {label}: `{value}`\n"));
    }
}

fn push_handoff_list(out: &mut String, values: &[String]) {
    if values.is_empty() {
        out.push_str("- None recorded.\n");
        return;
    }
    for value in values {
        out.push_str(&format!("- {value}\n"));
    }
}

/// Accept a feature: `Proposed → Ready`, gated on a complete contract (D2).
///
/// Phase A gate: `acceptance` and `affected_areas` both non-empty. (The
/// qa-baseline precondition F is layered in Phase B.) `--dry-run` previews the
/// gate at exit 0 without transitioning.
///
/// # Errors
///
/// Errors when the feature is not found, the source state is illegal for
/// `accept`, or (non-dry-run) the contract is incomplete.
pub fn accept(paths: &MaestroPaths, id: &str, dry_run: bool) -> Result<TransitionReport> {
    accept_inner(paths, id, None, dry_run)
}

pub fn accept_with_qa_none(
    paths: &MaestroPaths,
    id: &str,
    reason: &str,
    dry_run: bool,
) -> Result<TransitionReport> {
    if reason.trim().is_empty() {
        bail!("--reason is required with --qa none");
    }
    accept_inner(paths, id, Some(reason), dry_run)
}

fn accept_inner(
    paths: &MaestroPaths,
    id: &str,
    qa_none_reason: Option<&str>,
    dry_run: bool,
) -> Result<TransitionReport> {
    let (mut record, write) = load_record_for_update(paths, id)?;
    if let Some(reason) = qa_none_reason
        && matches!(
            record.status,
            FeatureStatus::Ready | FeatureStatus::InProgress
        )
    {
        let reason = reason.trim();
        if dry_run {
            return Ok(no_op_report(
                id,
                record.status,
                format!("would record qa: none for {id} ({reason})"),
            ));
        }
        record.qa = Some(QaDeclaration {
            surface: "none".to_string(),
            reason: reason.to_string(),
            amend_log_position: record.amends.len(),
        });
        record.updated_at = utc_now_timestamp();
        let status = record.status.clone();
        save_record(paths, &record, &write)?;
        return Ok(TransitionReport {
            id: id.to_string(),
            status,
            changed: true,
            note: format!("recorded qa: none for {id} ({reason})"),
        });
    }
    let target = match legal_transition(id, &record.status, FeatureVerb::Accept) {
        Transition::NoOp => {
            return Ok(no_op_report(
                id,
                record.status,
                format!("{id} is already ready"),
            ));
        }
        Transition::Illegal(message) => bail!(message),
        Transition::To(target) => target,
    };

    let mut gaps = Vec::new();
    if record.acceptance.is_empty() {
        gaps.push(format!(
            "acceptance (0 criteria) — fix: maestro feature set {id} --acceptance \"<criterion>\""
        ));
    }
    if record.affected_areas.is_empty() {
        gaps.push(format!(
            "affected_areas (0 areas) — fix: maestro feature set {id} --area \"<surface>\""
        ));
    }
    if let Some(gap) = handoff_gap_for_record(paths, &record)? {
        gaps.push(gap);
    }
    // F — a captured behavior baseline is a precondition of accept (before edits).
    let feat_dir = feature_sidecar_dir(paths, id);
    if qa_none_reason.is_none() && !qa::baseline_present(&feat_dir)? {
        gaps.push(format!(
                  "qa-baseline (.maestro/cards/{id}/qa.md {})\n    skill: maestro-card (qa-baseline)\n    target: .maestro/cards/{id}/qa.md\n    retry: maestro feature accept {id}\n    skip (no behavioral surface): maestro feature accept {id} --qa none --reason \"<why>\"",
                qa::baseline_absence(&feat_dir)
            ));
    }

    if dry_run {
        let note = if gaps.is_empty() {
            format!(
                "would accept {id} (-> ready); contract complete (acceptance={}, areas={}){}",
                record.acceptance.len(),
                record.affected_areas.len(),
                qa_none_reason
                    .map(|reason| format!("; qa: none ({})", reason.trim()))
                    .unwrap_or_default()
            )
        } else {
            format!(
                "would block accept {id} — contract incomplete:\n  {}",
                gaps.join("\n  ")
            )
        };
        return Ok(no_op_report(id, record.status, note));
    }

    if !gaps.is_empty() {
        bail!(
            "cannot accept {id} — contract incomplete:\n  {}",
            gaps.join("\n  ")
        );
    }

    let questions_note = if record.open_questions.is_empty() {
        String::new()
    } else {
        format!(
            "; note: {} open question(s) carried (non-blocking)",
            record.open_questions.len()
        )
    };
    let qa_note = qa_none_reason
        .map(|reason| format!("; qa: none ({})", reason.trim()))
        .unwrap_or_default();
    let summary = format!(
        "accepted {id} (-> ready); contract frozen (acceptance={}, areas={}){}{}",
        record.acceptance.len(),
        record.affected_areas.len(),
        qa_note,
        questions_note
    );
    record.status = target.clone();
    record.updated_at = utc_now_timestamp();
    if let Some(reason) = qa_none_reason {
        record.qa = Some(QaDeclaration {
            surface: "none".to_string(),
            reason: reason.trim().to_string(),
            amend_log_position: record.amends.len(),
        });
    }
    save_record(paths, &record, &write)?;
    Ok(TransitionReport {
        id: id.to_string(),
        status: target,
        changed: true,
        note: summary,
    })
}

/// Grow a frozen contract additively (append-only, audited). Ready / InProgress.
///
/// Re-adding a value already present is a no-op (safe retries). Only genuinely
/// new values are appended and recorded in `amend-log.yaml`.
///
/// # Errors
///
/// Errors when the feature is not found or its status forbids amend
/// (`Proposed` → use `set`; terminal → past growth).
pub fn amend(
    paths: &MaestroPaths,
    id: &str,
    additions: ContractAdditions,
    reason: &str,
) -> Result<AmendReport> {
    let (mut record, write) = load_record_for_update(paths, id)?;
    match record.status {
        FeatureStatus::Ready | FeatureStatus::InProgress => {}
        FeatureStatus::Proposed => bail!(
            "cannot amend {id} — not accepted; author the contract with `maestro feature set {id} --…` then `maestro feature accept {id}`"
        ),
        FeatureStatus::Closed | FeatureStatus::Cancelled => {
            bail!(
                "cannot amend {id} — terminal (status: {})",
                record.status.as_str()
            )
        }
    }

    for (field, values) in [
        ("acceptance", &additions.acceptance),
        ("affected_areas", &additions.affected_areas),
        ("non_goals", &additions.non_goals),
        ("open_questions", &additions.open_questions),
    ] {
        ensure_no_blank_values(field, values)?;
    }

    let added = AmendAdditions {
        acceptance: dedup_new(&record.acceptance, &additions.acceptance),
        affected_areas: dedup_new(&record.affected_areas, &additions.affected_areas),
        non_goals: dedup_new(&record.non_goals, &additions.non_goals),
        open_questions: dedup_new(&record.open_questions, &additions.open_questions),
    };

    if added.is_empty() {
        return Ok(AmendReport {
            id: id.to_string(),
            changed: false,
            added,
            note: format!("amend {id} — all values already present (no-op)"),
        });
    }

    record.acceptance.extend(added.acceptance.iter().cloned());
    record
        .affected_areas
        .extend(added.affected_areas.iter().cloned());
    record.non_goals.extend(added.non_goals.iter().cloned());
    record
        .open_questions
        .extend(added.open_questions.iter().cloned());
    let now = utc_now_timestamp();
    record.updated_at = now.clone();

    record.amends.push(AmendEntry {
        at: now,
        reason: reason.to_string(),
        added: added.clone(),
    });
    if !added.acceptance.is_empty() || !added.affected_areas.is_empty() {
        record.qa = None;
    }

    save_record(paths, &record, &write)?;

    let note = format!(
        "amended {id} (acceptance +{}, areas +{}, non_goals +{}, questions +{}); reason recorded",
        added.acceptance.len(),
        added.affected_areas.len(),
        added.non_goals.len(),
        added.open_questions.len()
    );
    Ok(AmendReport {
        id: id.to_string(),
        changed: true,
        added,
        note,
    })
}

/// Append one dated line to a feature's `notes.md`, creating it on first write.
pub fn note(paths: &MaestroPaths, id: &str, text: &str) -> Result<NoteReport> {
    let record = load_record(paths, id)?;
    if text.trim().is_empty() {
        bail!("feature note text cannot be empty");
    }
    let path = feature_sidecar_dir(paths, &record.id).join("notes.md");
    let append = append_note_file(&path, &record.title, text)?;
    Ok(NoteReport {
        id: record.id,
        created: append.created,
        line: append.line,
    })
}

/// Start work: `Ready → InProgress`.
///
/// # Errors
///
/// Errors when the feature is not found or the source state is illegal.
pub fn start(paths: &MaestroPaths, id: &str) -> Result<TransitionReport> {
    let (mut record, write) = load_record_for_update(paths, id)?;
    let target = match legal_transition(id, &record.status, FeatureVerb::Start) {
        Transition::NoOp => {
            return Ok(no_op_report(
                id,
                record.status,
                format!("{id} is already in_progress"),
            ));
        }
        Transition::Illegal(message) => bail!(message),
        Transition::To(target) => target,
    };
    record.status = target.clone();
    record.updated_at = utc_now_timestamp();
    save_record(paths, &record, &write)?;
    Ok(TransitionReport {
        id: id.to_string(),
        status: target,
        changed: true,
        note: format!("started {id} (-> in_progress)"),
    })
}

/// Close a feature: `InProgress → Closed`, gated (D5).
///
/// Phase A gate: condition 1 only — no LIVE child task
/// (`draft/exploring/ready/in_progress/needs_verification` block; `verified` and
/// terminal-settled do not). (Coverage E and the QA floor are layered in Phase
/// B.) `--dry-run` previews the gate at exit 0 without transitioning.
///
/// # Errors
///
/// Errors when the feature is not found, the source state is illegal, or
/// (non-dry-run) a live child task blocks close.
pub fn close(
    paths: &MaestroPaths,
    id: &str,
    outcome: Option<String>,
    dry_run: bool,
) -> Result<TransitionReport> {
    let (mut record, write) = load_record_for_update(paths, id)?;
    let target = match legal_transition(id, &record.status, FeatureVerb::Close) {
        Transition::NoOp => {
            // The outcome is set once, at close. On an already-closed no-op a new
            // `--outcome` cannot be recorded; say so rather than dropping it silently.
            let note = if outcome.is_some() {
                format!("{id} is already closed; --outcome not recorded (it is set once, at close)")
            } else {
                format!("{id} is already closed")
            };
            return Ok(no_op_report(id, record.status, note));
        }
        Transition::Illegal(message) => bail!(message),
        Transition::To(target) => target,
    };

    let gate = close_gaps_for_record(paths, id, &record)?;
    let gaps = gate.gaps;

    if dry_run {
        let note = if gaps.is_empty() {
            // gaps empty implies a baseline exists (a missing one is itself a gap); a
            // baseline with no scenarios cleared the gate by skipping, not proving.
            let qa = if gate.qa_declared_none {
                "qa: none"
            } else if gate
                .baseline
                .as_ref()
                .is_some_and(|b| b.scenario_ids.is_empty())
            {
                "qa-baseline skipped (no behavioral scenarios)"
            } else {
                "qa-baseline proven"
            };
            format!("would close {id} (-> closed); no live child tasks, {qa}")
        } else {
            format!("would block close {id}:\n  {}", gaps.join("\n  "))
        };
        return Ok(no_op_report(id, record.status, note));
    }

    if !gaps.is_empty() {
        bail!("cannot close {id}:\n  {}", gaps.join("\n  "));
    }

    record.status = target.clone();
    record.updated_at = utc_now_timestamp();
    if let Some(line) = outcome {
        record.outcome = Some(line);
    }
    save_record(paths, &record, &write)?;
    Ok(TransitionReport {
        id: id.to_string(),
        status: target,
        changed: true,
        note: format!("closed {id} (-> closed)"),
    })
}

pub fn close_gaps(paths: &MaestroPaths, id: &str) -> Result<Vec<String>> {
    let record = load_record(paths, id)?;
    Ok(close_gaps_for_record(paths, id, &record)?.gaps)
}

/// Sorted ids of the feature's verified child tasks whose recorded proof commit
/// no longer matches HEAD (dec-ac-7-final). Both commits come from `git::head`,
/// so a plain string compare matches the proof-staleness check. Empty outside a
/// real git repo (no HEAD to compare against) or when every proof is current.
///
/// The `feature close --dry-run` CLI renders this as a non-blocking `note:` line
/// inside its close preview; it never feeds `close_gaps_for_record`, so it cannot
/// turn a passing preview into a blocked one.
pub fn verified_child_commit_drift(paths: &MaestroPaths, feature_id: &str) -> Result<Vec<String>> {
    let Some(head) = git::head(paths.repo_root()).unwrap_or(None) else {
        return Ok(Vec::new());
    };
    let mut ids: Vec<String> = task::load_task_entries(&paths.tasks_dir())?
        .into_iter()
        .map(|entry| entry.task)
        .filter(|task| {
            task.feature_id.as_deref() == Some(feature_id) && task.state == TaskState::Verified
        })
        .filter(|task| {
            task.verification
                .verified_commit
                .as_deref()
                .is_some_and(|stored| stored != head)
        })
        .map(|task| task.id)
        .collect();
    ids.sort();
    Ok(ids)
}

fn close_gaps_for_record(
    paths: &MaestroPaths,
    id: &str,
    record: &FeatureRecord,
) -> Result<CloseGateReport> {
    let mut gaps = Vec::new();
    let task_entries = task::load_task_entries(&paths.tasks_dir())?;
    // D5 cond 1 -- no live child task may outlive its closed feature.
    let live = live_child_task_ids_in_entries(&task_entries, &record.id);
    if !live.is_empty() {
        gaps.push(format!(
            "{} live child task(s): {}\n    fix: verify or abandon them, then re-close",
            live.len(),
            live.join(", ")
        ));
    }
    // D5 cond 2/3 -- QA baseline present + fresh, every behavioral scenario proven.
    let feat_dir = feature_sidecar_dir(paths, id);
    let qa_declared_none = qa::qa_declared_none_fresh(record.qa.as_ref(), &record.amends);
    let mut baseline = None;
    if !qa_declared_none {
        baseline = qa::read_baseline(&feat_dir)?;
        let slices = qa::read_qa_slices(&feat_dir)?;
        let amend_log = AmendLog {
            entries: record.amends.clone(),
        };
        // Classify absent-vs-empty only when there is no usable baseline (the only path
        // that consumes the word); a present baseline skips the extra read.
        let absence = if baseline.is_none() {
            qa::baseline_absence(&feat_dir)
        } else {
            "missing"
        };
        gaps.extend(qa::close_qa_gaps(
            id,
            baseline.as_ref(),
            absence,
            &slices,
            &amend_log,
        ));
    }
    // D5 cond 4 -- the full feature acceptance contract must have a fresh
    // sweep run that resolved every ac-N item.
    if let Some(gap) = verification::acceptance_close_gap(record, &task_entries)? {
        gaps.push(gap);
    }
    Ok(CloseGateReport {
        gaps,
        qa_declared_none,
        baseline,
    })
}

/// Cancel a feature: non-terminal → `Cancelled`, cascading to live children (D6).
///
/// Every LIVE child task is abandoned (reason "feature cancelled: <reason>")
/// before the feature is flipped; verified / already-terminal children are
/// untouched and stay linked as history. The cascade is not transactional: if a
/// child abandon fails, it bails before the feature is cancelled.
///
/// # Errors
///
/// Errors when the feature is not found, it is already terminal (Closed), or a
/// child task cannot be abandoned.
pub fn cancel(paths: &MaestroPaths, id: &str, reason: &str, dry_run: bool) -> Result<CancelReport> {
    let (mut record, write) = load_record_for_update(paths, id)?;
    let target = match legal_transition(id, &record.status, FeatureVerb::Cancel) {
        Transition::NoOp => {
            return Ok(CancelReport {
                id: id.to_string(),
                changed: false,
                abandoned: Vec::new(),
                note: format!("{id} is already cancelled"),
            });
        }
        Transition::Illegal(message) => bail!(message),
        Transition::To(target) => target,
    };

    let live = live_child_task_ids(&paths.tasks_dir(), &record.id)?;

    // `--dry-run` previews exactly which child tasks the cascade would abandon,
    // mirroring accept/close/archive, before any irreversible mutation.
    if dry_run {
        let note = if live.is_empty() {
            format!("would cancel {id} (-> cancelled); no child tasks affected")
        } else {
            format!(
                "would cancel {id} (-> cancelled); would abandon {} child task(s): {}",
                live.len(),
                live.join(", ")
            )
        };
        return Ok(CancelReport {
            id: id.to_string(),
            changed: false,
            abandoned: live,
            note,
        });
    }

    let now = utc_now_timestamp();
    let summary = format!("feature cancelled: {reason}");
    for task_id in &live {
        task::transition_task(
            &paths.tasks_dir(),
            task_id,
            TaskState::Abandoned,
            "maestro",
            &now,
            TransitionDetails {
                summary: Some(summary.clone()),
                ..Default::default()
            },
        )
        .with_context(|| format!("failed to abandon child task {task_id} during cancel of {id}"))?;
    }

    record.status = target;
    record.cancel_reason = Some(reason.to_string());
    record.updated_at = utc_now_timestamp();
    save_record(paths, &record, &write)?;

    let note = if live.is_empty() {
        format!("cancelled {id}; no child tasks affected")
    } else {
        format!(
            "cancelled {id}; abandoned {} child task(s): {}",
            live.len(),
            live.join(", ")
        )
    };
    Ok(CancelReport {
        id: id.to_string(),
        changed: true,
        abandoned: live,
        note,
    })
}

/// List every feature joined with its on-demand task counts.
///
/// # Errors
///
/// Errors when a feature record is unparseable or schema-incompatible.
pub fn list(paths: &MaestroPaths) -> Result<Vec<FeatureView>> {
    let records = scan_records_strict(paths)?;
    let counts_by_feature = count_tasks_by_feature(&paths.tasks_dir())?;
    views_from_records(records, counts_by_feature)
}

/// [`list`] over an already-loaded task entry set, so query surfaces that need
/// task rows do not re-scan the same cards only to compute per-feature counts.
pub fn list_with_entries(
    paths: &MaestroPaths,
    task_entries: &[TaskEntry],
) -> Result<Vec<FeatureView>> {
    let records = scan_records_strict(paths)?;
    let counts_by_feature = count_tasks_by_feature_in_entries(task_entries);
    views_from_records(records, counts_by_feature)
}

fn views_from_records(
    records: Vec<FeatureRecord>,
    counts_by_feature: std::collections::HashMap<String, FeatureTaskCounts>,
) -> Result<Vec<FeatureView>> {
    Ok(records
        .into_iter()
        .map(|record| {
            let counts = counts_by_feature
                .get(&record.id)
                .cloned()
                .unwrap_or_default();
            view_from_record(record, counts, None)
        })
        .collect())
}

/// Roster of every feature card for `status` / `feature list`. A feature card
/// whose record is unparseable or schema-incompatible -- or whose card file
/// fails to load at all -- surfaces as `Unreadable` rather than being dropped,
/// so a single bad artifact never silently shrinks the board. Non-feature
/// cards are skipped.
pub fn list_tolerant(paths: &MaestroPaths) -> Vec<FeatureRosterEntry> {
    let task_entries = task::load_task_entries(&paths.tasks_dir()).unwrap_or_default();
    list_tolerant_with_entries(paths, &task_entries)
}

/// [`list_tolerant`] over an already-loaded task entry set, so a caller that
/// scanned the tasks for its own report (`status`) does not trigger a second
/// scan of the same cards for the per-feature counts.
pub fn list_tolerant_with_entries(
    paths: &MaestroPaths,
    task_entries: &[TaskEntry],
) -> Vec<FeatureRosterEntry> {
    let counts_by_feature = count_tasks_by_feature_in_entries(task_entries);
    let mut entries = Vec::new();
    for id in feature_card_ids(paths).unwrap_or_default() {
        let path = card_store::card_path(paths, &id);
        match card_store::load(&path) {
            Ok(Some(card)) if card.card_type == CardType::Feature => {
                // The card base carries the project scope (T4); capture it before
                // `record_from_card` folds the card into the project-less record.
                let project = card.project.clone();
                match record_from_card(card, path.display().to_string()) {
                    Ok(record) => {
                        let counts = counts_by_feature
                            .get(&record.id)
                            .cloned()
                            .unwrap_or_default();
                        entries.push(FeatureRosterEntry::Loaded(Box::new(view_from_record(
                            record, counts, project,
                        ))));
                    }
                    Err(error) => entries.push(unreadable_entry(id, path, &error)),
                }
            }
            Ok(_) => {}
            Err(error) => {
                // The card failed to parse, so its declared type is unknowable
                // from the typed load. Skip only when the raw text clearly
                // declares a non-feature type (that card's own surfaces report
                // it); anything else stays on the board as unreadable.
                if !raw_card_declares_non_feature(&path) {
                    entries.push(unreadable_entry(id, path, &error));
                }
            }
        }
    }
    entries
}

fn unreadable_entry(id: String, path: PathBuf, error: &anyhow::Error) -> FeatureRosterEntry {
    let typed_error = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<MaestroError>().cloned());
    let hint = typed_error.as_ref().and_then(MaestroError::hint);
    FeatureRosterEntry::Unreadable {
        id,
        path,
        error: format!("{error:#}"),
        hint,
        typed_error,
    }
}

/// Best-effort `type:` sniff of a card file that failed to load as a `Card`.
/// True only when the raw YAML clearly declares a non-feature type; an
/// unreadable file or an undeclared type counts as a possible feature.
fn raw_card_declares_non_feature(path: &Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    raw.lines().any(|line| {
        // Only a column-0 `type:` is the card's own field; an indented one is
        // a nested key (e.g. inside `extra`). The value side tolerates a
        // trailing comment and YAML quoting, so `type: "feature" # note`
        // still reads as a feature and surfaces as unreadable.
        line.strip_prefix("type:").is_some_and(|value| {
            let value = value
                .split('#')
                .next()
                .unwrap_or_default()
                .trim()
                .trim_matches(|ch| matches!(ch, '"' | '\''));
            !value.is_empty() && value != "feature"
        })
    })
}

/// Show one feature joined with its on-demand task counts.
///
/// # Errors
///
/// Errors when no feature has the given id, or the record is unparseable /
/// schema-incompatible.
pub fn show(paths: &MaestroPaths, id: &str) -> Result<FeatureView> {
    let record = load_record(paths, id)?;
    let task_entries = task::load_task_entries(&paths.tasks_dir())?;
    let counts = count_tasks_for_feature_in_entries(&task_entries, &record.id);
    let acceptance_coverage =
        verification::acceptance_coverage_for_record_in_entries(&record, &task_entries);
    let notes = read_notes_at(&feature_sidecar_dir(paths, id))?;
    let mut view = view_from_record(record, counts, None);
    view.acceptance_coverage = Some(acceptance_coverage);
    view.notes = notes;
    Ok(view)
}

/// Show one archived feature (L6b read-fallthrough), counting its archived
/// child tasks. An entangled child skipped by the cascade (§5.9 L6c) stays live,
/// so the count reads only the archive tree and may under-report it.
///
/// # Errors
///
/// Errors when no archived feature has the given id, or the record is
/// unparseable / schema-incompatible.
pub fn show_archived(paths: &MaestroPaths, id: &str) -> Result<FeatureView> {
    let record = load_archived_record(paths, id)?;
    let task_entries = task::load_archived_task_entries(paths)?;
    let counts = count_tasks_for_feature_in_entries(&task_entries, &record.id);
    let acceptance_coverage =
        verification::acceptance_coverage_for_record_in_entries(&record, &task_entries);
    let notes = archive_db::read_file(paths, id, "notes.md")?
        .map(String::from_utf8)
        .transpose()
        .context("archived notes.md is not UTF-8")?;
    let mut view = view_from_record(record, counts, None);
    view.acceptance_coverage = Some(acceptance_coverage);
    view.notes = notes;
    Ok(view)
}

/// Ensure a live feature id is valid and resolves to a compatible record.
pub fn ensure_exists(paths: &MaestroPaths, id: &str) -> Result<()> {
    load_record(paths, id).map(|_| ())
}

/// Current append-only amend-log length, for artifacts that record the behavior
/// position they cover.
pub fn amend_log_position(paths: &MaestroPaths, id: &str) -> Result<usize> {
    Ok(load_record(paths, id)?.amends.len())
}

/// List every archived feature joined with its archived task counts (L6b,
/// `feature list --all`).
///
/// # Errors
///
/// Errors when an archived feature record is unparseable or schema-incompatible.
pub fn list_archived(paths: &MaestroPaths) -> Result<Vec<FeatureView>> {
    let task_entries = task::load_archived_task_entries(paths)?;
    card_query::scan_archived(paths)?
        .into_iter()
        .filter(|card| card.card_type == CardType::Feature)
        .map(|card| {
            let artifact = archive_db::archive_db_file(paths)
                .join(&card.id)
                .join("card.yaml");
            let project = card.project.clone();
            let record = record_from_card(card, artifact.display().to_string())?;
            let counts = count_tasks_for_feature_in_entries(&task_entries, &record.id);
            Ok(view_from_record(record, counts, project))
        })
        .collect()
}

/// Scan-free id -> title map for display.
///
/// The one documented tolerant read: it skips a missing, unparseable, or
/// schema-incompatible record so live display paths never hard-fail. Display
/// only; never an authority for a gate or write.
pub fn titles(paths: &MaestroPaths) -> BTreeMap<String, String> {
    scan_records_tolerant(paths)
        .into_iter()
        .map(|record| (record.id, record.title))
        .collect()
}

/// Report the feature store's schema verdict as data for `maestro doctor`.
///
/// Never errors: an unparseable / schema-incompatible feature record is carried
/// in [`FeatureDiagnostic::found`] as `Err`. Counts `Feature`-typed cards from
/// the doctor's one shared store walk; envelope failures are reported centrally
/// there, so only a feature record that fails to convert lands here.
pub fn diagnose(cards: &[(Card, PathBuf)]) -> FeatureDiagnostic {
    let mut count = 0_usize;
    for (card, path) in cards {
        if card.card_type != CardType::Feature {
            continue;
        }
        if let Err(error) = record_from_card(card.clone(), path.display().to_string()) {
            return FeatureDiagnostic {
                expected: FEATURE_SCHEMA_VERSION,
                found: Err(format!("{error:#}")),
            };
        }
        count += 1;
    }
    FeatureDiagnostic {
        expected: FEATURE_SCHEMA_VERSION,
        found: Ok(count),
    }
}

/// The feature's current lifecycle status, loaded without the task, coverage,
/// and note joins `show` performs -- for callers that only branch on status.
pub fn status(paths: &MaestroPaths, id: &str) -> Result<FeatureStatus> {
    Ok(load_record(paths, id)?.status)
}

/// Render a [`FeatureStatus`] to its canonical snake_case label.
pub fn status_label(status: &FeatureStatus) -> &'static str {
    status.as_str()
}

/// The state-changing feature verbs (the rows of the §3.2 transition table).
#[derive(Clone, Copy, Debug)]
enum FeatureVerb {
    Accept,
    Start,
    Close,
    Cancel,
}

/// Pure legality verdict for a verb against a source state.
enum Transition {
    /// The transition is legal; proceed (gated verbs then check preconditions).
    To(FeatureStatus),
    /// Already in the target state; the caller returns a no-op at exit 0.
    NoOp,
    /// Illegal source state; the caller bails with this actionable message.
    Illegal(String),
}

fn legal_transition(id: &str, from: &FeatureStatus, verb: FeatureVerb) -> Transition {
    use FeatureStatus::{Cancelled, Closed, InProgress, Proposed, Ready};
    match (verb, from) {
        (FeatureVerb::Accept, Proposed) => Transition::To(Ready),
        (FeatureVerb::Accept, Ready) => Transition::NoOp,
        (FeatureVerb::Accept, InProgress) => Transition::Illegal(format!(
            "cannot accept {id} — already past accept (status: in_progress)"
        )),
        (FeatureVerb::Accept, Closed | Cancelled) => Transition::Illegal(format!(
            "cannot accept {id} — terminal (status: {})",
            from.as_str()
        )),

        (FeatureVerb::Start, Ready) => Transition::To(InProgress),
        (FeatureVerb::Start, InProgress) => Transition::NoOp,
        (FeatureVerb::Start, Proposed) => Transition::Illegal(format!(
            "cannot start {id} — not accepted; run `maestro feature accept {id}` first"
        )),
        (FeatureVerb::Start, Closed | Cancelled) => Transition::Illegal(format!(
            "cannot start {id} — terminal (status: {})",
            from.as_str()
        )),

        (FeatureVerb::Close, InProgress) => Transition::To(Closed),
        (FeatureVerb::Close, Closed) => Transition::NoOp,
        (FeatureVerb::Close, Proposed) => Transition::Illegal(format!(
            "cannot close {id} — not started; run `maestro feature accept {id}` then `maestro feature start {id}`"
        )),
        (FeatureVerb::Close, Ready) => Transition::Illegal(format!(
            "cannot close {id} — not started; run `maestro feature start {id}` first"
        )),
        (FeatureVerb::Close, Cancelled) => {
            Transition::Illegal(format!("cannot close {id} — terminal (status: cancelled)"))
        }

        (FeatureVerb::Cancel, Proposed | Ready | InProgress) => Transition::To(Cancelled),
        (FeatureVerb::Cancel, Cancelled) => Transition::NoOp,
        (FeatureVerb::Cancel, Closed) => {
            Transition::Illegal(format!("cannot cancel {id} — closed features are terminal"))
        }
    }
}

fn no_op_report(id: &str, status: FeatureStatus, note: String) -> TransitionReport {
    TransitionReport {
        id: id.to_string(),
        status,
        changed: false,
        note,
    }
}

/// Values in `incoming` not already present in `current` (and de-duplicated
/// within `incoming`), preserving order.
fn dedup_new(current: &[String], incoming: &[String]) -> Vec<String> {
    let mut added = Vec::new();
    for value in incoming {
        if !current.contains(value) && !added.contains(value) {
            added.push(value.clone());
        }
    }
    added
}

fn apply_acceptance_text_edits(
    feature_id: &str,
    acceptance: &mut [String],
    edits: &[AcceptanceTextEdit],
) -> Result<()> {
    for edit in edits {
        let normalized = normalize_acceptance_id(&edit.id).with_context(|| {
            format!(
                "invalid acceptance id for feature {feature_id}: {}",
                edit.id
            )
        })?;
        let Some(index) = normalized
            .strip_prefix("ac-")
            .and_then(|digits| digits.parse::<usize>().ok())
            .and_then(|number| number.checked_sub(1))
        else {
            bail!(
                "invalid acceptance id for feature {feature_id}: {}",
                edit.id
            );
        };
        let Some(slot) = acceptance.get_mut(index) else {
            bail!("unknown acceptance id for feature {feature_id}: {normalized}");
        };
        *slot = edit.text.clone();
    }
    Ok(())
}

fn view_from_record(
    record: FeatureRecord,
    counts: FeatureTaskCounts,
    project: Option<String>,
) -> FeatureView {
    FeatureView {
        id: record.id,
        title: record.title,
        status: record.status,
        counts,
        created_at: record.created_at,
        updated_at: record.updated_at,
        description: record.description,
        raw_request: record.raw_request,
        input_type: record.input_type,
        acceptance: record.acceptance,
        acceptance_coverage: None,
        affected_areas: record.affected_areas,
        non_goals: record.non_goals,
        open_questions: record.open_questions,
        outcome: record.outcome,
        cancel_reason: record.cancel_reason,
        qa_none_reason: record
            .qa
            .filter(|qa| qa.surface == "none")
            .map(|qa| qa.reason),
        // notes.md is read on demand by `show`, not on the list path.
        notes: None,
        project,
    }
}

/// The directory holding a feature's prose sidecars (`spec.md`, `notes.md`,
/// `qa.md`) beside its `card.yaml` at `cards/<id>/`, where migration copied them.
pub fn feature_sidecar_dir(paths: &MaestroPaths, id: &str) -> PathBuf {
    paths.cards_dir().join(id)
}

/// Read a feature's design notes (`notes.md`) from its directory, if present
/// and non-empty. `show`/`show_archived` overlay this onto the view; the list
/// path leaves `notes` as None to avoid per-row I/O.
fn read_notes_at(dir: &Path) -> Result<Option<String>> {
    Ok(read_to_string_if_exists(dir.join("notes.md"))?
        .map(|s| s.trim_end().to_string())
        .filter(|s| !s.is_empty()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NoteAppend {
    created: bool,
    line: String,
}

fn append_note_file(path: &Path, title: &str, text: &str) -> Result<NoteAppend> {
    let date = utc_now_timestamp()
        .split_once('T')
        .map(|(date, _)| date.to_string())
        .unwrap_or_else(|| "1970-01-01".to_string());
    let line = format!("{date}  {}", text.trim());
    let created = append_text_file(path, &format!("# {title}\n\n"), &format!("{line}\n"))
        .with_context(|| format!("failed to append feature note {}", path.display()))?;
    Ok(NoteAppend { created, line })
}

fn scaffold_spec_file(paths: &MaestroPaths, id: &str, title: &str) -> Result<()> {
    let path = feature_sidecar_dir(paths, id).join("spec.md");
    if path.exists() {
        return Ok(());
    }
    // S8: only the two sections every design starts from; fork walkthroughs
    // are composable from decision cards and land via `feature spec --section`.
    let contents = format!("# {title}\n\n## Current state\n\n## Problem\n\n");
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

/// Outcome of a spec-section write, for the verb echo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecSectionReport {
    /// Whether the section heading was newly added by this write.
    pub created_section: bool,
}

/// Write prose into one `## <section>` of a feature's `spec.md` (S8): append
/// to or replace the section body, scaffolding the file and creating the
/// section when absent. The spec is owner-edited prose, not record state, so
/// the write is an atomic replace without a CAS.
pub fn write_spec_section(
    paths: &MaestroPaths,
    id: &str,
    section: &str,
    text: &str,
    replace: bool,
) -> Result<SpecSectionReport> {
    let record = load_record(paths, id)?;
    let section = section.trim();
    if section.is_empty() || section.contains('\n') {
        bail!("section name must be one non-empty line");
    }
    let text = text.trim();
    if text.is_empty() {
        bail!("section text must not be empty");
    }
    scaffold_spec_file(paths, id, &record.title)?;
    let path = feature_sidecar_dir(paths, id).join("spec.md");
    let original = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let (contents, created_section) = patch_spec_section(&original, section, text, replace);
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(SpecSectionReport { created_section })
}

/// Patch one `## <section>` body in spec prose. A missing section is appended
/// at the end of the file. The section body runs to the next heading; blank
/// padding inside it is normalized to single blank lines around the content.
fn patch_spec_section(original: &str, section: &str, text: &str, replace: bool) -> (String, bool) {
    let heading = format!("## {section}");
    let lines: Vec<&str> = original.lines().collect();
    let Some(start) = lines
        .iter()
        .position(|line| line.trim_end() == heading.as_str())
    else {
        let mut out = original.trim_end().to_string();
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&format!("{heading}\n\n{text}\n"));
        return (out, true);
    };
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with("## ") || line.starts_with("# "))
        .map(|offset| start + 1 + offset)
        .unwrap_or(lines.len());
    let existing = lines[start + 1..end].join("\n");
    let existing = existing.trim();
    let body = if replace || existing.is_empty() {
        text.to_string()
    } else {
        format!("{existing}\n\n{text}")
    };
    let mut out = String::new();
    for line in &lines[..=start] {
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&body);
    out.push('\n');
    if end < lines.len() {
        out.push('\n');
        for line in &lines[end..] {
            out.push_str(line);
            out.push('\n');
        }
    }
    (out, false)
}

/// Path to a feature's archived card (`.maestro/archive/cards/<id>/card.yaml`).
pub(crate) fn archived_card_path(paths: &MaestroPaths, id: &str) -> PathBuf {
    paths.archive_cards_dir().join(id).join("card.yaml")
}

/// Reject a feature id that is not a single normal path component, so a verb
/// id like `../../x` or `/etc/x` cannot escape the features dir when it is
/// joined into a path. Ids are generated by `slugify_ascii` on create, but the
/// read/verb surface accepts an arbitrary id; this mirrors the task-id guard
/// (`validate_task_lookup_id`) so both aggregates enforce the same invariant.
pub(crate) fn validate_feature_id(id: &str) -> Result<()> {
    let mut components = Path::new(id).components();
    if id.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        bail!("invalid feature id: {id}");
    }
    Ok(())
}

/// Load one feature record from its card, erroring on absence or schema
/// incompatibility.
pub(crate) fn load_record(paths: &MaestroPaths, id: &str) -> Result<FeatureRecord> {
    validate_feature_id(id)?;
    let path = card_store::card_path(paths, id);
    let Some(card) = card_store::load(&path)? else {
        return Err(feature_not_found(paths, id));
    };
    if card.card_type != CardType::Feature {
        bail!("{id} is a {}, not a feature", card.card_type.as_str());
    }
    record_from_card(card, path.display().to_string())
}

/// Load a feature record for a read-modify-write, returning the load-time card
/// snapshot that makes the matching [`save_record`] a compare-and-set (SPEC D1):
/// the snapshot checked at write time is the one read at load time, which is what
/// closes the feature write race. Same errors as [`load_record`].
pub(crate) fn load_record_for_update(
    paths: &MaestroPaths,
    id: &str,
) -> Result<(FeatureRecord, CardSnapshot)> {
    validate_feature_id(id)?;
    let path = card_store::card_path(paths, id);
    let snapshot = card_store::load_with_snapshot(&path)?;
    let Some(card) = snapshot.card.clone() else {
        return Err(feature_not_found(paths, id));
    };
    if card.card_type != CardType::Feature {
        bail!("{id} is a {}, not a feature", card.card_type.as_str());
    }
    let record = record_from_card(card, path.display().to_string())?;
    Ok((record, snapshot))
}

/// The not-found error for a live feature read. When the id resolves in the
/// archive instead, point at the inspect/restore path (L6b) rather than a
/// dead-end "not found".
fn feature_not_found(paths: &MaestroPaths, id: &str) -> anyhow::Error {
    match load_archived_record(paths, id) {
        Ok(record) => anyhow!(
            "feature {id} is archived ({})\n  inspect: maestro feature show {id}\n  restore: maestro feature unarchive {id}\n  then: retry the command",
            record.status.as_str()
        ),
        Err(_) => anyhow!("feature not found: {id}"),
    }
}

/// Load one archived feature record from its card under `archive/cards/`,
/// erroring on absence, a non-feature card, or schema incompatibility. Lets the
/// archive reads (§5.9 L6b) load from the archive tree with the same strictness
/// as the live tree.
pub(crate) fn load_archived_record(paths: &MaestroPaths, id: &str) -> Result<FeatureRecord> {
    validate_feature_id(id)?;
    if let Some(archived) = archive_db::resolve(paths, id)? {
        if archived.card.card_type != CardType::Feature {
            bail!("feature not found: {id}");
        }
        return record_from_card(archived.card, archived.path.display().to_string());
    }
    let path = archived_card_path(paths, id);
    let Some(card) = card_store::load(&path)? else {
        bail!("feature not found: {id}");
    };
    if card.card_type != CardType::Feature {
        bail!("feature not found: {id}");
    }
    record_from_card(card, path.display().to_string())
}

/// Reconstruct a [`FeatureRecord`] from a feature card's slim `extra` payload
/// plus the envelope fields it omits, re-checking the feature schema the same way
/// the legacy read does. `artifact` names the card path for error messages.
fn record_from_card(card: Card, artifact: String) -> Result<FeatureRecord> {
    // A feature card minted natively by the card model (DN9 `maestro create -t
    // feature`) carries no `extra`, so reconstruct the record from the card's own
    // fields while feature behavior still consumes FeatureRecord.
    if card.extra.is_empty() {
        return Ok(record_from_native_card(card));
    }
    let Card {
        id,
        title,
        status,
        description,
        created_at,
        updated_at,
        extra,
        ..
    } = card;
    let mut extra = extra;
    fold::seed_string_if_absent(&mut extra, "id", &id);
    fold::seed_string_if_absent(&mut extra, "title", &title);
    let record_status = FeatureStatus::parse(&status).unwrap_or(FeatureStatus::Proposed);
    fold::seed_string_if_absent(&mut extra, "status", record_status.as_str());
    fold::seed_optional_string_if_absent(&mut extra, "description", description.as_deref());
    fold::seed_string_if_absent(&mut extra, "created_at", &created_at);
    fold::seed_string_if_absent(&mut extra, "updated_at", &updated_at);
    fold::ensure_supported_schema(&extra, &artifact, "feature")?;
    let mut record: FeatureRecord = fold::record_from_extra(extra, &artifact)?;
    // The card verbs (`update`) write only the top-level copy fields, so they
    // are the freshest source for what they own (SPEC DN3: the card status is
    // the single source of truth). The overlay is conservative: an unrecognized
    // status word and an absent description keep the record's own.
    record.id = id;
    record.title = title;
    if let Some(mapped) = FeatureStatus::parse(&status) {
        record.status = mapped;
    }
    if description.is_some() {
        record.description = description;
    }
    Ok(record)
}

/// Build a [`FeatureRecord`] from a native card's own fields (no `extra`
/// carrier). The product contract a migrated feature carries (acceptance,
/// non-goals, amends) has no native card fields yet, so the record keeps the
/// proposed defaults for those; `status` is mapped from the card's status word.
fn record_from_native_card(card: Card) -> FeatureRecord {
    let mut record = FeatureRecord::proposed(&card.id, &card.title, &card.created_at);
    record.updated_at = card.updated_at;
    record.description = card.description;
    record.status = FeatureStatus::parse(&card.status).unwrap_or(FeatureStatus::Proposed);
    record
}

/// Persist a feature record against its load-time card snapshot, so the write is
/// a CAS that rejects a racing writer (SPEC D1).
pub(crate) fn save_record(
    paths: &MaestroPaths,
    record: &FeatureRecord,
    snapshot: &CardSnapshot,
) -> Result<()> {
    let card = fold::feature_card(
        record.id.clone(),
        fold::record_to_mapping(record, "feature record")?,
        &utc_now_timestamp(),
    );
    card_store::save_folded_with_snapshot(&card_store::card_path(paths, &record.id), card, snapshot)
}

/// Create a new feature card through the store seam, which rejects a reserved
/// container name (e.g. `tasks`), an id already taken anywhere in the store,
/// and a concurrent create (CAS against an absent snapshot).
fn save_new_record(
    paths: &MaestroPaths,
    record: &FeatureRecord,
    project: Option<String>,
) -> Result<()> {
    let mut card = fold::feature_card(
        record.id.clone(),
        fold::record_to_mapping(record, "feature record")?,
        &utc_now_timestamp(),
    );
    card.project = project;
    card_store::create_card(paths, &card).map(|_| ())
}

/// Whether a live feature card exists for `id`.
fn live_record_exists(paths: &MaestroPaths, id: &str) -> bool {
    card_store::card_path(paths, id).exists()
}

/// Ids of card directories in the flat store, sorted (the shared store walk).
/// Shared by the record scan and the roster reader so both walk the store
/// identically.
fn feature_card_ids(paths: &MaestroPaths) -> Result<Vec<String>> {
    card_store::card_dir_ids(&paths.cards_dir())
}

/// Reconstruct every live feature record from `feature`-typed cards in the flat
/// card store. `tolerant` skips a card that fails to load (the strict callers
/// surface the first such error). Sorted by id.
fn scan_feature_cards(paths: &MaestroPaths, tolerant: bool) -> Result<Vec<FeatureRecord>> {
    let mut records = Vec::new();
    for id in feature_card_ids(paths)? {
        let path = card_store::card_path(paths, &id);
        match card_store::load(&path) {
            Ok(Some(card)) if card.card_type == CardType::Feature => {
                match record_from_card(card, path.display().to_string()) {
                    Ok(record) => records.push(record),
                    Err(error) if tolerant => {
                        let _ = error;
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(_) => {}
            Err(error) if tolerant => {
                let _ = error;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(records)
}

/// Strict scan: every present card must parse and be schema-`Exact`.
fn scan_records_strict(paths: &MaestroPaths) -> Result<Vec<FeatureRecord>> {
    scan_feature_cards(paths, false)
}

/// Tolerant scan: skip cards that fail to read, parse, or schema-classify.
fn scan_records_tolerant(paths: &MaestroPaths) -> Vec<FeatureRecord> {
    scan_feature_cards(paths, true).unwrap_or_default()
}

#[cfg(test)]
mod cutover_tests {
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::foundation::core::fs::ensure_dir;

    fn card_mode_repo(label: &str) -> (PathBuf, MaestroPaths) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock after Unix epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("maestro-cutover-{label}-{}-{nanos}", process::id()));
        let paths = MaestroPaths::new(&root);
        // The card store's mere existence flips dispatch to card mode.
        ensure_dir(paths.cards_dir()).expect("create cards dir");
        (root, paths)
    }

    #[test]
    fn feature_record_round_trips_through_slim_card_extra() {
        let mut record =
            FeatureRecord::proposed("csv-export", "CSV export", "2026-06-08T00:00:00Z");
        record.description = Some("export reports to CSV".to_string());
        record.updated_at = "2026-06-08T01:00:00Z".to_string();
        record.status = FeatureStatus::InProgress;
        record.acceptance = vec!["writes headers".to_string()];

        let card = fold::feature_card(
            record.id.clone(),
            fold::record_to_mapping(&record, "feature record").expect("serialize feature"),
            "2026-06-08T02:00:00Z",
        );
        for key in [
            "id",
            "title",
            "status",
            "created_at",
            "updated_at",
            "description",
        ] {
            assert!(
                !card
                    .extra
                    .contains_key(serde_yaml::Value::String(key.to_string())),
                "extra omits envelope-owned {key}"
            );
        }

        let reconstructed =
            record_from_card(card, "test".to_string()).expect("reconstruct the feature");
        assert_eq!(reconstructed, record);
    }

    /// In card mode the feature write race is closed: two readers each take a
    /// load-time snapshot; the first save wins, the second is rejected because
    /// `save_record` checks the snapshot read at load time, not a fresh one. A
    /// fresh-snapshot save would let the stale writer clobber the winner.
    #[test]
    fn card_mode_save_rejects_a_stale_feature_writer() {
        let (root, paths) = card_mode_repo("stale-writer");
        let id = create(&paths, "Race", None).expect("create writes a feature card");

        let (mut winner, winner_write) =
            load_record_for_update(&paths, &id).expect("first read for update");
        let (mut loser, loser_write) =
            load_record_for_update(&paths, &id).expect("second read for update");

        winner.description = Some("winner".to_string());
        save_record(&paths, &winner, &winner_write).expect("first writer commits");

        loser.description = Some("stale".to_string());
        let error = save_record(&paths, &loser, &loser_write)
            .expect_err("the stale writer must be rejected, not silently win");
        assert!(
            format!("{error:#}").contains("changed since it was read"),
            "{error:#}"
        );

        assert_eq!(
            load_record(&paths, &id)
                .expect("reload")
                .description
                .as_deref(),
            Some("winner"),
            "the winner's write survived"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Create refuses to mint a feature whose card already exists (card-mode
    /// analogue of the legacy "directory already exists" guard).
    #[test]
    fn card_mode_create_refuses_a_duplicate_id() {
        let (root, paths) = card_mode_repo("dup-id");
        let id = create(&paths, "Csv export", None).expect("first create");
        let error = create(&paths, "Csv export", None).expect_err("second create must fail");
        assert!(
            error
                .to_string()
                .contains(&format!("feature {id} already exists")),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// S8: the spec verb fills sections during brainstorm/plan -- appends
    /// accumulate, replace overwrites, an unknown section is created, and the
    /// rest of the file is left byte-identical.
    #[test]
    fn spec_section_writes_fill_the_scaffold() {
        let (root, paths) = card_mode_repo("spec-section");
        let id = create(&paths, "Csv export", None).expect("create scaffolds spec.md");
        let spec_path = feature_sidecar_dir(&paths, &id).join("spec.md");

        let first = write_spec_section(&paths, &id, "Current state", "One writer.", false)
            .expect("append into a scaffold section");
        assert!(!first.created_section);
        write_spec_section(&paths, &id, "Current state", "Two formats.", false)
            .expect("second append accumulates");
        let appended = std::fs::read_to_string(&spec_path).expect("spec");
        assert_eq!(
            appended,
            "# Csv export\n\n## Current state\n\nOne writer.\n\nTwo formats.\n\n## Problem\n\n"
        );

        write_spec_section(&paths, &id, "Current state", "Rewritten.", true)
            .expect("replace overwrites the body");
        let replaced = std::fs::read_to_string(&spec_path).expect("spec");
        assert_eq!(
            replaced,
            "# Csv export\n\n## Current state\n\nRewritten.\n\n## Problem\n\n"
        );

        let created = write_spec_section(&paths, &id, "Fork walkthroughs", "F1 vs F2.", false)
            .expect("an unknown section is created at the end");
        assert!(created.created_section);
        let grown = std::fs::read_to_string(&spec_path).expect("spec");
        assert_eq!(
            grown,
            "# Csv export\n\n## Current state\n\nRewritten.\n\n## Problem\n\n## Fork walkthroughs\n\nF1 vs F2.\n"
        );

        let error = write_spec_section(&paths, &id, "Current state", "   ", false)
            .expect_err("blank text is refused");
        assert!(
            format!("{error:#}").contains("must not be empty"),
            "{error:#}"
        );
        let error = write_spec_section(&paths, "ghost", "Current state", "text", false)
            .expect_err("a missing feature is refused");
        assert!(
            format!("{error:#}").contains("feature not found"),
            "{error:#}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A title that slugs to a reserved container name (`tasks`) must be
    /// refused, or the new feature's card.yaml would land inside the root task
    /// pool dir and turn it into a phantom container.
    #[test]
    fn card_mode_create_refuses_a_reserved_container_slug() {
        let (root, paths) = card_mode_repo("reserved-slug");
        let error = create(&paths, "Tasks", None).expect_err("reserved slug must be refused");
        assert!(
            error.to_string().contains("reserved by the card store"),
            "{error}"
        );
        assert!(
            !paths.cards_dir().join("tasks").join("card.yaml").exists(),
            "no card.yaml may be planted in the pool dir"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The roster's type sniff reads quoted and commented `type:` scalars,
    /// so a malformed feature card surfaces as unreadable instead of being
    /// dropped as a non-feature; an indented `type:` stays a nested key.
    #[test]
    fn type_sniff_reads_quoted_and_commented_scalars() {
        let (root, paths) = card_mode_repo("type-sniff");
        let file = paths.cards_dir().join("sniff.yaml");
        for (raw, non_feature) in [
            ("type: task", true),
            ("type: feature", false),
            ("type: \"feature\"", false),
            ("type: 'feature'", false),
            ("type: feature # container", false),
            ("type: \"task\" # worker", true),
            ("extra:\n  type: task", false),
        ] {
            std::fs::write(&file, raw).expect("write sniff fixture");
            assert_eq!(raw_card_declares_non_feature(&file), non_feature, "{raw:?}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
