use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::core::paths::MaestroPaths;

const EVENT_SCHEMA_VERSION: &str = "maestro.event.v1";
const UNATTRIBUTED_SESSION: &str = "unattributed";

pub fn record_stdin(paths: &MaestroPaths) -> Result<()> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .context("failed to read hook payload from stdin")?;
    record_payload(paths, &raw)
}

pub fn record_payload(paths: &MaestroPaths, raw: &str) -> Result<()> {
    let payload: Value = serde_json::from_str(raw).context("failed to parse hook payload JSON")?;
    let Some(event) = normalize_event(&payload) else {
        return Ok(());
    };
    append_event(paths, &event)
}

fn normalize_event(payload: &Value) -> Option<Value> {
    let event_type = event_type(payload)?;
    if !is_accepted_event(&event_type) {
        return None;
    }

    let session_id = string_field(payload, "session_id").filter(|value| !value.trim().is_empty());
    let mut event = Map::new();
    event.insert("schema_version".to_string(), json!(EVENT_SCHEMA_VERSION));
    event.insert("ts".to_string(), json!(timestamp()));
    event.insert(
        "event_type".to_string(),
        json!(normalized_event_type(&event_type)),
    );
    if let Some(session_id) = &session_id {
        event.insert("session_id".to_string(), json!(session_id));
    }

    copy_string(payload, &mut event, "agent");
    copy_string(payload, &mut event, "task_id");
    copy_string(payload, &mut event, "feature_id");
    copy_string(payload, &mut event, "tool_name");
    copy_string(payload, &mut event, "status");
    copy_string(payload, &mut event, "permission_decision");
    copy_string(payload, &mut event, "skill_name");
    copy_string(payload, &mut event, "activation_mode");
    copy_number(payload, &mut event, "duration_ms");

    if let Some(tool_input) = payload.get("tool_input") {
        event.insert("tool_input_hash".to_string(), json!(hash_value(tool_input)));
    }

    Some(Value::Object(event))
}

fn append_event(paths: &MaestroPaths, event: &Value) -> Result<()> {
    let session_id = event
        .get("session_id")
        .and_then(Value::as_str)
        .map(run_dir_name)
        .unwrap_or_else(|| UNATTRIBUTED_SESSION.to_string());
    let path = paths.runs_dir().join(session_id).join("events.jsonl");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let line = serde_json::to_string(event).context("failed to encode normalized hook event")?;
    writeln!(file, "{line}").with_context(|| format!("failed to append {}", path.display()))
}

fn event_type(payload: &Value) -> Option<String> {
    string_field(payload, "event_type")
        .or_else(|| string_field(payload, "hook_event_name"))
        .or_else(|| string_field(payload, "kind"))
        .or_else(|| string_field(payload, "event"))
        .or_else(|| string_field(payload, "type"))
}

fn is_accepted_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "SessionStart"
            | "UserPromptSubmit"
            | "PreToolUse"
            | "PermissionRequest"
            | "PostToolUse"
            | "Stop"
            | "SkillActivation"
            | "skill_activation"
    )
}

fn normalized_event_type(event_type: &str) -> &str {
    match event_type {
        "SkillActivation" => "skill_activation",
        other => other,
    }
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

fn string_field(source: &Value, field: &str) -> Option<String> {
    source
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn hash_value(value: &Value) -> String {
    let bytes =
        serde_json::to_vec(value).expect("invariant: serde_json::Value should serialize to JSON");
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("sha256:{hex}")
}

fn run_dir_name(session_id: &str) -> String {
    let sanitized = session_id
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => character,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        UNATTRIBUTED_SESSION.to_string()
    } else {
        sanitized
    }
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
