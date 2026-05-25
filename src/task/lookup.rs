use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::task::template::{load_task, TaskRecord, TaskSnapshot};

/// Resolve a task id or unique id-prefix to its `task.yaml` path.
pub fn resolve_task_yaml_path(tasks_dir: &Path, id: &str) -> Result<PathBuf> {
    let direct = tasks_dir.join(id).join("task.yaml");
    if direct.is_file() {
        return Ok(direct);
    }

    let prefix = format!("{id}-");
    let mut matches = Vec::new();
    for entry in fs::read_dir(tasks_dir)
        .with_context(|| format!("failed to read {}", tasks_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", tasks_dir.display()))?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name.starts_with(&prefix) {
            let path = entry.path().join("task.yaml");
            if path.is_file() {
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
