//! Move terminal tasks to and from the archive sibling tree (§5 L2/L3/L6).
//!
//! Archiving is a directory move out of the live `tasks/` scan into
//! `archive/tasks/`; unarchiving moves it back. The live scans skip `archive/`
//! for free (it is a sibling tree), so the move itself is the "scanner skip."

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::domain::task::doctor::load_task_records;
use crate::domain::task::lookup::{feature_id_for_task_path, resolve_task_yaml_path};
use crate::domain::task::template::{BlockerKind, load_task};
use crate::foundation::core::fs::ensure_dir;

/// Archive `id`: move `tasks/<id>` → `archive/tasks/<id>` (§5.3).
///
/// Idempotent (§5.4): an already-archived id is a no-op at exit 0. Refuses a
/// non-terminal task (only done tasks archive) or one a live task still
/// references through an unresolved blocker (L6c), naming the referrer.
pub fn archive_task(
    tasks_dir: &Path,
    archive_tasks_dir: &Path,
    id: &str,
    dry_run: bool,
) -> Result<String> {
    let Ok(task_path) = resolve_task_yaml_path(tasks_dir, id) else {
        // Not in the live tree: either already archived (no-op) or unknown.
        if resolve_task_yaml_path(archive_tasks_dir, id).is_ok() {
            return Ok(format!("already archived: {id}"));
        }
        bail!("task not found: {id}");
    };
    let task_dir = task_path
        .parent()
        .context("resolved task path is missing its directory")?;
    // The directory is named `<id>-<slug>` (move target), but the canonical id
    // in the record is what blockers reference and users type (match/messages).
    let dir_name = dir_name(task_dir)?;
    let (mut task, _) = load_task(&task_path)?;
    task.feature_id = feature_id_for_task_path(&task_path);
    let id = task.id.as_str();

    if task.state.is_live() {
        bail!(
            "cannot archive {id} — not done (state: {}); reject, abandon, supersede, or verify it first",
            task.state.as_str()
        );
    }
    if let Some(referrer) = live_task_referrer(tasks_dir, id)? {
        bail!(
            "cannot archive {id} — {referrer} is blocked by it; run `maestro task unblock {referrer} --blocker <blocker-id>` first (see `maestro task show {referrer}` for the blocker id)"
        );
    }

    if dry_run {
        return Ok(format!("would archive {id}"));
    }
    let target_root = archive_root_for_task(archive_tasks_dir, task.feature_id.as_deref())?;
    let target = target_root.join(&dir_name);
    if target.exists() {
        bail!(
            "cannot archive {id} — an archived copy already exists at {}",
            target.display()
        );
    }
    ensure_dir(&target_root)?;
    fs::rename(task_dir, &target).with_context(|| {
        format!(
            "failed to move {} to {}",
            task_dir.display(),
            target.display()
        )
    })?;
    Ok(format!("archived {id}"))
}

/// Unarchive `id`: move `archive/tasks/<id>` → `tasks/<id>` (§5.4, symmetric).
///
/// Idempotent: an already-live id is a no-op at exit 0.
pub fn unarchive_task(tasks_dir: &Path, archive_tasks_dir: &Path, id: &str) -> Result<String> {
    if resolve_task_yaml_path(tasks_dir, id).is_ok() {
        return Ok(format!("already live: {id}"));
    }
    let Ok(task_path) = resolve_task_yaml_path(archive_tasks_dir, id) else {
        bail!("archived task not found: {id}");
    };
    let archived_dir = task_path
        .parent()
        .context("resolved task path is missing its directory")?;
    let dir_name = dir_name(archived_dir)?;
    let (mut task, _) = load_task(&task_path)?;
    task.feature_id = feature_id_for_task_path(&task_path);
    let target_root = live_root_for_task(tasks_dir, task.feature_id.as_deref())?;
    let target = target_root.join(&dir_name);
    if target.exists() {
        bail!(
            "cannot unarchive {} — a live task already occupies that id",
            task.id
        );
    }
    ensure_dir(&target_root)?;
    fs::rename(archived_dir, &target).with_context(|| {
        format!(
            "failed to move {} to {}",
            archived_dir.display(),
            target.display()
        )
    })?;
    Ok(format!("unarchived {}", task.id))
}

/// Id of the lowest-numbered live task whose unresolved blocker names
/// `target_id` (L6c), or `None` when no live item references it. Shared with the
/// feature cascade so an entangled child is skipped, not blocked (§5.9).
pub(crate) fn live_task_referrer(tasks_dir: &Path, target_id: &str) -> Result<Option<String>> {
    let mut referrers: Vec<String> = load_task_records(tasks_dir)?
        .into_iter()
        .filter(|task| task.id != target_id && task.state.is_live())
        .filter(|task| {
            task.blockers.iter().any(|blocker| {
                blocker.resolved_at.is_none()
                    && blocker.blocked_ref.as_ref().is_some_and(|blocked_ref| {
                        blocked_ref.kind == BlockerKind::Task && blocked_ref.id == target_id
                    })
            })
        })
        .map(|task| task.id)
        .collect();
    referrers.sort();
    Ok(referrers.into_iter().next())
}

/// The on-disk directory name (`<id>-<slug>`), preserved across the move so
/// id-prefix lookups keep resolving the archived task.
fn dir_name(task_dir: &Path) -> Result<String> {
    task_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .with_context(|| {
            format!(
                "archive: cannot read directory name from {}",
                task_dir.display()
            )
        })
}

fn archive_root_for_task(
    archive_tasks_dir: &Path,
    feature_id: Option<&str>,
) -> Result<std::path::PathBuf> {
    let Some(feature_id) = feature_id else {
        return Ok(archive_tasks_dir.to_path_buf());
    };
    let archive_dir = archive_tasks_dir
        .parent()
        .context("archive tasks path is missing parent")?;
    Ok(archive_dir.join("features").join(feature_id).join("tasks"))
}

fn live_root_for_task(tasks_dir: &Path, feature_id: Option<&str>) -> Result<std::path::PathBuf> {
    let Some(feature_id) = feature_id else {
        return Ok(tasks_dir.to_path_buf());
    };
    let maestro_dir = tasks_dir.parent().context("tasks path is missing parent")?;
    Ok(maestro_dir.join("features").join(feature_id).join("tasks"))
}
