use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::card::suggest;
use crate::domain::task::template::{TaskRecord, TaskSnapshot};
use crate::domain::task::{cards, progress};
use crate::foundation::core::error::MaestroError;
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
    if let Some((task, _)) = cards::load_one(&paths, id)? {
        return Ok(Some(task));
    }
    Ok(progress::load_task_with_snapshot(&paths, id)?.map(|(task, _, _)| task))
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
    if let Some((task, resolved)) = cards::load_one(&paths, id)? {
        let task_dir = resolved
            .path()
            .parent()
            .map(Path::to_path_buf)
            .context("card path is missing parent directory")?;
        return Ok((task, TaskSnapshot::Card(Box::new(resolved)), task_dir));
    }
    if let Some((task, snapshot, task_dir)) = progress::load_task_with_snapshot(&paths, id)? {
        return Ok((task, TaskSnapshot::Progress(Box::new(snapshot)), task_dir));
    }

    // Hint-only near-match for the main.rs funnel; ids never fuzzy-resolve.
    let nearest = cards::scan(&paths).ok().and_then(|mut tasks| {
        if let Ok(progress_tasks) = progress::scan(&paths) {
            tasks.extend(progress_tasks);
        }
        suggest::did_you_mean(id, tasks.iter().map(|(task, _)| task.id.as_str()))
    });
    Err(MaestroError::IdNotFound {
        kind: "task",
        id: id.to_string(),
        nearest,
    }
    .into())
}
