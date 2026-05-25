use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::task::template::TaskState;

/// Computed task counts for a feature.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FeatureTaskCounts {
    /// Number of tasks that reference the feature.
    pub total: usize,
    /// Number of verified tasks that reference the feature.
    pub verified: usize,
}

#[derive(Debug, Deserialize)]
struct TaskSummary {
    feature_id: Option<String>,
    state: Option<TaskState>,
}

/// Count tasks by scanning `.maestro/tasks/**/task.yaml` on demand.
pub fn count_tasks_for_feature(tasks_dir: &Path, feature_id: &str) -> Result<FeatureTaskCounts> {
    let mut counts = FeatureTaskCounts::default();
    if !tasks_dir.exists() {
        return Ok(counts);
    }

    for entry in fs::read_dir(tasks_dir)
        .with_context(|| format!("failed to read tasks dir {}", tasks_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", tasks_dir.display()))?;
        let task_path = entry.path().join("task.yaml");
        if !task_path.is_file() {
            continue;
        }

        let contents = fs::read_to_string(&task_path)
            .with_context(|| format!("failed to read {}", task_path.display()))?;
        let task: TaskSummary = serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", task_path.display()))?;

        if task.feature_id.as_deref() == Some(feature_id) {
            counts.total += 1;
            if task.state == Some(TaskState::Verified) {
                counts.verified += 1;
            }
        }
    }

    Ok(counts)
}
