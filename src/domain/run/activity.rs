use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, ErrorKind};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::domain::run::append::{append_jsonl_line, open_managed_appendable};
use crate::domain::run::event::{UNATTRIBUTED_SESSION, run_dir_name};
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::SESSION_ACTIVITY_SCHEMA_VERSION;

#[derive(Clone, Debug, Serialize)]
pub struct ActivityRecord {
    pub kind: String,
    pub source: String,
    pub source_event_type: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<ActivityCommand>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActivityCommand {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_hash: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActivityCounts {
    pub events: usize,
    pub commands: usize,
    pub compactions: usize,
    pub counts: BTreeMap<String, usize>,
}

pub fn read_session_activity(
    paths: &MaestroPaths,
    session_id: &str,
) -> Result<Vec<ActivityRecord>> {
    let path = paths
        .runs_dir()
        .join(run_dir_name(session_id))
        .join("activity.jsonl");
    visit_activity_log(&path)
}

pub fn session_activity_counts(paths: &MaestroPaths, session_id: &str) -> Result<ActivityCounts> {
    read_session_activity(paths, session_id).map(|records| summarize_activity_records(&records))
}

pub(crate) fn summarize_activity_records(records: &[ActivityRecord]) -> ActivityCounts {
    let mut counts = BTreeMap::<String, usize>::new();
    for record in records {
        *counts.entry(record.kind.clone()).or_default() += 1;
    }
    let commands = counts
        .get("command_finished")
        .copied()
        .unwrap_or(0)
        .max(counts.get("command_started").copied().unwrap_or(0));
    let compactions = counts.get("compaction_observed").copied().unwrap_or(0);
    ActivityCounts {
        events: records.len(),
        commands,
        compactions,
        counts,
    }
}

fn visit_activity_log(path: &std::path::Path) -> Result<Vec<ActivityRecord>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => return Ok(Vec::new()),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    }
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };
    let mut records = Vec::new();
    let mut line = Vec::new();
    let mut reader = BufReader::new(file);
    loop {
        line.clear();
        let bytes = reader
            .read_until(b'\n', &mut line)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if bytes == 0 {
            break;
        }
        if !line.ends_with(b"\n") {
            break;
        }
        line.pop();
        if line.is_empty() {
            continue;
        }
        let Ok(raw_line) = std::str::from_utf8(&line) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(raw_line) else {
            continue;
        };
        if let Some(record) = activity_record_from_value(&value) {
            records.push(record);
        }
    }
    Ok(records)
}

pub(crate) fn append_activity_for_run_event(
    paths: &MaestroPaths,
    session_dir: &str,
    event: &Value,
) -> Result<()> {
    let Some(activity) = activity_from_run_event(event) else {
        return Ok(());
    };
    let relative_path = format!(".maestro/runs/{session_dir}/activity.jsonl");
    let mut file = open_managed_appendable(paths, &relative_path)?;
    append_jsonl_line(&mut file, &activity)
        .with_context(|| format!("failed to append {relative_path}"))
}

fn activity_from_run_event(event: &Value) -> Option<Value> {
    let source_event_type = source_event_type(event)?;
    let kind = activity_kind(source_event_type, event)?;

    let mut activity = Map::new();
    activity.insert(
        "schema_version".to_string(),
        json!(SESSION_ACTIVITY_SCHEMA_VERSION),
    );
    activity.insert("source".to_string(), json!("run_event"));
    activity.insert("source_event_type".to_string(), json!(source_event_type));
    activity.insert("kind".to_string(), json!(kind));
    activity.insert(
        "session_id".to_string(),
        json!(string_field(event, "session_id").unwrap_or(UNATTRIBUTED_SESSION)),
    );

    copy_string(event, &mut activity, "ts");
    copy_string(event, &mut activity, "agent_runtime");
    copy_string(event, &mut activity, "task_id");
    copy_string(event, &mut activity, "card_id");
    copy_string(event, &mut activity, "feature_id");
    copy_string(event, &mut activity, "status");
    copy_number(event, &mut activity, "duration_ms");
    copy_string(event, &mut activity, "action");
    copy_string(event, &mut activity, "target_kind");
    copy_string(event, &mut activity, "target_id");
    copy_string(event, &mut activity, "result");

    if let Some(command) = command_metadata(event) {
        activity.insert("command".to_string(), Value::Object(command));
    }

    Some(Value::Object(activity))
}

