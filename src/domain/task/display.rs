use std::collections::BTreeSet;

use crate::domain::task::blockers::has_unresolved_blockers;
use crate::domain::task::template::{StateHistoryEntry, TaskRecord, TaskState};
use crate::foundation::core::table;
use crate::foundation::core::time::{render_timestamp, timestamp_nanos};

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImplementMethod {
    method: &'static str,
    reason: &'static str,
    proof_required: &'static str,
}

fn implement_method(task: &TaskRecord, checks: &[String]) -> ImplementMethod {
    if let Some(reason) = task.lane.as_deref().and_then(skip_reason_for_lane) {
        return tdd_skipped(reason);
    }
    if let Some(reason) = skip_reason_for_checks(checks) {
        return tdd_skipped(reason);
    }
    if checks.is_empty() {
        if task.feature_id.is_some() {
            return ImplementMethod {
                method: "TDD required",
                reason: "feature acceptance is inherited",
                proof_required: "RED claim + GREEN claim",
            };
        }
        return ImplementMethod {
            method: "TDD pending",
            reason: "no locked check yet",
            proof_required: "add an observable check before implementation",
        };
    }
    ImplementMethod {
        method: "TDD required",
        reason: "locked check names observable behavior",
        proof_required: "RED claim + GREEN claim",
    }
}

pub fn render_implement_method_block(task: &TaskRecord, checks: &[String]) -> String {
    let method = implement_method(task, checks);
    format!(
        "implement_method: {}\nmethod_reason: {}\nproof_required: {}\n",
        method.method, method.reason, method.proof_required
    )
}

/// Render one task for `maestro task show`. `checks` is the task's acceptance
/// contract, read by the caller from the task record.
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
    if !task.covers.is_empty() {
        out.push_str(&format!("covers: {}\n", task.covers.join(", ")));
    }
    if let Some(risk) = task.risk.as_deref() {
        out.push_str(&format!("risk: {risk}\n"));
    }
    if let Some(claimed_by) = task.claimed_by.as_deref() {
        out.push_str(&format!("claimed_by: {claimed_by}\n"));
    }
    if task.atomic {
        out.push_str("atomic: true\n");
        if let Some(reason) = task.atomic_reason.as_deref() {
            out.push_str(&format!("atomic_reason: {reason}\n"));
        }
    }
    out.push_str(&format!(
        "created_at: {}\n",
        render_timestamp(&task.created_at)
    ));
    out.push_str(&format!(
        "updated_at: {}\n",
        render_timestamp(&task.updated_at)
    ));

    out.push_str(&render_implement_method_block(task, checks));

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
        .and_then(timestamp_nanos);
    let is_post_verification =
        |entry: &StateHistoryEntry| match (verified_at_ns, timestamp_nanos(&entry.at)) {
            (Some(verified_at), Some(entry_at)) => entry_at > verified_at,
            _ => false,
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
    if task.state == TaskState::NeedsVerification {
        out.push_str("proof: needs attention\n");
        out.push_str(&format!("next: maestro task proof {}\n", task.id));
    }

    if task.blockers.is_empty() {
        out.push_str("blockers: none\n");
    } else {
        out.push_str("blockers:\n");
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
    render_task_list_with_missing_checks(tasks, archived_ids, &BTreeSet::new())
}

pub fn render_task_list_with_missing_checks(
    tasks: &[TaskRecord],
    archived_ids: &BTreeSet<String>,
    missing_verify_contract_ids: &BTreeSet<String>,
) -> String {
    let rows: Vec<Vec<String>> = tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let mut state = state_label(task);
            if archived_ids.contains(&task.id) {
                state.push_str(" (archived)");
            }
            vec![
                (index + 1).to_string(),
                state,
                compact_next(task, missing_verify_contract_ids.contains(&task.id)).to_string(),
                task.title.clone(),
            ]
        })
        .collect();
    table::render_table(&["REF", "STATE", "NEXT", "TITLE"], &rows)
}

