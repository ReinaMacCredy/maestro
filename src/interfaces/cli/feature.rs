use anyhow::{Result, bail};

use crate::domain::decisions;
use crate::domain::feature::{
    self, ContractAdditions, ContractChangeCounts, ContractEdits, FeatureStatus,
};
use crate::domain::task;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::render_timestamp;
use crate::interfaces::cli::{FeatureArgs, FeatureCommand};
use crate::operations::feature_prepare;

/// Execute `maestro feature`.
pub fn run(args: FeatureArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        FeatureCommand::New {
            title,
            description,
            question,
        } => new_feature(&paths, &title, description, question),
        FeatureCommand::Set {
            id,
            acceptance,
            area,
            non_goal,
            question,
            clear_questions,
            add_acceptance,
            add_area,
            add_non_goal,
            add_question,
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
                open_questions: if !question.is_empty() {
                    opt_list(question)
                } else if clear_questions {
                    Some(Vec::new())
                } else {
                    None
                },
                description,
                raw_request: request,
                input_type,
                add_acceptance,
                add_affected_areas: add_area,
                add_non_goals: add_non_goal,
                add_open_questions: add_question,
            },
        ),
        FeatureCommand::Accept {
            id,
            qa,
            reason,
            dry_run,
        } => {
            let report = accept_feature(&paths, &id, qa, reason, dry_run)?;
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
        FeatureCommand::Start { id } => {
            let report = feature::start(&paths, &id)?;
            print_note(report.note)?;
            print_uncovered_acceptance_warning(&paths, &id)
        }
        FeatureCommand::Verify {
            id,
            prove,
            evidence,
            waive,
            reason,
        } => verify_feature(&paths, &id, prove, evidence, waive, reason),
        FeatureCommand::Note { id, text } => {
            let report = feature::note(&paths, &id, &text)?;
            if report.created {
                println!("noted {} (notes.md created)", report.id);
            } else {
                println!("noted {}", report.id);
            }
            println!("  {}", report.line);
            Ok(())
        }
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
        FeatureCommand::Spec { id } => show_feature_spec(&paths, &id),
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

fn accept_feature(
    paths: &MaestroPaths,
    id: &str,
    qa: Option<String>,
    reason: Option<String>,
    dry_run: bool,
) -> Result<feature::TransitionReport> {
    match (qa.as_deref(), reason.as_deref()) {
        (None, None) => feature::accept(paths, id, dry_run),
        (Some("none"), Some(reason)) if reason.trim().is_empty() => {
            bail!("--reason must not be empty with --qa none")
        }
        (Some("none"), Some(reason)) => feature::accept_with_qa_none(paths, id, reason, dry_run),
        (Some("none"), None) => bail!("--reason is required with --qa none"),
        (Some(other), _) => bail!("unsupported --qa value `{other}`; only `--qa none` is accepted"),
        (None, Some(_)) => bail!("--reason requires --qa none"),
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
            print_uncovered_acceptance_warning(paths, id)?;
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
            print_uncovered_acceptance_warning(paths, id)?;
            Ok(())
        }
    }
}

