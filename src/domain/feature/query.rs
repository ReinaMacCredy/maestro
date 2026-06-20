use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::domain::task::{self, TaskEntry, TaskState};

/// Computed task counts for a feature.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FeatureTaskCounts {
    /// Number of tasks owned by the feature.
    pub total: usize,
    /// Number of verified tasks that reference the feature.
    pub verified: usize,
}

/// Count tasks by scanning standalone and feature-owned task roots on demand.
pub fn count_tasks_for_feature(tasks_dir: &Path, feature_id: &str) -> Result<FeatureTaskCounts> {
    let entries = task::load_task_entries(tasks_dir)?;
    Ok(count_tasks_for_feature_in_entries(&entries, feature_id))
}

/// Count tasks for one feature from an already-loaded task entry set.
pub fn count_tasks_for_feature_in_entries(
    entries: &[TaskEntry],
    feature_id: &str,
) -> FeatureTaskCounts {
    let mut counts = FeatureTaskCounts::default();
    for entry in entries {
        if entry.task.feature_id.as_deref() != Some(feature_id) {
            continue;
        }
        counts.total += 1;
        if entry.task.state == TaskState::Verified {
            counts.verified += 1;
        }
    }
    counts
}

/// Count tasks for every feature by scanning standalone and feature-owned task roots once.
pub fn count_tasks_by_feature(tasks_dir: &Path) -> Result<HashMap<String, FeatureTaskCounts>> {
    Ok(count_tasks_by_feature_in_entries(&task::load_task_entries(
        tasks_dir,
    )?))
}

/// Count tasks for every feature from an already-loaded task entry set.
pub fn count_tasks_by_feature_in_entries(
    entries: &[TaskEntry],
) -> HashMap<String, FeatureTaskCounts> {
    let mut counts: HashMap<String, FeatureTaskCounts> = HashMap::new();

    for task_entry in entries {
        if let Some(feature_id) = task_entry.task.feature_id.as_deref() {
            let counts_entry = counts.entry(feature_id.to_string()).or_default();
            counts_entry.total += 1;
            if task_entry.task.state == TaskState::Verified {
                counts_entry.verified += 1;
            }
        }
    }

    counts
}

/// Ids of the feature's child tasks that are still live, sorted.
///
/// "Live" mirrors the D5 close-gate rule: a child blocks close (and is the target
/// of cancel cascade) while it is `draft/exploring/ready/in_progress/
/// needs_verification`. `verified` and the terminal-settled states do not.
pub fn live_child_task_ids(tasks_dir: &Path, feature_id: &str) -> Result<Vec<String>> {
    Ok(live_child_task_ids_in_entries(
        &task::load_task_entries(tasks_dir)?,
        feature_id,
    ))
}

/// Ids of the feature's live child tasks from an already-loaded task entry set, sorted.
pub fn live_child_task_ids_in_entries(entries: &[TaskEntry], feature_id: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for task_entry in entries {
        if task_entry.task.feature_id.as_deref() != Some(feature_id) {
            continue;
        }
        if task_entry.task.state.is_live() {
            ids.push(task_entry.task.id.clone());
        }
    }
    ids.sort();
    ids
}
