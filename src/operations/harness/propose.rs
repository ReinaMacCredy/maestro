use std::collections::BTreeSet;

use anyhow::{Result, bail};

use crate::domain::harness::backlog;
use crate::domain::harness::{BacklogConfig, BacklogItem, HistoryEntry, is_state_detector};
use crate::domain::task::{self, TaskState};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::{nanos_since_epoch_string, utc_now_timestamp};

use super::detect;

/// Refresh rule-based proposals into the backlog and return the full backlog
/// alongside the ids that are ready to measure (D7). The hint is derived from the
/// current detection run and never persisted, so the interface stays a pure
/// renderer and the state-detector predicate stays in this layer.
pub fn refresh(paths: &MaestroPaths) -> Result<(BacklogConfig, BTreeSet<String>)> {
    let proposals = detect::detect(paths)?;
    let fresh = proposals
        .iter()
        .map(|proposal| proposal.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    let backlog = backlog::refresh(paths, proposals)?;
    let ready = backlog
        .items
        .iter()
        .filter(|item| ready_to_measure(item, &fresh))
        .map(|item| item.id.clone())
        .collect();
    Ok((backlog, ready))
}

/// D7 hint: an accepted state-detector note whose detector is currently silent is
/// ready to be measured. Derived at read time and never persisted.
fn ready_to_measure(item: &BacklogItem, fresh: &BTreeSet<String>) -> bool {
    item.status == "accepted"
        && is_state_detector(&item.item_type)
        && !fresh.contains(&item.fingerprint)
}

/// Accept a proposal (D0/A): spawn a linked task and record the link. Re-accepting
/// is an error; the existing task is already linked. A measure that reverted the
/// note to `proposed` clears the old link, so the next accept spawns a fresh task
/// rather than silently reusing a closed one (impl-default (c)).
pub fn apply(paths: &MaestroPaths, id: &str) -> Result<BacklogItem> {
    let proposals = detect::detect(paths)?;
    let mut backlog = backlog::load(paths)?;
    backlog::merge_proposals(&mut backlog, proposals);

    // Read what we need before the task-creation side effect borrows `backlog`.
    let (status, title) = {
        let item = find(&backlog, id)?;
        (item.status.clone(), item.title.clone())
    };
    match status.as_str() {
        "accepted" => bail!("{id} is already accepted; its task is already linked"),
        "measured" => {
            bail!("{id} is already measured; run `maestro harness list` to re-derive it first")
        }
        _ => {}
    }

    let task = task::create_task(
        &paths.tasks_dir(),
        &title,
        None,
        None,
        None,
        &nanos_since_epoch_string(),
    )?;

    let item = find_mut(&mut backlog, id)?;
    item.status = "accepted".to_string();
    item.spawned_task = Some(task.id.clone());
    item.history.push(HistoryEntry {
        result: "accepted".to_string(),
        task: Some(task.id.clone()),
        at: utc_now_timestamp(),
    });
    let accepted = item.clone();
    backlog::save(paths, &backlog)?;
    Ok(accepted)
}

/// Measure an accepted proposal (the only path to `measured`). A state detector
/// gets an automatic verdict from present detection: silent → `measured`, still
/// emitting → back to `proposed` with the link cleared (D2). A behavioral detector
/// closes by human judgment — the deliberate measure on a verified task IS that
/// judgment (D1), with no silence check. Unless `force`, the linked task must be
/// verified first (impl-default (d)).
pub fn measure(paths: &MaestroPaths, id: &str, force: bool) -> Result<BacklogItem> {
    let proposals = detect::detect(paths)?;
    let fresh = proposals
        .iter()
        .map(|proposal| proposal.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    let mut backlog = backlog::load(paths)?;
    backlog::merge_proposals(&mut backlog, proposals);

    // Read identity + status before any gate or mutation.
    let (status, fingerprint, item_type, spawned_task) = {
        let item = find(&backlog, id)?;
        (
            item.status.clone(),
            item.fingerprint.clone(),
            item.item_type.clone(),
            item.spawned_task.clone(),
        )
    };
    match status.as_str() {
        "accepted" => {}
        "measured" => bail!("{id} is already measured"),
        _ => bail!("{id} is not accepted yet; run `maestro harness apply {id}` before measuring"),
    }

    if !force {
        match &spawned_task {
            Some(task_id) => {
                let record = task::load_task_record(&paths.tasks_dir(), task_id)?;
                if record.state != TaskState::Verified {
                    bail!(
                        "linked task {task_id} is not verified (state: {}); use --force to measure anyway",
                        record.state.as_str()
                    );
                }
            }
            None => bail!("{id} has no linked task to measure; use --force to measure anyway"),
        }
    }

    let now = utc_now_timestamp();
    let item = find_mut(&mut backlog, id)?;
    if is_state_detector(&item_type) && fresh.contains(&fingerprint) {
        // Friction persists: the improvement was ineffective. Revert to proposed and
        // drop the link so the next accept spawns a fresh task (impl-default (c)).
        item.history.push(HistoryEntry {
            result: "ineffective".to_string(),
            task: spawned_task,
            at: now,
        });
        item.status = "proposed".to_string();
        item.spawned_task = None;
    } else {
        item.history.push(HistoryEntry {
            result: "measured".to_string(),
            task: spawned_task,
            at: now,
        });
        item.status = "measured".to_string();
    }
    let measured = item.clone();
    backlog::save(paths, &backlog)?;
    Ok(measured)
}

fn find<'a>(backlog: &'a BacklogConfig, id: &str) -> Result<&'a BacklogItem> {
    backlog
        .items
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| anyhow::anyhow!("backlog item not found: {id}"))
}

fn find_mut<'a>(backlog: &'a mut BacklogConfig, id: &str) -> Result<&'a mut BacklogItem> {
    backlog
        .items
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or_else(|| anyhow::anyhow!("backlog item not found: {id}"))
}
