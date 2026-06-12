use anyhow::{Result, bail};

use crate::domain::decisions;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::table;
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
            lock,
            decision,
            rejected,
            preview,
            supersedes,
            id_only,
        } => {
            if lock {
                let decision = decision.expect("clap invariant: --lock requires --decision");
                new_locked_decision(
                    &paths,
                    &title,
                    context.as_deref(),
                    feature.as_deref(),
                    decisions::LockInputs {
                        decision: &decision,
                        rejected: &rejected,
                        preview: preview.as_deref(),
                        supersedes: &supersedes,
                    },
                    id_only,
                )
            } else {
                new_decision(
                    &paths,
                    &title,
                    context.as_deref(),
                    feature.as_deref(),
                    id_only,
                )
            }
        }
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
    id_only: bool,
) -> Result<()> {
    if title.trim().is_empty() {
        bail!("decision title cannot be empty; e.g. `maestro decision new \"Adopt X for Y\"`");
    }
    let report = decisions::create_open(paths, title, context, feature)?;
    if id_only {
        println!("{}", report.record.id);
        return Ok(());
    }
    println!("opened {} (status: open)", report.record.id);
    println!("store: {}", report.path.display());
    println!("{}", decisions::query::render_record(&report.record));
    Ok(())
}

/// One-shot open+lock for a pre-decided fork. Unlike the standalone lock,
/// `--rejected` stays optional: a fork the user already settled often has no
/// enumerated alternatives worth recording.
fn new_locked_decision(
    paths: &MaestroPaths,
    title: &str,
    context: Option<&str>,
    feature: Option<&str>,
    inputs: decisions::LockInputs<'_>,
    id_only: bool,
) -> Result<()> {
    if title.trim().is_empty() {
        bail!("decision title cannot be empty; e.g. `maestro decision new \"Adopt X for Y\"`");
    }
    let report = decisions::create_locked(paths, title, context, feature, inputs)?;
    if id_only {
        println!("{}", report.record.id);
        return Ok(());
    }
    print_lock_report(&report);
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
    print_lock_report(&report);
    Ok(())
}

fn print_lock_report(report: &decisions::DecisionLockReport) {
    println!("locked {}", report.record.id);
    println!("store: {}", report.path.display());
    println!("{}", decisions::query::render_record(&report.record));
    if let Some(line) = &report.note_line {
        println!("note:");
        println!("  {line}");
    }
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

    let rows: Vec<Vec<String>> = entries
        .iter()
        .map(|entry| {
            vec![
                entry.id.clone(),
                entry.status.clone(),
                home(&entry.source),
                entry.title.clone(),
            ]
        })
        .collect();
    print!(
        "{}",
        table::render_table(&["ID", "STATUS", "HOME", "TITLE"], &rows)
    );

    Ok(())
}

fn home(source: &decisions::DecisionSource) -> String {
    match source {
        decisions::DecisionSource::Global => "global".to_string(),
        decisions::DecisionSource::Feature { feature_id } => format!("feature:{feature_id}"),
        decisions::DecisionSource::Legacy => "legacy-md".to_string(),
    }
}