fn activity_record_from_value(value: &Value) -> Option<ActivityRecord> {
    Some(ActivityRecord {
        kind: string_field(value, "kind")?.to_string(),
        source: string_field(value, "source")?.to_string(),
        source_event_type: string_field(value, "source_event_type")?.to_string(),
        session_id: string_field(value, "session_id")?.to_string(),
        ts: string_field(value, "ts").map(str::to_string),
        task_id: string_field(value, "task_id").map(str::to_string),
        card_id: string_field(value, "card_id").map(str::to_string),
        feature_id: string_field(value, "feature_id").map(str::to_string),
        status: string_field(value, "status").map(str::to_string),
        duration_ms: value.get("duration_ms").and_then(Value::as_u64),
        command: value.get("command").and_then(activity_command_from_value),
    })
}

fn activity_command_from_value(value: &Value) -> Option<ActivityCommand> {
    let command = ActivityCommand {
        program: string_field(value, "program").map(str::to_string),
        file_path: string_field(value, "file_path").map(str::to_string),
        input_hash: string_field(value, "input_hash").map(str::to_string),
    };
    (command.program.is_some() || command.file_path.is_some() || command.input_hash.is_some())
        .then_some(command)
}

fn source_event_type(event: &Value) -> Option<&str> {
    string_field(event, "event_type")
        .or_else(|| string_field(event, "event"))
        .or_else(|| string_field(event, "kind"))
        .or_else(|| string_field(event, "type"))
}

fn activity_kind(source_event_type: &str, event: &Value) -> Option<&'static str> {
    match source_event_type {
        "PreToolUse" => Some("command_started"),
        "PostToolUse" => Some("command_finished"),
        "task_proof" => Some("proof_recorded"),
        "ownership_acquire" => Some("ownership_acquired"),
        "ownership_release" => Some("ownership_released"),
        "intervention" => Some("intervention"),
        "verification_passed" => Some("verification_passed"),
        "verification_failed" => Some("verification_failed"),
        "feature_ready" => Some("feature_ready"),
        "feature_closed" => Some("feature_closed"),
        "autonomy_action" => autonomy_action_kind(event),
        _ => None,
    }
}

fn autonomy_action_kind(event: &Value) -> Option<&'static str> {
    match (
        string_field(event, "target_kind"),
        string_field(event, "action"),
        string_field(event, "result"),
    ) {
        (Some("feature"), Some("feature_close"), _) => Some("feature_closed"),
        (Some("feature"), _, Some("closed")) => Some("feature_closed"),
        (Some("feature"), _, Some("ready")) => Some("feature_ready"),
        _ => None,
    }
}

fn command_metadata(event: &Value) -> Option<Map<String, Value>> {
    let mut command = Map::new();
    if let Some(tool_name) = string_field(event, "tool_name") {
        command.insert("program".to_string(), json!(tool_name));
    }
    copy_string(event, &mut command, "file_path");
    if let Some(input_hash) = string_field(event, "tool_input_hash") {
        command.insert("input_hash".to_string(), json!(input_hash));
    }
    if let Some(raw_command) = string_field(event, "command") {
        command.insert(
            "input_hash".to_string(),
            json!(sha256_prefixed(raw_command.as_bytes())),
        );
    }
    (!command.is_empty()).then_some(command)
}

fn copy_string(source: &Value, target: &mut Map<String, Value>, field: &str) {
    if let Some(value) = string_field(source, field) {
        target.insert(field.to_string(), json!(value));
    }
}

fn copy_number(source: &Value, target: &mut Map<String, Value>, field: &str) {
    if let Some(value) = source.get(field).and_then(Value::as_u64) {
        target.insert(field.to_string(), json!(value));
    }
}

fn string_field<'a>(source: &'a Value, field: &str) -> Option<&'a str> {
    source
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_proof_maps_to_proof_activity_without_claim_text() {
        let event = json!({
            "event": "task_proof",
            "session_id": "run-1",
            "task_id": "task-1",
            "claims": ["GREEN includes secret details"],
            "message": "raw proof message",
            "ts": "2026-07-01T00:00:00Z"
        });

        let activity = activity_from_run_event(&event).expect("task proof maps to activity");
        assert_eq!(activity["kind"], "proof_recorded");
        assert_eq!(activity["task_id"], "task-1");
        let raw = serde_json::to_string(&activity).expect("activity serializes");
        assert!(!raw.contains("GREEN includes secret details"));
        assert!(!raw.contains("raw proof message"));
    }

    #[test]
    fn intervention_maps_without_note_text() {
        let event = json!({
            "event_type": "intervention",
            "session_id": "run-1",
            "note": "human note may contain sensitive text",
            "topic": "course-correction",
            "ts": "2026-07-01T00:00:00Z"
        });

        let activity = activity_from_run_event(&event).expect("intervention maps to activity");
        assert_eq!(activity["kind"], "intervention");
        let raw = serde_json::to_string(&activity).expect("activity serializes");
        assert!(!raw.contains("sensitive text"));
    }
}
