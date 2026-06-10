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

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card::query::{Coarse, coarse_of, scan_dir_with_paths, scan_with_paths};
use crate::domain::card::store::is_dir_backed;
use crate::domain::feature::registry::{load_archived_record, load_record, validate_feature_id};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureArchiveReport {
    pub note: String,
    pub child_tasks: usize,
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
    validate_feature_id(id)?;
    let live_card = paths.cards_dir().join(id).join("card.yaml");
    let archive_card = paths.archive_cards_dir().join(id).join("card.yaml");

    let (record, feature_live) = if live_card.is_file() {
        (load_record(paths, id)?, true)
    } else if archive_card.is_file() {
        // Sweep re-run: the feature already moved; only stragglers remain.
        (load_archived_record(paths, id)?, false)
    } else {
        bail!("feature not found: {id}");
    };

    if !record.status.is_terminal() {
        bail!(
            "cannot archive {id} — not terminal (status: {}); ship or cancel it first",
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
            "cannot archive {id} — {} live child task(s): {}; ship or cancel the feature first",
            live_children.len(),
            live_children.join(", ")
        );
    }
    terminal_children.sort();

    if !dry_run {
        // Pre-flight no-clobber over the whole move set, so a collision aborts
        // the run before anything moves. A child inside the feature container
        // rides the container move; only outside homes move individually.
        let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
        if feature_live {
            moves.push((container.clone(), paths.archive_cards_dir().join(id)));
        }
        for (child, path) in &terminal_children {
            if path.starts_with(&container) {
                continue;
            }
            moves.push(child_move(
                child,
                path,
                &paths.cards_dir(),
                &paths.archive_cards_dir(),
            )?);
        }
        for (_, target) in &moves {
            if target.exists() {
                bail!(
                    "cannot archive {id} — an archived copy already exists at {}",
                    target.display()
                );
            }
        }
        if !moves.is_empty() {
            ensure_dir(paths.archive_cards_dir())?;
        }
        for (src, dst) in &moves {
            if let Some(parent) = dst.parent() {
                ensure_dir(parent)?;
            }
            fs::rename(src, dst).with_context(|| {
                format!("failed to move {} to {}", src.display(), dst.display())
            })?;
        }
    }

    let archived: Vec<String> = terminal_children.into_iter().map(|(id, _)| id).collect();

    Ok(FeatureArchiveReport {
        note: archive_note(id, dry_run, feature_live, &archived),
        child_tasks: archived.len(),
    })
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
    let live_dir = paths.cards_dir().join(id);
    let archive_dir = paths.archive_cards_dir().join(id);
    let feature_archived = archive_dir.join("card.yaml").is_file();

    if !feature_archived && !live_dir.join("card.yaml").is_file() {
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
