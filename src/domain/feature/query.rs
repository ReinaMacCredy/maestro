use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::domain::task;

/// Computed task counts for a feature.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FeatureTaskCounts {
    /// Number of tasks that reference the feature.
    pub total: usize,
    /// Number of verified tasks that reference the feature.
    pub verified: usize,
}

/// Count tasks by scanning `.maestro/tasks/**/task.yaml` on demand.
pub fn count_tasks_for_feature(tasks_dir: &Path, feature_id: &str) -> Result<FeatureTaskCounts> {
    Ok(count_tasks_by_feature(tasks_dir)?
        .remove(feature_id)
        .unwrap_or_default())
}

/// Count tasks for every feature by scanning `.maestro/tasks/**/task.yaml` once.
pub fn count_tasks_by_feature(tasks_dir: &Path) -> Result<HashMap<String, FeatureTaskCounts>> {
    let mut counts: HashMap<String, FeatureTaskCounts> = HashMap::new();
    if !tasks_dir.exists() {
        return Ok(counts);
    }

    for projection in task::load_feature_task_projections(tasks_dir)? {
        if let Some(feature_id) = projection.feature_id {
            let entry = counts.entry(feature_id).or_default();
            entry.total += 1;
            if projection.state == Some(task::TaskState::Verified) {
                entry.verified += 1;
            }
        }
    }

    Ok(counts)
}
