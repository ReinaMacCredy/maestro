use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::core::managed_path::{managed_path, SymlinkPolicy};
use crate::core::paths::MaestroPaths;
use crate::core::safe_write::write_string_atomic;
use crate::core::schema::RUN_EVIDENCE_SCHEMA_VERSION;
use crate::core::time::{parse_utc_timestamp, ParsedTimestamp};
use crate::hooks::event::{run_dir_name, string_field};

#[derive(Debug, Serialize)]
struct RunEvidence {
    schema_version: &'static str,
    session_id: String,
    agent: Option<String>,
    task_id: Option<String>,
    start_at: Option<String>,
    end_at: Option<String>,
    start_commit: Option<String>,
    end_commit: Option<String>,
    tools_used: BTreeMap<String, u64>,
    human_interventions: u64,
    duration_seconds: Option<u64>,
}

/// Aggregate `.maestro/runs/<session_id>/events.jsonl` into `run_evidence.yaml`.
pub fn write_for_session(paths: &MaestroPaths, session_id: &str) -> Result<()> {
    let run_dir_name = run_dir_name(session_id);
    managed_path(
        paths,
        &format!(".maestro/runs/{run_dir_name}"),
        SymlinkPolicy::RejectAllComponents,
    )?;
    let events_path = managed_path(
        paths,
        &format!(".maestro/runs/{run_dir_name}/events.jsonl"),
        SymlinkPolicy::RejectAllComponents,
    )?;
    let events = BufReader::new(
        File::open(&events_path)
            .with_context(|| format!("failed to read {}", events_path.display()))?,
    );
    let evidence = build_evidence(session_id, events);
    let yaml = serde_yaml::to_string(&evidence).context("failed to serialize run evidence")?;
    let evidence_path = managed_path(
        paths,
        &format!(".maestro/runs/{run_dir_name}/run_evidence.yaml"),
        SymlinkPolicy::RejectAllComponents,
    )?;
    write_string_atomic(evidence_path, &yaml)
}

fn build_evidence(session_id: &str, events: impl BufRead) -> RunEvidence {
    let mut agent = None;
    let mut task_id = None;
    let mut start = None;
    let mut end = None;
    let mut start_commit = None;
    let mut end_commit = None;
    let mut tools_used = BTreeMap::new();
    let mut prompts = 0_u64;

    for event in events
        .lines()
        .map_while(|line| line.ok())
        .filter_map(|line| parse_event_line(&line))
    {
        if agent.is_none() {
            agent = string_field(&event, "agent");
        }
        if task_id.is_none() {
            task_id = string_field(&event, "task_id");
        }
        if string_field(&event, "event_type").as_deref() == Some("PostToolUse") {
            if let Some(tool_name) = string_field(&event, "tool_name") {
                *tools_used.entry(tool_name).or_insert(0) += 1;
            }
        }
        if string_field(&event, "event_type").as_deref() == Some("UserPromptSubmit") {
            prompts += 1;
        }
        match string_field(&event, "event_type").as_deref() {
            Some("SessionStart") if start_commit.is_none() => {
                start_commit = string_field(&event, "commit");
            }
            Some("Stop") => {
                end_commit = string_field(&event, "commit");
            }
            _ => {}
        }
        if let Some(timestamp) = string_field(&event, "ts").and_then(|ts| parse_utc_timestamp(&ts))
        {
            if start.is_none() {
                start = Some(ParsedTimestamp {
                    raw: timestamp.raw.clone(),
                    nanos_since_epoch: timestamp.nanos_since_epoch,
                });
            }
            end = Some(timestamp);
        }
    }

    let duration_seconds = match (&start, &end) {
        (Some(start), Some(end)) if end.nanos_since_epoch >= start.nanos_since_epoch => {
            Some(((end.nanos_since_epoch - start.nanos_since_epoch) / 1_000_000_000) as u64)
        }
        _ => None,
    };
    RunEvidence {
        schema_version: RUN_EVIDENCE_SCHEMA_VERSION,
        session_id: session_id.to_string(),
        agent,
        task_id,
        start_at: start.map(|timestamp| timestamp.raw),
        end_at: end.map(|timestamp| timestamp.raw),
        start_commit,
        end_commit,
        tools_used,
        human_interventions: prompts.saturating_sub(1),
        duration_seconds,
    }
}

fn parse_event_line(line: &str) -> Option<Value> {
    serde_json::from_str(line).ok()
}