fn verify_feature(
    paths: &MaestroPaths,
    id: &str,
    prove: Option<String>,
    evidence: Option<String>,
    waive: Option<String>,
    reason: Option<String>,
) -> Result<()> {
    let update = match (prove, evidence, waive, reason) {
        (None, None, None, None) => None,
        (Some(ac_id), Some(evidence), None, None) => {
            Some(feature::FeatureProofUpdate::Explicit { ac_id, evidence })
        }
        (None, None, Some(ac_id), Some(reason)) => {
            Some(feature::FeatureProofUpdate::Waive { ac_id, reason })
        }
        (Some(_), None, None, None) => bail!("--prove requires --evidence"),
        (None, Some(_), None, None) => bail!("--evidence requires --prove"),
        (None, None, Some(_), None) => bail!("--waive requires --reason"),
        (None, None, None, Some(_)) => bail!("--reason requires --waive"),
        _ => bail!(
            "use bare `maestro feature verify {id}`, or exactly one of `--prove <ac-id> --evidence \"...\"` / `--waive <ac-id> --reason \"...\"`"
        ),
    };
    let report = feature::verify_feature(paths, id, update)?;
    if let Some(recorded) = report.recorded {
        println!("recorded {recorded}");
        println!("next: maestro feature verify {}", report.feature_id);
        return Ok(());
    }
    let Some(sweep) = report.sweep else {
        return Ok(());
    };
    println!(
        "checking contract ({} acceptance items):",
        sweep.items.len()
    );
    if !sweep.invalidated_by.is_empty() {
        println!("re-derived after: {}", sweep.invalidated_by.join("; "));
    }
    for (index, item) in sweep.items.iter().enumerate() {
        println!(
            "  [{}/{}] \"{}\"   {}",
            index + 1,
            sweep.items.len(),
            item.text,
            proof_label(&item.proof)
        );
    }
    let unresolved = sweep
        .items
        .iter()
        .filter(|item| matches!(item.proof, feature::AcceptanceProof::Missing))
        .collect::<Vec<_>>();
    if unresolved.is_empty() {
        println!("ok: every acceptance item has evidence");
    } else {
        println!(
            "blocked: {} acceptance item(s) have no fresh evidence: {}",
            unresolved.len(),
            unresolved
                .iter()
                .map(|item| item.ac_id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!(
            "fix: add task covers, record proof with `maestro feature verify {} --prove <ac-id> --evidence \"<observed>\"`, or waive with `--waive <ac-id> --reason \"<why>\"`",
            report.feature_id
        );
    }
    Ok(())
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
            Ok(report) => {
                print_feature_archive_note(&id, &report, dry_run);
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
    for id in &shipped {
        match feature::archive_feature(paths, id, dry_run) {
            Ok(report) => {
                archived += 1;
                child_tasks += report.child_tasks;
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
    // A terminal feature has no live children by construction, so nothing is
    // ever skipped; the line stays for receipt-shape stability.
    println!("  skipped: 0");
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

fn new_feature(
    paths: &MaestroPaths,
    title: &str,
    description: Option<String>,
    questions: Vec<String>,
) -> Result<()> {
    let id = feature::create(paths, title)?;
    let initialized = description.is_some() || !questions.is_empty();
    if initialized {
        feature::set(
            paths,
            &id,
            ContractEdits {
                description,
                open_questions: opt_list(questions),
                ..Default::default()
            },
        )?;
    }
    println!("created feature {id} (proposed)");
    println!("spec: .maestro/features/{id}/spec.md");
    println!("decisions: .maestro/features/{id}/decisions.yaml");
    if initialized {
        println!("initialized contract fields");
    }
    Ok(())
}

fn set_feature(paths: &MaestroPaths, id: &str, edits: ContractEdits) -> Result<()> {
    if edits.is_empty() {
        bail!(
            "no fields to set\n  maestro feature set {id} --acceptance \"<criterion>\" --area \"<surface>\"\n  maestro feature set {id} --add-acceptance \"<criterion>\"\n  flags: --acceptance --area --non-goal --question --clear-questions --add-acceptance --add-area --add-non-goal --add-question --description --request --type"
        );
    }
    let report = feature::set_with_report(paths, id, edits)?;
    print_set_report(id, &report);
    println!("next: qa-baseline skill -> .maestro/features/{id}/qa.md");
    println!("or: maestro feature accept {id} --qa none --reason \"<why no behavior>\"");
    println!("then: maestro feature accept {id}");
    if !report.view.open_questions.is_empty() {
        println!(
            "fork hint: open real forks with `maestro decision new \"<title>\" --feature {id} --context \"<why>\"`; keep --question for loose questions"
        );
    }
    Ok(())
}

fn print_set_report(id: &str, report: &feature::SetReport) {
    println!("set {id}");
    for line in change_lines("replaced", &report.replaced, &report.view) {
        println!("  {line}");
    }
    for line in change_lines("added", &report.added, &report.view) {
        println!("  {line}");
    }
    if report.replaced.is_empty() && report.added.is_empty() {
        println!("  no list values changed; scalar fields may have been refreshed");
    }
    println!(
        "  totals: acceptance={}, areas={}, non_goals={}, questions={}",
        report.view.acceptance.len(),
        report.view.affected_areas.len(),
        report.view.non_goals.len(),
        report.view.open_questions.len()
    );
}

fn change_lines(
    mode: &str,
    counts: &ContractChangeCounts,
    view: &feature::FeatureView,
) -> Vec<String> {
    let mut lines = Vec::new();
    push_count_line(
        &mut lines,
        mode,
        "acceptance",
        counts.acceptance,
        view.acceptance.len(),
    );
    push_count_line(
        &mut lines,
        mode,
        "areas",
        counts.affected_areas,
        view.affected_areas.len(),
    );
    push_count_line(
        &mut lines,
        mode,
        "non_goals",
        counts.non_goals,
        view.non_goals.len(),
    );
    push_count_line(
        &mut lines,
        mode,
        "questions",
        counts.open_questions,
        view.open_questions.len(),
    );
    if counts.description > 0 {
        lines.push("description replaced".to_string());
    }
    if counts.raw_request > 0 {
        lines.push("raw_request replaced".to_string());
    }
    if counts.input_type > 0 {
        lines.push("input_type replaced".to_string());
    }
    lines
}

fn push_count_line(lines: &mut Vec<String>, mode: &str, label: &str, changed: usize, total: usize) {
    if changed == 0 {
        return;
    }
    if mode == "added" {
        lines.push(format!("+{changed} {label} ({total} total)"));
    } else {
        lines.push(format!(
            "{label} replaced ({total}); other fields untouched"
        ));
    }
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
        if let Ok(view) = feature::show(paths, &report.id)
            && let Some(reason) = view.qa_none_reason.as_deref()
        {
            println!("  qa: none ({reason})");
        }
        let claims_only = claims_only_verified_count(paths, &report.id)?;
        if claims_only > 0 {
            println!("  verification: {claims_only} claims-only task(s)");
        }
        println!("inspect: maestro feature show {}", report.id);
        println!("next: maestro status");
        println!("optional: maestro feature archive {}", report.id);
    } else {
        println!("inspect: maestro feature show {}", report.id);
        println!("next: maestro status");
    }
    Ok(())
}

fn print_feature_archive_note(id: &str, report: &feature::FeatureArchiveReport, dry_run: bool) {
    println!("{}", report.note);
    if dry_run {
        println!("archive receipt preview:");
        println!("  feature: {id}");
        println!("  child tasks: {} would archive", report.child_tasks);
        println!("  skipped: 0");
        println!("writes: none");
        println!("run: maestro feature archive {id}");
    } else if report.note.starts_with("already archived") {
        println!("inspect: maestro feature show {id}");
        println!("next: maestro status");
    } else {
        println!("archive receipt:");
        println!("  feature: {id}");
        println!("  child tasks: {} archived", report.child_tasks);
        println!("  skipped: 0");
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
    if let Some(reason) = view.qa_none_reason.as_deref() {
        println!("qa: none ({reason})");
    }
    print_decision_summary(paths, &view.id)?;
    print_acceptance(
        paths,
        &view.id,
        &view.acceptance,
        view.acceptance_coverage.as_deref(),
        archived,
    )?;
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

fn print_decision_summary(paths: &MaestroPaths, id: &str) -> Result<()> {
    let records = decisions::decisions_for_feature(paths, id)?;
    let open = records
        .iter()
        .filter(|record| record.status == decisions::schema::DecisionStatus::Open)
        .count();
    let locked = records
        .iter()
        .filter(|record| record.status == decisions::schema::DecisionStatus::Locked)
        .count();
    let superseded = records
        .iter()
        .filter(|record| record.status == decisions::schema::DecisionStatus::Superseded)
        .count();
    println!(
        "decisions: {} (open: {open}, locked: {locked}, superseded: {superseded})",
        records.len()
    );
    Ok(())
}

fn show_feature_spec(paths: &MaestroPaths, id: &str) -> Result<()> {
    let view = match feature::show(paths, id) {
        Ok(view) => view,
        Err(error) => return show_unreadable_feature_spec(paths, id, error),
    };
    println!("status: {}", feature::status_label(&view.status));
    println!("feature: {}", view.id);
    println!();
    let spec_path = paths.features_dir().join(&view.id).join("spec.md");
    match std::fs::read_to_string(&spec_path) {
        Ok(spec) => print!("{}", spec.trim_end()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("# {}", view.title);
            println!();
            println!("(no spec.md found)");
        }
        Err(error) => bail!("failed to read {}: {error}", spec_path.display()),
    }
    println!();
    println!();
    println!("## Contract");
    if let Some(description) = view.description.as_deref() {
        println!("description: {description}");
    }
    print_plain_list("acceptance", &view.acceptance);
    print_plain_list("affected_areas", &view.affected_areas);
    print_plain_list("non_goals", &view.non_goals);
    print_plain_list("open_questions", &view.open_questions);

    let records = decisions::decisions_for_feature(paths, &view.id)?;
    println!();
    println!("## Decisions");
    let open = records
        .iter()
        .filter(|record| record.status == decisions::schema::DecisionStatus::Open)
        .collect::<Vec<_>>();
    if !open.is_empty() {
        println!("Open forks:");
        for record in &open {
            println!("- {}: {}", record.id, record.title);
            if let Some(context) = record.context.as_deref() {
                println!("  context: {context}");
            }
        }
    }
    let closed = records
        .iter()
        .filter(|record| record.status != decisions::schema::DecisionStatus::Open)
        .collect::<Vec<_>>();
    if closed.is_empty() && open.is_empty() {
        println!("- none");
    } else {
        for record in closed {
            println!(
                "- {} [{}]: {}",
                record.id,
                record.status.as_str(),
                record.title
            );
            if let Some(decision) = record.decision.as_deref() {
                println!("  decision: {decision}");
            }
            if let Some(preview) = record.preview.as_deref() {
                println!("  preview:");
                for line in preview.lines() {
                    println!("    {line}");
                }
            }
        }
    }

    if let Some(notes) = view.notes.as_deref() {
        println!();
        println!("## Recent notes");
        for line in notes
            .lines()
            .rev()
            .take(10)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            println!("{line}");
        }
    }
    Ok(())
}

fn show_unreadable_feature_spec(
    paths: &MaestroPaths,
    id: &str,
    error: anyhow::Error,
) -> Result<()> {
    let path = paths.features_dir().join(id).join("feature.yaml");
    println!("status: unreadable");
    println!("feature: {id}");
    println!("path: {}", path.display());
    println!("error: {error:#}");
    println!();
    println!("## Raw feature.yaml");
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            println!("```yaml");
            print!("{}", contents.trim_end());
            println!();
            println!("```");
        }
        Err(read_error) => println!("unavailable: {read_error}"),
    }
    println!();
    println!("## Decisions");
    match decisions::decisions_for_feature(paths, id) {
        Ok(records) if records.is_empty() => println!("- none"),
        Ok(records) => {
            for record in records {
                println!(
                    "- {} [{}]: {}",
                    record.id,
                    record.status.as_str(),
                    record.title
                );
            }
        }
        Err(error) => println!("- unreadable decisions.yaml: {error:#}"),
    }
    Ok(())
}

fn print_plain_list(label: &str, values: &[String]) {
    println!("{label}:");
    if values.is_empty() {
        println!("- none");
        return;
    }
    for value in values {
        println!("- {value}");
    }
}

fn print_acceptance(
    paths: &MaestroPaths,
    id: &str,
    fallback: &[String],
    coverage: Option<&[feature::AcceptanceCoverage]>,
    archived: bool,
) -> Result<()> {
    let loaded_coverage;
    let coverage = if let Some(coverage) = coverage {
        coverage
    } else {
        loaded_coverage = if archived {
            feature::acceptance_coverage_archived(paths, id)?
        } else {
            feature::acceptance_coverage(paths, id)?
        };
        &loaded_coverage
    };
    println!("acceptance:");
    if coverage.is_empty() {
        if fallback.is_empty() {
            println!("- none");
        }
        for (index, item) in fallback.iter().enumerate() {
            println!("- [{}] {}", feature::acceptance_id(index), item);
        }
        return Ok(());
    }
    for item in coverage {
        println!("- [{}] {}", item.ac_id, item.text);
        if !item.tasks.is_empty() {
            println!("  covers: {}", item.tasks.join(", "));
        }
    }
    Ok(())
}

fn print_uncovered_acceptance_warning(paths: &MaestroPaths, id: &str) -> Result<()> {
    let uncovered = feature::uncovered_acceptance(paths, id)?;
    if !uncovered.is_empty() {
        println!(
            "warning: {} acceptance item(s) have no covering task: {}",
            uncovered.len(),
            uncovered.join(", ")
        );
        println!("fix: maestro task set <task-id> --covers <ac-id>");
    }
    Ok(())
}

fn proof_label(proof: &feature::AcceptanceProof) -> String {
    match proof {
        feature::AcceptanceProof::Task(tasks) => format!("proof: {} OK", tasks.join(", ")),
        feature::AcceptanceProof::Qa(items) => format!("proof: {} OK", items.join(", ")),
        feature::AcceptanceProof::Explicit(evidence) => format!("proof: {evidence} OK"),
        feature::AcceptanceProof::Waived(reason) => format!("WAIVED: {reason}"),
        feature::AcceptanceProof::Missing => "NO FRESH EVIDENCE".to_string(),
    }
}

fn list_features(paths: &MaestroPaths, all: bool) -> Result<()> {
    let roster = feature::list_tolerant(paths);
    let unreadable = roster
        .iter()
        .filter_map(|entry| match entry {
            feature::FeatureRosterEntry::Loaded(_) => None,
            feature::FeatureRosterEntry::Unreadable { id, path: _, error } => {
                Some((id.clone(), error.clone()))
            }
        })
        .collect::<Vec<_>>();
    let views = roster
        .into_iter()
        .filter_map(|entry| match entry {
            feature::FeatureRosterEntry::Loaded(view) => Some(*view),
            feature::FeatureRosterEntry::Unreadable { .. } => None,
        })
        .collect::<Vec<_>>();
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

    if shown.is_empty() && unreadable.is_empty() {
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
        for (id, error) in &unreadable {
            println!(
                "{}\tunreadable\tfix: maestro migrate-v2\tmaestro feature spec {}\t0\t0\t{}",
                id, id, error
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

fn claims_only_verified_count(paths: &MaestroPaths, feature_id: &str) -> Result<usize> {
    Ok(task::load_task_records(&paths.tasks_dir())?
        .into_iter()
        .filter(|task| {
            task.feature_id.as_deref() == Some(feature_id)
                && task.state == task::TaskState::Verified
                && task.verification.claims_only
        })
        .count())
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
