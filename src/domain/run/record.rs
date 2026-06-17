use anyhow::{Context, Result};
use serde_json::{Map, Value, json};

use crate::domain::run::append::append_normalized_event;
use crate::domain::run::event::{
    UNATTRIBUTED_SESSION, is_accepted_event, normalized_event_type, run_dir_name, string_field,
};
use crate::domain::run::evidence::write_evidence_for_session;
use crate::foundation::core::git;
use crate::foundation::core::hash::sha256_prefixed;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::EVENT_SCHEMA_VERSION;
use crate::foundation::core::time::utc_now_timestamp;

/// Outcome of recording one hook payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordOutcome {
    /// The payload was a recognized hook event and was appended.
    Recorded {
        event_type: String,
        run_dir: String,
        /// The event's session id, or `None` when unattributed.
        session_id: Option<String>,
    },
    /// The payload was not a recognized hook event; nothing was recorded.
    Ignored { event_type: Option<String> },
}

/// Normalize and append one hook payload into the managed Run event log.
pub fn record_hook_event(paths: &MaestroPaths, payload: &Value) -> Result<RecordOutcome> {
    let Some(mut event) = normalize_event(payload) else {
        return Ok(RecordOutcome::Ignored {
            event_type: event_type(payload),
        });
    };
    attach_commit_snapshot(paths, &mut event);
    let event_type = event
        .get("event_type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let session_id = event
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let run_dir = session_id
        .as_deref()
        .map(run_dir_name)
        .unwrap_or_else(|| UNATTRIBUTED_SESSION.to_string());
    append_normalized_event(paths, &event)?;
    if is_stop_event(&event) {
        // A Stop with no session id must read back the same run bucket
        // `append_normalized_event` just wrote to. Append maps a missing session
        // to the `unattributed` dir, and `run_dir_name("")` resolves to that
        // same dir; passing the `UNATTRIBUTED_SESSION` string here instead would
        // be treated as a real session and encode to `%75nattributed`, so
        // evidence would look in the wrong directory and find no events.
        let session_id = event
            .get("session_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        write_evidence_for_session(paths, session_id).context("failed to write run evidence")?;
    }
    Ok(RecordOutcome::Recorded {
        event_type,
        run_dir,
        session_id,
    })
}

fn normalize_event(payload: &Value) -> Option<Value> {
    let event_type = event_type(payload)?;
    if !is_accepted_event(&event_type) {
        return None;
    }

    let session_id = string_field(payload, "session_id").filter(|value| !value.trim().is_empty());
    let mut event = Map::new();
    event.insert("schema_version".to_string(), json!(EVENT_SCHEMA_VERSION));
    event.insert("ts".to_string(), json!(utc_now_timestamp()));
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
    copy_string(payload, &mut event, "card_id");
    copy_string(payload, &mut event, "tool_name");
    copy_string(payload, &mut event, "status");
    copy_string(payload, &mut event, "permission_decision");
    copy_string(payload, &mut event, "skill_name");
    copy_string(payload, &mut event, "activation_mode");
    copy_number(payload, &mut event, "duration_ms");

    if let Some(tool_input) = payload.get("tool_input") {
        event.insert("tool_input_hash".to_string(), json!(hash_value(tool_input)));
        // Keep the edited path so a peer's warm-file overlap can be surfaced
        // (src/domain/run/active.rs); the rest of tool_input stays hashed away.
        if let Some(file_path) = tool_input.get("file_path").and_then(Value::as_str) {
            let trimmed = file_path.trim();
            if !trimmed.is_empty() {
                event.insert("file_path".to_string(), json!(trimmed));
            }
        }
    }

    Some(Value::Object(event))
}

fn attach_commit_snapshot(paths: &MaestroPaths, event: &mut Value) {
    if !matches!(
        event.get("event_type").and_then(Value::as_str),
        Some("SessionStart" | "Stop")
    ) {
        return;
    }
    let Ok(Some(head)) = git::head(paths.repo_root()) else {
        return;
    };
    if let Some(object) = event.as_object_mut() {
        object.insert("commit".to_string(), json!(head));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_file_path_from_edit_tool_input_and_keeps_the_hash() {
        let payload = json!({
            "event_type": "PostToolUse",
            "session_id": "cli-test",
            "tool_name": "Edit",
            "tool_input": {"file_path": "src/auth/login.rs", "old_string": "a", "new_string": "b"},
        });
        let event = normalize_event(&payload).expect("Edit PostToolUse is an accepted event");
        let object = event.as_object().expect("normalized event is an object");
        assert_eq!(
            object.get("file_path").and_then(Value::as_str),
            Some("src/auth/login.rs"),
            "file_path must be retained, not hashed away"
        );
        assert!(
            object.contains_key("tool_input_hash"),
            "tool_input_hash must still be recorded alongside file_path"
        );
    }

    #[test]
    fn omits_file_path_when_tool_input_carries_none() {
        let payload = json!({
            "event_type": "PostToolUse",
            "session_id": "cli-test",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"},
        });
        let event = normalize_event(&payload).expect("Bash PostToolUse is an accepted event");
        let object = event.as_object().expect("normalized event is an object");
        assert!(
            !object.contains_key("file_path"),
            "no file_path field when tool_input has none"
        );
        assert!(object.contains_key("tool_input_hash"));
    }

    #[test]
    fn omits_file_path_when_blank() {
        let payload = json!({
            "event_type": "PostToolUse",
            "session_id": "cli-test",
            "tool_name": "Edit",
            "tool_input": {"file_path": "   "},
        });
        let event = normalize_event(&payload).expect("accepted event");
        assert!(
            !event.as_object().expect("object").contains_key("file_path"),
            "a blank file_path is dropped"
        );
    }
}
