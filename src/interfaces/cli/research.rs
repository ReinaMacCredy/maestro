use anyhow::Result;

use crate::domain::research;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{ResearchArgs, ResearchCommand};

pub fn run(args: ResearchArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    match args.command {
        ResearchCommand::Check {
            card_id,
            intended_project,
            json,
        } => check(&paths, &card_id, intended_project.as_deref(), json),
    }
}

fn check(
    paths: &MaestroPaths,
    card_id: &str,
    intended_project: Option<&str>,
    json: bool,
) -> Result<()> {
    let report = research::check(paths, card_id, intended_project)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!("research: {}", report.status);
    for reason in &report.reasons {
        println!("reason: {reason}");
    }
    println!("next: {}", report.next);
    Ok(())
}
