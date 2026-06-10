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

pub fn run_card_fold() -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    let now = crate::foundation::core::time::utc_now_timestamp();
    let report = operations::card_migrate::run(&paths, &now)?;
    println!("folded legacy trees into the card store");
    println!("features: {}", report.features);
    println!("tasks: {}", report.tasks);
    println!("decisions: {}", report.decisions);
    println!("ideas: {}", report.ideas);
    println!("skipped: {}", report.skipped);
    match report.backup {
        Some(path) => println!("backup: {}", path.display()),
        None => println!("backup: none (.maestro had nothing to snapshot)"),
    }
    let containers = operations::container_migrate::run(&paths)?;
    println!("folded flat card dirs into containers");
    println!("decisions: {}", containers.decisions);
    println!("ideas: {}", containers.ideas);
    println!("tasks: {}", containers.tasks);
    println!("finished interrupted moves: {}", containers.finished);
    println!("next: maestro doctor");
    Ok(())
}
