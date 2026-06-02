//! Move terminal features (and their terminal child tasks) to and from the
//! archive sibling tree (§5 L2/L3/L6 + §5.9 child-task cascade).
//!
//! A feature directory is named exactly `<id>` (no `<id>-<slug>` split, unlike
//! tasks), so the move is `features/<id>` ↔ `archive/features/<id>`. Child tasks
//! live in the separate `tasks/` tree, so `feature archive` cascade-archives the
//! feature's terminal children (parallel to the cancel cascade in
//! [`super::registry::cancel`]).
//!
//! The cascade order is **children first, feature last**: a partial archive then
//! leaves the feature in the live tree, so a re-run (`feature archive` is
//! idempotent) re-scans and sweeps the rest. The child sweep is *unconditional*
//! (it always scans the live `tasks/` tree), while only the feature-dir move is
//! gated on the feature still being live — so a child skipped by L6c on the
//! first run is still swept once its blocker clears, even though the feature dir
//! already moved.

use std::fs;

use anyhow::{Context, Result, bail};

use crate::domain::feature::registry::{load_record_at, validate_feature_id};
use crate::domain::task::{self, TaskState};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;

/// Archive a terminal feature and cascade-archive its terminal child tasks
/// (§5.9).
///
/// Resolves the record from the live tree, or the archive tree on a sweep
/// re-run. The child sweep always scans the live `tasks/` tree; a child a live
/// task still references through an unresolved blocker is **skipped with a
/// warning** (L6c), not blocked — clearing the reference and re-running sweeps
/// it. The feature dir moves only if it is still live.
///
/// Idempotent (§5.4): re-running on an already-archived feature with nothing
/// left to sweep is a no-op at exit 0.
///
/// # Errors
///
/// Errors when the feature is not found, is not terminal, has a live child task
/// (which a terminal feature cannot by construction), or a move fails.
pub fn archive_feature(paths: &MaestroPaths, id: &str, dry_run: bool) -> Result<String> {
    let live_yaml = paths.features_dir().join(id).join("feature.yaml");
    let archive_yaml = paths.archive_features_dir().join(id).join("feature.yaml");

    let (record, feature_live) = if live_yaml.is_file() {
        (load_record_at(&live_yaml, id)?, true)
    } else if archive_yaml.is_file() {
        // Sweep re-run: the feature already moved; only stragglers remain.
        (load_record_at(&archive_yaml, id)?, false)
    } else {
        bail!("feature not found: {id}");
    };

    if !record.status.is_terminal() {
        bail!(
            "cannot archive {id} — not terminal (status: {}); ship or cancel it first",
            record.status.as_str()
        );
    }

    let tasks_dir = paths.tasks_dir();
    let archive_tasks_dir = paths.archive_tasks_dir();

    // §5.9: child tasks live in the separate tasks/ tree. Partition the live
    // scan by liveness so a stray live child is refused defensively.
    let mut live_children = Vec::new();
    let mut terminal_children = Vec::new();
    for projection in task::load_feature_task_projections(&tasks_dir)? {
        if projection.feature_id.as_deref() != Some(id) {
            continue;
        }
        if projection.state.as_ref().is_some_and(TaskState::is_live) {
            live_children.push(projection.id);
        } else {
            terminal_children.push(projection.id);
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

    // Children first (fail-safe): the sweep is unconditional so a child skipped
    // by L6c on an earlier run still archives once its blocker clears.
    let mut archived = Vec::new();
    let mut skipped = Vec::new();
    for child in &terminal_children {
        if let Some(referrer) = task::live_task_referrer(&tasks_dir, child)? {
            skipped.push((child.clone(), referrer));
        } else if dry_run {
            archived.push(child.clone());
        } else {
            task::archive_task(&tasks_dir, &archive_tasks_dir, child, false)
                .with_context(|| format!("failed to archive child task {child} of feature {id}"))?;
            archived.push(child.clone());
        }
    }

    // Feature last: move a still-live feature dir; a sweep re-run leaves it put.
    let feature_changed = feature_live && !dry_run;
    if feature_changed {
        let live_dir = paths.features_dir().join(id);
        let archive_dir = paths.archive_features_dir().join(id);
        if archive_dir.exists() {
            bail!(
                "cannot archive {id} — an archived copy already exists at {}",
                archive_dir.display()
            );
        }
        ensure_dir(paths.archive_features_dir())?;
        fs::rename(&live_dir, &archive_dir).with_context(|| {
            format!(
                "failed to move {} to {}",
                live_dir.display(),
                archive_dir.display()
            )
        })?;
    }

    Ok(archive_note(id, dry_run, feature_live, &archived, &skipped))
}

/// Restore an archived feature and its archived child tasks (§5.9, symmetric).
///
/// Children first, feature last (same fail-safe order): a partial restore leaves
/// the feature archived so a re-run re-scans `archive/tasks/` and sweeps the
/// rest. Idempotent: an already-live feature with no archived children is a
/// no-op at exit 0.
///
/// # Errors
///
/// Errors when no archived feature has the given id, a live feature already
/// occupies the id, or a move fails.
pub fn unarchive_feature(paths: &MaestroPaths, id: &str) -> Result<String> {
    // Unlike `archive_feature`, this path never goes through `load_record_at`
    // (it stats the dir and renames directly), so guard the id here before any
    // join can escape the archive tree.
    validate_feature_id(id)?;
    let live_dir = paths.features_dir().join(id);
    let archive_dir = paths.archive_features_dir().join(id);
    let feature_archived = archive_dir.join("feature.yaml").is_file();

    if !feature_archived && !live_dir.join("feature.yaml").is_file() {
        bail!("archived feature not found: {id}");
    }

    let tasks_dir = paths.tasks_dir();
    let archive_tasks_dir = paths.archive_tasks_dir();

    // Children first: restore every archived task that names this feature.
    let mut restored = Vec::new();
    for projection in task::load_feature_task_projections(&archive_tasks_dir)? {
        if projection.feature_id.as_deref() == Some(id) {
            restored.push(projection.id);
        }
    }
    restored.sort();
    for child in &restored {
        task::unarchive_task(&tasks_dir, &archive_tasks_dir, child)
            .with_context(|| format!("failed to restore child task {child} of feature {id}"))?;
    }

    // Feature last.
    if feature_archived {
        if live_dir.exists() {
            bail!("cannot unarchive {id} — a live feature already occupies that id");
        }
        ensure_dir(paths.features_dir())?;
        fs::rename(&archive_dir, &live_dir).with_context(|| {
            format!(
                "failed to move {} to {}",
                archive_dir.display(),
                live_dir.display()
            )
        })?;
    }

    Ok(unarchive_note(id, feature_archived, &restored))
}

/// Compose the `feature archive` summary across first-run, sweep-re-run,
/// dry-run, and true no-op cases.
fn archive_note(
    id: &str,
    dry_run: bool,
    feature_live: bool,
    archived: &[String],
    skipped: &[(String, String)],
) -> String {
    // True no-op: feature already archived and nothing left to sweep.
    if !feature_live && archived.is_empty() && skipped.is_empty() {
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
    if !skipped.is_empty() {
        let detail = skipped
            .iter()
            .map(|(child, referrer)| format!("{child} (blocks live {referrer})"))
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!(
            "skipped {} live-referenced child task(s): {detail}; clear the reference and re-run `maestro feature archive {id}` to sweep",
            skipped.len()
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
