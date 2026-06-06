use std::fs;

use anyhow::{Context, Result, bail};

use crate::domain::decisions;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::interfaces::cli::{DecisionArgs, DecisionCommand};

/// Execute `maestro decision`.
pub fn run(args: DecisionArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        DecisionCommand::New {
            title,
            context,
            decision,
            alternative,
            consequence,
            feature,
        } => new_decision(
            &paths,
            &title,
            context.as_deref(),
            decision.as_deref(),
            &alternative,
            &consequence,
            feature.as_deref(),
        ),
        DecisionCommand::Show { id } => show_decision(&paths, &id),
        DecisionCommand::List => list_decisions(&paths),
    }
}

fn new_decision(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    decision: Option<&str>,
    alternatives: &[String],
    consequences: &[String],
    feature: Option<&str>,
) -> Result<()> {
    // An empty title slugifies to a malformed `decision-NNN-.md`; reject it at
    // the boundary with a remedy rather than writing the husk (T2).
    if title.trim().is_empty() {
        bail!("decision title cannot be empty; e.g. `maestro decision new \"Adopt X for Y\"`");
    }
    let has_sections = context.is_some()
        || decision.is_some()
        || !alternatives.is_empty()
        || !consequences.is_empty()
        || feature.is_some();
    let number = if has_sections {
        decisions::create_complete(
            paths,
            title,
            context,
            decision,
            alternatives,
            consequences,
            feature,
        )?
    } else {
        decisions::create(paths, title)?
    };
    let file_name = decisions::decision_file_name(number, title);
    println!("created decision decision-{number:03}");
    if has_sections {
        println!("complete: .maestro/decisions/{file_name}");
    } else {
        println!(
            "# template at .maestro/decisions/{file_name} — fill in Context / Decision / Alternatives"
        );
    }
    Ok(())
}

fn show_decision(paths: &MaestroPaths, id: &str) -> Result<()> {
    let path = decisions::resolve_decision_path(&paths.decisions_dir(), id)?;
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read decision file {}", path.display()))?;
    print!("{contents}");
    Ok(())
}

fn list_decisions(paths: &MaestroPaths) -> Result<()> {
    let entries = decisions::decision_entries(&paths.decisions_dir())?;
    if entries.is_empty() {
        println!("no decisions found");
        return Ok(());
    }

    println!("ID\tFILE\tTITLE");
    for entry in entries {
        println!(
            "{}\t{}\t{}",
            decisions::decision_display_id(&entry.file_name),
            entry.file_name,
            decisions::decision_title(&entry.path)?
        );
    }

    Ok(())
}
