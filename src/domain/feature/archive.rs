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
use crate::domain::card::store::{
    CardSnapshot, card_path, is_dir_backed, load_entries, load_with_snapshot,
    remove_dir_with_snapshot, save_entries,
};
use crate::domain::card::{archive_db, live_db};
use crate::domain::feature::registry::{
    archived_card_path, load_archived_record, load_record, validate_feature_id,
};
use crate::foundation::core::fs::{append_text_file, ensure_dir};
use crate::foundation::core::hash::sha256_prefixed;
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

struct DirArchiveMove {
    card_id: String,
    record_path: PathBuf,
    source_dir: PathBuf,
    target_dir: PathBuf,
    snapshot: CardSnapshot,
}

struct DbArchiveMove {
    card_id: String,
    source_relpath: PathBuf,
    expected_raw: String,
}

enum ArchiveMove {
    Dir(Box<DirArchiveMove>),
    Db(DbArchiveMove),
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

fn archive_feature_checked(
    paths: &MaestroPaths,
    id: &str,
    dry_run: bool,
    expected_live_card_hash: Option<&str>,
) -> Result<FeatureArchiveReport> {
    validate_feature_id(id)?;
    let live_card = card_path(paths, id);
    let archive_card = archived_card_path(paths, id);
    let live_db_card = if live_card.is_file() {
        None
    } else {
        live_db::resolve(paths, id)?
    };
    let live_card_snapshot = if live_card.is_file() {
        let bytes = fs::read(&live_card)
            .with_context(|| format!("failed to read {}", live_card.display()))?;
        Some((sha256_prefixed(&bytes), bytes))
    } else {
        live_db_card.as_ref().map(|db_card| {
            (
                sha256_prefixed(db_card.raw.as_bytes()),
                db_card.raw.as_bytes().to_vec(),
            )
        })
    };
    if let (Some(expected), Some((actual, _))) =
        (expected_live_card_hash, live_card_snapshot.as_ref())
        && expected != actual
    {
        bail!(
            "cannot archive {id} — target card changed since preflight (expected {expected}, found {actual}); re-run the command"
        );
    }

    let (record, feature_live) = if live_card.is_file() || live_db_card.is_some() {
        (load_record(paths, id)?, true)
    } else if archive_card.is_file() || archive_db::contains_card_id(paths, id)? {
        // Sweep re-run: the feature already moved; only stragglers remain.
        (load_archived_record(paths, id)?, false)
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
        if live_card.is_file()
            && let Some((snapshot_hash, snapshot_bytes)) = live_card_snapshot.as_ref()
        {
            let current_bytes = fs::read(&live_card)
                .with_context(|| format!("failed to re-read {}", live_card.display()))?;
            if current_bytes != *snapshot_bytes {
                let current_hash = sha256_prefixed(&current_bytes);
                bail!(
                    "cannot archive {id} — target card changed since preflight (expected {snapshot_hash}, found {current_hash}); re-run the command"
                );
            }
        } else if let Some(db_card) = live_db_card.as_ref() {
            let Some(current) = live_db::resolve(paths, id)? else {
                bail!(
                    "cannot archive {id} — target card changed since preflight; re-run the command"
                );
            };
            if current.raw != db_card.raw {
                let expected_hash = sha256_prefixed(db_card.raw.as_bytes());
                let current_hash = sha256_prefixed(current.raw.as_bytes());
                bail!(
                    "cannot archive {id} — target card changed since preflight (expected {expected_hash}, found {current_hash}); re-run the command"
                );
            }
        }
        // Pre-flight no-clobber over the whole move set, so a collision aborts
        // the run before anything moves. A child inside the feature container
        // rides the container move; only outside homes move individually.
        let mut moves = Vec::new();
        if feature_live {
            if let Some(db_card) = live_db_card.as_ref() {
                moves.push(ArchiveMove::Db(DbArchiveMove {
                    card_id: id.to_string(),
                    source_relpath: PathBuf::from(id),
                    expected_raw: db_card.raw.clone(),
                }));
            } else {
                moves.push(ArchiveMove::Dir(Box::new(DirArchiveMove {
                    card_id: id.to_string(),
                    record_path: live_card.clone(),
                    source_dir: container.clone(),
                    target_dir: paths.archive_cards_dir().join(id),
                    snapshot: load_with_snapshot(&live_card)?,
                })));
            }
        }
        for (child, path) in &terminal_children {
            if path.starts_with(&container) {
                continue;
            }
            if path.starts_with(live_db::db_file(paths)) {
                let Some(db_child) = live_db::resolve(paths, child)? else {
                    bail!(
                        "cannot archive {id} — child {child} changed since preflight; re-run the command"
                    );
                };
                moves.push(ArchiveMove::Db(DbArchiveMove {
                    card_id: child.clone(),
                    source_relpath: PathBuf::from(child),
                    expected_raw: db_child.raw,
                }));
            } else {
                let (src, dst) =
                    child_move(child, path, &paths.cards_dir(), &paths.archive_cards_dir())?;
                moves.push(ArchiveMove::Dir(Box::new(DirArchiveMove {
                    card_id: child.clone(),
                    record_path: path.clone(),
                    source_dir: src,
                    target_dir: dst,
                    snapshot: load_with_snapshot(path)?,
                })));
            }
        }
        for item in &moves {
            if let Some(target_dir) = item.archive_target_dir(paths)
                && target_dir.exists()
            {
                bail!(
                    "cannot archive {id} — an archived copy already exists at {}",
                    target_dir.display()
                );
            }
            if archive_db::contains_card_id(paths, item.card_id())? {
                bail!(
                    "cannot archive {id} — archived card {} already exists in the archive DB",
                    item.card_id()
                );
            }
        }
        for item in &moves {
            archive_and_remove(paths, item)?;
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

impl ArchiveMove {
    fn card_id(&self) -> &str {
        match self {
            ArchiveMove::Dir(item) => &item.card_id,
            ArchiveMove::Db(item) => &item.card_id,
        }
    }

    fn archive_target_dir(&self, paths: &MaestroPaths) -> Option<PathBuf> {
        match self {
            ArchiveMove::Dir(item) => Some(item.target_dir.clone()),
            ArchiveMove::Db(item) => Some(paths.archive_cards_dir().join(&item.source_relpath)),
        }
    }
}

fn archive_and_remove(paths: &MaestroPaths, item: &ArchiveMove) -> Result<()> {
    match item {
        ArchiveMove::Dir(item) => archive_and_remove_dir(paths, item),
        ArchiveMove::Db(item) => archive_and_remove_db(paths, item),
    }
}

fn archive_and_remove_dir(paths: &MaestroPaths, item: &DirArchiveMove) -> Result<()> {
    let relative = item
        .source_dir
        .strip_prefix(paths.cards_dir())
        .with_context(|| {
            format!(
                "failed to make {} relative to card store",
                item.source_dir.display()
            )
        })?;
    archive_db::archive_directory(paths, &item.card_id, &item.source_dir, relative)?;
    if let Err(error) = maybe_trigger_archive_race(item) {
        rollback_archive_snapshot(paths, &item.card_id)?;
        return Err(error);
    }
    if let Err(error) = remove_dir_with_snapshot(&item.record_path, &item.snapshot) {
        let detail = error.to_string();
        rollback_archive_snapshot(paths, &item.card_id)?;
        bail!(
            "failed to remove archived live card {}: {detail}",
            item.source_dir.display()
        );
    }
    Ok(())
}

fn archive_and_remove_db(paths: &MaestroPaths, item: &DbArchiveMove) -> Result<()> {
    let archived = live_db::archive_card_snapshot(paths, &item.card_id, &item.source_relpath)?;
    if archived.raw != item.expected_raw {
        rollback_archive_snapshot(paths, &item.card_id)?;
        let expected_hash = sha256_prefixed(item.expected_raw.as_bytes());
        let current_hash = sha256_prefixed(archived.raw.as_bytes());
        bail!(
            "cannot archive {} — target card changed since preflight (expected {expected_hash}, found {current_hash}); re-run the command",
            item.card_id
        );
    }
    if let Err(error) = live_db::remove_card_if_unchanged(paths, &archived.card, &archived.raw) {
        let detail = error.to_string();
        rollback_archive_snapshot(paths, &item.card_id)?;
        bail!(
            "failed to remove archived DB card {}: {detail}",
            item.card_id
        );
    }
    Ok(())
}

fn rollback_archive_snapshot(paths: &MaestroPaths, card_id: &str) -> Result<()> {
    archive_db::delete_snapshots(paths, &[card_id.to_string()])
        .with_context(|| format!("failed to roll back archive DB snapshot {card_id}"))?;
    Ok(())
}

#[cfg(debug_assertions)]
fn maybe_trigger_archive_race(item: &DirArchiveMove) -> Result<()> {
    if std::env::var("MAESTRO_TEST_ARCHIVE_RACE").ok().as_deref()
        != Some("feature-archive-stale-before-remove")
    {
        return Ok(());
    }
    if item.record_path.file_name().and_then(|name| name.to_str()) != Some("card.yaml") {
        return Ok(());
    }
    fs::write(
        &item.record_path,
        format!(
            "schema_version: maestro.card.v1\nid: {}\ntype: feature\ntitle: Race changed before archive remove\nstatus: cancelled\ncreated_at: \"1\"\nupdated_at: \"1\"\n",
            item.card_id
        ),
    )
    .with_context(|| {
        format!(
            "failed to plant racing archive change at {}",
            item.record_path.display()
        )
    })
}

#[cfg(not(debug_assertions))]
fn maybe_trigger_archive_race(_item: &DirArchiveMove) -> Result<()> {
    Ok(())
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
    let mut dir_moves = Vec::new();
    let mut db_moves = Vec::new();
    // Entry sweeps grouped by live container file, ids in scan (id) order.
    let mut entry_sweeps: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    let mut swept: Vec<String> = Vec::new();
    let mut lid_lines = String::new();
    let mut kept_rules: Vec<String> = Vec::new();
    let date = utc_now_timestamp()[..10].to_string();
    let live_db_file = live_db::db_file(paths);

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
        if path.starts_with(&live_db_file) {
            let Some(db_card) = live_db::resolve(paths, &card.id)? else {
                bail!(
                    "cannot sweep {} — DB-backed card changed since preflight; re-run the command",
                    card.id
                );
            };
            db_moves.push(DbArchiveMove {
                card_id: card.id.clone(),
                source_relpath: PathBuf::from(&card.id),
                expected_raw: db_card.raw,
            });
        } else if is_dir_backed(&path) {
            let (src, dst) = child_move(
                &card.id,
                &path,
                &paths.cards_dir(),
                &paths.archive_cards_dir(),
            )?;
            dir_moves.push(DirArchiveMove {
                card_id: card.id.clone(),
                record_path: path.clone(),
                source_dir: src,
                target_dir: dst,
                snapshot: load_with_snapshot(&path)?,
            });
        } else {
            entry_sweeps.entry(path).or_default().push(card.id.clone());
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
    for item in &dir_moves {
        if item.target_dir.exists() {
            bail!(
                "cannot sweep {} — an archived copy already exists at {}",
                item.card_id,
                item.target_dir.display()
            );
        }
        if archive_db::contains_card_id(paths, &item.card_id)? {
            bail!(
                "cannot sweep {} — an archived copy already exists in the archive DB",
                item.card_id
            );
        }
    }
    let mut entry_stages = Vec::new();
    for item in &db_moves {
        let target_dir = paths.archive_cards_dir().join(&item.source_relpath);
        if target_dir.exists() {
            bail!(
                "cannot sweep {} — an archived copy already exists at {}",
                item.card_id,
                target_dir.display()
            );
        }
        if archive_db::contains_card_id(paths, &item.card_id)? {
            bail!(
                "cannot sweep {} — an archived copy already exists in the archive DB",
                item.card_id
            );
        }
    }
    for (live_file, ids) in &entry_sweeps {
        let live = load_entries(live_file)?;
        let relative = live_file.strip_prefix(paths.cards_dir()).with_context(|| {
            format!("entry file outside the store root: {}", live_file.display())
        })?;
        for id in ids {
            if archive_db::contains_card_id(paths, id)? {
                bail!("cannot sweep {id} — an archived copy already exists in the archive DB");
            }
        }
        let (sweep, keep): (Vec<_>, Vec<_>) = live
            .cards
            .iter()
            .cloned()
            .partition(|card| ids.contains(&card.id));
        entry_stages.push((live_file.clone(), live, keep, relative.to_path_buf(), sweep));
    }

    for item in &dir_moves {
        archive_and_remove_dir(paths, item)?;
    }
    for item in &db_moves {
        archive_and_remove_db(paths, item)?;
    }
    for (live_file, live_snapshot, keep, relative, sweep) in &entry_stages {
        for card in sweep {
            let source_relpath = PathBuf::from("entries").join(relative).join(&card.id);
            archive_db::archive_virtual_card(paths, &card.id, card, &source_relpath)?;
        }
        save_entries(live_file, keep, live_snapshot)?;
    }
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
    let db_feature = archive_db::resolve(paths, id)?;
    if let Some(feature) = &db_feature
        && feature.card.card_type == CardType::Feature
    {
        let mut children: Vec<_> = archive_db::scan(paths)?
            .into_iter()
            .filter(|archived| {
                archived.card.parent.as_deref() == Some(id) && archived.card.card_type.workable()
            })
            .collect();
        children.sort_by(|a, b| a.card.id.cmp(&b.card.id));
        let mut snapshot_ids = vec![feature.snapshot_id.clone()];
        for child in &children {
            if child.snapshot_id != feature.snapshot_id {
                snapshot_ids.push(child.snapshot_id.clone());
            }
        }
        snapshot_ids.sort();
        snapshot_ids.dedup();
        archive_db::restore_snapshots(paths, &snapshot_ids)?;
        let restored: Vec<String> = children.into_iter().map(|child| child.card.id).collect();
        return Ok(unarchive_note(id, true, &restored));
    }

    let live_dir = paths.cards_dir().join(id);
    let archive_dir = paths.archive_cards_dir().join(id);
    let feature_archived = archived_card_path(paths, id).is_file();

    if !feature_archived && !card_path(paths, id).is_file() {
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
        let (src, dst) = child_move(child, path, &paths.archive_cards_dir(), &paths.cards_dir())?;
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

/// The movable directory pair for a child living outside the feature
/// container: its own record dir, mirrored at the same store-relative path
/// under the destination root. An entry-backed child has no directory of its
/// own to move -- only reachable by hand-editing a parent onto a root entry --
/// so the cascade aborts loud rather than guessing.
fn child_move(
    child: &str,
    record: &Path,
    from_root: &Path,
    to_root: &Path,
) -> Result<(PathBuf, PathBuf)> {
    if !is_dir_backed(record) {
        bail!(
            "cannot cascade {child} — it is an entry in {}; move it by hand",
            record.display()
        );
    }
    let dir = record
        .parent()
        .with_context(|| format!("record path missing parent: {}", record.display()))?;
    let relative = dir.strip_prefix(from_root).with_context(|| {
        format!(
            "child {child} lives outside the store root: {}",
            dir.display()
        )
    })?;
    Ok((dir.to_path_buf(), to_root.join(relative)))
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
