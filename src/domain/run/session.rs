use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::Result;
use serde::Serialize;

use crate::domain::card;
use crate::foundation::core::paths::MaestroPaths;

use super::activity::{ActivityRecord, read_session_activity, summarize_activity_records};
use super::event::run_dir_name;
use super::reader::visit_event_log;

#[derive(Clone, Debug, Serialize)]
pub struct SessionReadout {
    pub session_id: String,
    pub outcome: String,
    pub ownership: String,
    pub activity: SessionActivitySummary,
    pub lifecycle: SessionLifecycleSummary,
    pub proof: SessionProofSummary,
    pub tasks: Vec<SessionTaskSummary>,
    pub sources: SessionSources,
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionActivitySummary {
    pub events: usize,
    pub commands: usize,
    pub compactions: usize,
    pub counts: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionLifecycleSummary {
    pub events: usize,
    pub counts: BTreeMap<String, usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_ts: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionProofSummary {
    pub events: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionTaskSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(rename = "type")]
    pub card_type: String,
    pub proof_events: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionSources {
    pub activity: String,
    pub lifecycle: String,
    pub proof: String,
    pub transcript: String,
}

#[derive(Default)]
struct EventFold {
    counts: BTreeMap<String, usize>,
    last: Option<(String, String)>,
    task_ids: BTreeSet<String>,
    card_ids: BTreeSet<String>,
    proof_events: usize,
    proof_by_task: BTreeMap<String, usize>,
    ownership: String,
    outcome: String,
}

pub fn session_readout(paths: &MaestroPaths, session_id: &str) -> Result<SessionReadout> {
    let activities = read_session_activity(paths, session_id)?;
    let activity = summarize_activity(&activities);
    let mut fold = fold_lifecycle(paths, session_id)?;
    for activity in &activities {
        if let Some(task_id) = &activity.task_id {
            fold.task_ids.insert(task_id.clone());
        }
        if let Some(card_id) = &activity.card_id {
            fold.card_ids.insert(card_id.clone());
        }
    }

    let cards = card::query::scan(paths).unwrap_or_default();
    let by_id: HashMap<&str, &card::schema::Card> =
        cards.iter().map(|card| (card.id.as_str(), card)).collect();
    let mut ids = fold.task_ids.clone();
    ids.extend(fold.card_ids.iter().cloned());
    let mut tasks: Vec<SessionTaskSummary> = ids
        .into_iter()
        .map(|id| {
            let card = by_id.get(id.as_str());
            SessionTaskSummary {
                proof_events: fold.proof_by_task.get(&id).copied().unwrap_or(0),
                title: card
                    .map(|card| card.title.clone())
                    .unwrap_or_else(|| "(not in store)".to_string()),
                status: card
                    .map(|card| card::query::canonical_status(&card.status).to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                card_type: card
                    .map(|card| card.card_type.as_str().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                id,
            }
        })
        .collect();
    tasks.sort_by(|left, right| left.id.cmp(&right.id));

    let lifecycle = SessionLifecycleSummary {
        events: fold.counts.values().sum(),
        counts: fold.counts,
        last_action: fold.last.as_ref().map(|(_, action)| action.clone()),
        last_ts: fold.last.map(|(ts, _)| ts),
    };
    Ok(SessionReadout {
        session_id: session_id.to_string(),
        outcome: fold.outcome,
        ownership: fold.ownership,
        activity,
        lifecycle,
        proof: SessionProofSummary {
            events: fold.proof_events,
        },
        tasks,
        sources: SessionSources {
            activity: "ledger".to_string(),
            lifecycle: "runs".to_string(),
            proof: "task store + run events".to_string(),
            transcript: "unavailable".to_string(),
        },
        gaps: vec!["transcript backfill unavailable".to_string()],
    })
}

fn summarize_activity(records: &[ActivityRecord]) -> SessionActivitySummary {
    let counts = summarize_activity_records(records);
    SessionActivitySummary {
        events: counts.events,
        commands: counts.commands,
        compactions: counts.compactions,
        counts: counts.counts,
    }
}

fn fold_lifecycle(paths: &MaestroPaths, session_id: &str) -> Result<EventFold> {
    let path = paths
        .runs_dir()
        .join(run_dir_name(session_id))
        .join("events.jsonl");
    let mut fold = EventFold {
        ownership: "unknown".to_string(),
        outcome: "unknown".to_string(),
        ..EventFold::default()
    };
    visit_event_log(&path, |record| {
        let event = record.event();
        let kind = event
            .event_type()
            .or_else(|| event.alias_kind())
            .unwrap_or("<unknown>")
            .to_string();
        *fold.counts.entry(kind.clone()).or_default() += 1;
        if let Some(ts) = event.timestamp()
            && fold
                .last
                .as_ref()
                .is_none_or(|(last_ts, _)| ts >= last_ts.as_str())
        {
            fold.last = Some((ts.to_string(), kind.clone()));
        }
        if let Some(task_id) = event.task_id() {
            fold.task_ids.insert(task_id.to_string());
            if kind == "task_proof" {
                *fold.proof_by_task.entry(task_id.to_string()).or_default() += 1;
            }
        }
        if let Some(card_id) = event.card_id() {
            fold.card_ids.insert(card_id.to_string());
        }
        match kind.as_str() {
            "task_proof" => fold.proof_events += 1,
            "ownership_acquire" => {
                fold.ownership = "active".to_string();
                fold.outcome = "in_progress".to_string();
            }
            "ownership_release" => {
                fold.ownership = event.status().unwrap_or("released").to_string();
                fold.outcome = match event.status() {
                    Some("done") => "done".to_string(),
                    Some(status) => status.to_string(),
                    None => "released".to_string(),
                };
            }
            _ => {}
        }
        Ok(())
    })?;
    if fold.outcome == "unknown" && !fold.counts.is_empty() {
        fold.outcome = "activity_observed".to_string();
    }
    Ok(fold)
}
