use anyhow::Result;

use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{WatchArgs, WatchCommand};
use crate::interfaces::tui::task_list_watch;

/// Execute `maestro watch`. Bare runs the live board loop (reusing the watch
/// poll loop, now sourced from the card graph); `snapshot` renders one frame.
/// Both take an optional feature-id positional that focuses on one feature.
pub fn run(args: WatchArgs) -> Result<()> {
    let paths = MaestroPaths::new(discover_repo_root()?);
    match args.command {
        Some(WatchCommand::Snapshot { id }) => {
            print!("{}", task_list_watch::render_board(&paths, id.as_deref())?);
            Ok(())
        }
        None => task_list_watch::run_board(&paths, args.id.as_deref(), args.interval.unwrap_or(2)),
    }
}
