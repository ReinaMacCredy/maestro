use anyhow::Result;

use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::operations;

pub fn run() -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let report = operations::migrate::run(&paths)?;
    println!("migrated maestro artifacts to v2");
    println!("tasks: {}", report.tasks);
    println!("features: {}", report.features);
    println!("removed: {}", report.removed);
    println!("pruned_backups: {}", report.pruned_backups);
    println!("pruned_runs: {}", report.pruned_runs);
    Ok(())
}
