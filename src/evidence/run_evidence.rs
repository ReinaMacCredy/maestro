use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::core::git;
use crate::core::paths::MaestroPaths;
use crate::core::safe_write::write_string_atomic;
use crate::core::schema::RUN_EVIDENCE_SCHEMA_VERSION;
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

#[derive(Debug)]
struct Timestamp {
    raw: String,
    nanos_since_epoch: i128,
}

/// Aggregate `.maestro/runs/<session_id>/events.jsonl` into `run_evidence.yaml`.
pub fn write_for_session(paths: &MaestroPaths, session_id: &str) -> Result<()> {
    let run_dir = paths.runs_dir().join(run_dir_name(session_id));
    let events_path = run_dir.join("events.jsonl");
    let events = BufReader::new(
        File::open(&events_path)
            .with_context(|| format!("failed to read {}", events_path.display()))?,
    );
    let evidence = build_evidence(paths, session_id, events);
    let yaml = serde_yaml::to_string(&evidence).context("failed to serialize run evidence")?;
    write_string_atomic(run_dir.join("run_evidence.yaml"), &yaml)
}

fn build_evidence(paths: &MaestroPaths, session_id: &str, events: impl BufRead) -> RunEvidence {
    let mut agent = None;
    let mut task_id = None;
    let mut start = None;
    let mut end = None;
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
        if let Some(tool_name) = string_field(&event, "tool_name") {
            *tools_used.entry(tool_name).or_insert(0) += 1;
        }
        if string_field(&event, "event_type").as_deref() == Some("UserPromptSubmit") {
            prompts += 1;
        }
        if let Some(timestamp) = string_field(&event, "ts").and_then(|ts| parse_timestamp(&ts)) {
            if start.is_none() {
                start = Some(Timestamp {
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
    let head = git::head(paths.repo_root()).unwrap_or(None);

    RunEvidence {
        schema_version: RUN_EVIDENCE_SCHEMA_VERSION,
        session_id: session_id.to_string(),
        agent,
        task_id,
        start_at: start.map(|timestamp| timestamp.raw),
        end_at: end.map(|timestamp| timestamp.raw),
        start_commit: head.clone(),
        end_commit: head,
        tools_used,
        human_interventions: prompts.saturating_sub(1),
        duration_seconds,
    }
}

fn parse_event_line(line: &str) -> Option<Value> {
    serde_json::from_str(line).ok()
}

fn parse_timestamp(value: &str) -> Option<Timestamp> {
    let value = value.strip_suffix('Z')?;
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i64>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let seconds = time_parts.next()?;
    if time_parts.next().is_some() {
        return None;
    }

    let (second, nanos) = parse_seconds(seconds)?;
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    let days = days_from_civil(year, month, day)?;
    let seconds_since_epoch = days * 86_400 + i128::from(hour * 3_600 + minute * 60 + second);
    Some(Timestamp {
        raw: format!("{value}Z"),
        nanos_since_epoch: seconds_since_epoch * 1_000_000_000 + i128::from(nanos),
    })
}

fn parse_seconds(value: &str) -> Option<(u32, u32)> {
    let (seconds, fraction) = match value.split_once('.') {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (value, None),
    };
    let seconds = seconds.parse::<u32>().ok()?;
    let nanos = match fraction {
        Some(fraction) if fraction.is_empty() || fraction.len() > 9 => return None,
        Some(fraction) => {
            if !fraction.chars().all(|character| character.is_ascii_digit()) {
                return None;
            }
            let padded = format!("{fraction:0<9}");
            padded.parse::<u32>().ok()?
        }
        None => 0,
    };
    Some((seconds, nanos))
}

fn days_from_civil(year: i64, month: u32, day: u32) -> Option<i128> {
    if day > days_in_month(year, month)? {
        return None;
    }
    let adjusted_year = year - i64::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(i128::from(era * 146_097 + day_of_era - 719_468))
}

fn days_in_month(year: i64, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
