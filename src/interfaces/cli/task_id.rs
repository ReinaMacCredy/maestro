use anyhow::Result;

use crate::domain::task::{self, TaskState};
use crate::foundation::core::paths::MaestroPaths;

pub(crate) fn resolve_optional_task_id(
    paths: &MaestroPaths,
    explicit_id: Option<String>,
    missing_message: &'static str,
) -> Result<String> {
    if let Some(task_id) = explicit_id {
        return Ok(task_id);
    }
    if let Ok(task_id) = std::env::var("MAESTRO_CURRENT_TASK") {
        if !task_id.trim().is_empty() {
            return Ok(task_id);
        }
    }
    let tasks = task::load_task_records(&paths.tasks_dir())?;
    let open_tasks = tasks
        .iter()
        .filter(|task| task.state == TaskState::NeedsVerification)
        .collect::<Vec<_>>();
    if open_tasks.len() == 1 {
        return Ok(open_tasks[0].id.clone());
    }
    if tasks.len() == 1 {
        return Ok(tasks[0].id.clone());
    }
    anyhow::bail!(missing_message);
}
