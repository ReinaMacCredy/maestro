use std::fs;

use anyhow::{Context, Result};

use crate::commands::{DecisionArgs, DecisionCommand};
use crate::core::fs::ensure_dir;
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::core::safe_write::write_string_atomic;
use crate::decisions::query::{
    decision_entries, decision_id, parse_decision_number, resolve_decision_path,
};
use crate::decisions::template::{decision_file_name, decision_markdown};

/// Execute `maestro decision`.
pub fn run(args: DecisionArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        DecisionCommand::New { title } => new_decision(&paths, &title),
        DecisionCommand::Show { id } => show_decision(&paths, &id),
        DecisionCommand::List => list_decisions(&paths),
    }
}

fn new_decision(paths: &MaestroPaths, title: &str) -> Result<()> {
    ensure_dir(paths.decisions_dir())?;
    let number = next_decision_number(&paths.decisions_dir())?;
    let file_name = decision_file_name(number, title);
    let path = paths.decisions_dir().join(&file_name);
    let contents = decision_markdown(number, title);
    write_string_atomic(&path, &contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    println!("created decision decision-{number:03}");
    Ok(())
}

fn show_decision(paths: &MaestroPaths, id: &str) -> Result<()> {
    let path = resolve_decision_path(&paths.decisions_dir(), id)?;
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read decision file {}", path.display()))?;
    print!("{contents}");
    Ok(())
}

fn list_decisions(paths: &MaestroPaths) -> Result<()> {
    let entries = decision_entries(&paths.decisions_dir())?;
    if entries.is_empty() {
        println!("no decisions found");
        return Ok(());
    }

    for entry in entries {
        println!("{}\t{}", decision_id(&entry.file_name), entry.file_name);
    }

    Ok(())
}

fn next_decision_number(decisions_dir: &std::path::Path) -> Result<u32> {
    let mut max_number = 0_u32;
    for entry in decision_entries(decisions_dir)? {
        if let Some(number) = parse_decision_number(&entry.file_name) {
            max_number = max_number.max(number);
        }
    }
    Ok(max_number + 1)
}
