use std::collections::BTreeSet;

use crate::domain::task::blockers::has_unresolved_blockers;
use crate::domain::task::template::{StateHistoryEntry, TaskRecord, TaskState};
use crate::foundation::core::time::render_timestamp;

/// Render one task for `maestro task show`. `checks` is the task's acceptance
/// contract, read by the caller from the sibling `acceptance.yaml`.
pub fn render_task(task: &TaskRecord, checks: &[String]) -> String {
    let mut out = String::new();
    out.push_str(&format!("id: {}\n", task.id));
    out.push_str(&format!("title: {}\n", task.title));
    out.push_str(&format!("state: {}\n", state_label(task)));
    if task.state == TaskState::Superseded
        && let Some(by) = task
            .state_history
            .iter()
            .rev()
            .find(|entry| entry.state == TaskState::Superseded)
            .and_then(|entry| entry.to.as_deref())
    {
        out.push_str(&format!("superseded_by: {by}\n"));
    }
    if let Some(feature_id) = task.feature_id.as_deref() {
        out.push_str(&format!("feature: {feature_id}\n"));
    }
    if let Some(claimed_by) = task.claimed_by.as_deref() {
        out.push_str(&format!("claimed_by: {claimed_by}\n"));
    }
    out.push_str(&format!("created_at: {}\n", render_timestamp(&task.created_at)));
    out.push_str(&format!("updated_at: {}\n", render_timestamp(&task.updated_at)));

    out.push_str("checks:\n");
    if checks.is_empty() {
        out.push_str("- none\n");
    } else {
        for check in checks {
            out.push_str(&format!("- {check}\n"));
        }
    }

    // Completion + verification write summary/claims into history entries;
    // surface what the task did and asserted, not just its current state. Take
    // the latest of each independently so a later summary-only entry (e.g.
    // verification) does not hide the completion's claims.
    if let Some(summary) = task
        .state_history
        .iter()
        .rev()
        .find_map(|entry| entry.summary.as_deref())
    {
        out.push_str(&format!("summary: {summary}\n"));
    }
    // Claims recorded after `verified_at` were not part of what `task verify`
    // proved. Show the verified completion's claims AND any later ones, marking
    // the latter `(unverified)`, so a post-verification `update --claim` can
    // neither masquerade as verified nor hide what verification actually proved.
    let verified_at_ns = task
        .verification
        .verified_at
        .as_deref()
        .and_then(|value| value.trim().parse::<u64>().ok());
    let is_post_verification = |entry: &StateHistoryEntry| {
        match (verified_at_ns, entry.at.trim().parse::<u64>().ok()) {
            (Some(verified_at), Some(entry_at)) => entry_at > verified_at,
            _ => false,
        }
    };
    let verified_claims = task
        .state_history
        .iter()
        .rev()
        .find(|entry| !entry.claims.is_empty() && !is_post_verification(entry));
    let unverified_claims = task
        .state_history
        .iter()
        .rev()
        .find(|entry| !entry.claims.is_empty() && is_post_verification(entry));
    if verified_claims.is_some() || unverified_claims.is_some() {
        out.push_str("claims:\n");
        for claim in verified_claims.iter().flat_map(|entry| &entry.claims) {
            out.push_str(&format!("- {claim}\n"));
        }
        for claim in unverified_claims.iter().flat_map(|entry| &entry.claims) {
            out.push_str(&format!("- {claim} (unverified)\n"));
        }
    }

    // Proof binding lives on the record (not verification.json); show what
    // proves a verified task.
    if let Some(verified_at) = task.verification.verified_at.as_deref() {
        out.push_str(&format!("verified_at: {}\n", render_timestamp(verified_at)));
    }
    if let Some(commit) = task.verification.verified_commit.as_deref() {
        out.push_str(&format!("verified_commit: {commit}\n"));
    }

    out.push_str("blockers:\n");
    if task.blockers.is_empty() {
        out.push_str("- none\n");
    } else {
        for blocker in &task.blockers {
            let status = if blocker.resolved_at.is_some() {
                "resolved"
            } else {
                "open"
            };
            out.push_str(&format!(
                "- {} ({status}): {}\n",
                blocker.id, blocker.reason
            ));
        }
    }
    out
}

/// Render a compact list for `maestro task list`. Ids in `archived_ids` are
/// marked `(archived)` so `--all` distinguishes an archived row from a
/// live-terminal one sharing the same state (e.g. both `rejected`).
pub fn render_task_list(tasks: &[TaskRecord], archived_ids: &BTreeSet<String>) -> String {
    let mut out = String::new();
    out.push_str("ID\tSTATE\tTITLE\n");
    for task in tasks {
        let mut state = state_label(task);
        if archived_ids.contains(&task.id) {
            state.push_str(" (archived)");
        }
        out.push_str(&format!("{}\t{}\t{}\n", task.id, state, task.title));
    }
    out
}

fn state_label(task: &TaskRecord) -> String {
    let base = task.state.as_str();
    if has_unresolved_blockers(task) {
        format!("{base} / blocked")
    } else {
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_task_list_marks_only_archived_ids() {
        let live = TaskRecord::draft("task-001", "Live", "2026-06-02T00:00:00Z");
        let archived = TaskRecord::draft("task-002", "Archived", "2026-06-02T00:00:00Z");
        let archived_ids = BTreeSet::from(["task-002".to_string()]);

        let out = render_task_list(&[live, archived], &archived_ids);
        let row = |id: &str| {
            out.lines()
                .find(|l| l.starts_with(id))
                .unwrap_or_else(|| panic!("{id} row present"))
                .to_string()
        };

        assert!(!row("task-001").contains("(archived)"));
        assert!(row("task-002").contains("(archived)"));
    }
}
