use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::domain::{card, task};
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

#[derive(Clone, Debug, Default)]
struct TranscriptBackfill {
    commands: usize,
    compactions: usize,
    counts: BTreeMap<String, usize>,
}

pub fn session_readout(paths: &MaestroPaths, session_id: &str) -> Result<SessionReadout> {
    let activities = read_session_activity(paths, session_id)?;
    let transcript = read_transcript_backfill(session_id)?;
    let activity = summarize_activity(&activities, transcript.as_ref());
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
        .map(|id| session_task_summary(paths, &id, by_id.get(id.as_str()).copied(), &fold))
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
            activity: activity_source(&activities, transcript.as_ref()).to_string(),
            lifecycle: "runs".to_string(),
            proof: "task store + run events".to_string(),
            transcript: if transcript.is_some() {
                "backfill".to_string()
            } else {
                "unavailable".to_string()
            },
        },
        gaps: transcript
            .is_none()
            .then(|| "transcript backfill unavailable".to_string())
            .into_iter()
            .collect(),
    })
}

fn session_task_summary(
    paths: &MaestroPaths,
    id: &str,
    card: Option<&card::schema::Card>,
    fold: &EventFold,
) -> SessionTaskSummary {
    if let Some(task) = task::try_load_task_record(&paths.tasks_dir(), id)
        .ok()
        .flatten()
    {
        return SessionTaskSummary {
            id: id.to_string(),
            title: task.title,
            status: task.state.as_str().to_string(),
            card_type: "task".to_string(),
            proof_events: fold.proof_by_task.get(id).copied().unwrap_or(0),
        };
    }
    SessionTaskSummary {
        id: id.to_string(),
        proof_events: fold.proof_by_task.get(id).copied().unwrap_or(0),
        title: card
            .map(|card| card.title.clone())
            .unwrap_or_else(|| "(not in store)".to_string()),
        status: card
            .map(|card| card::query::canonical_status(&card.status).to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        card_type: card
            .map(|card| card.card_type.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string()),
    }
}

fn summarize_activity(
    records: &[ActivityRecord],
    transcript: Option<&TranscriptBackfill>,
) -> SessionActivitySummary {
    let counts = summarize_activity_records(records);
    if records.is_empty()
        && let Some(transcript) = transcript
    {
        return SessionActivitySummary {
            events: transcript.commands + transcript.compactions,
            commands: transcript.commands,
            compactions: transcript.compactions,
            counts: transcript.counts.clone(),
        };
    }
    SessionActivitySummary {
        events: counts.events,
        commands: counts.commands,
        compactions: counts.compactions,
        counts: counts.counts,
    }
}

fn activity_source(
    records: &[ActivityRecord],
    transcript: Option<&TranscriptBackfill>,
) -> &'static str {
    if records.is_empty() && transcript.is_some() {
        "ledger + transcript backfill"
    } else {
        "ledger"
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

fn read_transcript_backfill(session_id: &str) -> Result<Option<TranscriptBackfill>> {
    let Some(path) = find_transcript_file(session_id)? else {
        return Ok(None);
    };
    summarize_transcript_file(&path).map(Some)
}

fn find_transcript_file(session_id: &str) -> Result<Option<PathBuf>> {
    let Some(root) = codex_sessions_dir() else {
        return Ok(None);
    };
    find_transcript_file_under(&root, session_id)
}

fn codex_sessions_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME") {
        return Some(PathBuf::from(home).join("sessions"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex/sessions"))
}

fn find_transcript_file_under(root: &Path, session_id: &str) -> Result<Option<PathBuf>> {
    match fs::read_dir(root) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry?;
                let file_type = entry.file_type()?;
                let path = entry.path();
                if file_type.is_dir() {
                    if let Some(found) = find_transcript_file_under(&path, session_id)? {
                        return Ok(Some(found));
                    }
                } else if file_type.is_file()
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".jsonl") && name.contains(session_id))
                {
                    return Ok(Some(path));
                }
            }
            Ok(None)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn summarize_transcript_file(path: &Path) -> Result<TranscriptBackfill> {
    let file = File::open(path)?;
    let mut backfill = TranscriptBackfill::default();
    for line in BufReader::new(file).lines() {
        let line = line?;
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match value.get("type").and_then(Value::as_str) {
            Some("compacted") => {
                backfill.compactions += 1;
                *backfill
                    .counts
                    .entry("transcript_compaction_observed".to_string())
                    .or_default() += 1;
            }
            Some("response_item")
                if value
                    .get("payload")
                    .and_then(|payload| payload.get("type"))
                    .and_then(Value::as_str)
                    == Some("function_call") =>
            {
                backfill.commands += 1;
                *backfill
                    .counts
                    .entry("transcript_command_observed".to_string())
                    .or_default() += 1;
            }
            _ => {}
        }
    }
    Ok(backfill)
}
