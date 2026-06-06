use anyhow::{Result, bail};

use serde_json::json;

use crate::domain::task;
use crate::domain::{proof, run};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::schema::EVENT_SCHEMA_VERSION;
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
            run,
        } => create_event(task_id, message, payload, claim, run),
        EventCommand::Intervention { note, topic, run } => intervention_event(note, topic, run),
    }
}

fn intervention_event(note: String, topic: Option<String>, run: Option<String>) -> Result<()> {
    if note.trim().is_empty() {
        bail!("--note must not be empty");
    }
    if topic
        .as_deref()
        .is_some_and(|topic| topic.trim().is_empty())
    {
        bail!("--topic must not be empty when supplied");
    }
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let run_id = run.unwrap_or_else(super::cli_run_id);
    let mut event = json!({
        "schema_version": EVENT_SCHEMA_VERSION,
        "ts": utc_now_timestamp(),
        "event_type": "intervention",
        "session_id": run_id,
        "note": note.trim(),
    });
    if let Some(topic) = topic {
        event
            .as_object_mut()
            .expect("invariant: event is an object")
            .insert("topic".to_string(), json!(topic.trim()));
    }
    run::append_manual_event(&paths, &run_id, &event)?;
    println!("recorded intervention event for run {run_id}");
    Ok(())
}

fn create_event(
    task_id: Option<String>,
    message: Option<String>,
    payload: Option<String>,
    explicit_claims: Vec<String>,
    run: Option<String>,
) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let task_id = resolve_optional_task_id(
        &paths,
        task_id,
        "--task-id is required or set MAESTRO_CURRENT_TASK",
    )?;
    if explicit_claims.iter().any(|claim| claim.trim().is_empty()) {
        bail!(
            "`--claim` must not be empty; pass the proof to verify against, e.g. --claim \"cargo test passes\""
        );
    }
    // A proof event must point at a real task; reject orphan refs so
    // `event create --task-id task-999` fails loudly instead of logging a
    // dangling event with exit 0 (T2).
    task::load_task_record(&paths.tasks_dir(), &task_id)?;
    let run = run.unwrap_or_else(super::cli_run_id);
    proof::record_claim(&paths, &run, &task_id, message, payload, explicit_claims)?;
    println!("created task_proof event for run {run}");
    Ok(())
}
