use anyhow::{Result, bail};

use crate::domain::card;
use crate::domain::decisions;
use crate::domain::feature::{
    self, ContractAdditions, ContractChangeCounts, ContractEdits, FeatureStatus,
};
use crate::domain::task;
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::table;
use crate::foundation::core::time::render_timestamp;
use crate::interfaces::cli::{FeatureArgs, FeatureCommand, feature_next_label, recovery_label};
use crate::operations::{feature_prepare, feature_ship};

/// Execute `maestro feature`.
pub fn run(args: FeatureArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);

    match args.command {
        FeatureCommand::New {
            title,
            description,
            question,
            project,
            id_only,
        } => new_feature(&paths, &title, description, question, project, id_only),
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
            edit_acceptance,
            text,
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
                edit_acceptance: paired_acceptance_edits(edit_acceptance, text)?,
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
            if !dry_run {
                let _ = super::active::worktree_advisory(&paths);
            }
            Ok(())
        }
        FeatureCommand::Prepare { id, from, draft } => {
            prepare_feature(&paths, &id, from.as_deref(), draft)?;
            let _ = super::active::worktree_advisory(&paths);
            Ok(())
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
            print_uncovered_acceptance_warning(&paths, &id, CoverageFix::Locked)
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
        FeatureCommand::Spec {
            id,
            section,
            append,
            replace,
        } => feature_spec(&paths, &id, section, append, replace),
        FeatureCommand::List { all } => list_features(&paths, all),
        FeatureCommand::Archive {
            id,
            closed,
            dry_run,
        } => archive_features(&paths, id, closed, dry_run),
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
            print_uncovered_acceptance_warning(paths, id, CoverageFix::Plan)?;
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
            print_uncovered_acceptance_warning(paths, id, CoverageFix::Locked)?;
            Ok(())
        }
    }
}

