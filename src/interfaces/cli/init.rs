use anyhow::Result;

use crate::interfaces::cli::InitArgs;
use crate::operations::init::{self, InitOptions, InitOutcome};

/// Execute `maestro init`.
pub fn run(args: InitArgs) -> Result<()> {
    let outcome = init::run(&InitOptions {
        dry_run: args.dry_run,
        merge: args.merge,
        force: args.force,
    })?;

    if let InitOutcome::DryRun(plan) = outcome {
        print!("{}", init::render_dry_run(&plan));
    }

    Ok(())
}
