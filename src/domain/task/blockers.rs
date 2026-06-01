use anyhow::{Result, bail};

use crate::domain::task::template::{Blocker, BlockerKind, BlockerRef, BlockerSource, TaskRecord};

/// Add an unresolved blocker to a task.
pub fn add_blocker(
    task: &mut TaskRecord,
    id: String,
    kind: BlockerKind,
    blocked_ref: Option<BlockerRef>,
    title: String,
    reason: String,
    created_at: String,
) {
    task.blockers.push(Blocker {
        id,
        kind,
        blocked_ref,
        title,
        reason,
        source: BlockerSource::Command,
        created_at: created_at.clone(),
        resolved_at: None,
    });
    task.updated_at = created_at;
}

/// Resolve an existing blocker.
pub fn resolve_blocker(task: &mut TaskRecord, blocker_id: &str, resolved_at: String) -> Result<()> {
    let Some(blocker) = task
        .blockers
        .iter_mut()
        .find(|blocker| blocker.id == blocker_id)
    else {
        bail!("blocker not found: {blocker_id}");
    };

    blocker.resolved_at = Some(resolved_at.clone());
    task.updated_at = resolved_at;
    Ok(())
}

/// Whether the task has any unresolved blockers.
pub fn has_unresolved_blockers(task: &TaskRecord) -> bool {
    task.blockers
        .iter()
        .any(|blocker| blocker.resolved_at.is_none())
}
