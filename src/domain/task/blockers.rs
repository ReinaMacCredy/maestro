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
        let open: Vec<&str> = task
            .blockers
            .iter()
            .filter(|blocker| blocker.resolved_at.is_none())
            .map(|blocker| blocker.id.as_str())
            .collect();
        if open.is_empty() {
            bail!(
                "blocker not found: {blocker_id}; task {} has no open blockers",
                task.id
            );
        }
        bail!(
            "blocker not found: {blocker_id}; pass an open blocker's blk- id: {}",
            open.join(", ")
        );
    };

    if blocker.resolved_at.is_some() {
        bail!("blocker {blocker_id} is already resolved");
    }
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
