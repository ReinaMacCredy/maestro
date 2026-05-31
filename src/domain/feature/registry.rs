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
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::domain::feature::qa;
use crate::domain::feature::query::{
    count_tasks_by_feature, count_tasks_for_feature, live_child_task_ids, FeatureTaskCounts,
};
use crate::domain::feature::schema::{
    AmendAdditions, AmendEntry, AmendLog, FeatureRecord, FeatureStatus,
};
use crate::domain::task::{self, TaskState, TransitionDetails};
use crate::foundation::core::fs::{ensure_dir, read_to_string_if_exists};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{classify, Compat, FEATURE_SCHEMA_VERSION};
use crate::foundation::core::slug::slugify_ascii;
use crate::foundation::core::time::nanos_since_epoch_string;

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
    /// Affected surfaces.
    pub affected_areas: Vec<String>,
    /// Explicit non-goals.
    pub non_goals: Vec<String>,
    /// Open questions (non-blocking).
    pub open_questions: Vec<String>,
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
    }
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
    if feature_yaml_path(paths, &id).exists() {
        bail!("feature {id} already exists");
    }
    // L6a: an archived feature still owns its slug — refuse to reissue it.
    if archived_feature_yaml_path(paths, &id).exists() {
        bail!("feature {id} already exists in the archive; `maestro feature unarchive {id}` or choose a different title");
    }
    let record = FeatureRecord::proposed(&id, title, &nanos_since_epoch_string());
    save_record(paths, &record)?;
    Ok(id)
}

