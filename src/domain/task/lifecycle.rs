use anyhow::{Result, bail};

use crate::domain::task::blockers::has_unresolved_blockers;
use crate::domain::task::template::{StateHistoryEntry, TaskRecord, TaskState};

/// Transition a task forward or terminally, appending state history.
pub fn transition(
    task: &mut TaskRecord,
    to: TaskState,
    by: &str,
    at: &str,
    details: TransitionDetails,
) -> Result<()> {
    validate_transition(task, &to)?;
    task.state = to.clone();
    if to == TaskState::InProgress {
        task.claimed_by = Some(by.to_string());
        task.claimed_at = Some(at.to_string());
    }
    task.state_history.push(StateHistoryEntry {
        state: to,
        at: at.to_string(),
        by: by.to_string(),
        to: details.to,
        summary: details.summary,
        claims: details.claims,
        open_items: details.open_items,
    });
    task.updated_at = at.to_string();

    Ok(())
}

/// Append a non-transition state-history entry and update the task timestamp.
pub fn append_history(task: &mut TaskRecord, by: &str, at: &str, details: TransitionDetails) {
    task.state_history.push(StateHistoryEntry {
        state: task.state.clone(),
        at: at.to_string(),
        by: by.to_string(),
        to: details.to,
        summary: details.summary,
        claims: details.claims,
        open_items: details.open_items,
    });
    task.updated_at = at.to_string();
}

/// Optional metadata for state-history entries.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TransitionDetails {
    pub to: Option<String>,
    pub summary: Option<String>,
    pub claims: Vec<String>,
    pub open_items: Vec<String>,
}

fn validate_transition(task: &TaskRecord, to: &TaskState) -> Result<()> {
    if is_terminal(&task.state) {
        bail!(
            "task {} is in terminal state {}; terminal tasks cannot transition",
            task.id,
            task.state.as_str()
        );
    }

    match (&task.state, to) {
        (TaskState::Draft, TaskState::Exploring) => Ok(()),
        (TaskState::Exploring, TaskState::Ready) => validate_ready(task),
        (TaskState::Ready, TaskState::InProgress) => {
            if !task.acceptance_locked {
                bail!(
                    "task {} acceptance is not locked; run `maestro task accept {}` before claiming",
                    task.id,
                    task.id
                );
            }
            if has_unresolved_blockers(task) {
                bail!("{}", blockers_remedy(task));
            }
            Ok(())
        }
        (TaskState::InProgress, TaskState::NeedsVerification) => {
            if has_unresolved_blockers(task) {
                bail!("{}", blockers_remedy(task));
            }
            Ok(())
        }
        (
            TaskState::Draft | TaskState::Exploring | TaskState::Ready,
            TaskState::NeedsVerification,
        ) => bail!(
            "task {} is {}; run `maestro task claim {}` to start work before completing it",
            task.id,
            task.state.as_str(),
            task.id
        ),
        (TaskState::NeedsVerification, TaskState::Verified) => {
            bail!(
                "the verified transition is owned by `maestro task verify {}`; it cannot be set directly",
                task.id
            )
        }
        (_, TaskState::Rejected | TaskState::Abandoned | TaskState::Superseded) => Ok(()),
        (TaskState::Draft, TaskState::Ready) => bail!(
            "cannot accept task {} directly from draft; run `maestro task explore {}` first, then `maestro task accept {}`",
            task.id,
            task.id,
            task.id
        ),
        (current, target) if current == target => bail!(
            "task {} is already {}",
            task.id,
            task.state.as_str()
        ),
        // Verified is a settled success terminus (not is_terminal, so it falls
        // through here). reject/supersede above can still close it; a forward verb
        // means the user wants new work, so point at a follow-up task rather than
        // leaving the bare catch-all dead end.
        (TaskState::Verified, _) => bail!(
            "task {} is verified, a settled success; start new work with `maestro task create` rather than re-opening it",
            task.id
        ),
        _ => bail!(
            "cannot transition task {} from {} to {}",
            task.id,
            task.state.as_str(),
            to.as_str()
        ),
    }
}

/// Error text for a claim/complete blocked by unresolved blockers: names them
/// and the verb to clear them.
fn blockers_remedy(task: &TaskRecord) -> String {
    let open: Vec<&str> = task
        .blockers
        .iter()
        .filter(|blocker| blocker.resolved_at.is_none())
        .map(|blocker| blocker.id.as_str())
        .collect();
    format!(
        "task {} has unresolved blockers ({}); resolve them with `maestro task unblock {} --blocker <blk-id>`",
        task.id,
        open.join(", "),
        task.id
    )
}

fn validate_ready(task: &TaskRecord) -> Result<()> {
    if task.lane.as_deref() == Some("tiny") || task.acceptance_locked {
        Ok(())
    } else {
        bail!("acceptance must exist and be locked before ready")
    }
}

pub(crate) fn is_terminal(state: &TaskState) -> bool {
    matches!(
        state,
        TaskState::Rejected | TaskState::Abandoned | TaskState::Superseded
    )
}
