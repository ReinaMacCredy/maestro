use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::Result;
use serde_json::{json, Value};

use crate::domain::run;
use crate::domain::task;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::EVENT_SCHEMA_VERSION;
use crate::foundation::core::time::utc_now_timestamp;

/// List all managed `.maestro/runs/**/events.jsonl` files.
pub fn managed_event_files(paths: &MaestroPaths) -> Result<Vec<PathBuf>> {
    Ok(run::managed_event_logs(paths)?
        .into_iter()
        .map(|log| log.path().to_path_buf())
        .collect())
}

/// Record a task proof event into the managed run log.
///
/// Gathers the task's recorded claims, merges any explicit claims, and writes a
/// schema-stamped, session-attributed `task_proof` event through the hardened
/// run append seam. `run` is both the logical session id stamped on the event
/// and the run directory it lands in.
pub fn record_claim(
    paths: &MaestroPaths,
    run: &str,
    task_id: &str,
    message: Option<String>,
    payload: Option<String>,
    explicit_claims: Vec<String>,
) -> Result<()> {
    let mut claims = task_claims(paths, task_id);
    claims.extend(explicit_claims);
    let claims = dedupe_claims(claims);

    let mut event = json!({
        "event": "task_proof",
        "schema_version": EVENT_SCHEMA_VERSION,
        "session_id": run,
        "task_id": task_id,
        "ts": utc_now_timestamp(),
    });
    if let Some(message) = message.or_else(|| payload.clone()) {
        event["message"] = Value::String(message);
    }
    if let Some(payload) = payload {
        event["payload"] = parse_payload(payload);
    }
    if !claims.is_empty() {
        event["claims"] = Value::Array(claims.into_iter().map(Value::String).collect());
    }
    run::append_manual_event(paths, run, &event)
}

fn task_claims(paths: &MaestroPaths, task_id: &str) -> Vec<String> {
    let Ok(task) = task::load_task_record(&paths.tasks_dir(), task_id) else {
        return Vec::new();
    };
    task.state_history
        .iter()
        .flat_map(|entry| entry.claims.iter())
        .map(|claim| claim.trim())
        .filter(|claim| !claim.is_empty())
        .map(str::to_string)
        .collect()
}

fn dedupe_claims(claims: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    claims
        .into_iter()
        .filter(|claim| seen.insert(claim.clone()))
        .collect()
}

fn parse_payload(payload: String) -> Value {
    serde_json::from_str(&payload).unwrap_or(Value::String(payload))
}