/// Author a Proposed feature's contract (declarative replace-per-field).
///
/// # Errors
///
/// Errors when the feature is not found or its contract is frozen (status is
/// past `Proposed`).
pub fn set(paths: &MaestroPaths, id: &str, edits: ContractEdits) -> Result<FeatureView> {
    let mut record = load_record(paths, id)?;
    if record.status != FeatureStatus::Proposed {
        bail!(
            "cannot edit {id} — contract frozen at accept (status: {}); grow it with `maestro feature amend {id} --add-acceptance \"…\" --reason \"…\"`",
            record.status.as_str()
        );
    }
    if let Some(value) = edits.acceptance {
        record.acceptance = value;
    }
    if let Some(value) = edits.affected_areas {
        record.affected_areas = value;
    }
    if let Some(value) = edits.non_goals {
        record.non_goals = value;
    }
    if let Some(value) = edits.open_questions {
        record.open_questions = value;
    }
    if let Some(value) = edits.description {
        record.description = Some(value);
    }
    if let Some(value) = edits.raw_request {
        record.raw_request = Some(value);
    }
    if let Some(value) = edits.input_type {
        record.input_type = Some(value);
    }
    record.updated_at = nanos_since_epoch_string();
    save_record(paths, &record)?;
    let counts = count_tasks_for_feature(&paths.tasks_dir(), &record.id)?;
    Ok(view_from_record(record, counts))
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
    let mut record = load_record(paths, id)?;
    let target = match legal_transition(id, &record.status, FeatureVerb::Accept) {
        Transition::NoOp => {
            return Ok(no_op_report(id, record.status, format!("{id} is already ready")))
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
    if !qa::baseline_present(&feature_dir(paths, id))? {
        gaps.push(format!(
            "qa-baseline (.maestro/features/{id}/baseline.md missing) — fix: run the qa-baseline skill to capture behavior before edits"
        ));
    }

    if dry_run {
        let note = if gaps.is_empty() {
            format!(
                "would accept {id} (→ ready); contract complete (acceptance={}, areas={})",
                record.acceptance.len(),
                record.affected_areas.len()
            )
        } else {
            format!("would block accept {id} — contract incomplete:\n  {}", gaps.join("\n  "))
        };
        return Ok(no_op_report(id, record.status, note));
    }

    if !gaps.is_empty() {
        bail!("cannot accept {id} — contract incomplete:\n  {}", gaps.join("\n  "));
    }

    let questions_note = if record.open_questions.is_empty() {
        String::new()
    } else {
        format!(
            "; note: {} open question(s) carried (non-blocking)",
            record.open_questions.len()
        )
    };
    let summary = format!(
        "accepted {id} (→ ready); contract frozen (acceptance={}, areas={}){}",
        record.acceptance.len(),
        record.affected_areas.len(),
        questions_note
    );
    record.status = target.clone();
    record.updated_at = nanos_since_epoch_string();
    save_record(paths, &record)?;
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
    let mut record = load_record(paths, id)?;
    match record.status {
        FeatureStatus::Ready | FeatureStatus::InProgress => {}
        FeatureStatus::Proposed => bail!(
            "cannot amend {id} — not accepted; author the contract with `maestro feature set {id} --…` then `maestro feature accept {id}`"
        ),
        FeatureStatus::Shipped | FeatureStatus::Cancelled => {
            bail!("cannot amend {id} — terminal (status: {})", record.status.as_str())
        }
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
    record.affected_areas.extend(added.affected_areas.iter().cloned());
    record.non_goals.extend(added.non_goals.iter().cloned());
    record.open_questions.extend(added.open_questions.iter().cloned());
    let now = nanos_since_epoch_string();
    record.updated_at = now.clone();

    let mut log = load_amend_log(paths, id)?;
    log.entries.push(AmendEntry {
        at: now,
        reason: reason.to_string(),
        added: added.clone(),
    });

    save_record(paths, &record)?;
    save_amend_log(paths, id, &log)?;

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

/// Start work: `Ready → InProgress`.
///
/// # Errors
///
/// Errors when the feature is not found or the source state is illegal.
pub fn start(paths: &MaestroPaths, id: &str) -> Result<TransitionReport> {
    let mut record = load_record(paths, id)?;
    let target = match legal_transition(id, &record.status, FeatureVerb::Start) {
        Transition::NoOp => {
            return Ok(no_op_report(id, record.status, format!("{id} is already in_progress")))
        }
        Transition::Illegal(message) => bail!(message),
        Transition::To(target) => target,
    };
    record.status = target.clone();
    record.updated_at = nanos_since_epoch_string();
    save_record(paths, &record)?;
    Ok(TransitionReport {
        id: id.to_string(),
        status: target,
        changed: true,
        note: format!("started {id} (→ in_progress)"),
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
pub fn ship(paths: &MaestroPaths, id: &str, dry_run: bool) -> Result<TransitionReport> {
    let mut record = load_record(paths, id)?;
    let target = match legal_transition(id, &record.status, FeatureVerb::Ship) {
        Transition::NoOp => {
            return Ok(no_op_report(id, record.status, format!("{id} is already shipped")))
        }
        Transition::Illegal(message) => bail!(message),
        Transition::To(target) => target,
    };

    let mut gaps = Vec::new();
    // D5 cond 1 — no live child task may outlive its shipped feature.
    let live = live_child_task_ids(&paths.tasks_dir(), &record.id)?;
    if !live.is_empty() {
        gaps.push(format!(
            "{} live child task(s): {}\n    fix: verify or abandon them, then re-ship",
            live.len(),
            live.join(", ")
        ));
    }
    // D5 cond 2/3 — QA baseline present + fresh, every behavioral scenario proven.
    let feat_dir = feature_dir(paths, id);
    let baseline = qa::read_baseline(&feat_dir)?;
    let slices = qa::read_qa_slices(&feat_dir)?;
    let amend_log = load_amend_log(paths, id)?;
    gaps.extend(qa::ship_qa_gaps(id, baseline.as_ref(), &slices, &amend_log));

    if dry_run {
        let note = if gaps.is_empty() {
            format!("would ship {id} (→ shipped); no live child tasks, qa-baseline proven")
        } else {
            format!("would block ship {id}:\n  {}", gaps.join("\n  "))
        };
        return Ok(no_op_report(id, record.status, note));
    }

    if !gaps.is_empty() {
        bail!("cannot ship {id}:\n  {}", gaps.join("\n  "));
    }

    record.status = target.clone();
    record.updated_at = nanos_since_epoch_string();
    save_record(paths, &record)?;
    Ok(TransitionReport {
        id: id.to_string(),
        status: target,
        changed: true,
        note: format!("shipped {id} (→ shipped)"),
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
pub fn cancel(paths: &MaestroPaths, id: &str, reason: &str) -> Result<CancelReport> {
    let mut record = load_record(paths, id)?;
    let target = match legal_transition(id, &record.status, FeatureVerb::Cancel) {
        Transition::NoOp => {
            return Ok(CancelReport {
                id: id.to_string(),
                changed: false,
                abandoned: Vec::new(),
                note: format!("{id} is already cancelled"),
            })
        }
        Transition::Illegal(message) => bail!(message),
        Transition::To(target) => target,
    };

    let live = live_child_task_ids(&paths.tasks_dir(), &record.id)?;
    let now = nanos_since_epoch_string();
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
    record.updated_at = nanos_since_epoch_string();
    save_record(paths, &record)?;

    let note = if live.is_empty() {
        format!("cancelled {id}; no child tasks affected")
    } else {
        format!("cancelled {id}; abandoned {} child task(s): {}", live.len(), live.join(", "))
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
            let counts = counts_by_feature.get(&record.id).cloned().unwrap_or_default();
            view_from_record(record, counts)
        })
        .collect())
}

/// Show one feature joined with its on-demand task counts.
///
/// # Errors
///
/// Errors when no feature has the given id, or the record is unparseable /
/// schema-incompatible.
pub fn show(paths: &MaestroPaths, id: &str) -> Result<FeatureView> {
    let record = load_record(paths, id)?;
    let counts = count_tasks_for_feature(&paths.tasks_dir(), &record.id)?;
    Ok(view_from_record(record, counts))
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
    let counts = count_tasks_for_feature(&paths.archive_tasks_dir(), &record.id)?;
    Ok(view_from_record(record, counts))
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
            let counts = counts_by_feature.get(&record.id).cloned().unwrap_or_default();
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
    let dir = paths.features_dir();
    let found = if !dir.is_dir() {
        Err(format!("{} is missing", dir.display()))
    } else {
        scan_records_strict(paths)
            .map(|records| records.len())
            .map_err(|error| error.to_string())
    };
    FeatureDiagnostic {
        expected: FEATURE_SCHEMA_VERSION,
        found,
    }
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
        (FeatureVerb::Accept, InProgress) => {
            Transition::Illegal(format!("cannot accept {id} — already past accept (status: in_progress)"))
        }
        (FeatureVerb::Accept, Shipped | Cancelled) => {
            Transition::Illegal(format!("cannot accept {id} — terminal (status: {})", from.as_str()))
        }

        (FeatureVerb::Start, Ready) => Transition::To(InProgress),
        (FeatureVerb::Start, InProgress) => Transition::NoOp,
        (FeatureVerb::Start, Proposed) => {
            Transition::Illegal(format!("cannot start {id} — not accepted; run `maestro feature accept {id}` first"))
        }
        (FeatureVerb::Start, Shipped | Cancelled) => {
            Transition::Illegal(format!("cannot start {id} — terminal (status: {})", from.as_str()))
        }

        (FeatureVerb::Ship, InProgress) => Transition::To(Shipped),
        (FeatureVerb::Ship, Shipped) => Transition::NoOp,
        (FeatureVerb::Ship, Proposed) => Transition::Illegal(format!(
            "cannot ship {id} — not started; run `maestro feature accept {id}` then `maestro feature start {id}`"
        )),
        (FeatureVerb::Ship, Ready) => {
            Transition::Illegal(format!("cannot ship {id} — not started; run `maestro feature start {id}` first"))
        }
        (FeatureVerb::Ship, Cancelled) => {
            Transition::Illegal(format!("cannot ship {id} — terminal (status: cancelled)"))
        }

        (FeatureVerb::Cancel, Proposed | Ready | InProgress) => Transition::To(Cancelled),
        (FeatureVerb::Cancel, Cancelled) => Transition::NoOp,
        (FeatureVerb::Cancel, Shipped) => {
            Transition::Illegal(format!("cannot cancel {id} — shipped features are terminal"))
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
        affected_areas: record.affected_areas,
        non_goals: record.non_goals,
        open_questions: record.open_questions,
    }
}

fn feature_dir(paths: &MaestroPaths, id: &str) -> PathBuf {
    paths.features_dir().join(id)
}

fn feature_yaml_path(paths: &MaestroPaths, id: &str) -> PathBuf {
    feature_dir(paths, id).join("feature.yaml")
}

/// Path to a feature's record under the archive tree (`.maestro/archive/features/<id>/feature.yaml`).
fn archived_feature_yaml_path(paths: &MaestroPaths, id: &str) -> PathBuf {
    paths.archive_features_dir().join(id).join("feature.yaml")
}

fn amend_log_path(paths: &MaestroPaths, id: &str) -> PathBuf {
    feature_dir(paths, id).join("amend-log.yaml")
}

/// Load one feature record, erroring on absence or schema incompatibility.
fn load_record(paths: &MaestroPaths, id: &str) -> Result<FeatureRecord> {
    load_record_at(&feature_yaml_path(paths, id), id)
}

/// Load a feature record from an explicit `feature.yaml` path, erroring on
/// absence or schema incompatibility. Lets the archive reads (§5.9) load from
/// the archive tree with the same strictness as the live tree.
pub(crate) fn load_record_at(path: &Path, id: &str) -> Result<FeatureRecord> {
    let Some(contents) = read_to_string_if_exists(path)? else {
        bail!("feature {id} not found");
    };
    let record: FeatureRecord = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if classify(&record.schema_version, FEATURE_SCHEMA_VERSION) != Compat::Exact {
        bail!(
            "schema mismatch for {}: expected {}, found {}",
            path.display(),
            FEATURE_SCHEMA_VERSION,
            record.schema_version
        );
    }
    Ok(record)
}

fn save_record(paths: &MaestroPaths, record: &FeatureRecord) -> Result<()> {
    let dir = feature_dir(paths, &record.id);
    ensure_dir(&dir)?;
    let contents = serde_yaml::to_string(record).context("failed to serialize feature record")?;
    let path = dir.join("feature.yaml");
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn load_amend_log(paths: &MaestroPaths, id: &str) -> Result<AmendLog> {
    let path = amend_log_path(paths, id);
    match read_to_string_if_exists(&path)? {
        Some(contents) => serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display())),
        None => Ok(AmendLog::default()),
    }
}

fn save_amend_log(paths: &MaestroPaths, id: &str, log: &AmendLog) -> Result<()> {
    let dir = feature_dir(paths, id);
    ensure_dir(&dir)?;
    let contents = serde_yaml::to_string(log).context("failed to serialize amend log")?;
    let path = dir.join("amend-log.yaml");
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))
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

/// Strict scan: every present record must parse and be schema-`Exact`.
fn scan_records_strict(paths: &MaestroPaths) -> Result<Vec<FeatureRecord>> {
    feature_ids(&paths.features_dir())?
        .iter()
        .map(|id| load_record(paths, id))
        .collect()
}

/// Tolerant scan: skip records that fail to read, parse, or schema-classify.
fn scan_records_tolerant(paths: &MaestroPaths) -> Vec<FeatureRecord> {
    let Ok(ids) = feature_ids(&paths.features_dir()) else {
        return Vec::new();
    };
    ids.iter()
        .filter_map(|id| load_record(paths, id).ok())
        .collect()
}
