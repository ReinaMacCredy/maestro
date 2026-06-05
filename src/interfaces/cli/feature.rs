use anyhow::{Result, bail};

use crate::domain::feature::{self, ContractAdditions, ContractEdits, FeatureStatus};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::render_timestamp;
use crate::interfaces::cli::{FeatureArgs, FeatureCommand};
use crate::operations::feature_prepare;

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
        FeatureCommand::Accept { id, dry_run } => {
            let report = feature::accept(&paths, &id, dry_run)?;
            print_note(report.note)?;
            if report.changed && report.status == FeatureStatus::Ready {
                println!("next: maestro feature prepare {} --draft", report.id);
            }
            Ok(())
        }
        FeatureCommand::Prepare { id, from, draft } => {
            prepare_feature(&paths, &id, from.as_deref(), draft)
        }
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
        FeatureCommand::Ship {
            id,
            outcome,
            dry_run,
        } => ship_feature(&paths, &id, outcome, dry_run),
        FeatureCommand::Cancel {
            id,
            reason,
            dry_run,
        } => cancel_feature(&paths, &id, &reason, dry_run),
        FeatureCommand::Show { id } => show_feature(&paths, &id),
        FeatureCommand::List { all } => list_features(&paths, all),
        FeatureCommand::Archive {
            id,
            shipped,
            dry_run,
        } => archive_features(&paths, id, shipped, dry_run),
        FeatureCommand::Unarchive { id } => match feature::unarchive_feature(&paths, &id) {
            Ok(note) => {
                print_feature_unarchive_note(&id, &note);
                Ok(())
            }
            Err(error) => bail!(
                "{}",
                feature_unarchive_error_message(&id, &error.to_string())
            ),
        },
    }
}

