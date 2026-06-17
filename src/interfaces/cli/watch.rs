use anyhow::Result;

use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{WatchArgs, WatchCommand};
use crate::interfaces::tui::task_list_watch;

/// Execute `maestro watch`.
pub fn run(args: WatchArgs) -> Result<()> {
    match args.command {
        WatchCommand::Snapshot => snapshot(),
    }
}

fn snapshot() -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    print!("{}", task_list_watch::render_board(&paths)?);
    Ok(())
}
