use std::env;

use anyhow::Result;

use crate::core::backup::backup_operation_timestamp;
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::update::{run_update, BinaryStatus, UpdateOptions, UpdateOutcome};

/// Execute `maestro update`.
pub fn run() -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let executable_path = env::current_exe()?;
    let backup_timestamp = backup_operation_timestamp()?;
    let outcome = run_update(&UpdateOptions {
        paths: &paths,
        executable_path: &executable_path,
        backup_timestamp: &backup_timestamp,
    })?;

    print_outcome(&outcome);

    Ok(())
}

fn print_outcome(outcome: &UpdateOutcome) {
    match &outcome.binary_status {
        BinaryStatus::Skipped { reason } => println!("binary update skipped: {reason}"),
        BinaryStatus::Replaced { path } => println!("binary updated: {}", path.display()),
    }

    if outcome.skill_backups.is_empty() {
        println!("bundled skills re-extracted");
    } else {
        println!("bundled skills re-extracted; edited skills backed up:");
        for backup in &outcome.skill_backups {
            println!("{} -> {}", backup.skill_name, backup.path.display());
        }
    }

    if !outcome.schema_mismatches.is_empty() {
        println!(
            "core harness/install schema mismatch detected; run `maestro migrate` before writing artifacts:"
        );
        for mismatch in &outcome.schema_mismatches {
            println!(
                "{} expected {} found {}",
                mismatch.path.display(),
                mismatch.expected,
                mismatch.found
            );
        }
    }
}
