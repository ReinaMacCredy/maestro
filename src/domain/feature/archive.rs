//! Move terminal feature cards, including their settled child cards, to and
//! from the archive sibling tree (§5 L2/L3/L6 + §5.9 child cascade).
//!
//! The cascade is a query (SPEC E4): the move set is the feature card plus
//! every task-kind card whose `parent` is the feature. In the container layout the
//! feature's own directory already bundles its pooled tasks, decision entries,
//! and prose, so moving `cards/<id>` moves them all; a child living OUTSIDE
//! the container (a pre-migration flat dir, or a root-pooled task) moves as
//! its own directory, mirrored to the same store-relative path under
//! `archive/cards/`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card::query::{Coarse, coarse_of, scan_dir_with_paths, scan_with_paths};
use crate::domain::card::schema::CardType;
use crate::domain::card::{self, ScannedArchiveItem};
use crate::domain::conflict;
use crate::domain::feature::registry::{
    list as list_features, load_archived_record, load_record, validate_feature_id,
};
use crate::foundation::core::fs::{append_text_file, ensure_dir};
use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::utc_now_timestamp;

/// First-write header of `archive/cards/INDEX.md`, shared by the feature
/// digest (A2) and the loose sweep (R2) so either writer can create the file.
const INDEX_HEADER: &str = "# Archived cards\n\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureArchiveReport {
    pub note: String,
    pub child_tasks: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoArchiveReceipt {
    pub feature_id: String,
    pub canonical_store_path: String,
    pub invoking_checkout_path: String,
    pub worker_source: String,
    pub target_card_hash: Option<String>,
    pub final_target_head: String,
    pub tested_head: String,
    pub authority_ref: String,
    pub merge_back_disposition: String,
    pub qa_result: String,
    pub run_id: String,
    pub event_id: String,
    pub event_hash: String,
    pub event_path: String,
    pub archive_path: String,
    pub restore_command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveCandidateAction {
    ArchiveNow,
    ReleaseOnly,
    NeedsClose,
    NeedsDecision,
    Blocked,
}

impl ArchiveCandidateAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ArchiveNow => "ARCHIVE_NOW",
            Self::ReleaseOnly => "RELEASE_ONLY",
            Self::NeedsClose => "NEEDS_CLOSE",
            Self::NeedsDecision => "NEEDS_DECISION",
            Self::Blocked => "BLOCKED",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveCandidate {
    pub id: String,
    pub title: String,
    pub status: String,
    pub action: ArchiveCandidateAction,
    pub reasons: Vec<String>,
    pub child_tasks: usize,
    pub archived: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ArchiveGateEvidence {
    pub authority_ref: Option<String>,
    pub authority_target: Option<String>,
    pub authority_head: Option<String>,
    pub authority_state: Option<String>,
    pub tested_head: Option<String>,
    pub qa_result: Option<String>,
    pub qa_evidence: Vec<String>,
    pub run_id: Option<String>,
    pub canonical_store: Option<String>,
    pub target_card_hash: Option<String>,
    pub allow_dirty_target_card: bool,
}

impl ArchiveGateEvidence {
    fn requires_exact_head(&self) -> bool {
        self.authority_ref.is_some()
            || self.authority_target.is_some()
            || self.authority_head.is_some()
            || self.authority_state.is_some()
            || self.tested_head.is_some()
            || self.qa_result.is_some()
            || !self.qa_evidence.is_empty()
            || self.run_id.is_some()
            || self.canonical_store.is_some()
            || self.target_card_hash.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArchiveApplySelection {
    One(String),
    All,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveApplyPlan {
    pub selection: ArchiveApplySelection,
    pub candidates: Vec<ArchiveCandidate>,
}

impl ArchiveApplyPlan {
    pub fn archive_targets(&self) -> Vec<String> {
        self.candidates
            .iter()
            .filter(|candidate| candidate.action == ArchiveCandidateAction::ArchiveNow)
            .map(|candidate| candidate.id.clone())
            .collect()
    }
}

/// Append the durable minimum receipt for an evidence-gated auto-archive.
///
/// This is intentionally separate from the older archive digest line: the digest
/// preserves the existing archive history shape, while this line ties the move
/// to the tested commit, authority, QA verdict, and run-ledger event.
pub fn append_auto_archive_receipt(
    paths: &MaestroPaths,
    receipt: &AutoArchiveReceipt,
) -> Result<String> {
    validate_feature_id(&receipt.feature_id)?;
    let date = utc_now_timestamp()[..10].to_string();
    let line = format!(
        "- {date} auto_archive {}: canonical_store `{}`; invoking_checkout `{}`; worker_source `{}`; target_card_hash `{}`; final_head `{}`; tested_head `{}`; authority `{}`; merge_back `{}`; qa `{}`; run `{}`; event `{}` `{}` at `{}`; archive `{}`; restore `{}`\n",
        receipt.feature_id,
        index_location_cell(paths, &receipt.canonical_store_path),
        index_location_cell(paths, &receipt.invoking_checkout_path),
        index_location_cell(paths, &receipt.worker_source),
        index_cell(receipt.target_card_hash.as_deref().unwrap_or("none")),
        index_cell(&receipt.final_target_head),
        index_cell(&receipt.tested_head),
        index_cell(&receipt.authority_ref),
        index_cell(&receipt.merge_back_disposition),
        index_cell(&receipt.qa_result),
        index_cell(&receipt.run_id),
        index_cell(&receipt.event_id),
        index_cell(&receipt.event_hash),
        index_cell(&receipt.event_path),
        index_cell(&receipt.archive_path),
        index_cell(&receipt.restore_command),
    );
    append_text_file(paths.archive_index_file(), INDEX_HEADER, &line)?;
    Ok(line)
}

/// Archive a terminal feature and its settled child cards (§5.9).
///
/// Resolves the record from the live tree, or the archive tree on a sweep
/// re-run. Children are the task-kind cards whose `parent` is the feature;
/// every member must be settled (coarse-closed) before anything moves.
///
/// Idempotent (§5.4): re-running on an already-archived feature with nothing
/// left to sweep is a no-op at exit 0.
///
/// # Errors
///
/// Errors when the feature is not found, is not terminal, has a live child,
/// an archived copy already occupies a target, or a move fails.
pub fn archive_feature(
    paths: &MaestroPaths,
    id: &str,
    dry_run: bool,
) -> Result<FeatureArchiveReport> {
    archive_feature_checked(paths, id, dry_run, None)
}

pub fn archive_feature_with_expected_hash(
    paths: &MaestroPaths,
    id: &str,
    dry_run: bool,
    expected_live_card_hash: Option<&str>,
) -> Result<FeatureArchiveReport> {
    archive_feature_checked(paths, id, dry_run, expected_live_card_hash)
}

pub fn archive_candidate(
    paths: &MaestroPaths,
    id: &str,
    evidence: &ArchiveGateEvidence,
) -> Result<ArchiveCandidate> {
    validate_feature_id(id)?;
    if let Ok(record) = load_record(paths, id) {
        return candidate_from_live_record(paths, record, evidence);
    }
    if let Ok(record) = load_archived_record(paths, id) {
        return Ok(ArchiveCandidate {
            id: record.id,
            title: record.title,
            status: record.status.as_str().to_string(),
            action: ArchiveCandidateAction::ReleaseOnly,
            reasons: vec![
                "target is already archived; release stale active ownership only".to_string(),
            ],
            child_tasks: 0,
            archived: true,
        });
    }
    Ok(ArchiveCandidate {
        id: id.to_string(),
        title: String::new(),
        status: "missing".to_string(),
        action: ArchiveCandidateAction::Blocked,
        reasons: vec![format!(
            "target feature is missing from current store `{}`; run from the owning/orchestrator checkout that owns the live target card",
            canonical_path(&paths.maestro_dir()).display()
        )],
        child_tasks: 0,
        archived: false,
    })
}

pub fn archive_candidates(
    paths: &MaestroPaths,
    evidence: &ArchiveGateEvidence,
) -> Result<Vec<ArchiveCandidate>> {
    let mut candidates = Vec::new();
    for view in list_features(paths)? {
        candidates.push(archive_candidate(paths, &view.id, evidence)?);
    }
    candidates.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(candidates)
}

pub fn archive_apply_plan(
    paths: &MaestroPaths,
    selection: ArchiveApplySelection,
    evidence: &ArchiveGateEvidence,
) -> Result<ArchiveApplyPlan> {
    let candidates = match &selection {
        ArchiveApplySelection::One(id) => vec![archive_candidate(paths, id, evidence)?],
        ArchiveApplySelection::All => archive_candidates(paths, evidence)?,
    };
    Ok(ArchiveApplyPlan {
        selection,
        candidates,
    })
}

fn candidate_from_live_record(
    paths: &MaestroPaths,
    record: crate::domain::feature::schema::FeatureRecord,
    evidence: &ArchiveGateEvidence,
) -> Result<ArchiveCandidate> {
    let (child_tasks, mut blockers) = archive_target_blockers(paths, &record.id)?;
    let action = if !record.status.is_terminal() {
        if matches!(
            record.status,
            crate::domain::feature::schema::FeatureStatus::Proposed
                | crate::domain::feature::schema::FeatureStatus::Ready
        ) {
            blockers.push(format!(
                "not terminal (status: {}); decide whether to close, cancel, or keep it live",
                record.status.as_str()
            ));
            ArchiveCandidateAction::NeedsDecision
        } else {
            blockers.push(format!(
                "not terminal (status: {}); close or cancel it first",
                record.status.as_str()
            ));
            ArchiveCandidateAction::NeedsClose
        }
    } else {
        blockers.extend(exact_head_gate_blockers(paths, &record.id, evidence)?);
        if blockers.is_empty() {
            ArchiveCandidateAction::ArchiveNow
        } else {
            ArchiveCandidateAction::Blocked
        }
    };

    let reasons = if blockers.is_empty() {
        vec!["terminal feature with no archive blockers".to_string()]
    } else {
        blockers
    };

    Ok(ArchiveCandidate {
        id: record.id,
        title: record.title,
        status: record.status.as_str().to_string(),
        action,
        reasons,
        child_tasks,
        archived: false,
    })
}

fn archive_target_blockers(paths: &MaestroPaths, id: &str) -> Result<(usize, Vec<String>)> {
    let mut terminal_children = 0usize;
    let mut blockers = Vec::new();
    for (card, _path) in scan_with_paths(paths)? {
        if !card.card_type.workable() {
            continue;
        }
        let linked_to_target =
            card.parent.as_deref() == Some(id) || card.deps.iter().any(|dep| dep.target == id);
        if !linked_to_target {
            continue;
        }
        let coarse = coarse_of(&card.status);
        if card.parent.as_deref() == Some(id) && coarse == Some(Coarse::Closed) {
            terminal_children += 1;
        }
        if coarse != Some(Coarse::Closed) {
            let claimed = card
                .claimed_by
                .as_deref()
                .map(|owner| format!(", claimed_by={owner}"))
                .unwrap_or_default();
            blockers.push(format!("{} status={}{}", card.id, card.status, claimed));
        }
    }
    blockers.sort();
    if !blockers.is_empty() {
        blockers = vec![format!(
            "live or claimed descendant/linked work item(s): {}",
            blockers.join(", ")
        )];
    }
    Ok((terminal_children, blockers))
}

fn exact_head_gate_blockers(
    paths: &MaestroPaths,
    id: &str,
    evidence: &ArchiveGateEvidence,
) -> Result<Vec<String>> {
    let mut blockers = archive_conflict_blockers(paths, id)?;
    if !evidence.requires_exact_head() {
        return Ok(blockers);
    }

    let required = [
        ("authority ref", evidence.authority_ref.as_deref()),
        ("authority target", evidence.authority_target.as_deref()),
        ("authority head", evidence.authority_head.as_deref()),
        ("authority state", evidence.authority_state.as_deref()),
        ("tested head", evidence.tested_head.as_deref()),
        ("qa result", evidence.qa_result.as_deref()),
        ("canonical store", evidence.canonical_store.as_deref()),
    ];
    for (name, value) in required {
        if value.is_none_or(|value| value.trim().is_empty()) {
            blockers.push(format!("missing {name} for exact-HEAD archive authority"));
        }
    }
    if evidence.qa_evidence.is_empty() {
        blockers.push("missing QA evidence for exact-HEAD archive authority".to_string());
    }
    if !blockers.is_empty() {
        return Ok(blockers);
    }

    let authority_target = evidence.authority_target.as_deref().unwrap_or_default();
    if authority_target != id {
        blockers.push(format!(
            "authority target `{authority_target}` does not match feature id `{id}`"
        ));
    }
    let authority_state = evidence.authority_state.as_deref().unwrap_or_default();
    if authority_state != "current" {
        let authority_ref = evidence.authority_ref.as_deref().unwrap_or("<unknown>");
        blockers.push(format!(
            "authority `{authority_ref}` is `{authority_state}`, not current"
        ));
    }
    let qa_result = evidence.qa_result.as_deref().unwrap_or_default();
    if !qa_passed(qa_result) {
        blockers.push(format!("QA result must be pass/passed, got `{qa_result}`"));
    }

    let current_store_path = canonical_path(&paths.maestro_dir());
    let canonical_store_path = canonical_path(Path::new(
        evidence.canonical_store.as_deref().unwrap_or_default(),
    ));
    if current_store_path != canonical_store_path {
        blockers.push(format!(
            "current store `{}` is not canonical store `{}`",
            current_store_path.display(),
            canonical_store_path.display()
        ));
    }

    if let Some(expected_hash) = evidence.target_card_hash.as_deref() {
        match live_target_card_hash(paths, id)? {
            Some(actual_hash) if actual_hash == expected_hash => {}
            Some(actual_hash) => blockers.push(format!(
                "target card changed since preflight (expected {expected_hash}, found {actual_hash})"
            )),
            None => blockers.push(format!(
                "target feature is missing from current store `{}`",
                current_store_path.display()
            )),
        }
    }

    match git::snapshot(paths.repo_root()) {
        Ok(snapshot) => {
            let Some(current_head) = snapshot.head.as_deref() else {
                blockers.push("git HEAD is unborn; commit the delivered work first".to_string());
                return Ok(blockers);
            };
            let tested_head = evidence.tested_head.as_deref().unwrap_or_default();
            if current_head != tested_head {
                blockers.push(format!(
                    "tested head {tested_head} does not match current HEAD {current_head}"
                ));
            }
            let authority_head = evidence.authority_head.as_deref().unwrap_or_default();
            if authority_head != current_head {
                blockers.push(format!(
                    "authority head {authority_head} does not match current HEAD {current_head}"
                ));
            }
            let allowed_dirty_paths =
                archive_allowed_dirty_paths(id, evidence.allow_dirty_target_card);
            let run_id = evidence.run_id.as_deref().unwrap_or_default();
            let relevant_dirty = relevant_dirty_paths(
                &snapshot.dirty_paths,
                id,
                run_id,
                &evidence.qa_evidence,
                &allowed_dirty_paths,
            );
            if !relevant_dirty.is_empty() {
                blockers.push(format!(
                    "relevant dirty path(s) at {current_head}: {}",
                    relevant_dirty.join(", ")
                ));
            }
        }
        Err(error) => blockers.push(format!(
            "git state is unavailable: {error:#}; commit the delivered work first"
        )),
    }

    Ok(blockers)
}

fn archive_conflict_blockers(paths: &MaestroPaths, id: &str) -> Result<Vec<String>> {
    let roots = git::worktree_roots(paths.repo_root())
        .unwrap_or_else(|_| vec![paths.repo_root().to_path_buf()]);
    let root_paths = roots.iter().map(MaestroPaths::new).collect::<Vec<_>>();
    let now = utc_now_timestamp();
    let mut conflicts = conflict::active_notices(&root_paths, &now)?
        .into_iter()
        .filter(|notice| notice.peer_card == id || notice.asserter_card == id)
        .map(|notice| {
            format!(
                "{}->{}: {}",
                notice.asserter_session, notice.peer_card, notice.reason
            )
        })
        .collect::<Vec<_>>();
    conflicts.sort();
    if conflicts.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(vec![format!(
            "unresolved Maestro conflict(s): {}",
            conflicts.join(", ")
        )])
    }
}

fn live_target_card_hash(paths: &MaestroPaths, id: &str) -> Result<Option<String>> {
    card::live_card_hash(paths, id)
}

fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn archive_allowed_dirty_paths(id: &str, allow_dirty_target_card: bool) -> Vec<PathBuf> {
    if allow_dirty_target_card {
        vec![
            Path::new(".maestro")
                .join("cards")
                .join(id)
                .join("card.yaml"),
        ]
    } else {
        Vec::new()
    }
}

fn relevant_dirty_paths(
    dirty_paths: &[PathBuf],
    id: &str,
    run_id: &str,
    qa_evidence: &[String],
    allowed_dirty_paths: &[PathBuf],
) -> Vec<String> {
    let evidence_paths = qa_evidence_paths(qa_evidence);
    dirty_paths
        .iter()
        .filter(|path| {
            !allowed_dirty_paths.iter().any(|allowed| *path == allowed)
                && path_is_auto_archive_relevant(path, id, run_id, &evidence_paths)
        })
        .map(|path| path.display().to_string())
        .collect()
}

fn qa_evidence_paths(qa_evidence: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for item in qa_evidence {
        for token in item.split_whitespace() {
            let Some(value) = token
                .strip_prefix("path=")
                .or_else(|| token.strip_prefix("paths="))
            else {
                continue;
            };
            for path in value
                .split(',')
                .map(str::trim)
                .filter(|path| !path.is_empty())
            {
                paths.push(PathBuf::from(path));
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn path_is_auto_archive_relevant(
    path: &Path,
    id: &str,
    run_id: &str,
    evidence_paths: &[PathBuf],
) -> bool {
    path.starts_with(Path::new(".maestro").join("cards").join(id))
        || path == Path::new(".maestro/archive/cards/INDEX.md")
        || path.starts_with(Path::new(".maestro").join("archive").join("cards").join(id))
        || (!run_id.is_empty() && path.starts_with(Path::new(".maestro").join("runs").join(run_id)))
        || evidence_paths
            .iter()
            .any(|evidence_path| path == evidence_path || path.starts_with(evidence_path))
}

fn qa_passed(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "pass" | "passed"
    )
}

fn archive_feature_checked(
    paths: &MaestroPaths,
    id: &str,
    dry_run: bool,
    expected_live_card_hash: Option<&str>,
) -> Result<FeatureArchiveReport> {
    validate_feature_id(id)?;
    let live_card_hash = live_target_card_hash(paths, id)?;
    if let (Some(expected), Some(actual)) = (expected_live_card_hash, live_card_hash.as_ref())
        && expected != actual
    {
        bail!(
            "cannot archive {id} — target card changed since preflight (expected {expected}, found {actual}); re-run the command"
        );
    }

    let feature_live = card::live_card_exists(paths, id)?;
    let feature_archived = card::archived_card_exists(paths, id)?;
    let record = if feature_live {
        load_record(paths, id)?
    } else if feature_archived {
        // Sweep re-run: the feature already moved; only stragglers remain.
        load_archived_record(paths, id)?
    } else {
        bail!("feature not found: {id}");
    };

    if !record.status.is_terminal() {
        bail!(
            "cannot archive {id} — not terminal (status: {}); close or cancel it first",
            record.status.as_str()
        );
    }

    // Children are linked by `parent`, wherever they live. Partition by
    // coarse liveness so the set moves only after every member is settled.
    // Only task-kind children gate the move: decision/idea entries are records
    // of rulings, not workable children — an open fork on a cancelled feature
    // must not wedge archive. They live in the container files and ride the
    // directory move.
    let container = paths.cards_dir().join(id);
    let mut live_children = Vec::new();
    let mut terminal_children = Vec::new();
    for (card, path) in scan_with_paths(paths)? {
        if card.parent.as_deref() != Some(id) {
            continue;
        }
        if !card.card_type.workable() {
            continue;
        }
        if coarse_of(&card.status) == Some(Coarse::Closed) {
            terminal_children.push((card.id, path));
        } else {
            live_children.push(card.id);
        }
    }
    if !live_children.is_empty() {
        live_children.sort();
        bail!(
            "cannot archive {id} — {} live child task(s): {}; close or cancel the feature first",
            live_children.len(),
            live_children.join(", ")
        );
    }
    terminal_children.sort();

    if !dry_run {
        // Pre-flight no-clobber over the whole move set, so a collision aborts
        // the run before anything moves. A child inside the feature container
        // rides the container move; only outside homes move individually.
        let mut moves = Vec::new();
        if feature_live {
            let Some(feature_move) = card::prepare_live_archive_move(paths, id)? else {
                bail!(
                    "cannot archive {id} — target card changed since preflight; re-run the command"
                );
            };
            moves.push(feature_move);
        }
        for (child, path) in &terminal_children {
            if path.starts_with(&container) {
                continue;
            }
            moves.push(card::prepare_scanned_archive_move(
                paths,
                child,
                path,
                &paths.cards_dir(),
                &paths.archive_cards_dir(),
            )?);
        }
        for item in &moves {
            let target_dir = item.archive_target_dir(paths);
            if target_dir.exists() {
                bail!(
                    "cannot archive {id} — an archived copy already exists at {}",
                    target_dir.display()
                );
            }
            if card::archive_contains_card_id(paths, item.card_id())? {
                bail!(
                    "cannot archive {id} — archived card {} already exists in the archive DB",
                    item.card_id()
                );
            }
        }
        for item in &moves {
            card::archive_card_move(paths, item)?;
        }
        // SPEC-archive-memory A2: one digest line per archived feature, after
        // the moves succeed and only on the feature-moving run -- a sweep
        // re-run (feature already archived) must not duplicate it. "closed"
        // is the coarse word (DN3); the outcome is the write-once
        // `close --outcome` line.
        if feature_live {
            let outcome = record.outcome.as_deref().unwrap_or("no outcome recorded");
            let line = format!(
                "- {} {id}: closed -- {outcome}; {} child task(s)\n",
                &utc_now_timestamp()[..10],
                terminal_children.len()
            );
            append_text_file(paths.archive_index_file(), INDEX_HEADER, &line)?;
        }
    }

    let archived: Vec<String> = terminal_children.into_iter().map(|(id, _)| id).collect();

    Ok(FeatureArchiveReport {
        note: archive_note(id, dry_run, feature_live, &archived),
        child_tasks: archived.len(),
    })
}

/// What `maestro archive --loose` did: swept ids (boxed) and the locked loose
/// decisions deliberately left live (kept rules).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LooseSweepReport {
    pub swept: Vec<String>,
    pub kept_rules: Vec<String>,
}

/// Sweep terminal parentless cards into the archive (SPEC-archive-memory-2 R2).
///
/// Loose means parent-less and not a feature. Workable cards and ideas sweep
/// once coarse-closed; decisions sweep only when `superseded` -- a `locked`
/// loose decision is standing law and stays live, reported as a kept rule.
/// Every swept card appends one lid line to `archive/cards/INDEX.md`.
///
/// Dir-backed cards move like cascade children (same store-relative path under
/// `archive/cards/`). Entry-backed cards move between container files: the
/// archive-side append commits before the live-side removal, so a torn run
/// leaves a duplicate to clean up rather than losing the card.
///
/// Idempotent: a store with nothing loose to sweep is a no-op at exit 0.
pub fn archive_loose(paths: &MaestroPaths) -> Result<LooseSweepReport> {
    let mut moves = Vec::new();
    // Entry sweeps grouped by live container file, ids in scan (id) order.
    let mut entry_sweeps: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    let mut swept: Vec<String> = Vec::new();
    let mut lid_lines = String::new();
    let mut kept_rules: Vec<String> = Vec::new();
    let date = utc_now_timestamp()[..10].to_string();

    for (card, path) in scan_with_paths(paths)? {
        if card.parent.is_some() || card.card_type == CardType::Feature {
            continue;
        }
        let sweeps = match card.card_type {
            CardType::Decision => match card.status.as_str() {
                "superseded" => true,
                "locked" => {
                    kept_rules.push(card.id.clone());
                    false
                }
                _ => false,
            },
            _ => coarse_of(&card.status) == Some(Coarse::Closed),
        };
        if !sweeps {
            continue;
        }
        match card::prepare_scanned_archive_item(
            paths,
            &card.id,
            &path,
            &paths.cards_dir(),
            &paths.archive_cards_dir(),
        )? {
            ScannedArchiveItem::Move(item) => moves.push(item),
            ScannedArchiveItem::Entry { file, id } => {
                entry_sweeps.entry(file).or_default().push(id);
            }
        }
        lid_lines.push_str(&format!(
            "- {date} {}: {} -- {}\n",
            card.id, card.status, card.title
        ));
        swept.push(card.id);
    }

    if swept.is_empty() {
        return Ok(LooseSweepReport { swept, kept_rules });
    }

    // Pre-flight the whole sweep before anything moves: dir targets must be
    // free and no archive container may already hold a swept id.
    for item in &moves {
        let target_dir = item.archive_target_dir(paths);
        if target_dir.exists() {
            bail!(
                "cannot sweep {} — an archived copy already exists at {}",
                item.card_id(),
                target_dir.display()
            );
        }
        if card::archive_contains_card_id(paths, item.card_id())? {
            bail!(
                "cannot sweep {} — an archived copy already exists in the archive DB",
                item.card_id()
            );
        }
    }
    let entry_stages = card::prepare_entry_archive_stages(paths, entry_sweeps)?;

    for item in &moves {
        card::archive_card_move(paths, item)?;
    }
    card::archive_entry_stages(paths, &entry_stages)?;
    append_text_file(paths.archive_index_file(), INDEX_HEADER, &lid_lines)?;

    Ok(LooseSweepReport { swept, kept_rules })
}

/// Restore an archived feature and its archived child cards (§5.9, symmetric).
///
/// Children are the archived task-kind cards whose `parent` is the feature;
/// each member directory moves back to the live store. Idempotent: an
/// already-live feature with no archived children is a no-op at exit 0.
///
/// # Errors
///
/// Errors when no archived feature has the given id, a live card already
/// occupies a target id, or a move fails.
pub fn unarchive_feature(paths: &MaestroPaths, id: &str) -> Result<String> {
    validate_feature_id(id)?;
    if let Some(restored) = card::restore_archived_feature_snapshot(paths, id)? {
        return Ok(unarchive_note(id, true, &restored));
    }

    let live_dir = paths.cards_dir().join(id);
    let archive_dir = paths.archive_cards_dir().join(id);
    let feature_archived = card::archived_dir_card_exists(paths, id);

    if !feature_archived && !card::live_dir_card_exists(paths, id) {
        bail!("archived feature not found: {id}");
    }

    // Same task-kind cut as the archive side, so round-trip receipts agree.
    let mut children: Vec<(String, PathBuf)> = scan_dir_with_paths(&paths.archive_cards_dir())?
        .into_iter()
        .filter(|(card, _)| card.parent.as_deref() == Some(id) && card.card_type.workable())
        .map(|(card, path)| (card.id, path))
        .collect();
    children.sort();

    // Pre-flight no-clobber over the whole restore set before anything moves.
    // A child inside the archived container rides the container move back.
    let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
    if feature_archived {
        if live_dir.exists() {
            bail!("cannot unarchive {id} — a live feature already occupies that id");
        }
        moves.push((archive_dir.clone(), live_dir));
    }
    for (child, path) in &children {
        if path.starts_with(&archive_dir) {
            continue;
        }
        let (src, dst) =
            card::card_dir_move(child, path, &paths.archive_cards_dir(), &paths.cards_dir())?;
        if dst.exists() {
            bail!(
                "cannot unarchive {id} — a live copy of {child} already occupies {}",
                dst.display()
            );
        }
        moves.push((src, dst));
    }
    if !moves.is_empty() {
        ensure_dir(paths.cards_dir())?;
    }
    for (src, dst) in &moves {
        if let Some(parent) = dst.parent() {
            ensure_dir(parent)?;
        }
        fs::rename(src, dst)
            .with_context(|| format!("failed to move {} to {}", src.display(), dst.display()))?;
    }

    let restored: Vec<String> = children.into_iter().map(|(id, _)| id).collect();
    Ok(unarchive_note(id, feature_archived, &restored))
}

/// Compose the `feature archive` summary across first-run, sweep-re-run,
/// dry-run, and true no-op cases.
fn archive_note(id: &str, dry_run: bool, feature_live: bool, archived: &[String]) -> String {
    // True no-op: feature already archived and nothing left to sweep.
    if !feature_live && archived.is_empty() {
        return format!("already archived: {id}");
    }

    let mut parts = Vec::new();
    if feature_live {
        let verb = if dry_run { "would archive" } else { "archived" };
        parts.push(format!("{verb} feature {id}"));
    } else {
        let tail = if dry_run {
            "; would sweep remaining child task(s)"
        } else {
            ""
        };
        parts.push(format!("feature {id} already archived{tail}"));
    }
    if !archived.is_empty() {
        let verb = if dry_run {
            "would archive"
        } else if feature_live {
            "archived"
        } else {
            "swept"
        };
        parts.push(format!(
            "{verb} {} child task(s): {}",
            archived.len(),
            archived.join(", ")
        ));
    }
    parts.join("; ")
}

fn index_cell(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('`', "'")
}

fn index_location_cell(paths: &MaestroPaths, value: &str) -> String {
    let repo = paths
        .repo_root()
        .canonicalize()
        .unwrap_or_else(|_| paths.repo_root().to_path_buf());
    let repo = repo.display().to_string();
    let raw_repo = paths.repo_root().display().to_string();
    let value = value.replace(&repo, ".").replace(&raw_repo, ".");
    index_cell(&value)
}

/// Compose the `feature unarchive` summary.
fn unarchive_note(id: &str, feature_changed: bool, restored: &[String]) -> String {
    if !feature_changed && restored.is_empty() {
        return format!("already live: {id}");
    }
    let mut parts = Vec::new();
    if feature_changed {
        parts.push(format!("unarchived feature {id}"));
    } else {
        parts.push(format!("feature {id} already live"));
    }
    if !restored.is_empty() {
        parts.push(format!(
            "restored {} child task(s): {}",
            restored.len(),
            restored.join(", ")
        ));
    }
    parts.join("; ")
}
