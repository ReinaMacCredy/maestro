use anyhow::Result;

use crate::interfaces::cli::InitArgs;
use crate::operations::init::{self, InitOptions, InitOutcome};

/// Execute `maestro init`.
pub fn run(args: InitArgs) -> Result<()> {
    // `--yes` is the non-interactive idempotent default for scripted/agent runs:
    // with no explicit mode it behaves like `--merge` (keep existing files, create
    // only what is missing, exit 0 on re-run). An explicit `--force` still wins so
    // a deliberate refresh is honored rather than silently downgraded to merge.
    let outcome = init::run(&InitOptions {
        dry_run: args.dry_run,
        merge: args.merge || (args.yes && !args.force),
        force: args.force,
    })?;

    match outcome {
        InitOutcome::DryRun { plan, preview } => {
            print!("{}", init::render_dry_run(&plan, &preview));
        }
        InitOutcome::Applied { behind } if behind > 0 => {
            let noun = if behind == 1 { "folder" } else { "folders" };
            println!("{behind} {noun} behind this maestro version; run `maestro sync` to resync");
        }
        InitOutcome::Applied { .. } => {}
    }

    Ok(())
}
