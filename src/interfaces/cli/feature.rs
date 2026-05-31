use anyhow::{bail, Result};

use crate::domain::feature::{self, ContractAdditions, ContractEdits};
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::interfaces::cli::{FeatureArgs, FeatureCommand};

/// Execute `maestro feature`.
pub fn run(args: FeatureArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        FeatureCommand::New { title } => new_feature(&paths, &title),
        FeatureCommand::Set {
            id,
            acceptance,
            area,
            non_goal,
            question,
            description,
            request,
            input_type,
        } => set_feature(
            &paths,
            &id,
            ContractEdits {
                acceptance: opt_list(acceptance),
                affected_areas: opt_list(area),
                non_goals: opt_list(non_goal),
                open_questions: opt_list(question),
                description,
                raw_request: request,
                input_type,
            },
        ),
        FeatureCommand::Accept { id, dry_run } => print_note(feature::accept(&paths, &id, dry_run)?.note),
        FeatureCommand::Amend {
            id,
            add_acceptance,
            add_area,
            add_non_goal,
            add_question,
            reason,
        } => amend_feature(
            &paths,
            &id,
            ContractAdditions {
                acceptance: add_acceptance,
                affected_areas: add_area,
                non_goals: add_non_goal,
                open_questions: add_question,
            },
            &reason,
        ),
        FeatureCommand::Start { id } => print_note(feature::start(&paths, &id)?.note),
        FeatureCommand::Ship { id, dry_run } => print_note(feature::ship(&paths, &id, dry_run)?.note),
        FeatureCommand::Cancel { id, reason } => print_note(feature::cancel(&paths, &id, &reason)?.note),
        FeatureCommand::Show { id } => show_feature(&paths, &id),
        FeatureCommand::List { all } => list_features(&paths, all),
        FeatureCommand::Archive { id, shipped, dry_run } => {
            archive_features(&paths, id, shipped, dry_run)
        }
        FeatureCommand::Unarchive { id } => print_note(feature::unarchive_feature(&paths, &id)?),
    }
}

/// Dispatch `feature archive`: exactly one of a single id or `--shipped`.
fn archive_features(
    paths: &MaestroPaths,
    id: Option<String>,
    shipped: bool,
    dry_run: bool,
) -> Result<()> {
    match (id, shipped) {
        (Some(id), false) => print_note(feature::archive_feature(paths, &id, dry_run)?),
        (None, true) => archive_shipped(paths, dry_run),
        (Some(_), true) | (None, false) => bail!(
            "provide a feature id or --shipped, not both\n  maestro feature archive <id>\n  maestro feature archive --shipped"
        ),
    }
}

/// Bulk-archive every shipped feature (§5 L3). Collect-and-continue: one
/// feature's failure never aborts the sweep; the summary exits non-zero iff any
/// failed, so a re-run safely retries (archived features no-op, failures retry).
fn archive_shipped(paths: &MaestroPaths, dry_run: bool) -> Result<()> {
    let shipped: Vec<String> = feature::list(paths)?
        .into_iter()
        .filter(|view| view.status == feature::FeatureStatus::Shipped)
        .map(|view| view.id)
        .collect();

    if shipped.is_empty() {
        println!("no shipped features to archive");
        return Ok(());
    }

    let mut failures = Vec::new();
    for id in &shipped {
        match feature::archive_feature(paths, id, dry_run) {
            Ok(note) => println!("{note}"),
            Err(err) => failures.push(format!("{id}: {err:#}")),
        }
    }

    let verb = if dry_run { "would archive" } else { "archived" };
    println!("# {verb} {} of {} shipped feature(s)", shipped.len() - failures.len(), shipped.len());

    if !failures.is_empty() {
        bail!(
            "{} shipped feature(s) failed to archive (re-run to retry):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
    Ok(())
}

fn new_feature(paths: &MaestroPaths, title: &str) -> Result<()> {
    let id = feature::create(paths, title)?;
    println!("created feature {id} (proposed)");
    Ok(())
}

fn set_feature(paths: &MaestroPaths, id: &str, edits: ContractEdits) -> Result<()> {
    if edits.is_empty() {
        bail!(
            "no fields to set\n  maestro feature set {id} --acceptance \"<criterion>\" --area \"<surface>\"\n  flags: --acceptance --area --non-goal --question --description --request --type"
        );
    }
    let view = feature::set(paths, id, edits)?;
    println!(
        "set {id}; acceptance={}, areas={}, non_goals={}, questions={}",
        view.acceptance.len(),
        view.affected_areas.len(),
        view.non_goals.len(),
        view.open_questions.len()
    );
    Ok(())
}

fn amend_feature(
    paths: &MaestroPaths,
    id: &str,
    additions: ContractAdditions,
    reason: &str,
) -> Result<()> {
    if additions.is_empty() {
        bail!(
            "no values to amend\n  maestro feature amend {id} --add-acceptance \"<criterion>\" --reason \"<why>\"\n  add-flags: --add-acceptance --add-area --add-non-goal --add-question"
        );
    }
    print_note(feature::amend(paths, id, additions, reason)?.note)
}

fn show_feature(paths: &MaestroPaths, id: &str) -> Result<()> {
    // L6b: reads cross the boundary — fall through to the archive so a
    // historical reference to an archived feature still renders.
    let view = match feature::show(paths, id) {
        Ok(view) => view,
        Err(live_err) => feature::show_archived(paths, id).map_err(|_| live_err)?,
    };

    println!("id: {}", view.id);
    println!("title: {}", view.title);
    println!("status: {}", feature::status_label(&view.status));
    println!("tasks_total: {}", view.counts.total);
    println!("tasks_verified: {}", view.counts.verified);
    println!("created_at: {}", view.created_at);
    println!("updated_at: {}", view.updated_at);
    if let Some(description) = view.description.as_deref() {
        println!("description: {description}");
    }
    if let Some(request) = view.raw_request.as_deref() {
        println!("raw_request: {request}");
    }
    if let Some(input_type) = view.input_type.as_deref() {
        println!("input_type: {input_type}");
    }
    print_list("acceptance", &view.acceptance);
    print_list("affected_areas", &view.affected_areas);
    print_list("non_goals", &view.non_goals);
    print_list("open_questions", &view.open_questions);

    Ok(())
}

fn list_features(paths: &MaestroPaths, all: bool) -> Result<()> {
    let views = feature::list(paths)?;
    let hidden = views.iter().filter(|view| view.status.is_terminal()).count();
    let shown: Vec<_> = if all {
        // L6b: --all also reads the archive sibling tree.
        let mut all_views = views;
        all_views.extend(feature::list_archived(paths)?);
        all_views
    } else {
        views
            .into_iter()
            .filter(|view| !view.status.is_terminal())
            .collect()
    };

    if shown.is_empty() {
        println!("no features found");
    } else {
        for view in &shown {
            println!(
                "{}\t{}\ttasks={}\tverified={}\t{}",
                view.id,
                feature::status_label(&view.status),
                view.counts.total,
                view.counts.verified,
                view.title
            );
        }
    }

    if !all && hidden > 0 {
        println!("# {hidden} terminal feature(s) hidden; use --all to include");
    }

    Ok(())
}

fn print_note(note: String) -> Result<()> {
    println!("{note}");
    Ok(())
}

fn print_list(label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    println!("{label}:");
    for item in items {
        println!("  - {item}");
    }
}

fn opt_list(values: Vec<String>) -> Option<Vec<String>> {
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}
