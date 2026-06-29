use anyhow::Result;

use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::SyncArgs;
use crate::operations::harness;
use crate::operations::sync::{self, SyncOptions};

/// Execute `maestro sync`.
pub fn run(args: SyncArgs) -> Result<()> {
    let outcome = sync::run(&SyncOptions {
        dry_run: args.dry_run,
        global_skills: args.global_skills,
    })?;
    print!("{}", sync::render(&outcome));
    if !args.global_skills {
        let paths = MaestroPaths::new(discover_repo_root()?);
        if paths.maestro_dir().is_dir() {
            println!(
                "{}",
                harness::complete_readout(&paths)?.runtime_summary_line()
            );
        }
    }
    Ok(())
}
