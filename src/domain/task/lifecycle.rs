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
    if is_terminal(&task.state)
        && !(task.state == TaskState::Verified && *to == TaskState::Superseded)
    {
        bail!("terminal task state cannot transition");
    }

    match (&task.state, to) {
        (TaskState::Draft, TaskState::Exploring) => Ok(()),
        (TaskState::Exploring, TaskState::Ready) => validate_ready(task),
        (TaskState::Ready, TaskState::InProgress) => {
            if !task.acceptance_locked {
                bail!("acceptance must be locked before claim");
            }
            if has_unresolved_blockers(task) {
                bail!("task has unresolved blockers");
            }
            Ok(())
        }
        (TaskState::InProgress, TaskState::NeedsVerification) => {
            if has_unresolved_blockers(task) {
                bail!("task has unresolved blockers");
            }
            Ok(())
        }
        (TaskState::NeedsVerification, TaskState::Verified) => {
            bail!("verified transition is owned by verification subsystem")
        }
        (_, TaskState::Rejected | TaskState::Abandoned | TaskState::Superseded) => Ok(()),
        _ => bail!(
            "cannot transition task from {} to {}",
            task.state.as_str(),
            to.as_str()
        ),
    }
}

fn validate_ready(task: &TaskRecord) -> Result<()> {
    if task.lane.as_deref() == Some("tiny") || task.acceptance_locked {
        Ok(())
    } else {
        bail!("acceptance must exist and be locked before ready")
    }
}

fn is_terminal(state: &TaskState) -> bool {
    matches!(
        state,
        TaskState::Rejected | TaskState::Abandoned | TaskState::Superseded
    )
}
