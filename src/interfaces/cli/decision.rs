use anyhow::{Result, bail};

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
            feature,
        } => new_decision(&paths, &title, context.as_deref(), feature.as_deref()),
        DecisionCommand::Lock {
            id,
            decision,
            rejected,
            preview,
            supersedes,
        } => lock_decision(
            &paths,
            &id,
            &decision,
            &rejected,
            preview.as_deref(),
            &supersedes,
        ),
        DecisionCommand::Show { id } => show_decision(&paths, &id),
        DecisionCommand::List => list_decisions(&paths),
    }
}

fn new_decision(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
) -> Result<()> {
    if title.trim().is_empty() {
        bail!("decision title cannot be empty; e.g. `maestro decision new \"Adopt X for Y\"`");
    }
    let report = decisions::create_open(paths, title, context, feature)?;
    println!("opened {} (status: open)", report.record.id);
    println!("store: {}", report.path.display());
    println!("{}", decisions::query::render_record(&report.record));
    Ok(())
}

fn lock_decision(
    paths: &MaestroPaths,
    id: &str,
    decision: &str,
    rejected: &[String],
    preview: Option<&str>,
    supersedes: &[String],
) -> Result<()> {
    if rejected.is_empty() {
        bail!("decision lock requires at least one --rejected \"<option: why>\"");
    }
    let report = decisions::lock(paths, id, decision, rejected, preview, supersedes)?;
    println!("locked {}", report.record.id);
    println!("store: {}", report.path.display());
    println!("{}", decisions::query::render_record(&report.record));
    if let Some(line) = report.note_line {
        println!("note:");
        println!("  {line}");
    }
    Ok(())
}

fn show_decision(paths: &MaestroPaths, id: &str) -> Result<()> {
    match decisions::show(paths, id)? {
        decisions::DecisionContent::Structured { record, path, .. } => {
            println!("store: {}", path.display());
            print!("{}", decisions::query::render_record(&record));
        }
        decisions::DecisionContent::Legacy { contents, path, .. } => {
            println!("legacy: {}", path.display());
            print!("{contents}");
        }
    }
    Ok(())
}

fn list_decisions(paths: &MaestroPaths) -> Result<()> {
    let entries = decisions::list_tolerant(paths);
    if entries.is_empty() {
        println!("no decisions found");
        return Ok(());
    }

    println!("ID\tSTATUS\tHOME\tTITLE");
    for entry in entries {
        println!(
            "{}\t{}\t{}\t{}",
            entry.id,
            entry.status,
            home(&entry.source),
            entry.title
        );
    }

    Ok(())
}

fn home(source: &decisions::DecisionSource) -> String {
    match source {
        decisions::DecisionSource::Global => "global".to_string(),
        decisions::DecisionSource::Feature { feature_id } => format!("feature:{feature_id}"),
        decisions::DecisionSource::Legacy => "legacy-md".to_string(),
    }
}
