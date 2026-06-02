use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::task::template::{TaskRecord, TaskSnapshot, load_task};

/// Resolve a task id or unique id-prefix to its `task.yaml` path.
pub fn resolve_task_yaml_path(tasks_dir: &Path, id: &str) -> Result<PathBuf> {
    validate_task_lookup_id(id)?;
    let direct = tasks_dir.join(id).join("task.yaml");
    if valid_task_yaml_path(&direct)? {
        return Ok(direct);
    }

    let prefix = format!("{id}-");
    let mut matches = Vec::new();
    // A missing tasks dir (fresh repo) has no entries to match: report the id, not
    // a raw ENOENT path. Mirrors load_task_entries, which guards its read the same way.
    if !tasks_dir.is_dir() {
        bail!("task not found: {id}");
    }
    for entry in fs::read_dir(tasks_dir)
        .with_context(|| format!("failed to read {}", tasks_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", tasks_dir.display()))?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name.starts_with(&prefix) {
            let Some(path) = task_yaml_path_for_entry(&entry)? else {
                continue;
            };
            if valid_task_yaml_path(&path)? {
                matches.push(path);
            }
        }
    }
    matches.sort();

    match matches.len() {
        0 => bail!("task not found: {id}"),
        1 => Ok(matches.remove(0)),
        _ => bail!("task id {id} is ambiguous"),
    }
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

/// Load a task by id or unique id-prefix with its optimistic save snapshot.
pub fn load_task_with_snapshot(
    tasks_dir: &Path,
    id: &str,
) -> Result<(TaskRecord, TaskSnapshot, PathBuf)> {
    let task_path = resolve_task_yaml_path(tasks_dir, id)?;
    let task_dir = task_path
        .parent()
        .map(Path::to_path_buf)
        .context("task path is missing parent directory")?;
    let (task, snapshot) = load_task(&task_path)?;
    Ok((task, snapshot, task_dir))
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
