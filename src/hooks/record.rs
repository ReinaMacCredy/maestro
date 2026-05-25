use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

use crate::core::hash::sha256_prefixed;
use crate::core::paths::MaestroPaths;
use crate::core::schema::EVENT_SCHEMA_VERSION;
use crate::evidence::run_evidence;
use crate::hooks::event::{
    is_accepted_event, normalized_event_type, run_dir_name, string_field, UNATTRIBUTED_SESSION,
};

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
    append_event(paths, &event)?;
    if is_stop_event(&event) {
        let session_id = event
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or(UNATTRIBUTED_SESSION);
        let _ = run_evidence::write_for_session(paths, session_id);
    }
    Ok(())
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
    let mut file = match open_event_file(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            open_event_file(&path).with_context(|| format!("failed to open {}", path.display()))?
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to open {}", path.display()));
        }
    };
    let line = serde_json::to_string(event).context("failed to encode normalized hook event")?;
    writeln!(file, "{line}").with_context(|| format!("failed to append {}", path.display()))
}

fn open_event_file(path: &std::path::Path) -> io::Result<fs::File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn event_type(payload: &Value) -> Option<String> {
    string_field(payload, "event_type")
        .or_else(|| string_field(payload, "hook_event_name"))
        .or_else(|| string_field(payload, "kind"))
        .or_else(|| string_field(payload, "event"))
        .or_else(|| string_field(payload, "type"))
}

fn is_stop_event(event: &Value) -> bool {
    event.get("event_type").and_then(Value::as_str) == Some("Stop")
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

fn hash_value(value: &Value) -> String {
    let bytes =
        serde_json::to_vec(value).expect("invariant: serde_json::Value should serialize to JSON");
    sha256_prefixed(&bytes)
}

fn timestamp() -> String {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "1970-01-01T00:00:00.000000000Z".to_string();
    };
    format_unix_timestamp(duration.as_secs(), duration.subsec_nanos())
}

fn format_unix_timestamp(seconds: u64, nanos: u32) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{nanos:09}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let adjusted_year = year + i64::from(month <= 2);
    (adjusted_year, month as u32, day as u32)
}
