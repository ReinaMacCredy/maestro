use anyhow::{Result, bail};

use crate::domain::proof;
use crate::domain::task;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
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
        } => create_event(task_id, message, payload, claim, &run),
    }
}

fn create_event(
    task_id: Option<String>,
    message: Option<String>,
    payload: Option<String>,
    explicit_claims: Vec<String>,
    run: &str,
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
    proof::record_claim(&paths, run, &task_id, message, payload, explicit_claims)?;
    println!("created task_proof event for run {run}");
    Ok(())
}
