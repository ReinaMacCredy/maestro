use std::collections::BTreeSet;

use anyhow::Result;
use serde_json::{json, Value};

use crate::domain::run;
use crate::domain::task;
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::task_id::resolve_optional_task_id;
use crate::interfaces::cli::{EventArgs, EventCommand};

/// Execute `maestro event`.
pub fn run(args: EventArgs) -> Result<()> {
    match args.command {
        EventCommand::Create {
            task_id,
            message,
            payload,
            claim,
            event,
            run,
        } => create_event(task_id, message, payload, claim, &event, &run),
    }
}

fn create_event(
    task_id: Option<String>,
    message: Option<String>,
    payload: Option<String>,
    explicit_claims: Vec<String>,
    event: &str,
    run: &str,
) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let task_id = resolve_optional_task_id(
        &paths,
        task_id,
        "--task-id is required or set MAESTRO_CURRENT_TASK",
    )?;
    let mut claims = task_claims(&paths, &task_id);
    claims.extend(explicit_claims);
    let claims = dedupe_claims(claims);
    let mut event_value = json!({
        "event": event,
        "task_id": task_id,
        "ts": utc_now_timestamp(),
    });
    if let Some(message) = message.or_else(|| payload.clone()) {
        event_value["message"] = Value::String(message);
    }
    if let Some(payload) = payload {
        event_value["payload"] = parse_payload(payload);
    }
    if !claims.is_empty() {
        event_value["claims"] = Value::Array(claims.into_iter().map(Value::String).collect());
    }
    run::append_manual_event(&paths, run, &event_value)?;
    println!("created {event} event for run {run}");
    Ok(())
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
