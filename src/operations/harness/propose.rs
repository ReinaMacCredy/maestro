use std::collections::BTreeSet;
use std::fs;

use anyhow::{Context, Result, bail};

use crate::domain::harness::backlog;
use crate::domain::harness::{
    BacklogConfig, BacklogItem, EscalationPolicy, HistoryEntry, is_state_detector,
};
use crate::domain::run;
use crate::domain::task::{self, TaskState, TransitionDetails};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::time::{timestamp_nanos, utc_now_timestamp};

use super::{detect, policy};

/// Read the persisted backlog for display through the operations facade. A
/// missing backlog file reads as an empty backlog, so a fresh repo reports
/// cleanly instead of leaking a `failed to read` IO error to the interface.
pub fn load_backlog(paths: &MaestroPaths) -> Result<BacklogConfig> {
    backlog::load(paths)
}

/// Refresh rule-based proposals into the backlog and return the full backlog
/// alongside the ids that are ready to measure (D7). The hint is derived from the
/// current detection run and never persisted, so the interface stays a pure
/// renderer and the state-detector predicate stays in this layer.
pub fn refresh(
    paths: &MaestroPaths,
) -> Result<(BacklogConfig, BTreeSet<String>, Vec<OverThresholdItem>)> {
    let escalation = policy::load_policy(paths)?;
    let proposals = detect::detect_with_policy(paths, &escalation)?;
    let fresh = proposals
        .iter()
        .map(|proposal| proposal.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    let mut snapshot = backlog::load_with_snapshot(paths)?;
    backlog::merge_proposals(&mut snapshot.backlog, proposals);
    if escalation.enabled {
        snapshot.backlog.evidence_stamp = policy::evidence_stamp(paths)?;
    }
    backlog::apply_escalation_policy(&mut snapshot.backlog, &escalation);
    backlog::save_with_snapshot(paths, &snapshot.backlog, &snapshot)?;
    let ready = snapshot
        .backlog
        .items
        .iter()
        .filter(|item| ready_to_measure(item, &fresh) && linked_task_verified(paths, item))
        .map(|item| item.id.clone())
        .collect();
    let over_threshold = over_threshold_items_from_backlog(&snapshot.backlog, &escalation);
    Ok((snapshot.backlog, ready, over_threshold))
}

/// Guarded hot-verb refresh: detect only when the evidence stamp changed.
fn refresh_if_stale(paths: &MaestroPaths) -> Result<(BacklogConfig, EscalationPolicy)> {
    let escalation = policy::load_policy(paths)?;
    if !escalation.enabled {
        return Ok((backlog::load(paths)?, escalation));
    }
    let stamp = policy::evidence_stamp(paths)?;
    let mut snapshot = backlog::load_with_snapshot(paths)?;
    if snapshot.backlog.evidence_stamp == stamp {
        return Ok((snapshot.backlog, escalation));
    }
    let proposals = detect::detect_with_policy(paths, &escalation)?;
    backlog::merge_proposals(&mut snapshot.backlog, proposals);
    snapshot.backlog.evidence_stamp = stamp;
    backlog::apply_escalation_policy(&mut snapshot.backlog, &escalation);
    backlog::save_with_snapshot(paths, &snapshot.backlog, &snapshot)?;
    Ok((snapshot.backlog, escalation))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverThresholdItem {
    pub id: String,
    pub item_type: String,
    pub title: String,
    pub priority: String,
    pub occurrences: usize,
    pub sessions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedItem {
    pub item: BacklogItem,
    pub checks: Vec<String>,
    pub used_preset: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnappliedItem {
    pub item: BacklogItem,
    pub task: UnappliedTask,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnappliedTask {
    Abandoned(String),
    Archived(String),
    Missing(String),
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditHint {
    pub sessions_since_audit: usize,
    pub every_sessions: usize,
}

pub fn over_threshold_items(paths: &MaestroPaths) -> Result<Vec<OverThresholdItem>> {
    let (backlog, escalation) = refresh_if_stale(paths)?;
    if !escalation.enabled {
        return Ok(Vec::new());
    }
    Ok(over_threshold_items_from_backlog(&backlog, &escalation))
}

pub fn propose_agent_audit(
    paths: &MaestroPaths,
    title: &str,
    evidence: &str,
    topic: Option<&str>,
    session_id: &str,
) -> Result<BacklogItem> {
    if title.trim().is_empty() {
        bail!("--title must not be empty");
    }
    if evidence.trim().is_empty() {
        bail!("--evidence must not be empty");
    }
    if topic.is_some_and(|topic| topic.trim().is_empty()) {
        bail!("--topic must not be empty when supplied");
    }
    let topic = topic
        .map(normalize_agent_topic)
        .filter(|topic| !topic.is_empty())
        .unwrap_or_else(|| normalize_agent_topic(title));
    if topic.is_empty() {
        bail!("proposal topic could not be derived; pass --topic <slug>");
    }
    let fingerprint = format!("agent_audit:{topic}");
    let mut snapshot = backlog::load_with_snapshot(paths)?;
    let mut sessions_hit = vec![session_id.to_string()];
    if let Some(existing) = snapshot
        .backlog
        .items
        .iter()
        .find(|item| item.fingerprint == fingerprint && item.status != "dismissed")
    {
        sessions_hit.extend(existing.sessions_hit.iter().cloned());
    }
    sessions_hit.sort();
    sessions_hit.dedup();
    let mut item = manual_proposal(
        session_id.to_string(),
        "agent_audit",
        &topic,
        title.trim(),
        vec![format!("{session_id}: {}", evidence.trim())],
        sessions_hit.clone(),
        "agent-audit",
    );
    item.fingerprint = fingerprint;
    backlog::merge_proposals_preserving_absent(&mut snapshot.backlog, vec![item.clone()]);
    let escalation = policy::load_policy(paths)?;
    backlog::apply_escalation_policy(&mut snapshot.backlog, &escalation);
    backlog::save_with_snapshot(paths, &snapshot.backlog, &snapshot)?;
    Ok(snapshot
        .backlog
        .items
        .into_iter()
        .find(|existing| existing.fingerprint == item.fingerprint)
        .unwrap_or(item))
}

pub fn audit_overdue_hint(paths: &MaestroPaths) -> Result<Option<AuditHint>> {
    let Some(config) = policy::load_config(paths)? else {
        return Ok(None);
    };
    let Some(audit) = config.audit else {
        return Ok(None);
    };
    if audit.every_sessions == 0 {
        return Ok(None);
    }
    let backlog = backlog::load(paths)?;
    let latest_audit_at = backlog
        .items
        .iter()
        .filter(|item| item.provenance == "agent-audit")
        .filter_map(|item| timestamp_nanos(&item.last_seen).map(|at| (at, item.last_seen.as_str())))
        .max_by_key(|(at, _)| *at)
        .map(|(_, raw)| raw.to_string());
    let latest_audit_nanos = latest_audit_at
        .as_deref()
        .and_then(timestamp_nanos)
        .unwrap_or(i128::MIN);
    let mut sessions = BTreeSet::new();
    run::visit_managed_events(paths, |record| {
        let event_at = record
            .event()
            .timestamp()
            .and_then(timestamp_nanos)
            .unwrap_or(i128::MAX);
        if event_at > latest_audit_nanos {
            sessions.insert(record.session_id().to_string());
        }
        Ok(())
    })?;
    if sessions.len() >= audit.every_sessions {
        Ok(Some(AuditHint {
            sessions_since_audit: sessions.len(),
            every_sessions: audit.every_sessions,
        }))
    } else {
        Ok(None)
    }
}

fn over_threshold_items_from_backlog(
    backlog: &BacklogConfig,
    escalation: &EscalationPolicy,
) -> Vec<OverThresholdItem> {
    if !escalation.enabled {
        return Vec::new();
    }
    backlog
        .items
        .iter()
        .filter(|item| field_or_default(&item.status, "proposed") == "proposed")
        .filter(|item| escalation.over_threshold(item.sessions_hit.len()))
        .map(|item| OverThresholdItem {
            id: item.id.clone(),
            item_type: item.item_type.clone(),
            title: item.title.clone(),
            priority: escalation.priority_for(item.sessions_hit.len(), &item.priority),
            occurrences: item.occurrences,
            sessions: item.sessions_hit.len(),
        })
        .collect()
}

/// D7 hint: an accepted state-detector note whose detector is currently silent is
/// ready to be measured. Derived at read time and never persisted.
fn ready_to_measure(item: &BacklogItem, fresh: &BTreeSet<String>) -> bool {
    item.status == "accepted"
        && is_state_detector(&item.item_type)
        && !fresh.contains(&item.fingerprint)
}

/// Load a note's linked task from the live tree, falling back to the archive so a
/// verified spawned task that was archived (normal terminal cleanup) still resolves
/// -- matching how `query proof` and `task show` read across the boundary. The
/// `(ready to measure)` hint and the `measure` gate share this so they never
/// disagree about whether a closed task can be measured.
fn load_linked_task(paths: &MaestroPaths, task_id: &str) -> Result<task::TaskRecord> {
    task::load_task_record(&paths.tasks_dir(), task_id)
        .or_else(|_| task::load_task_record(&paths.archive_tasks_dir(), task_id))
}

/// True when the note's linked task exists and is verified -- the precondition the
/// no-force `measure` enforces below. The hint must not promise a measure the gate
/// would refuse, so a missing link or an unverified/absent task withholds it.
fn linked_task_verified(paths: &MaestroPaths, item: &BacklogItem) -> bool {
    let Some(task_id) = &item.spawned_task else {
        return false;
    };
    load_linked_task(paths, task_id).is_ok_and(|record| record.state == TaskState::Verified)
}

/// Run detection and merge fresh proposals into the loaded backlog without
/// persisting, returning the backlog and the set of currently-detected
/// fingerprints (used by the measure verdict). Re-derive runs on every command
/// per SPEC §5.1, so apply and measure share this step.
fn detect_and_merge(paths: &MaestroPaths) -> Result<(backlog::BacklogSnapshot, BTreeSet<String>)> {
    let escalation = policy::load_policy(paths)?;
    let proposals = detect::detect_with_policy(paths, &escalation)?;
    let fresh = proposals
        .iter()
        .map(|proposal| proposal.fingerprint.clone())
        .collect::<BTreeSet<_>>();
    let mut snapshot = backlog::load_with_snapshot(paths)?;
    backlog::merge_proposals(&mut snapshot.backlog, proposals);
    if escalation.enabled {
        snapshot.backlog.evidence_stamp = policy::evidence_stamp(paths)?;
    }
    backlog::apply_escalation_policy(&mut snapshot.backlog, &escalation);
    Ok((snapshot, fresh))
}

/// Accept a proposal (D0/A): spawn a linked task and record the link. Re-accepting
/// is an error; the existing task is already linked. A measure that reverted the
/// note to `proposed` clears the old link, so the next accept spawns a fresh task
/// rather than silently reusing a closed one (impl-default (c)).
pub fn apply(paths: &MaestroPaths, id: &str, checks: Vec<String>) -> Result<AppliedItem> {
    let (mut snapshot, _) = detect_and_merge(paths)?;

    let item = snapshot.backlog.find_mut(id)?;
    match item.status.as_str() {
        "accepted" => bail!("{id} is already accepted; its task is already linked"),
        // detect_and_merge above reopens a measured state detector to `proposed`
        // whenever its friction is live (reopen_if_regressed), so reaching this
        // state-detector arm means the friction is already gone -- it reopens on
        // its own if it recurs, with nothing to apply now. A behavioral item's
        // measured state is terminal and never reopens.
        "measured" if is_state_detector(&item.item_type) => bail!(
            "{id} is already measured; its friction is resolved and it reopens automatically if it recurs -- nothing to apply now"
        ),
        "measured" => bail!(
            "{id} is already measured; a measured {} item is closed and re-detection will not reopen it",
            item.item_type
        ),
        _ => {}
    }

    let title = item.title.clone();
    let (checks, used_preset) = if checks.is_empty() {
        (vec![default_check(item)], true)
    } else {
        (checks, false)
    };
    let actor = "maestro-harness";
    let now = utc_now_timestamp();
    let task = task::create_task(
        &paths.tasks_dir(),
        &title,
        task::CreateTaskOptions {
            feature: None,
            covers: Vec::new(),
            lane: None,
            risk: None,
            checks: checks.clone(),
            created_at: now.clone(),
        },
    )?;
    task::transition_task(
        &paths.tasks_dir(),
        &task.id,
        TaskState::Exploring,
        actor,
        &now,
        TransitionDetails::default(),
    )?;
    let task = task::accept_task(&paths.tasks_dir(), &task.id, actor, &now)?;
    item.status = "accepted".to_string();
    item.spawned_task = Some(task.id.clone());
    item.history.push(HistoryEntry {
        result: "accepted".to_string(),
        task: Some(task.id.clone()),
        note: None,
        at: utc_now_timestamp(),
    });
    let accepted = item.clone();
    if let Err(error) = backlog::save_with_snapshot(paths, &snapshot.backlog, &snapshot) {
        let rollback = rollback_spawned_task(paths, &task.id);
        return Err(save_failure_after_spawn(error, rollback, &task.id));
    }
    Ok(AppliedItem {
        item: accepted,
        checks,
        used_preset,
    })
}

fn rollback_spawned_task(paths: &MaestroPaths, task_id: &str) -> Result<()> {
    let task_yaml = task::task_yaml_path(&paths.tasks_dir(), task_id)?;
    let task_dir = task_yaml
        .parent()
        .ok_or_else(|| anyhow::anyhow!("task path has no parent: {}", task_yaml.display()))?;
    fs::remove_dir_all(task_dir)
        .with_context(|| format!("failed to roll back spawned task {}", task_dir.display()))
}

/// Compose the error for a `save_with_snapshot` failure that follows spawning a
/// task. The save error is the actionable one -- a concurrent-store change tells
/// the caller to re-run -- so a best-effort rollback must never mask it: on
/// rollback failure the leftover-task note is attached as added context while the
/// save error stays the underlying cause. Extracted so both branches are unit-
/// testable without filesystem fault injection.
fn save_failure_after_spawn(
    save_error: anyhow::Error,
    rollback: Result<()>,
    task_id: &str,
) -> anyhow::Error {
    match rollback {
        Ok(()) => save_error,
        Err(rollback_error) => save_error.context(format!(
            "spawned task {task_id} was left behind: {rollback_error:#}"
        )),
    }
}

pub fn dismiss(paths: &MaestroPaths, id: &str, reason: &str) -> Result<BacklogItem> {
    if reason.trim().is_empty() {
        bail!("dismiss reason must not be empty");
    }
    let (mut snapshot, _) = detect_and_merge(paths)?;
    let now = utc_now_timestamp();
    let item = snapshot.backlog.find_mut(id)?;
    item.status = "dismissed".to_string();
    item.dismissal_reason = Some(reason.trim().to_string());
    item.history.push(HistoryEntry {
        result: "dismissed".to_string(),
        task: item.spawned_task.clone(),
        note: None,
        at: now,
    });
    let dismissed = item.clone();
    backlog::save_with_snapshot(paths, &snapshot.backlog, &snapshot)?;
    Ok(dismissed)
}

pub fn unapply(paths: &MaestroPaths, id: &str, reason: Option<&str>) -> Result<UnappliedItem> {
    let reason = reason.map(str::trim).filter(|reason| !reason.is_empty());
    let mut snapshot = backlog::load_with_snapshot(paths)?;
    let now = utc_now_timestamp();
    let (spawned_task, status) = {
        let item = snapshot.backlog.find(id)?;
        (item.spawned_task.clone(), item.status.clone())
    };

    if status != "accepted" {
        bail!("{id} is not accepted; run `maestro harness apply {id}` before unapplying");
    }

    let task = match spawned_task.as_deref() {
        Some(task_id) => unapply_linked_task(paths, task_id, &now)?,
        None => UnappliedTask::None,
    };
    let task_note = match &task {
        UnappliedTask::Archived(task_id) => Some(format!("linked task {task_id} is archived")),
        UnappliedTask::Missing(task_id) => Some(format!("linked task {task_id} is missing")),
        UnappliedTask::None => Some("no linked task was recorded".to_string()),
        UnappliedTask::Abandoned(_) => None,
    };
    let note = match (reason, task_note.as_deref()) {
        (Some(reason), Some(task_note)) => Some(format!("{reason}; {task_note}")),
        (Some(reason), None) => Some(reason.to_string()),
        (None, Some(task_note)) => Some(task_note.to_string()),
        (None, None) => None,
    };

    let item = snapshot.backlog.find_mut(id)?;
    item.status = "proposed".to_string();
    item.spawned_task = None;
    item.history.push(HistoryEntry {
        result: "unapplied".to_string(),
        task: spawned_task,
        note,
        at: now,
    });
    let unapplied = item.clone();
    backlog::save_with_snapshot(paths, &snapshot.backlog, &snapshot)?;
    Ok(UnappliedItem {
        item: unapplied,
        task,
    })
}

fn unapply_linked_task(paths: &MaestroPaths, task_id: &str, now: &str) -> Result<UnappliedTask> {
    match task::load_task_record(&paths.tasks_dir(), task_id) {
        Ok(record) => {
            if !matches!(
                record.state,
                TaskState::Draft | TaskState::Exploring | TaskState::Ready
            ) {
                bail!(
                    "linked task {task_id} is {}; use `maestro harness measure` or close the task before unapplying",
                    record.state.as_str()
                );
            }
            task::transition_task(
                &paths.tasks_dir(),
                task_id,
                TaskState::Abandoned,
                "maestro-harness",
                now,
                TransitionDetails {
                    summary: Some("harness proposal unapplied".to_string()),
                    ..TransitionDetails::default()
                },
            )?;
            Ok(UnappliedTask::Abandoned(task_id.to_string()))
        }
        Err(_) => {
            if task::load_task_record(&paths.archive_tasks_dir(), task_id).is_ok() {
                Ok(UnappliedTask::Archived(task_id.to_string()))
            } else {
                Ok(UnappliedTask::Missing(task_id.to_string()))
            }
        }
    }
}

/// Measure an accepted proposal (the only path to `measured`). A state detector
/// gets an automatic verdict from present detection: silent → `measured`, still
/// emitting → back to `proposed` with the link cleared (D2). A behavioral detector
/// closes by human judgment — the deliberate measure on a verified task IS that
/// judgment (D1), with no silence check. Unless `force`, the linked task must be
/// verified first (impl-default (d)).
///
/// Returns the resulting item plus whether the detector's friction is still live
/// (currently emitting). The interface uses that flag to frame the verdict: a
/// reverted state detector reads as "ineffective", and a behavioral item closed by
/// judgment while still emitting gets a "friction still detected" warning (T9).
pub fn measure(paths: &MaestroPaths, id: &str, force: bool) -> Result<(BacklogItem, bool)> {
    let (mut snapshot, fresh) = detect_and_merge(paths)?;

    // Read identity + status before any gate or mutation.
    let (status, fingerprint, item_type, spawned_task) = {
        let item = snapshot.backlog.find(id)?;
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

    let friction_live = if item_type == "agent_audit" {
        audit_reproposed_since_apply(snapshot.backlog.find(id)?)
    } else {
        fresh.contains(&fingerprint)
    };

    if !force {
        match &spawned_task {
            Some(task_id) => {
                let Ok(record) = load_linked_task(paths, task_id) else {
                    bail!(
                        "linked task {task_id} could not be loaded; use --force to measure anyway"
                    );
                };
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
    let item = snapshot.backlog.find_mut(id)?;
    if (is_state_detector(&item_type) || item_type == "agent_audit") && friction_live {
        // Friction persists: the improvement was ineffective. Revert to proposed and
        // drop the link so the next accept spawns a fresh task (impl-default (c)).
        item.history.push(HistoryEntry {
            result: "ineffective".to_string(),
            task: spawned_task,
            note: None,
            at: now,
        });
        item.status = "proposed".to_string();
        item.spawned_task = None;
    } else {
        item.history.push(HistoryEntry {
            result: "measured".to_string(),
            task: spawned_task,
            note: None,
            at: now,
        });
        item.status = "measured".to_string();
    }
    let measured = item.clone();
    backlog::save_with_snapshot(paths, &snapshot.backlog, &snapshot)?;
    Ok((measured, friction_live))
}

fn default_check(item: &BacklogItem) -> String {
    match item.item_type.as_str() {
        "missing_verification" => {
            "verification command is added to harness.yml and detector is silent".to_string()
        }
        "recurring_blocker" => {
            format!("{} is resolved and detector is silent", item.title)
        }
        "missing_skill" => format!("{} is added and detector is silent", item.title),
        "rediscovered_decision" => format!("{} and detector is silent", item.title),
        "recurring_intervention" => {
            "repeated correction prompts are addressed and detector is silent".to_string()
        }
        "explicit_intervention" => {
            let topic = field_or_default(&item.topic, &item.source);
            format!(
                "guidance for {topic} is recorded in repo instructions and no new intervention events on that topic appear for the next measured sessions"
            )
        }
        "agent_audit" => {
            let topic = field_or_default(&item.topic, &item.source);
            format!(
                "{topic} improvement is implemented and a fresh maestro-audit pass does not re-propose it"
            )
        }
        _ => format!(
            "{} improvement is implemented and detector is silent",
            item.title
        ),
    }
}

fn manual_proposal(
    source: String,
    item_type: impl Into<String>,
    subject: impl AsRef<str>,
    title: impl Into<String>,
    evidence: Vec<String>,
    sessions_hit: Vec<String>,
    provenance: &str,
) -> BacklogItem {
    let item_type = item_type.into();
    let topic = subject.as_ref().to_string();
    let now = utc_now_timestamp();
    let occurrences = sessions_hit.len();
    BacklogItem {
        id: String::new(),
        fingerprint: format!("{item_type}:{topic}"),
        source,
        provenance: provenance.to_string(),
        topic,
        item_type,
        title: title.into(),
        priority: String::new(),
        occurrences,
        sessions_hit,
        first_seen: now.clone(),
        last_seen: now,
        status: "proposed".to_string(),
        evidence,
        spawned_task: None,
        dismissal_reason: None,
        history: Vec::new(),
    }
}

fn audit_reproposed_since_apply(item: &BacklogItem) -> bool {
    let Some(accepted_at) = item
        .history
        .iter()
        .rev()
        .find(|entry| entry.result == "accepted")
        .map(|entry| entry.at.as_str())
    else {
        return false;
    };
    timestamp_nanos(&item.last_seen)
        .zip(timestamp_nanos(accepted_at))
        .is_some_and(|(last_seen, accepted_at)| last_seen > accepted_at)
}

fn normalize_agent_topic(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-")
}

fn field_or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAVE_ERROR: &str =
        "backlog.yaml is being written by another Maestro process; re-run the command";

    #[test]
    fn save_failure_after_spawn_returns_the_save_error_when_rollback_succeeds() {
        let composed = save_failure_after_spawn(anyhow::anyhow!(SAVE_ERROR), Ok(()), "task-002");
        let rendered = format!("{composed:#}");
        assert!(
            rendered.contains("re-run the command"),
            "actionable save error must surface: {rendered}"
        );
        assert!(
            !rendered.contains("left behind"),
            "no leftover-task note when rollback succeeds: {rendered}"
        );
    }

    #[test]
    fn save_failure_after_spawn_keeps_the_save_error_as_cause_when_rollback_fails() {
        let composed = save_failure_after_spawn(
            anyhow::anyhow!(SAVE_ERROR),
            Err(anyhow::anyhow!("permission denied removing task dir")),
            "task-002",
        );
        let rendered = format!("{composed:#}");
        // The actionable save error is never masked by the rollback failure ...
        assert!(
            rendered.contains("re-run the command"),
            "save error must not be masked: {rendered}"
        );
        // ... and the leftover task is reported so it can be cleaned up.
        assert!(
            rendered.contains("spawned task task-002 was left behind"),
            "leftover-task note must be attached: {rendered}"
        );
        assert!(
            rendered.contains("permission denied removing task dir"),
            "rollback cause must be attached: {rendered}"
        );
    }
}
