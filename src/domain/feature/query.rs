use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::domain::task;
use crate::domain::task::TaskState;

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

/// Ids of the feature's child tasks that are still live, sorted.
///
/// "Live" mirrors the D5 ship-gate rule: a child blocks ship (and is the target
/// of cancel cascade) while it is `draft/exploring/ready/in_progress/
/// needs_verification`. `verified` and the terminal-settled states do not.
pub fn live_child_task_ids(tasks_dir: &Path, feature_id: &str) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    if !tasks_dir.exists() {
        return Ok(ids);
    }
    for projection in task::load_feature_task_projections(tasks_dir)? {
        if projection.feature_id.as_deref() != Some(feature_id) {
            continue;
        }
        if projection.state.as_ref().is_some_and(task_state_is_live) {
            ids.push(projection.id);
        }
    }
    ids.sort();
    Ok(ids)
}

/// Whether a child task in this state blocks its feature from shipping.
///
/// Matched exhaustively so a new `TaskState` variant forces this gate to be
/// reconsidered rather than silently defaulting.
fn task_state_is_live(state: &TaskState) -> bool {
    match state {
        TaskState::Draft
        | TaskState::Exploring
        | TaskState::Ready
        | TaskState::InProgress
        | TaskState::NeedsVerification => true,
        TaskState::Verified
        | TaskState::Rejected
        | TaskState::Abandoned
        | TaskState::Superseded => false,
    }
}
