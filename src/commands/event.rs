use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::commands::{EventArgs, EventCommand};
use crate::core::fs::ensure_dir;
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::core::time::utc_now_timestamp;
use crate::task::doctor::load_task_records;
use crate::task::lookup::load_task_with_snapshot;
use crate::task::template::TaskState;

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
    let task_id = resolve_optional_task_id(&paths, task_id)?;
    let run_dir = paths.runs_dir().join(run);
    ensure_dir(&run_dir)?;
    let path = run_dir.join("events.jsonl");
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
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    writeln!(file, "{event_value}")
        .with_context(|| format!("failed to write {}", path.display()))?;
    println!("created event {}", path.display());
    Ok(())
}

fn task_claims(paths: &MaestroPaths, task_id: &str) -> Vec<String> {
    let Ok((task, _, _)) = load_task_with_snapshot(&paths.tasks_dir(), task_id) else {
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

fn resolve_optional_task_id(paths: &MaestroPaths, task_id: Option<String>) -> Result<String> {
    if let Some(task_id) = task_id {
        return Ok(task_id);
    }
    if let Ok(task_id) = std::env::var("MAESTRO_CURRENT_TASK") {
        if !task_id.trim().is_empty() {
            return Ok(task_id);
        }
    }
    let tasks = load_task_records(&paths.tasks_dir())?;
    let open_tasks = tasks
        .iter()
        .filter(|task| task.state == TaskState::NeedsVerification)
        .collect::<Vec<_>>();
    if open_tasks.len() == 1 {
        return Ok(open_tasks[0].id.clone());
    }
    if tasks.len() == 1 {
        return Ok(tasks[0].id.clone());
    }
    anyhow::bail!("--task-id is required or set MAESTRO_CURRENT_TASK");
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
