use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::task::cards;
use crate::domain::task::template::{TaskRecord, TaskSnapshot};
use crate::foundation::core::paths::MaestroPaths;

/// Reconstruct the repo's [`MaestroPaths`] from a tasks directory so the
/// store-mode dispatch can reach the card store (mirrors `doctor`'s use of the
/// same parent walk). `tasks_dir` is `.maestro/tasks`, so its grandparent is the
/// repo root. An off-`.maestro` directory (e.g. the archive tasks tree) walks to
/// a non-repo-root whose `cards/` is absent, so it falls back to Legacy -- which
/// is exactly what the still-legacy archive scans need until P4.
pub(crate) fn paths_for_tasks_dir(tasks_dir: &Path) -> Option<MaestroPaths> {
    tasks_dir
        .parent()
        .and_then(Path::parent)
        .map(MaestroPaths::new)
}

/// Resolve a task id or unique id-prefix to its `task.yaml` path.
pub fn resolve_task_yaml_path(tasks_dir: &Path, id: &str) -> Result<PathBuf> {
    validate_task_lookup_id(id)?;

    let prefix = format!("{id}-");
    let mut matches = Vec::new();
    for task_path in task_yaml_paths(tasks_dir)? {
        let Some(name) = task_path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
        else {
            continue;
        };
        if name.starts_with(&prefix) {
            matches.push(task_path);
        }
    }
    matches.sort();

    match matches.len() {
        0 => {
            if archived_task_exists(tasks_dir, id)? {
                bail!(
                    "task {id} is archived\n  inspect: maestro task show {id}\n  restore: maestro task unarchive {id}\n  then: retry the command"
                );
            }
            bail!("task not found: {id}")
        }
        1 => Ok(matches.remove(0)),
        _ => bail!("task id {id} is ambiguous"),
    }
}

fn archived_task_exists(tasks_dir: &Path, id: &str) -> Result<bool> {
    let Some(maestro_dir) = tasks_dir.parent() else {
        return Ok(false);
    };
    let archive_tasks = maestro_dir.join("archive/tasks");
    let prefix = format!("{id}-");
    if archive_tasks.is_dir() {
        for task_path in task_yaml_paths_in_single_root(&archive_tasks)? {
            let Some(name) = task_path
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
            else {
                continue;
            };
            if name.starts_with(&prefix) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Return every managed task.yaml path under the standalone root and feature roots.
pub fn task_yaml_paths(tasks_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for root in task_roots(tasks_dir)? {
        collect_task_yaml_paths_in_root(&root, &mut paths)?;
    }
    paths.sort();
    Ok(paths)
}

fn task_yaml_paths_in_single_root(root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_task_yaml_paths_in_root(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

/// Return standalone and feature-owned task roots derived from the standalone root anchor.
pub(crate) fn task_roots(tasks_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut roots = vec![tasks_dir.to_path_buf()];
    roots.extend(feature_task_roots(tasks_dir)?);
    roots.sort();
    roots.dedup();
    Ok(roots)
}

fn collect_task_yaml_paths_in_root(root: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        let entry = entry.with_context(|| format!("failed to list {}", root.display()))?;
        if let Some(path) = task_yaml_path_for_entry(&entry)? {
            paths.push(path);
        }
    }
    Ok(())
}

fn feature_task_roots(tasks_dir: &Path) -> Result<Vec<PathBuf>> {
    let Some(parent) = tasks_dir.parent() else {
        return Ok(Vec::new());
    };
    let features_dir = parent.join("features");
    if !features_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut roots = Vec::new();
    for entry in fs::read_dir(&features_dir)
        .with_context(|| format!("failed to read {}", features_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", features_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if file_type.is_dir() && !file_type.is_symlink() {
            roots.push(entry.path().join("tasks"));
        }
    }
    roots.sort();
    Ok(roots)
}

pub fn feature_id_for_task_path(path: &Path) -> Option<String> {
    let task_dir = path.parent()?;
    let tasks_dir = task_dir.parent()?;
    if tasks_dir.file_name().and_then(|name| name.to_str()) != Some("tasks") {
        return None;
    }
    let feature_dir = tasks_dir.parent()?;
    let features_dir = feature_dir.parent()?;
    if features_dir.file_name().and_then(|name| name.to_str()) != Some("features") {
        return None;
    }
    feature_dir
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
}

/// Return a task entry's `task.yaml` path when the entry is a real directory.
pub fn task_yaml_path_for_entry(entry: &fs::DirEntry) -> Result<Option<PathBuf>> {
    let file_type = entry
        .file_type()
        .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
    if !file_type.is_dir() || file_type.is_symlink() {
        return Ok(None);
    }
    let path = entry.path().join("task.yaml");
    if valid_task_yaml_path(&path)? {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

/// Return true only for a real `task.yaml` file inside a real task directory.
pub fn valid_task_yaml_path(path: &Path) -> Result<bool> {
    let Some(task_dir) = path.parent() else {
        return Ok(false);
    };
    if fs::symlink_metadata(task_dir)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Ok(false);
    }
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(false);
    };
    Ok(metadata.is_file() && !metadata.file_type().is_symlink())
}

fn validate_task_lookup_id(id: &str) -> Result<()> {
    let mut components = Path::new(id).components();
    if id.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        bail!("invalid task id: {id}");
    }
    Ok(())
}

/// Load a task by id with its optimistic save snapshot. Reads the `Task`-typed
/// card at `cards/<id>/card.yaml` (no archive fallback -- the card archive tree
/// is its own scan).
pub fn load_task_with_snapshot(
    tasks_dir: &Path,
    id: &str,
) -> Result<(TaskRecord, TaskSnapshot, PathBuf)> {
    let paths =
        paths_for_tasks_dir(tasks_dir).context("cannot resolve maestro paths from tasks dir")?;
    validate_task_lookup_id(id)?;
    let Some((task, snapshot, path)) = cards::load_one(&paths, id)? else {
        bail!("task not found: {id}");
    };
    let task_dir = path
        .parent()
        .map(Path::to_path_buf)
        .context("card path is missing parent directory")?;
    Ok((
        task,
        TaskSnapshot::Card {
            path,
            snapshot: Box::new(snapshot),
        },
        task_dir,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tasks_dir_reports_not_found_not_a_raw_io_error() {
        // No tasks dir yet (fresh repo). The `task` CLI ensure_dir's it, but the
        // domain lookup must not leak an ENOENT path for callers that do not.
        let dir = Path::new("/nonexistent/maestro/tasks");
        let err = resolve_task_yaml_path(dir, "task-001")
            .expect_err("a missing tasks dir must not resolve to a path");
        let message = err.to_string();
        assert!(
            message.contains("task not found: task-001"),
            "expected a clean not-found, got: {message}"
        );
        assert!(
            !message.contains("failed to read"),
            "must not leak the raw read_dir error: {message}"
        );
    }
}
