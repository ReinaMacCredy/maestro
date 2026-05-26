use anyhow::Result;

use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::interfaces::cli::{WatchArgs, WatchCommand};
use crate::interfaces::tui::task_list_watch;
use crate::task::doctor::load_task_records;

/// Execute `maestro watch`.
pub fn run(args: WatchArgs) -> Result<()> {
    match args.command {
        WatchCommand::Snapshot => snapshot(),
    }
}

fn snapshot() -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let tasks = load_task_records(&paths.tasks_dir())?;
    print!("{}", task_list_watch::render_snapshot(&paths, &tasks)?);
    Ok(())
}
