use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::task::lookup::task_yaml_path_for_entry;
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
    Ok(count_tasks_by_feature(tasks_dir)?
        .remove(feature_id)
        .unwrap_or_default())
}

/// Count tasks for every feature by scanning `.maestro/tasks/**/task.yaml` once.
pub fn count_tasks_by_feature(tasks_dir: &Path) -> Result<HashMap<String, FeatureTaskCounts>> {
    let mut counts = HashMap::new();
    if !tasks_dir.exists() {
        return Ok(counts);
    }

    for entry in fs::read_dir(tasks_dir)
        .with_context(|| format!("failed to read tasks dir {}", tasks_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", tasks_dir.display()))?;
        let Some(task_path) = task_yaml_path_for_entry(&entry)? else {
            continue;
        };

        let contents = fs::read_to_string(&task_path)
            .with_context(|| format!("failed to read {}", task_path.display()))?;
        let task: TaskSummary = serde_yaml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", task_path.display()))?;

        if let Some(feature_id) = task.feature_id {
            let entry = counts.entry(feature_id).or_default();
            entry.total += 1;
            if task.state == Some(TaskState::Verified) {
                entry.verified += 1;
            }
        }
    }

    Ok(counts)
}
