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
//! (`accept`, `ship`) layer their preconditions on top, emitting actionable
//! errors that name the gap and the fix command.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};

use crate::domain::card::schema::{Card, CardType};
use crate::domain::card::store::{self as card_store, CardSnapshot};
use crate::domain::card::fold;
use crate::domain::feature::qa;
use crate::domain::feature::query::{
    FeatureTaskCounts, count_tasks_by_feature, count_tasks_for_feature,
    count_tasks_for_feature_in_entries, live_child_task_ids,
};
use crate::domain::feature::schema::{
    AmendAdditions, AmendEntry, AmendLog, FeatureRecord, FeatureStatus, QaDeclaration,
    normalize_acceptance_id,
};
use crate::domain::feature::verification;
use crate::domain::task::{self, TaskState, TransitionDetails};
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::{append_text_file, read_to_string_if_exists};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{Compat, FEATURE_SCHEMA_VERSION, classify};
use crate::foundation::core::slug::slugify_ascii;
use crate::foundation::core::time::utc_now_timestamp;

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
    /// One-line shipped outcome, set at `ship --outcome`.
    pub outcome: Option<String>,
    /// Operator reason recorded at `cancel --reason`.
    pub cancel_reason: Option<String>,
    /// Reason for an explicit `qa: none` declaration.
    pub qa_none_reason: Option<String>,
    /// Design notes (`notes.md`), read on demand by `show`. None elsewhere.
    pub notes: Option<String>,
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