fn state_label(task: &TaskRecord) -> String {
    let base = task.state.as_str();
    if has_unresolved_blockers(task) {
        format!("{base} / blocked")
    } else {
        base.to_string()
    }
}

fn compact_next(task: &TaskRecord, missing_verify_contract: bool) -> &'static str {
    if has_unresolved_blockers(task) {
        return "run: inspect_blocker";
    }
    match task.state {
        TaskState::Draft | TaskState::Exploring if missing_verify_contract => "template: add_check",
        TaskState::Draft => "run: explore",
        TaskState::Exploring => "run: accept",
        TaskState::Ready => "run: claim",
        TaskState::InProgress => "template: complete",
        TaskState::NeedsVerification => "run: verify",
        TaskState::Verified
        | TaskState::Rejected
        | TaskState::Abandoned
        | TaskState::Superseded => "run: status",
    }
}

fn tdd_skipped(reason: &'static str) -> ImplementMethod {
    ImplementMethod {
        method: "TDD skipped",
        reason,
        proof_required: "skip-reason claim + relevant verification",
    }
}

fn skip_reason_for_lane(lane: &str) -> Option<&'static str> {
    match lane.trim().to_ascii_lowercase().as_str() {
        "light" => Some("lane light"),
        "explore" => Some("lane explore"),
        "spike" => Some("lane spike"),
        _ => None,
    }
}

fn skip_reason_for_checks(checks: &[String]) -> Option<&'static str> {
    let mut reason = None;
    for check in checks {
        reason = Some(skip_reason_for_check(check)?);
    }
    reason
}

fn skip_reason_for_check(check: &str) -> Option<&'static str> {
    let text = check.to_ascii_lowercase();
    if text.contains("docs-only")
        || text.contains("documentation-only")
        || text.contains("markdown-only")
        || text.contains("readme-only")
    {
        return Some("docs-only check");
    }
    if text.contains("config-only") || text.contains("configuration-only") {
        return Some("config-only check");
    }
    if text.contains("behavior held constant")
        || text.contains("no behavior change")
        || text.contains("non-behavioral")
        || text.contains("mechanical")
    {
        return Some("mechanical/no-behavior-change check");
    }
    if text.contains("spike") || text.contains("throwaway") {
        return Some("spike check");
    }
    None
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
        let row = |title: &str| {
            out.lines()
                .find(|line| line.contains(title))
                .unwrap_or_else(|| panic!("{title} row present"))
                .to_string()
        };

        assert!(!row("Live").contains("(archived)"));
        assert!(row("Archived").contains("(archived)"));
    }

    #[test]
    fn render_task_list_marks_missing_verify_contract_as_template_action() {
        let task = TaskRecord::draft("task-001", "Needs check", "2026-06-02T00:00:00Z");
        let missing = BTreeSet::from(["task-001".to_string()]);

        let out = render_task_list_with_missing_checks(&[task], &BTreeSet::new(), &missing);

        assert!(out.contains("template: add_check"), "{out}");
    }

    #[test]
    fn render_task_list_has_no_inspect_column() {
        let task = TaskRecord::draft("task-001", "No inspect col", "2026-06-02T00:00:00Z");
        let out = render_task_list(&[task], &BTreeSet::new());
        assert!(!out.contains("INSPECT"), "{out}");
        assert!(!out.contains("maestro task show"), "{out}");
    }

    #[test]
    fn render_task_show_points_needs_verification_at_query_proof() {
        let mut task = TaskRecord::draft("task-001", "Needs proof", "2026-06-02T00:00:00Z");
        task.state = TaskState::NeedsVerification;

        let out = render_task(&task, &["observable check".to_string()]);

        assert!(out.contains("proof: needs attention"), "{out}");
        assert!(out.contains("next: maestro task proof task-001"), "{out}");
    }
}
