use crate::domain::task::blockers::has_unresolved_blockers;
use crate::domain::task::template::TaskRecord;

/// Render one task for `maestro task show`.
pub fn render_task(task: &TaskRecord) -> String {
    let mut out = String::new();
    out.push_str(&format!("id: {}\n", task.id));
    out.push_str(&format!("title: {}\n", task.title));
    out.push_str(&format!("state: {}\n", state_label(task)));
    if let Some(feature_id) = task.feature_id.as_deref() {
        out.push_str(&format!("feature: {feature_id}\n"));
    }
    if let Some(claimed_by) = task.claimed_by.as_deref() {
        out.push_str(&format!("claimed_by: {claimed_by}\n"));
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

/// Render a compact list for `maestro task list`.
pub fn render_task_list(tasks: &[TaskRecord]) -> String {
    let mut out = String::new();
    out.push_str("ID\tSTATE\tTITLE\n");
    for task in tasks {
        out.push_str(&format!(
            "{}\t{}\t{}\n",
            task.id,
            state_label(task),
            task.title
        ));
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
