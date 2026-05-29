use anyhow::Result;

use crate::domain::proof;
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
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
    proof::record_claim(
        &paths,
        run,
        &task_id,
        event,
        message,
        payload,
        explicit_claims,
    )?;
    println!("created {event} event for run {run}");
    Ok(())
}
