use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::task::cards;
use crate::domain::task::template::{TaskRecord, TaskSnapshot};
use crate::foundation::core::paths::MaestroPaths;

/// Reconstruct the repo's [`MaestroPaths`] from a tasks directory so the task
/// facade can reach the card store. `tasks_dir` is `.maestro/tasks` (a paths
/// carrier the facade signatures still take), so its grandparent is the repo
/// root.
pub(crate) fn paths_for_tasks_dir(tasks_dir: &Path) -> Option<MaestroPaths> {
    tasks_dir
        .parent()
        .and_then(Path::parent)
        .map(MaestroPaths::new)
}

pub(super) fn validate_task_lookup_id(id: &str) -> Result<()> {
    let mut components = Path::new(id).components();
    if id.is_empty()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        bail!("invalid task id: {id}");
    }
    Ok(())
}

/// The [`load_task_with_snapshot`] read with true absence as `Ok(None)`: a
/// read failure (parse, schema, symlink) still propagates, so a caller
/// probing for fallbacks cannot mistake an unreadable live task for a
/// missing one.
pub(crate) fn try_load_task_record(tasks_dir: &Path, id: &str) -> Result<Option<TaskRecord>> {
    let paths =
        paths_for_tasks_dir(tasks_dir).context("cannot resolve maestro paths from tasks dir")?;
    validate_task_lookup_id(id)?;
    Ok(cards::load_one(&paths, id)?.map(|(task, _)| task))
}

/// Load a task by id with its optimistic save snapshot. Reads the `Task`-typed
/// card from whatever home the resolver finds (no archive fallback -- the card
/// archive tree is its own scan).
pub fn load_task_with_snapshot(
    tasks_dir: &Path,
    id: &str,
) -> Result<(TaskRecord, TaskSnapshot, PathBuf)> {
    let paths =
        paths_for_tasks_dir(tasks_dir).context("cannot resolve maestro paths from tasks dir")?;
    validate_task_lookup_id(id)?;
    let Some((task, resolved)) = cards::load_one(&paths, id)? else {
        bail!("task not found: {id}");
    };
    let task_dir = resolved
        .path()
        .parent()
        .map(Path::to_path_buf)
        .context("card path is missing parent directory")?;
    Ok((task, TaskSnapshot::Card(Box::new(resolved)), task_dir))
}
