use anyhow::Result;

use crate::interfaces::cli::SyncArgs;
use crate::operations::sync::{self, SyncOptions};

/// Execute `maestro sync`.
pub fn run(args: SyncArgs) -> Result<()> {
    let outcome = sync::run(&SyncOptions {
        dry_run: args.dry_run,
        global_skills: args.global_skills,
    })?;
    print!("{}", sync::render(&outcome));
    Ok(())
}