fn prepare_feature(
    paths: &MaestroPaths,
    id: &str,
    plan_file: Option<&std::path::Path>,
    draft: bool,
) -> Result<()> {
    match (plan_file, draft) {
        (Some(_), true) => bail!("use either --from <plan-file> or --draft, not both"),
        (None, false) => bail!(
            "feature prepare requires --from <plan-file> or --draft\n  maestro feature prepare {id} --draft\n  maestro feature prepare {id} --from <plan-file>"
        ),
        (None, true) => {
            let report = feature_prepare::write_draft(paths, id)?;
            if report.written {
                println!("wrote {}", report.path.display());
            } else {
                println!("draft exists: {}", report.path.display());
            }
            println!("review and run:");
            println!(
                "  maestro feature prepare {id} --from {}",
                report.path.display()
            );
            Ok(())
        }
        (Some(plan_file), false) => {
            let actor = super::actor();
            let report = feature_prepare::prepare_from_file(paths, id, plan_file, &actor)?;
            println!("prepared {} task(s)", report.task_count);
            if report.started {
                println!("started {} -> in_progress", report.feature_id);
            } else if report.remained_ready {
                println!("feature remains ready");
            }
            println!("prepared:");
            for task in &report.prepared {
                let state = if task.blocked {
                    "ready / blocked"
                } else {
                    "ready"
                };
                println!("  {} {:<15} {}", task.id, state, task.title);
            }
            if !report.blockers.is_empty() {
                println!("blockers:");
                for blocker in &report.blockers {
                    println!(
                        "  {} {} {}",
                        blocker.task_id, blocker.blocker_id, blocker.reason
                    );
                }
            }
            if report.ready_count > 0 {
                println!("next: maestro task claim --next");
            } else {
                println!(
                    "next: maestro task list --feature {} --blocked",
                    report.feature_id
                );
            }
            Ok(())
        }
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
        (Some(id), false) => match feature::archive_feature(paths, &id, dry_run) {
            Ok(note) => {
                print_feature_archive_note(&id, &note, dry_run);
                Ok(())
            }
            Err(error) => bail!("{}", feature_archive_error_message(&id, &error.to_string())),
        },
        (None, true) => archive_shipped(paths, dry_run),
        (Some(_), true) => bail!(
            "provide a feature id or --shipped, not both\n  maestro feature archive <id>\n  maestro feature archive --shipped"
        ),
        (None, false) => bail!(
            "provide a feature id or --shipped\n  maestro feature archive <id>\n  maestro feature archive --shipped"
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
    let mut archived = 0usize;
    let mut child_tasks = 0usize;
    let mut skipped = 0usize;
    for id in &shipped {
        match feature::archive_feature(paths, id, dry_run) {
            Ok(note) => {
                archived += 1;
                child_tasks += feature_child_count(&note);
                skipped += feature_skipped_count(&note);
            }
            Err(err) => failures.push(format!("{id}: {err:#}")),
        }
    }

    if dry_run {
        println!("dry-run: would archive shipped features");
    } else {
        println!("archived shipped features");
    }
    println!("archive summary:");
    let feature_verb = if dry_run { "would archive" } else { "archived" };
    let task_verb = if dry_run { "would archive" } else { "archived" };
    println!("  features: {archived} {feature_verb}");
    println!("  child tasks: {child_tasks} {task_verb}");
    println!("  skipped: {skipped}");
    println!("  failed: {}", failures.len());

    if !failures.is_empty() {
        println!("failed:");
        for failure in &failures {
            println!("  - {failure}");
        }
        println!("next:");
        println!("  retry: maestro feature archive --shipped");
        bail!(
            "{} shipped feature(s) failed to archive (re-run to retry):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
    if dry_run {
        println!("writes: none");
        println!("run: maestro feature archive --shipped");
    } else {
        println!("next: maestro status");
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
        "set {id} (replace-per-field); acceptance={}, areas={}, non_goals={}, questions={}",
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
    if reason.trim().is_empty() {
        bail!("`--reason` must not be empty; record why the contract is growing (it is audited)");
    }
    if additions.is_empty() {
        bail!(
            "no values to amend\n  maestro feature amend {id} --add-acceptance \"<criterion>\" --reason \"<why>\"\n  add-flags: --add-acceptance --add-area --add-non-goal --add-question"
        );
    }
    print_note(feature::amend(paths, id, additions, reason)?.note)
}

fn cancel_feature(paths: &MaestroPaths, id: &str, reason: &str, dry_run: bool) -> Result<()> {
    if reason.trim().is_empty() {
        bail!(
            "blocked: feature cancel needs an audited reason\nreason: --reason is empty\nrun: maestro feature cancel {id} --reason \"<why this feature is being cancelled>\""
        );
    }
    let report = match feature::cancel(paths, id, reason, dry_run) {
        Ok(report) => report,
        Err(error) => bail!(
            "{}",
            feature_cancel_error_message(id, reason, &error.to_string())
        ),
    };
    println!("{}", report.note);
    println!("cancel receipt:");
    println!("  feature: {}", report.id);
    println!("  abandoned_tasks: {}", report.abandoned.len());
    if dry_run {
        println!("writes: none");
        println!("retry: maestro feature cancel {id} --reason \"<reason>\"");
    } else if report.changed {
        println!("inspect: maestro feature show {}", report.id);
        println!("next: maestro status");
        println!("optional: maestro feature archive {}", report.id);
    } else {
        println!("inspect: maestro feature show {}", report.id);
        println!("next: maestro status");
    }
    Ok(())
}

fn ship_feature(
    paths: &MaestroPaths,
    id: &str,
    outcome: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let report = feature::ship(paths, id, outcome, dry_run)?;
    println!("{}", report.note);
    if dry_run {
        println!("ship preview:");
        println!("  feature: {}", report.id);
        println!("  target: shipped");
        println!("writes: none");
        println!(
            "retry: maestro feature ship {} --outcome \"<outcome>\"",
            report.id
        );
    } else if report.changed && report.status == FeatureStatus::Shipped {
        println!("ship receipt:");
        println!("  feature: {}", report.id);
        println!("  status: shipped");
        println!("inspect: maestro feature show {}", report.id);
        println!("next: maestro status");
        println!("optional: maestro feature archive {}", report.id);
    } else {
        println!("inspect: maestro feature show {}", report.id);
        println!("next: maestro status");
    }
    Ok(())
}

fn print_feature_archive_note(id: &str, note: &str, dry_run: bool) {
    println!("{note}");
    let child_tasks = feature_child_count(note);
    let skipped = feature_skipped_count(note);
    if dry_run {
        println!("archive receipt preview:");
        println!("  feature: {id}");
        println!("  child tasks: {child_tasks} would archive");
        println!("  skipped: {skipped}");
        println!("writes: none");
        println!("run: maestro feature archive {id}");
    } else if note.starts_with("already archived") {
        println!("inspect: maestro feature show {id}");
        println!("next: maestro status");
    } else {
        println!("archive receipt:");
        println!("  feature: {id}");
        println!("  child tasks: {child_tasks} archived");
        println!("  skipped: {skipped}");
        println!("inspect: maestro feature show {id}");
        println!("next: maestro status");
        println!("restore: maestro feature unarchive {id}");
    }
}

fn print_feature_unarchive_note(id: &str, note: &str) {
    println!("{note}");
    let child_tasks = count_before_marker(note, " child task(s)").unwrap_or(0);
    if note.starts_with("already live") {
        println!("inspect: maestro feature show {id}");
        println!("next: maestro status");
    } else {
        println!("restore receipt:");
        println!("  feature: {id}");
        println!("  child tasks: {child_tasks} restored");
        println!("inspect: maestro feature show {id}");
        println!("next: maestro status");
        println!("optional: maestro feature archive {id}");
    }
}

fn feature_archive_error_message(id: &str, error: &str) -> String {
    if error.contains("not terminal") {
        return format!(
            "cannot archive {id}:\n  not terminal\nnext:\n  ship: maestro feature ship {id} --outcome \"<outcome>\"\n  or cancel: maestro feature cancel {id} --reason \"<reason>\""
        );
    }
    if error.contains("live child task") {
        return format!(
            "cannot archive {id}:\n  live child tasks\nnext:\n  inspect: maestro feature show {id}\n  retry: maestro feature archive {id}"
        );
    }
    if error.contains("feature not found") {
        return format!(
            "cannot archive {id}:\n  feature not found\nnext:\n  list features: maestro feature list --all"
        );
    }
    if error.contains("archived copy already exists") {
        return format!(
            "cannot archive {id}:\n  archived copy already exists\ninspect:\n  live: maestro feature show {id}\n  archived: .maestro/archive/features/{id}\nnext:\n  resolve the duplicate archive, then retry: maestro feature archive {id}"
        );
    }
    error.to_string()
}

fn feature_unarchive_error_message(id: &str, error: &str) -> String {
    if error.contains("archived feature not found") {
        return format!(
            "cannot unarchive {id}:\n  archived feature not found\nnext:\n  list archived features: maestro feature list --all"
        );
    }
    if error.contains("live feature already occupies") {
        return format!(
            "cannot unarchive {id}:\n  live feature already exists\ninspect:\n  live: maestro feature show {id}\n  archived: .maestro/archive/features/{id}\nnext:\n  resolve the live feature conflict, then retry: maestro feature unarchive {id}"
        );
    }
    error.to_string()
}

fn feature_cancel_error_message(id: &str, reason: &str, error: &str) -> String {
    if error.contains("shipped features are terminal") || error.contains("terminal") {
        return format!(
            "blocked: cannot cancel {id}\nreason: shipped features are terminal\ninspect: maestro feature show {id}\nnext: maestro feature archive {id}"
        );
    }
    if error.contains("failed to abandon child task") {
        return format!(
            "blocked: cancel cascade failed\nfeature: {id}\nreason: {error}\ninspect: maestro feature show {id}\nretry: maestro feature cancel {id} --reason \"{reason}\""
        );
    }
    error.to_string()
}

fn feature_child_count(note: &str) -> usize {
    count_before_marker(note, " child task(s)").unwrap_or(0)
}

fn feature_skipped_count(note: &str) -> usize {
    count_before_marker(note, " live-referenced child task(s)").unwrap_or(0)
}

fn count_before_marker(note: &str, marker: &str) -> Option<usize> {
    let prefix = note.split(marker).next()?;
    prefix.split_whitespace().last()?.parse().ok()
}

fn show_feature(paths: &MaestroPaths, id: &str) -> Result<()> {
    // L6b: reads cross the boundary — fall through to the archive so a
    // historical reference to an archived feature still renders.
    let (view, archived) = match feature::show(paths, id) {
        Ok(view) => (view, false),
        Err(live_err) => (
            feature::show_archived(paths, id).map_err(|_| live_err)?,
            true,
        ),
    };

    println!("id: {}", view.id);
    println!("title: {}", view.title);
    println!("status: {}", feature::status_label(&view.status));
    if archived {
        println!("archived: true");
    }
    // An archived view counts only the archive tree; an L6c-skipped child stays live,
    // so disclose the live referrers it omits rather than reporting a misleading total.
    let live_unarchived = if archived {
        feature::query::count_tasks_for_feature(&paths.tasks_dir(), id)?.total
    } else {
        0
    };
    if live_unarchived > 0 {
        println!(
            "tasks_total: {} ({live_unarchived} live task(s) not archived)",
            view.counts.total
        );
    } else {
        println!("tasks_total: {}", view.counts.total);
    }
    println!("tasks_verified: {}", view.counts.verified);
    println!("created_at: {}", render_timestamp(&view.created_at));
    println!("updated_at: {}", render_timestamp(&view.updated_at));
    if let Some(description) = view.description.as_deref() {
        println!("description: {description}");
    }
    if let Some(request) = view.raw_request.as_deref() {
        println!("raw_request: {request}");
    }
    if let Some(input_type) = view.input_type.as_deref() {
        println!("input_type: {input_type}");
    }
    if let Some(outcome) = view.outcome.as_deref() {
        println!("outcome: {outcome}");
    }
    if let Some(cancel_reason) = view.cancel_reason.as_deref() {
        println!("cancel_reason: {cancel_reason}");
    }
    print_list("acceptance", &view.acceptance);
    print_list("affected_areas", &view.affected_areas);
    print_list("non_goals", &view.non_goals);
    print_list("open_questions", &view.open_questions);
    if let Some(notes) = view.notes.as_deref() {
        println!("notes:");
        for line in notes.lines() {
            println!("  {line}");
        }
    }

    Ok(())
}

fn list_features(paths: &MaestroPaths, all: bool) -> Result<()> {
    let views = feature::list(paths)?;
    let hidden = views
        .iter()
        .filter(|view| view.status.is_terminal())
        .count();
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
        println!("ID\tSTATE\tNEXT\tINSPECT\tTASKS\tVERIFIED\tTITLE");
        for view in &shown {
            let title = match view.outcome.as_deref() {
                Some(outcome) => format!("{} -- {outcome}", view.title),
                None => view.title.clone(),
            };
            println!(
                "{}\t{}\t{}\tmaestro feature show {}\t{}\t{}\t{}",
                view.id,
                feature::status_label(&view.status),
                feature_next_label(view),
                view.id,
                view.counts.total,
                view.counts.verified,
                title
            );
        }
    }

    if !all && hidden > 0 {
        println!("# {hidden} terminal feature(s) hidden; use --all to include");
    }

    Ok(())
}

fn feature_next_label(view: &feature::FeatureView) -> &'static str {
    match view.status {
        FeatureStatus::Proposed => "template: set_contract",
        FeatureStatus::Ready => "run: prepare_feature",
        FeatureStatus::InProgress
            if view.counts.total > 0 && view.counts.total == view.counts.verified =>
        {
            "template: ship_feature"
        }
        FeatureStatus::InProgress => "run: resolve_tasks",
        FeatureStatus::Shipped | FeatureStatus::Cancelled => "run: archive_feature",
    }
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