struct ShipGateReport {
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
pub fn create(paths: &MaestroPaths, title: &str) -> Result<String> {
    let id = slugify_ascii(title);
    if id.is_empty() {
        bail!("feature title must contain at least one ASCII letter or digit");
    }
    if live_record_exists(paths, &id) {
        bail!("feature {id} already exists");
    }
    // L6a: an archived feature still owns its slug — refuse to reissue it.
    if archived_feature_yaml_path(paths, &id).exists() {
        bail!(
            "feature {id} already exists in the archive; `maestro feature unarchive {id}` or choose a different title"
        );
    }
    let record = FeatureRecord::proposed(&id, title, &utc_now_timestamp());
    save_new_record(paths, &record)?;
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
        FeatureStatus::Shipped | FeatureStatus::Cancelled => bail!(
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
        view: view_from_record(record, counts),
        replaced,
        added,
        edited_acceptance: edits.edit_acceptance.len(),
    })
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
    // F — a captured behavior baseline is a precondition of accept (before edits).
    let feat_dir = feature_sidecar_dir(paths, id);
    if qa_none_reason.is_none() && !qa::baseline_present(&feat_dir)? {
        gaps.push(format!(
                  "qa-baseline (.maestro/cards/{id}/qa.md {})\n    skill: qa-baseline\n    target: .maestro/cards/{id}/qa.md\n    retry: maestro feature accept {id}",
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
        FeatureStatus::Shipped | FeatureStatus::Cancelled => {
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

/// Ship a feature: `InProgress → Shipped`, gated (D5).
///
/// Phase A gate: condition 1 only — no LIVE child task
/// (`draft/exploring/ready/in_progress/needs_verification` block; `verified` and
/// terminal-settled do not). (Coverage E and the QA floor are layered in Phase
/// B.) `--dry-run` previews the gate at exit 0 without transitioning.
///
/// # Errors
///
/// Errors when the feature is not found, the source state is illegal, or
/// (non-dry-run) a live child task blocks ship.
pub fn ship(
    paths: &MaestroPaths,
    id: &str,
    outcome: Option<String>,
    dry_run: bool,
) -> Result<TransitionReport> {
    let (mut record, write) = load_record_for_update(paths, id)?;
    let target = match legal_transition(id, &record.status, FeatureVerb::Ship) {
        Transition::NoOp => {
            // The outcome is set once, at ship. On an already-shipped no-op a new
            // `--outcome` cannot be recorded; say so rather than dropping it silently.
            let note = if outcome.is_some() {
                format!("{id} is already shipped; --outcome not recorded (it is set once, at ship)")
            } else {
                format!("{id} is already shipped")
            };
            return Ok(no_op_report(id, record.status, note));
        }
        Transition::Illegal(message) => bail!(message),
        Transition::To(target) => target,
    };

    let gate = ship_gaps_for_record(paths, id, &record)?;
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
            format!("would ship {id} (-> shipped); no live child tasks, {qa}")
        } else {
            format!("would block ship {id}:\n  {}", gaps.join("\n  "))
        };
        return Ok(no_op_report(id, record.status, note));
    }

    if !gaps.is_empty() {
        bail!("cannot ship {id}:\n  {}", gaps.join("\n  "));
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
        note: format!("shipped {id} (-> shipped)"),
    })
}

pub fn ship_gaps(paths: &MaestroPaths, id: &str) -> Result<Vec<String>> {
    let record = load_record(paths, id)?;
    Ok(ship_gaps_for_record(paths, id, &record)?.gaps)
}

fn ship_gaps_for_record(
    paths: &MaestroPaths,
    id: &str,
    record: &FeatureRecord,
) -> Result<ShipGateReport> {
    let mut gaps = Vec::new();
    // D5 cond 1 -- no live child task may outlive its shipped feature.
    let live = live_child_task_ids(&paths.tasks_dir(), &record.id)?;
    if !live.is_empty() {
        gaps.push(format!(
            "{} live child task(s): {}\n    fix: verify or abandon them, then re-ship",
            live.len(),
            live.join(", ")
        ));
    }
    // D5 cond 2/3 -- QA baseline present + fresh, every behavioral scenario proven.
    let feat_dir = feature_sidecar_dir(paths, id);
    let qa_declared_none = record
        .qa
        .as_ref()
        .is_some_and(|qa| qa.surface == "none" && qa.amend_log_position == record.amends.len());
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
        gaps.extend(qa::ship_qa_gaps(
            id,
            baseline.as_ref(),
            absence,
            &slices,
            &amend_log,
        ));
    }
    // D5 cond 4 -- the full feature acceptance contract must have a fresh
    // sweep run that resolved every ac-N item.
    if let Some(gap) = verification::acceptance_ship_gap(paths, record)? {
        gaps.push(gap);
    }
    Ok(ShipGateReport {
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
/// Errors when the feature is not found, it is already terminal (Shipped), or a
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
    // mirroring accept/ship/archive, before any irreversible mutation.
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
    Ok(records
        .into_iter()
        .map(|record| {
            let counts = counts_by_feature
                .get(&record.id)
                .cloned()
                .unwrap_or_default();
            view_from_record(record, counts)
        })
        .collect())
}

/// Roster of every feature card for `status` / `feature list`. A feature card
/// whose record is unparseable or schema-incompatible surfaces as `Unreadable`
/// rather than being dropped, so a single bad artifact never silently shrinks
/// the board. A card that fails to load at all (corrupt file, unknown type) is
/// left to `doctor`; non-feature cards are skipped.
pub fn list_tolerant(paths: &MaestroPaths) -> Vec<FeatureRosterEntry> {
    let counts_by_feature = count_tasks_by_feature(&paths.tasks_dir()).unwrap_or_default();
    let mut entries = Vec::new();
    for id in feature_card_ids(paths).unwrap_or_default() {
        let path = card_store::card_path(paths, &id);
        match card_store::load(&path) {
            Ok(Some(card)) if card.card_type == CardType::Feature => {
                match record_from_card(card, path.display().to_string()) {
                    Ok(record) => {
                        let counts = counts_by_feature
                            .get(&record.id)
                            .cloned()
                            .unwrap_or_default();
                        entries.push(FeatureRosterEntry::Loaded(Box::new(view_from_record(
                            record, counts,
                        ))));
                    }
                    Err(error) => {
                        let typed_error = error
                            .chain()
                            .find_map(|cause| cause.downcast_ref::<MaestroError>().cloned());
                        let hint = typed_error.as_ref().and_then(MaestroError::hint);
                        entries.push(FeatureRosterEntry::Unreadable {
                            id,
                            path,
                            error: format!("{error:#}"),
                            hint,
                            typed_error,
                        });
                    }
                }
            }
            Ok(_) => {}
            Err(_) => {}
        }
    }
    entries
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
    let mut view = view_from_record(record, counts);
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
    let record = load_record_at(&archived_feature_yaml_path(paths, id), id)?;
    let task_entries = task::load_task_entries(&paths.archive_tasks_dir())?;
    let counts = count_tasks_for_feature_in_entries(&task_entries, &record.id);
    let acceptance_coverage =
        verification::acceptance_coverage_for_record_in_entries(&record, &task_entries);
    let notes = read_notes_at(&paths.archive_features_dir().join(id))?;
    let mut view = view_from_record(record, counts);
    view.acceptance_coverage = Some(acceptance_coverage);
    view.notes = notes;
    Ok(view)
}

/// Ensure a live feature id is valid and resolves to a compatible record.
pub fn ensure_exists(paths: &MaestroPaths, id: &str) -> Result<()> {
    load_record(paths, id).map(|_| ())
}

/// List every archived feature joined with its archived task counts (L6b,
/// `feature list --all`).
///
/// # Errors
///
/// Errors when an archived feature record is unparseable or schema-incompatible.
pub fn list_archived(paths: &MaestroPaths) -> Result<Vec<FeatureView>> {
    let archive_features_dir = paths.archive_features_dir();
    let counts_by_feature = count_tasks_by_feature(&paths.archive_tasks_dir())?;
    feature_ids(&archive_features_dir)?
        .iter()
        .map(|id| {
            let record = load_record_at(&archive_features_dir.join(id).join("feature.yaml"), id)?;
            let counts = counts_by_feature
                .get(&record.id)
                .cloned()
                .unwrap_or_default();
            Ok(view_from_record(record, counts))
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
/// Never errors: an absent features dir or an unparseable / schema-incompatible
/// record is carried in [`FeatureDiagnostic::found`] as `Err`.
pub fn diagnose(paths: &MaestroPaths) -> FeatureDiagnostic {
    // Feature cards live in the flat card store, not a per-entity directory, so
    // count them by scanning the store; an absent store reads as zero features,
    // not a missing-directory error.
    let found = scan_records_strict(paths)
        .map(|records| records.len())
        .map_err(|error| format!("{error:#}"));
    FeatureDiagnostic {
        expected: FEATURE_SCHEMA_VERSION,
        found,
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
    Ship,
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
    use FeatureStatus::{Cancelled, InProgress, Proposed, Ready, Shipped};
    match (verb, from) {
        (FeatureVerb::Accept, Proposed) => Transition::To(Ready),
        (FeatureVerb::Accept, Ready) => Transition::NoOp,
        (FeatureVerb::Accept, InProgress) => Transition::Illegal(format!(
            "cannot accept {id} — already past accept (status: in_progress)"
        )),
        (FeatureVerb::Accept, Shipped | Cancelled) => Transition::Illegal(format!(
            "cannot accept {id} — terminal (status: {})",
            from.as_str()
        )),

        (FeatureVerb::Start, Ready) => Transition::To(InProgress),
        (FeatureVerb::Start, InProgress) => Transition::NoOp,
        (FeatureVerb::Start, Proposed) => Transition::Illegal(format!(
            "cannot start {id} — not accepted; run `maestro feature accept {id}` first"
        )),
        (FeatureVerb::Start, Shipped | Cancelled) => Transition::Illegal(format!(
            "cannot start {id} — terminal (status: {})",
            from.as_str()
        )),

        (FeatureVerb::Ship, InProgress) => Transition::To(Shipped),
        (FeatureVerb::Ship, Shipped) => Transition::NoOp,
        (FeatureVerb::Ship, Proposed) => Transition::Illegal(format!(
            "cannot ship {id} — not started; run `maestro feature accept {id}` then `maestro feature start {id}`"
        )),
        (FeatureVerb::Ship, Ready) => Transition::Illegal(format!(
            "cannot ship {id} — not started; run `maestro feature start {id}` first"
        )),
        (FeatureVerb::Ship, Cancelled) => {
            Transition::Illegal(format!("cannot ship {id} — terminal (status: cancelled)"))
        }

        (FeatureVerb::Cancel, Proposed | Ready | InProgress) => Transition::To(Cancelled),
        (FeatureVerb::Cancel, Cancelled) => Transition::NoOp,
        (FeatureVerb::Cancel, Shipped) => Transition::Illegal(format!(
            "cannot cancel {id} — shipped features are terminal"
        )),
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

fn view_from_record(record: FeatureRecord, counts: FeatureTaskCounts) -> FeatureView {
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
    }
}

/// The directory holding a feature's prose sidecars (`spec.md`, `notes.md`,
/// `qa.md`) beside its `card.yaml` at `cards/<id>/`, where migration copied them.
pub(crate) fn feature_sidecar_dir(paths: &MaestroPaths, id: &str) -> PathBuf {
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
    let contents =
        format!("# {title}\n\n## Current state\n\n## Problem\n\n## Fork walkthroughs\n\n");
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

/// Path to a feature's record under the archive tree (`.maestro/archive/features/<id>/feature.yaml`).
fn archived_feature_yaml_path(paths: &MaestroPaths, id: &str) -> PathBuf {
    paths.archive_features_dir().join(id).join("feature.yaml")
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
        bail!("feature not found: {id}");
    };
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
        bail!("feature not found: {id}");
    };
    let record = record_from_card(card, path.display().to_string())?;
    Ok((record, snapshot))
}

/// Reconstruct a [`FeatureRecord`] from a feature card's verbatim source mapping
/// (`extra`, the COPY-design payload), re-checking the feature schema the same
/// way the legacy read does. `artifact` names the card path for error messages.
fn record_from_card(card: Card, artifact: String) -> Result<FeatureRecord> {
    // A feature card minted natively by the card model (DN9 `maestro create -t
    // feature`) carries no `extra`, so reconstruct the record from the card's own
    // fields. Mirrors the task reader; retires with the carrier in S4 (E7).
    if card.extra.is_empty() {
        return Ok(record_from_native_card(card));
    }
    let record: FeatureRecord = serde_yaml::from_value(Value::Mapping(card.extra))
        .with_context(|| format!("failed to parse {artifact}"))?;
    if classify(&record.schema_version, FEATURE_SCHEMA_VERSION) != Compat::Exact {
        return Err(MaestroError::SchemaMismatch {
            artifact,
            expected: FEATURE_SCHEMA_VERSION,
            found: record.schema_version,
        }
        .into());
    }
    Ok(record)
}

/// Build a [`FeatureRecord`] from a native card's own fields (no `extra`
/// carrier). The product contract a migrated feature carries (acceptance,
/// non-goals, amends) has no native home yet (the S4 gap), so the record keeps
/// the proposed defaults for those; `status` is mapped from the card's status
/// word.
fn record_from_native_card(card: Card) -> FeatureRecord {
    let mut record = FeatureRecord::proposed(&card.id, &card.title, &card.created_at);
    record.updated_at = card.updated_at;
    record.description = card.description;
    record.status = match card.status.as_str() {
        "ready" => FeatureStatus::Ready,
        "in_progress" => FeatureStatus::InProgress,
        "shipped" | "closed" => FeatureStatus::Shipped,
        "cancelled" => FeatureStatus::Cancelled,
        _ => FeatureStatus::Proposed,
    };
    record
}

/// Serialize a feature record to the YAML mapping the card builder folds into
/// `extra`. Round-trips with [`record_from_card`]; feeding the same mapping the
/// migration reads off disk keeps a saved card byte-identical to a migrated one.
fn record_to_mapping(record: &FeatureRecord) -> Result<Mapping> {
    match serde_yaml::to_value(record).context("failed to serialize feature record")? {
        Value::Mapping(map) => Ok(map),
        _ => bail!("feature record did not serialize to a mapping"),
    }
}

/// Load a feature record from an explicit `feature.yaml` path, erroring on
/// absence or schema incompatibility. Lets the archive reads (§5.9) load from
/// the archive tree with the same strictness as the live tree.
pub(crate) fn load_record_at(path: &Path, id: &str) -> Result<FeatureRecord> {
    validate_feature_id(id)?;
    let Some(contents) = read_to_string_if_exists(path)? else {
        bail!("feature not found: {id}");
    };
    let record: FeatureRecord = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if classify(&record.schema_version, FEATURE_SCHEMA_VERSION) != Compat::Exact {
        return Err(MaestroError::SchemaMismatch {
            artifact: path.display().to_string(),
            expected: FEATURE_SCHEMA_VERSION,
            found: record.schema_version,
        }
        .into());
    }
    Ok(record)
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
        record_to_mapping(record)?,
        &utc_now_timestamp(),
    );
    card_store::save_with_snapshot(&card_store::card_path(paths, &record.id), &card, snapshot)
}

/// Create a new feature card against an absent snapshot, so a concurrent create
/// is rejected by the CAS (the card-store analogue of the legacy `.alloc-`
/// atomic-create guard).
fn save_new_record(paths: &MaestroPaths, record: &FeatureRecord) -> Result<()> {
    let path = card_store::card_path(paths, &record.id);
    let snapshot = card_store::load_with_snapshot(&path)?;
    if snapshot.card.is_some() {
        bail!("feature {} already exists", record.id);
    }
    let card = fold::feature_card(
        record.id.clone(),
        record_to_mapping(record)?,
        &utc_now_timestamp(),
    );
    card_store::save_with_snapshot(&path, &card, &snapshot)
}

/// Whether a live feature card exists for `id`.
fn live_record_exists(paths: &MaestroPaths, id: &str) -> bool {
    card_store::card_path(paths, id).exists()
}

/// Reconstruct every live feature record. Card mode reads the flat card store
/// and keeps `feature`-typed cards; legacy mode scans the per-feature
/// directories. `tolerant` skips a card that fails to load (the strict callers
/// surface the first such error). Sorted by id.
/// Ids of feature-card directories in the flat store, sorted. Skips symlinked
/// dirs and dirs without a `card.yaml`; an absent store yields no ids. Shared by
/// the record scan and the roster reader so both walk the store identically.
fn feature_card_ids(paths: &MaestroPaths) -> Result<Vec<String>> {
    let cards_dir = paths.cards_dir();
    let mut ids = Vec::new();
    if !cards_dir.is_dir() {
        return Ok(ids);
    }
    for entry in fs::read_dir(&cards_dir)
        .with_context(|| format!("failed to read {}", cards_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", cards_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        if !entry.path().join("card.yaml").is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            ids.push(name.to_string());
        }
    }
    ids.sort();
    Ok(ids)
}

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

/// Ids of feature directories that contain a real `feature.yaml`, sorted.
fn feature_ids(features_dir: &Path) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    if !features_dir.is_dir() {
        return Ok(ids);
    }
    for entry in fs::read_dir(features_dir)
        .with_context(|| format!("failed to read {}", features_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", features_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        if !entry.path().join("feature.yaml").is_file() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            ids.push(name.to_string());
        }
    }
    ids.sort();
    Ok(ids)
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
        // The card store's mere existence flips dispatch to card mode (P1).
        ensure_dir(paths.cards_dir()).expect("create cards dir");
        (root, paths)
    }

    /// In card mode the feature write race is closed: two readers each take a
    /// load-time snapshot; the first save wins, the second is rejected because
    /// `save_record` checks the snapshot read at load time, not a fresh one. A
    /// fresh-snapshot save would let the stale writer clobber the winner.
    #[test]
    fn card_mode_save_rejects_a_stale_feature_writer() {
        let (root, paths) = card_mode_repo("stale-writer");
        let id = create(&paths, "Race").expect("create writes a feature card");

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
        let id = create(&paths, "Csv export").expect("first create");
        let error = create(&paths, "Csv export").expect_err("second create must fail");
        assert!(
            error
                .to_string()
                .contains(&format!("feature {id} already exists")),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
