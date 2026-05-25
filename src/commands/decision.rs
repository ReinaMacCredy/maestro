use std::fs;

use anyhow::{bail, Context, Result};

use crate::commands::{DecisionArgs, DecisionCommand};
use crate::core::fs::ensure_dir;
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::core::safe_write::write_string_atomic;
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
    let path = resolve_decision_path(paths, id)?;
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

    for file_name in entries {
        let id = file_name
            .split_once('-')
            .map(|_| file_name.split('.').next().unwrap_or(file_name.as_str()))
            .unwrap_or(file_name.as_str());
        println!("{id}\t{file_name}");
    }

    Ok(())
}

fn next_decision_number(decisions_dir: &std::path::Path) -> Result<u32> {
    let mut max_number = 0_u32;
    for file_name in decision_entries(decisions_dir)? {
        if let Some(number) = parse_decision_number(&file_name) {
            max_number = max_number.max(number);
        }
    }
    Ok(max_number + 1)
}

fn resolve_decision_path(paths: &MaestroPaths, id: &str) -> Result<std::path::PathBuf> {
    let decisions_dir = paths.decisions_dir();

    if id.ends_with(".md") {
        let path = decisions_dir.join(id);
        if path.is_file() {
            return Ok(path);
        }
    }

    let direct = decisions_dir.join(format!("{id}.md"));
    if direct.is_file() {
        return Ok(direct);
    }

    let prefix = format!("{id}-");
    let mut matches = decision_entries(&decisions_dir)?
        .into_iter()
        .filter(|entry| entry.starts_with(&prefix))
        .collect::<Vec<_>>();
    matches.sort();

    if matches.len() == 1 {
        return Ok(decisions_dir.join(&matches[0]));
    }

    if matches.is_empty() {
        bail!("decision {id} not found");
    }
    bail!("decision {id} is ambiguous");
}

fn decision_entries(decisions_dir: &std::path::Path) -> Result<Vec<String>> {
    if !decisions_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(decisions_dir)
        .with_context(|| format!("failed to read {}", decisions_dir.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to read entry in {}", decisions_dir.display()))?;
        if !entry.path().is_file() {
            continue;
        }
        let Some(file_name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if file_name.starts_with("decision-") && file_name.ends_with(".md") {
            entries.push(file_name);
        }
    }
    entries.sort();
    Ok(entries)
}

fn parse_decision_number(file_name: &str) -> Option<u32> {
    let number = file_name.strip_prefix("decision-")?.split('-').next()?;
    number.parse::<u32>().ok()
}