fn verify_feature(
    paths: &MaestroPaths,
    id: &str,
    prove: Vec<String>,
    evidence: Vec<String>,
    waive: Vec<String>,
    reason: Vec<String>,
) -> Result<()> {
    if prove.len() != evidence.len() {
        bail!("each --prove needs its --evidence");
    }
    if waive.len() != reason.len() {
        bail!("each --waive needs its --reason");
    }
    let mut updates = prove
        .into_iter()
        .zip(evidence)
        .map(|(ac_id, evidence)| feature::FeatureProofUpdate::Explicit { ac_id, evidence })
        .collect::<Vec<_>>();
    updates.extend(
        waive
            .into_iter()
            .zip(reason)
            .map(|(ac_id, reason)| feature::FeatureProofUpdate::Waive { ac_id, reason }),
    );
    let report = feature::verify_feature(paths, id, updates)?;
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
        print_green_sweep_next(paths, &report.feature_id)?;
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

fn print_green_sweep_next(paths: &MaestroPaths, feature_id: &str) -> Result<()> {
    // Only the lifecycle status drives the next-step hint; avoid show's task,
    // coverage, and note joins (the InProgress arm re-loads via ship_gaps anyway).
    match feature::status(paths, feature_id)? {
        FeatureStatus::Proposed => {}
        FeatureStatus::Ready => println!("next: maestro feature start {feature_id}"),
        FeatureStatus::InProgress => {
            let gaps = feature::ship_gaps(paths, feature_id)?;
            if gaps.is_empty() {
                println!("next: maestro feature ship {feature_id} --outcome \"<outcome>\"");
            } else {
                println!("not yet shippable:");
                println!("  {}", gaps.join("\n  "));
            }
        }
        FeatureStatus::Shipped | FeatureStatus::Cancelled => {}
    }
    Ok(())
}

/// Dispatch `feature archive`: exactly one of a single id or `--closed`.
fn archive_features(
    paths: &MaestroPaths,
    id: Option<String>,
    closed: bool,
    dry_run: bool,
) -> Result<()> {
    match (id, closed) {
        (Some(id), false) => match feature::archive_feature(paths, &id, dry_run) {
            Ok(report) => {
                print_feature_archive_note(&id, &report, dry_run);
                Ok(())
            }
            Err(error) => bail!("{}", feature_archive_error_message(&id, &error.to_string())),
        },
        (None, true) => archive_closed(paths, dry_run),
        (Some(_), true) => bail!(
            "provide a feature id or --closed, not both\n  maestro feature archive <id>\n  maestro feature archive --closed"
        ),
        (None, false) => bail!(
            "provide a feature id or --closed\n  maestro feature archive <id>\n  maestro feature archive --closed"
        ),
    }
}

/// Bulk-archive every closed (terminal) feature (§5 L3). Collect-and-continue:
/// one feature's failure never aborts the sweep; the summary exits non-zero iff
/// any failed, so a re-run safely retries (archived features no-op, failures
/// retry).
fn archive_closed(paths: &MaestroPaths, dry_run: bool) -> Result<()> {
    let closed: Vec<String> = feature::list(paths)?
        .into_iter()
        .filter(|view| view.status.is_terminal())
        .map(|view| view.id)
        .collect();

    if closed.is_empty() {
        println!("no closed features to archive");
        return Ok(());
    }

    let mut failures = Vec::new();
    let mut archived = 0usize;
    let mut child_tasks = 0usize;
    for id in &closed {
        match feature::archive_feature(paths, id, dry_run) {
            Ok(report) => {
                archived += 1;
                child_tasks += report.child_tasks;
            }
            Err(err) => failures.push(format!("{id}: {err:#}")),
        }
    }

    if dry_run {
        println!("dry-run: would archive closed features");
    } else {
        println!("archived closed features");
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
        println!("  retry: maestro feature archive --closed");
        bail!(
            "{} closed feature(s) failed to archive (re-run to retry):\n  {}",
            failures.len(),
            failures.join("\n  ")
        );
    }
    if dry_run {
        println!("writes: none");
        println!("run: maestro feature archive --closed");
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
    project: Option<String>,
    id_only: bool,
) -> Result<()> {
    let project = super::resolve_project(project, paths)?;
    let id = feature::create(paths, title, project)?;
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
    super::emit_card_touch(paths, &id);
    if id_only {
        println!("{id}");
        return Ok(());
    }
    println!("created feature {id} (proposed)");
    println!("spec: .maestro/cards/{id}/spec.md");
    println!("fill: maestro feature spec {id} --section \"Current state\" --append \"<text>\"");
    println!("decisions: maestro decision new \"<title>\" --feature {id}");
    if initialized {
        println!("initialized contract fields");
    }
    Ok(())
}

fn set_feature(paths: &MaestroPaths, id: &str, edits: ContractEdits) -> Result<()> {
    if edits.is_empty() {
        bail!(
            "no fields to set\n  maestro feature set {id} --acceptance \"<criterion>\" --area \"<surface>\"\n  flags: --acceptance --area --non-goal --question --description --request --type"
        );
    }
    let report = feature::set_with_report(paths, id, edits)?;
    super::emit_card_touch(paths, id);
    print_set_report(id, &report);
    println!("next: maestro feature accept {id}");
    if !report.view.open_questions.is_empty() {
        println!(
            "fork hint: open real forks with `maestro decision new \"<title>\" --feature {id} --context \"<why>\"`; keep --question for loose questions"
        );
    }
    Ok(())
}

fn paired_acceptance_edits(
    edit_acceptance: Vec<String>,
    text: Vec<String>,
) -> Result<Vec<feature::AcceptanceTextEdit>> {
    if edit_acceptance.len() != text.len() {
        bail!(
            "{} --edit-acceptance but {} --text: each --edit-acceptance needs its --text",
            edit_acceptance.len(),
            text.len()
        );
    }
    Ok(edit_acceptance
        .into_iter()
        .zip(text)
        .map(|(id, text)| feature::AcceptanceTextEdit { id, text })
        .collect())
}

fn print_set_report(id: &str, report: &feature::SetReport) {
    println!("set {id}");
    for line in change_lines("replaced", &report.replaced, &report.view) {
        println!("  {line}");
    }
    for line in change_lines("added", &report.added, &report.view) {
        println!("  {line}");
    }
    if report.edited_acceptance > 0 {
        println!("  acceptance edited ({})", report.edited_acceptance);
    }
    if report.replaced.is_empty() && report.added.is_empty() && report.edited_acceptance == 0 {
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
        println!("next: maestro card archive {}", report.id);
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
    let report = feature_ship::ship(paths, id, outcome, dry_run)?;
    println!("{}", report.note);
    if dry_run {
        println!("ship preview:");
        println!("  feature: {}", report.id);
        // dec-ac-7-final: a non-blocking reminder that verified children carry
        // proof from older commits. It never feeds the ship gate, so it cannot
        // turn a passing preview into a blocked one.
        let drifted = feature::verified_child_commit_drift(paths, &report.id)?;
        if !drifted.is_empty() {
            println!(
                "  note: {} child task(s) verified at older commits (HEAD moved); re-verify if their code changed: {} (advisory; does not block ship)",
                drifted.len(),
                drifted.join(", ")
            );
        }
        println!("  target: shipped");
        println!("  full verify suite would run before shipping");
        println!("writes: none");
        println!(
            "retry: maestro feature ship {} --outcome \"<outcome>\"",
            report.id
        );
    } else if report.changed && report.status == FeatureStatus::Shipped {
        println!("ship receipt:");
        println!("  feature: {}", report.id);
        println!("  status: shipped");
        println!("  full verify suite passed");
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
        println!("next: maestro card archive {}", report.id);
        println!("retro: anything to make a permanent rule?");
        println!("  record it: maestro harness propose --title \"<rule>\" --evidence \"<why>\"");
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
            "cannot archive {id}:\n  archived copy already exists\ninspect:\n  live: maestro feature show {id}\n  archived: .maestro/archive/cards/{id}\nnext:\n  resolve the duplicate archive, then retry: maestro feature archive {id}"
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
            "cannot unarchive {id}:\n  live feature already exists\ninspect:\n  live: maestro feature show {id}\n  archived: .maestro/archive/cards/{id}\nnext:\n  resolve the live feature conflict, then retry: maestro feature unarchive {id}"
        );
    }
    if error.contains("a live copy of") {
        let detail = error.split(" — ").nth(1).unwrap_or(error);
        return format!(
            "cannot unarchive {id}:\n  {detail}\ninspect:\n  live: maestro feature show {id}\n  archived: .maestro/archive/cards/{id}\nnext:\n  resolve the live copy conflict, then retry: maestro feature unarchive {id}"
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

fn feature_spec(
    paths: &MaestroPaths,
    id: &str,
    section: Option<String>,
    append: Option<String>,
    replace: Option<String>,
) -> Result<()> {
    match (section, append, replace) {
        (None, None, None) => show_feature_spec(paths, id),
        (Some(section), Some(text), None) => write_feature_spec(paths, id, &section, &text, false),
        (Some(section), None, Some(text)) => write_feature_spec(paths, id, &section, &text, true),
        (Some(section), None, None) => bail!(
            "--section needs the text to write\n  append: maestro feature spec {id} --section \"{section}\" --append \"<text>\"\n  replace: maestro feature spec {id} --section \"{section}\" --replace \"<text>\""
        ),
        (None, _, _) => bail!(
            "--append/--replace need --section\n  maestro feature spec {id} --section \"<name>\" --append \"<text>\""
        ),
        (Some(_), Some(_), Some(_)) => unreachable!("clap rejects --append with --replace"),
    }
}

fn write_feature_spec(
    paths: &MaestroPaths,
    id: &str,
    section: &str,
    text: &str,
    replace: bool,
) -> Result<()> {
    let report = feature::write_spec_section(paths, id, section, text, replace)?;
    super::emit_card_touch(paths, id);
    let verb = if replace { "replaced" } else { "appended to" };
    let created = if report.created_section {
        " (new section)"
    } else {
        ""
    };
    println!("{verb} section \"{}\"{created}", section.trim());
    // The section body runs to the next heading, so headings inside the
    // written text become section boundaries a later --section edit stops at.
    if text
        .lines()
        .any(|line| line.starts_with("## ") || line.starts_with("# "))
    {
        println!(
            "note: the text contains markdown headings, which start new sections; a later --section \"{}\" edit stops at the first one",
            section.trim()
        );
    }
    println!("spec: .maestro/cards/{id}/spec.md");
    println!("inspect: maestro feature spec {id}");
    Ok(())
}

fn show_feature_spec(paths: &MaestroPaths, id: &str) -> Result<()> {
    // L6b: reads cross the boundary -- mirror `show_feature`'s archive
    // fallthrough so a historical spec still renders. Only when neither tree
    // resolves does the unreadable-card recovery view take over, carrying the
    // live error.
    let (view, archived) = match feature::show(paths, id) {
        Ok(view) => (view, false),
        Err(live_err) => match feature::show_archived(paths, id) {
            Ok(view) => (view, true),
            Err(_) => return show_unreadable_feature_spec(paths, id, live_err),
        },
    };
    println!("status: {}", feature::status_label(&view.status));
    println!("feature: {}", view.id);
    if archived {
        println!("archived: true");
    }
    println!();
    let sidecar_dir = if archived {
        paths.archive_cards_dir().join(&view.id)
    } else {
        feature::feature_sidecar_dir(paths, &view.id)
    };
    let spec_path = sidecar_dir.join("spec.md");
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
    let path = card::store::card_path(paths, id);
    println!("status: unreadable");
    println!("feature: {id}");
    println!("path: {}", path.display());
    println!("error: {error:#}");
    println!();
    println!("## Raw card.yaml");
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

/// What can still close an uncovered acceptance item at this point: prepared
/// tasks are accepted on creation, so `task set --covers` never works right
/// after prepare/start; the fix is the plan file or new work, never a locked task.
enum CoverageFix {
    /// Tasks come from the plan file; coverage is authored as `covers:` lines.
    Plan,
    /// Existing tasks are acceptance-locked; cover with new work or evidence.
    Locked,
}

fn print_uncovered_acceptance_warning(
    paths: &MaestroPaths,
    id: &str,
    fix: CoverageFix,
) -> Result<()> {
    let uncovered = feature::uncovered_acceptance(paths, id)?;
    if uncovered.is_empty() {
        return Ok(());
    }
    println!(
        "warning: {} acceptance item(s) have no covering task: {}",
        uncovered.len(),
        uncovered.join(", ")
    );
    match fix {
        CoverageFix::Plan => {
            println!(
                "fix: add `covers: <ac-id>` to task lines in the plan before `prepare --from`"
            );
        }
        CoverageFix::Locked => {
            println!("fix: maestro task create \"<title>\" --feature {id} --covers <ac-id>");
            println!(
                "     or prove directly: maestro feature verify {id} --prove <ac-id> --evidence \"<proof>\""
            );
        }
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
    let mut views = Vec::new();
    let mut unreadable = Vec::new();
    for entry in feature::list_tolerant(paths) {
        match entry {
            feature::FeatureRosterEntry::Loaded(view) => views.push(*view),
            feature::FeatureRosterEntry::Unreadable {
                id, error, hint, ..
            } => unreadable.push((id, error, hint)),
        }
    }
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
        let mut rows: Vec<Vec<String>> = shown
            .iter()
            .map(|view| {
                let title = match view.outcome.as_deref() {
                    Some(outcome) => format!("{} -- {outcome}", view.title),
                    None => view.title.clone(),
                };
                vec![
                    view.id.clone(),
                    feature::status_label(&view.status).to_string(),
                    feature_next_label(view).to_string(),
                    view.counts.total.to_string(),
                    view.counts.verified.to_string(),
                    title,
                ]
            })
            .collect();
        for (id, error, hint) in &unreadable {
            rows.push(vec![
                id.clone(),
                "unreadable".to_string(),
                recovery_label(hint.as_deref()).to_string(),
                "0".to_string(),
                "0".to_string(),
                error.clone(),
            ]);
        }
        print!(
            "{}",
            table::render_table(
                &["ID", "STATE", "NEXT", "TASKS", "VERIFIED", "TITLE"],
                &rows
            )
        );
        println!("inspect any: maestro feature show <id>");
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
