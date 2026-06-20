use anyhow::{Context, Result, bail};

use crate::domain::feature;
use crate::domain::proof;
use crate::domain::task;
use crate::domain::task::{BlockerKind, BlockerTarget, TaskRecord, TaskState, TransitionDetails};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::utc_now_timestamp;
use crate::interfaces::cli::query;
use crate::interfaces::cli::status;
use crate::interfaces::cli::task_id::resolve_optional_task_id;
use crate::interfaces::cli::verify;
use crate::interfaces::cli::{TaskArgs, TaskCommand};
use crate::interfaces::tui::task_list_watch;

/// Execute `maestro task`.
pub fn run(args: TaskArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    // Read verbs (list/show/doctor) must not scaffold: a pure inspect should leave
    // disk untouched, matching feature/decision/query. The sole first-write mutator,
    // `create`, mints its card via the card store; every other mutator loads an
    // existing task.
    let actor = super::actor();

    match args.command {
        TaskCommand::Create {
            title,
            feature,
            covers,
            lane,
            risk,
            check,
            project,
            id_only,
        } => create_task(
            &paths, &title, feature, covers, lane, risk, check, project, id_only,
        ),
        TaskCommand::Set {
            id,
            check,
            feature,
            no_feature,
            covers,
            verify_command,
            clear_verify_command,
        } => set_task(
            &paths,
            &id,
            check,
            covers,
            feature,
            no_feature,
            verify_command,
            clear_verify_command,
            &actor,
        ),
        TaskCommand::Explore { id } => explore_task(&paths, &id, &actor),
        TaskCommand::Accept { id } => accept_task(&paths, &id, &actor),
        TaskCommand::Claim { id, next } => match (id, next) {
            (Some(id), false) => claim_task(&paths, &id, &actor),
            (None, true) => claim_next_task(&paths, &actor),
            (Some(_), true) => {
                bail!("use `maestro task claim --next` or `maestro task claim <id>`, not both")
            }
            (None, false) => bail!("task claim requires <id> or --next"),
        },
        TaskCommand::Complete {
            id,
            summary,
            claim,
            proof,
        } => {
            if claim.iter().any(|claim| claim.trim().is_empty()) {
                bail!(
                    "`--claim` must not be empty; pass the proof to verify against, e.g. --claim \"cargo test passes\""
                );
            }
            complete_task(&paths, &id, summary, claim, proof, &actor)
        }
        TaskCommand::Verify { id } => {
            let id = resolve_optional_task_id(
                &paths,
                id,
                "task id is required or set MAESTRO_CURRENT_TASK",
            )?;
            verify::run_for_task(&paths, &id, &actor)
        }
        TaskCommand::Next { json } => status::run_task_next(&paths, json),
        TaskCommand::Note { id, text } => {
            let report = task::note(&paths.tasks_dir(), &id, &text)?;
            if report.created {
                println!("noted {} (notes.md created)", report.id);
            } else {
                println!("noted {}", report.id);
            }
            Ok(())
        }
        TaskCommand::Update { id, summary, claim } => {
            update_task(&paths, &id, summary, claim, &actor)
        }
        TaskCommand::Block { id, reason, by } => {
            if reason.trim().is_empty() {
                bail!(
                    "`--reason` must not be empty; say why the task is blocked, e.g. --reason \"waiting on task-002\""
                );
            }
            block_task(&paths, &id, &reason, by, &actor)
        }
        TaskCommand::Unblock { id, blocker } => unblock_task(&paths, &id, &blocker, &actor),
        TaskCommand::Reject { id, reason } => {
            if reason.trim().is_empty() {
                bail!(task_terminal_reason_required(&id, "reject", "rejected"));
            }
            terminal_task(&paths, &id, TaskState::Rejected, reason, None, &actor)
        }
        TaskCommand::Abandon { id, reason } => {
            if reason.trim().is_empty() {
                bail!(task_terminal_reason_required(&id, "abandon", "abandoned"));
            }
            terminal_task(&paths, &id, TaskState::Abandoned, reason, None, &actor)
        }
        TaskCommand::Supersede { id, by, reason } => {
            if reason.trim().is_empty() {
                bail!(task_terminal_reason_required(
                    &id,
                    "supersede",
                    "superseded"
                ));
            }
            supersede_task(&paths, &id, &by, &reason, &actor)
        }
        TaskCommand::Show { id } => show_task(&paths, id),
        TaskCommand::List {
            blocked,
            blocked_by,
            blocks,
            feature,
            ready,
            all,
            watch,
            interval,
        } => list_tasks(
            &paths,
            TaskListFilters {
                blocked,
                blocked_by,
                blocks,
                feature,
                ready,
                all,
                watch,
                interval,
            },
        ),
        TaskCommand::Watch { id, interval } => watch_tasks(&paths, id, interval),
        TaskCommand::Proof {
            task_id,
            task_id_flag,
        } => query::run_proof(task_id, task_id_flag),
        TaskCommand::Doctor => doctor_tasks(&paths),
        TaskCommand::Archive { id, dry_run: _ } => {
            bail!("{}", per_task_archive_retired(&id))
        }
        TaskCommand::Unarchive { id } => {
            bail!("{}", per_task_archive_retired(&id))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn create_task(
    paths: &MaestroPaths,
    title: &str,
    feature: Option<String>,
    covers: Vec<String>,
    lane: Option<String>,
    risk: Option<String>,
    checks: Vec<String>,
    project: Option<String>,
    id_only: bool,
) -> Result<()> {
    if let Some(target) = feature.as_deref() {
        guard_feature_target(paths, target)?;
    }
    let project = super::resolve_project(project, paths)?;
    let now = utc_now_timestamp();
    let task = task::create_task(
        &paths.tasks_dir(),
        title,
        task::CreateTaskOptions {
            feature,
            covers,
            lane,
            risk,
            checks,
            project,
            created_at: now,
        },
    )?;

    if id_only {
        println!("{}", task.id);
        return Ok(());
    }
    let checks = task::load_task_checks(&paths.tasks_dir(), &task)?;
    print_task_create_handoff(&task, &checks);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn set_task(
    paths: &MaestroPaths,
    id: &str,
    checks: Vec<String>,
    covers: Vec<String>,
    feature: Option<String>,
    no_feature: bool,
    verify_command: Option<String>,
    clear_verify_command: bool,
    actor: &str,
) -> Result<()> {
    let changing_feature = feature.is_some() || no_feature;
    let changing_verify_command = verify_command.is_some() || clear_verify_command;
    if checks.is_empty() && covers.is_empty() && !changing_feature && !changing_verify_command {
        bail!(
            "task set requires --check, --covers, --feature, --no-feature, --verify-command, or --clear-verify-command\n  maestro task set {id} --check \"...\"\n  maestro task set {id} --covers ac-1\n  maestro task set {id} --feature <feature-id>\n  maestro task set {id} --no-feature\n  maestro task set {id} --verify-command \"cargo test --test foo\"\n  maestro task set {id} --clear-verify-command"
        );
    }

    // Theme II cross-aggregate guard lives here in the interface layer so the
    // task domain stays clear of the feature aggregate: a link may change only
    // while both the current and target feature are non-terminal.
    if changing_feature {
        guard_feature_link(paths, id, feature.as_deref())?;
    }

    if !checks.is_empty() {
        let (task, replaced) = task::set_checks(&paths.tasks_dir(), id, checks)?;
        if replaced > 0 {
            println!(
                "note: replaced {replaced} existing check(s); `--check` replaces the whole list, so re-pass any you want to keep"
            );
        }
        println!("updated {} checks", task.id);
        let checks = task::load_task_checks(&paths.tasks_dir(), &task)?;
        print_verify_block(&task, &checks);
        print_task_next_for_state(&task, &checks);
    }

    if !covers.is_empty() {
        let (task, replaced) = task::set_covers(&paths.tasks_dir(), id, covers)?;
        if replaced > 0 {
            println!(
                "note: replaced {replaced} existing cover link(s); `--covers` replaces the whole list, so re-pass any you want to keep"
            );
        }
        println!("updated {} covers", task.id);
    }

    if changing_feature {
        let now = utc_now_timestamp();
        let target = if no_feature { None } else { feature };
        let task = task::set_feature(&paths.tasks_dir(), id, target, actor, &now)?;
        match &task.feature_id {
            Some(feature_id) => println!("updated {} -> feature {feature_id}", task.id),
            None => println!("updated {} -> no feature", task.id),
        }
    }

    if changing_verify_command {
        let target = if clear_verify_command {
            None
        } else {
            verify_command
        };
        let task = task::set_verify_command(&paths.tasks_dir(), id, target)?;
        match &task.verify_command {
            Some(command) => println!(
                "updated {} -> verify command `{command}` (task verify runs only this, not stack.verify)",
                task.id
            ),
            None => println!(
                "updated {} -> verify command cleared (task verify now uses claims/proof; the repo-global stack.verify runs at feature close)",
                task.id
            ),
        }
    }
    Ok(())
}

fn guard_feature_link(paths: &MaestroPaths, id: &str, target: Option<&str>) -> Result<()> {
    let task = task::load_task_record(&paths.tasks_dir(), id)?;
    // Fail fast before any write: a combined `--check --feature` set would
    // otherwise persist the checks before set_feature's settled-state guard
    // fires. A settled task's link is frozen history; this mirrors (and
    // pre-empts) the authoritative domain guard in task::set_feature.
    if !task.state.is_live() {
        bail!(
            "task {id} is {}; its feature link is settled history and cannot change",
            task.state.as_str()
        );
    }
    if let Some(current) = task.feature_id.as_deref() {
        // A dangling current link (feature unreadable) is permissive so the
        // task can be re-pointed or detached to repair it; only a resolved
        // terminal feature freezes the link as history.
        if let Some(status) = feature::show(paths, current).ok().map(|view| view.status)
            && status.is_terminal()
        {
            bail!(
                "task {id} is linked to feature {current} ({}); its link is settled history and cannot change",
                feature::status_label(&status)
            );
        }
    }
    if let Some(target) = target {
        guard_feature_target(paths, target)?;
    }
    Ok(())
}

/// Validate that a feature-link TARGET exists and is non-terminal. Shared by
/// `task create --feature` and `task set --feature` so neither can persist a
/// dangling or settled link.
fn guard_feature_target(paths: &MaestroPaths, target: &str) -> Result<()> {
    let view = feature::show(paths, target).with_context(|| {
        format!("target feature `{target}` not found; create it with `maestro feature new`")
    })?;
    if view.status.is_terminal() {
        bail!(
            "target feature {target} is {}; tasks cannot be attached to a terminal feature",
            feature::status_label(&view.status)
        );
    }
    Ok(())
}

fn accept_task(paths: &MaestroPaths, id: &str, actor: &str) -> Result<()> {
    let now = utc_now_timestamp();
    let task = task::accept_task(&paths.tasks_dir(), id, actor, &now)?;
    let checks = task::load_task_checks(&paths.tasks_dir(), &task)?;

    println!("accepted {} -> {}", task.id, task.state.as_str());
    print_verify_block(&task, &checks);
    println!("acceptance locked");
    println!("next: maestro task claim {}", task.id);
    Ok(())
}

fn claim_task(paths: &MaestroPaths, id: &str, actor: &str) -> Result<()> {
    if let Ok(task) = task::load_task_record(&paths.tasks_dir(), id)
        && matches!(task.state, TaskState::Draft | TaskState::Exploring)
    {
        let checks = task::load_task_checks(&paths.tasks_dir(), &task).unwrap_or_default();
        bail!("{}", claim_not_ready_message(&task, &checks));
    }
    let now = utc_now_timestamp();
    let task = task::claim_task(&paths.tasks_dir(), id, actor, &now)?;
    let checks = task::load_task_checks(&paths.tasks_dir(), &task)?;
    println!("updated {} -> {}", task.id, task.state.as_str());
    print_verify_block(&task, &checks);
    println!("finish with proof:");
    println!(
        "  maestro task complete {} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\"",
        task.id
    );
    Ok(())
}

fn claim_next_task(paths: &MaestroPaths, actor: &str) -> Result<()> {
    let tasks = task::load_task_records(&paths.tasks_dir())?;
    let Some(next) = tasks
        .iter()
        .find(|task| task.state == TaskState::Ready && !task::has_unresolved_blockers(task))
    else {
        bail!("no ready, unblocked task to claim; run `maestro task next`");
    };
    let now = utc_now_timestamp();
    let task = task::claim_task(&paths.tasks_dir(), &next.id, actor, &now)?;
    let checks = task::load_task_checks(&paths.tasks_dir(), &task)?;
    println!("claimed {} -> {}", task.id, task.state.as_str());
    print_claim_next_context(paths, &task)?;
    println!("title: {}", task.title);
    print_acceptance_checks(&checks);
    println!("finish with proof:");
    println!(
        "  maestro task complete {} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\"",
        task.id
    );
    Ok(())
}

fn print_claim_next_context(paths: &MaestroPaths, task: &TaskRecord) -> Result<()> {
    let Some(feature_id) = task.feature_id.as_deref() else {
        return Ok(());
    };
    println!("feature: {feature_id}");
    let feature_tasks: Vec<TaskRecord> = task::load_task_records(&paths.tasks_dir())?
        .into_iter()
        .filter(|candidate| candidate.feature_id.as_deref() == Some(feature_id))
        .collect();
    // Opaque card ids don't sort in plan order, so the chain follows the
    // dependency edges instead of id adjacency: the task this one waits on and
    // the first task waiting on this one.
    let prev = task
        .blockers
        .iter()
        .filter_map(|blocker| blocker.blocked_ref.as_ref())
        .filter(|target| target.kind == BlockerKind::Task)
        .find_map(|target| feature_tasks.iter().find(|c| c.id == target.id));
    let next = feature_tasks.iter().find(|candidate| {
        candidate.id != task.id
            && candidate.blockers.iter().any(|blocker| {
                blocker
                    .blocked_ref
                    .as_ref()
                    .is_some_and(|target| target.kind == BlockerKind::Task && target.id == task.id)
            })
    });
    if prev.is_none() && next.is_none() {
        return Ok(());
    }
    println!("chain:");
    for candidate in [prev, Some(task), next].into_iter().flatten() {
        println!(
            "  {} {:<8} {}",
            candidate.id,
            task_chain_state(candidate, &task.id),
            candidate.title
        );
    }
    Ok(())
}

fn task_chain_state(task: &TaskRecord, current_id: &str) -> &'static str {
    if task.id == current_id {
        "current"
    } else if task::has_unresolved_blockers(task) {
        "blocked"
    } else {
        task.state.as_str()
    }
}

fn print_acceptance_checks(checks: &[String]) {
    if checks.is_empty() {
        println!("acceptance: inherited from feature");
        return;
    }
    println!("acceptance:");
    for check in checks {
        println!("- {check}");
    }
}

fn explore_task(paths: &MaestroPaths, id: &str, actor: &str) -> Result<()> {
    let task = transition_task_record(
        paths,
        id,
        TaskState::Exploring,
        actor,
        TransitionDetails::default(),
    )?;
    let checks = task::load_task_checks(&paths.tasks_dir(), &task)?;
    println!("updated {} -> {}", task.id, task.state.as_str());
    print_verify_block(&task, &checks);
    print_task_next_for_state(&task, &checks);
    Ok(())
}

fn complete_task(
    paths: &MaestroPaths,
    id: &str,
    summary: String,
    claims: Vec<String>,
    proof_texts: Vec<String>,
    actor: &str,
) -> Result<()> {
    if proof_texts
        .iter()
        .any(|proof_text| proof_text.trim().is_empty())
    {
        bail!("`--proof` must not be empty; pass observed evidence text");
    }
    let task = transition_task_record(
        paths,
        id,
        TaskState::NeedsVerification,
        actor,
        TransitionDetails {
            summary: Some(summary),
            claims,
            ..TransitionDetails::default()
        },
    )?;
    println!("completed {} -> {}", task.id, task.state.as_str());
    if !proof_texts.is_empty() {
        let proof_text = proof_texts.join("\n");
        proof::record_claim(
            paths,
            &super::cli_run_id(),
            &task.id,
            Some(proof_text.clone()),
            None,
            Vec::new(),
        )?;
        println!("auto: recorded task_proof event");
        println!("recorded proof ({} bytes)", proof_text.len());
    }
    println!("auto: maestro task verify {}", task.id);
    match verify::run_for_task(paths, &task.id, actor) {
        Ok(()) => {
            status::print_harness_friction_epilogue(paths)?;
            Ok(())
        }
        Err(error) => {
            eprintln!("task remains: needs_verification");
            eprintln!("next: maestro task proof {}", task.id);
            eprintln!("then: fix proof and run maestro task verify {}", task.id);
            Err(error)
        }
    }
}

fn transition_task_record(
    paths: &MaestroPaths,
    id: &str,
    to: TaskState,
    actor: &str,
    details: TransitionDetails,
) -> Result<TaskRecord> {
    let now = utc_now_timestamp();
    task::transition_task(&paths.tasks_dir(), id, to, actor, &now, details)
}

fn supersede_task(
    paths: &MaestroPaths,
    id: &str,
    by: &str,
    reason: &str,
    actor: &str,
) -> Result<()> {
    let now = utc_now_timestamp();
    let task = match task::supersede_task(&paths.tasks_dir(), id, by, reason, actor, &now) {
        Ok(task) => task,
        Err(error) => bail!(
            "{}",
            task_terminal_error_message(id, Some(by), &error.to_string())
        ),
    };
    print_terminal_receipt(&task, reason, Some(by));
    Ok(())
}

fn terminal_task(
    paths: &MaestroPaths,
    id: &str,
    to: TaskState,
    reason: String,
    replacement: Option<&str>,
    actor: &str,
) -> Result<()> {
    let task = match transition_task_record(
        paths,
        id,
        to,
        actor,
        TransitionDetails {
            summary: Some(reason.clone()),
            to: replacement.map(str::to_string),
            ..TransitionDetails::default()
        },
    ) {
        Ok(task) => task,
        Err(error) => bail!(
            "{}",
            task_terminal_error_message(id, replacement, &error.to_string())
        ),
    };
    print_terminal_receipt(&task, &reason, replacement);
    Ok(())
}

fn print_task_create_handoff(task: &TaskRecord, checks: &[String]) {
    println!("created {} ({})", task.id, task.state.as_str());
    if let Some(feature_id) = task.feature_id.as_deref() {
        println!("feature: {feature_id}");
    }
    print_verify_block(task, checks);
    print_task_next_for_state(task, checks);
}

fn print_verify_block(task: &TaskRecord, checks: &[String]) {
    if !checks.is_empty() {
        println!("verify+ locked:");
        println!("  checks: {}", checks.len());
        if task.feature_id.is_some() {
            println!("  feature gate: qa-baseline + qa-slice at feature accept/close");
        }
        return;
    }

    if task.feature_id.is_none() {
        println!("verify+ missing:");
        println!(
            "  next: maestro task set {} --check \"<observable result>\"",
            task.id
        );
    }
}

fn print_task_next_for_state(task: &TaskRecord, checks: &[String]) {
    let has_verify_contract = task.feature_id.is_some() || !checks.is_empty();
    match task.state {
        TaskState::Draft if has_verify_contract => {
            println!("next: maestro task explore {}", task.id);
        }
        TaskState::Draft => {}
        TaskState::Exploring if has_verify_contract => {
            println!("next: maestro task accept {}", task.id);
        }
        TaskState::Exploring => {}
        TaskState::Ready => println!("next: maestro task claim {}", task.id),
        TaskState::InProgress => {
            println!("finish with proof:");
            println!(
                "  maestro task complete {} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\"",
                task.id
            );
        }
        TaskState::NeedsVerification => println!("next: maestro task verify {}", task.id),
        TaskState::Verified
        | TaskState::Rejected
        | TaskState::Abandoned
        | TaskState::Superseded => println!("next: maestro status"),
    }
}

fn claim_not_ready_message(task: &TaskRecord, checks: &[String]) -> String {
    let mut lines = vec![
        format!("blocked: task {} is not ready to claim", task.id),
        format!("state: {}", task.state.as_str()),
    ];
    match task.state {
        TaskState::Draft => {
            if task.feature_id.is_none() && checks.is_empty() {
                lines.push(format!(
                    "next: maestro task set {} --check \"<observable result>\"",
                    task.id
                ));
                lines.push(format!("then: maestro task explore {}", task.id));
            } else {
                lines.push(format!("next: maestro task explore {}", task.id));
            }
        }
        TaskState::Exploring => {
            if task.feature_id.is_none() && checks.is_empty() {
                lines.push(format!(
                    "next: maestro task set {} --check \"<observable result>\"",
                    task.id
                ));
            }
            lines.push(format!("next: maestro task accept {}", task.id));
        }
        _ => lines.push(format!("next: maestro task show {}", task.id)),
    }
    lines.push("exit: 1".to_string());
    lines.join("\n")
}

fn task_terminal_reason_required(id: &str, verb: &str, state: &str) -> String {
    format!(
        "blocked: task {verb} needs an audited reason\nreason: --reason is empty\nrun: maestro task {verb} {id} --reason \"<why this task is {state}>\""
    )
}

fn task_terminal_error_message(id: &str, replacement: Option<&str>, error: &str) -> String {
    if error.contains("terminal state") {
        return format!(
            "blocked: {id} is already terminal\nstate: {}\ninspect: maestro task show {id}\nnext: maestro status",
            parse_terminal_state(error).unwrap_or("unknown")
        );
    }
    if error.contains("supersede target") {
        let target = replacement.unwrap_or("<replacement-task-id>");
        return format!(
            "blocked: supersede target not found\ntask: {id}\ntarget: {target}\ninspect: maestro task show {id}\nnext: maestro task list\nretry: maestro task supersede {id} --by <replacement-task-id> --reason \"<reason>\""
        );
    }
    if error.contains("by itself") {
        return format!(
            "blocked: cannot supersede {id} by itself\nreason: --by must name a different task\ninspect: maestro task show {id}\nretry: maestro task supersede {id} --by <replacement-task-id> --reason \"<reason>\""
        );
    }
    error.to_string()
}

fn parse_terminal_state(error: &str) -> Option<&str> {
    let state = error
        .split_once("terminal state ")?
        .1
        .split_once(';')?
        .0
        .trim();
    (!state.is_empty()).then_some(state)
}

fn print_terminal_receipt(task: &TaskRecord, reason: &str, replacement: Option<&str>) {
    println!(
        "{} {} (-> {})",
        terminal_verb(task),
        task.id,
        task.state.as_str()
    );
    println!("terminal receipt:");
    println!("  reason: {reason}");
    if let Some(replacement) = replacement {
        println!("  replacement: {replacement}");
    }
    println!("inspect: maestro task show {}", task.id);
    println!("next: maestro status");
}

fn terminal_verb(task: &TaskRecord) -> &'static str {
    match task.state {
        TaskState::Rejected => "rejected",
        TaskState::Abandoned => "abandoned",
        TaskState::Superseded => "superseded",
        _ => "closed",
    }
}

/// Per-task archive was retired (SPEC E4: archive is a feature-cascade only).
/// A finished task stays as closed history; a whole feature and its child tasks
/// archive together. Redirect rather than leave the legacy "task not found"
/// dead-end on an existing card.
fn per_task_archive_retired(id: &str) -> String {
    format!(
        "blocked: per-task archive removed\n\
         task: {id}\n\
         why: archive is now a feature-level cascade; a finished task stays as closed history\n\
         close instead: maestro card close {id}\n\
         archive a feature and its tasks: maestro card archive <feature>"
    )
}

fn block_task(
    paths: &MaestroPaths,
    id: &str,
    reason: &str,
    by: Option<String>,
    actor: &str,
) -> Result<()> {
    let now = utc_now_timestamp();
    let target = BlockerTarget::from_ref(paths, by)?;
    let (task, blocker_id) = task::block_task(&paths.tasks_dir(), id, reason, target, actor, &now)?;

    println!("blocked {} ({blocker_id})", task.id);
    Ok(())
}

fn unblock_task(paths: &MaestroPaths, id: &str, blocker_id: &str, actor: &str) -> Result<()> {
    let now = utc_now_timestamp();
    let task = task::unblock_task(&paths.tasks_dir(), id, blocker_id, actor, &now)?;

    println!("unblocked {} ({blocker_id})", task.id);
    Ok(())
}

fn update_task(
    paths: &MaestroPaths,
    id: &str,
    summary: Option<String>,
    claims: Vec<String>,
    actor: &str,
) -> Result<()> {
    if summary.is_none() && claims.is_empty() {
        bail!(
            "task update requires --summary or --claim\n  maestro task update {id} --summary \"...\"\n  maestro task update {id} --claim \"...\""
        );
    }
    let now = utc_now_timestamp();
    let task = task::update_task_history(
        &paths.tasks_dir(),
        id,
        actor,
        &now,
        TransitionDetails {
            summary,
            claims,
            ..TransitionDetails::default()
        },
    )?;
    println!("updated {}", task.id);
    Ok(())
}

fn show_task(paths: &MaestroPaths, id: Option<String>) -> Result<()> {
    // Mirror the env handling in resolve_optional_task_id (treat empty as unset,
    // no leaked VarError chain) but keep `show` strict: no single-task auto-detect.
    let task_id = match id {
        Some(id) => id,
        None => match std::env::var("MAESTRO_CURRENT_TASK") {
            Ok(id) if !id.trim().is_empty() => id,
            _ => bail!("task id is required or set MAESTRO_CURRENT_TASK for `maestro task show`"),
        },
    };
    // L6b: reads cross the boundary — fall through to the archived card tree so
    // a historical reference to an archived task still renders.
    let (task, archived) = match task::load_task_record(&paths.tasks_dir(), &task_id) {
        Ok(task) => (task, false),
        Err(live_err) => match task::load_archived_task_record(paths, &task_id) {
            Ok(Some((task, _))) => (task, true),
            _ => return Err(live_err),
        },
    };
    let checks = task::load_task_checks(&paths.tasks_dir(), &task)?;
    print!("{}", task::render_task(&task, &checks));
    // Disclose an archive-resolved view so a user cannot mistake an archived task
    // for a live one (mirrors `feature show`'s `archived: true` marker).
    if archived {
        println!("archived: true");
    }
    Ok(())
}

struct TaskListFilters {
    blocked: bool,
    blocked_by: Option<String>,
    blocks: Option<String>,
    feature: Option<String>,
    ready: bool,
    all: bool,
    watch: bool,
    interval: Option<u64>,
}

fn list_tasks(paths: &MaestroPaths, filters: TaskListFilters) -> Result<()> {
    if filters.watch {
        return task_list_watch::run(filters.interval.unwrap_or(2), || {
            let tasks = filtered_tasks(paths, &filters)?;
            task_list_watch::render_snapshot(paths, &tasks)
        });
    }

    // Bare list scans the live tree only (P2 hot path); `--all` also reads the
    // archive (§5.4 / §5.7b), so the hidden-count hint stays live-tree only.
    let mut all_tasks = task::load_task_records(&paths.tasks_dir())?;
    let mut archived_ids = std::collections::BTreeSet::new();
    if filters.all {
        let archived: Vec<TaskRecord> = task::load_archived_task_entries(paths)?
            .into_iter()
            .map(|entry| entry.task)
            .collect();
        archived_ids.extend(archived.iter().map(|t| t.id.clone()));
        all_tasks.extend(archived);
    }
    let shown = task::filter_tasks(all_tasks.clone(), &task_filter(&filters, filters.all));
    if shown.is_empty() {
        // Match `harness list` / `decision list`: an empty result says so
        // instead of leaving a bare header (T8).
        println!("no tasks found");
    } else {
        let missing_verify_contract_ids = task::missing_verify_contract_ids(paths, &shown)?;
        print!(
            "{}",
            task::render_task_list_with_missing_checks(
                &shown,
                &archived_ids,
                &missing_verify_contract_ids,
            )
        );
        println!("inspect any: maestro task show <id>");
    }
    if !filters.all {
        let with_terminal = task::filter_tasks(all_tasks, &task_filter(&filters, true));
        let hidden = with_terminal.len() - shown.len();
        if hidden > 0 {
            println!("# {hidden} terminal task(s) hidden; use --all to include");
        }
    }
    Ok(())
}

/// Build a [`task::TaskFilter`] from the CLI flags, choosing whether terminal
/// tasks are kept (used for the shown set and, with `true`, the hidden count).
fn task_filter(filters: &TaskListFilters, include_terminal: bool) -> task::TaskFilter {
    task::TaskFilter {
        ready: filters.ready,
        blocked: filters.blocked,
        blocked_by: filters.blocked_by.clone(),
        blocks: filters.blocks.clone(),
        feature_id: filters.feature.clone(),
        claimed_by: None,
        include_terminal,
    }
}

fn watch_tasks(paths: &MaestroPaths, id: Option<String>, interval: Option<u64>) -> Result<()> {
    task_list_watch::run(interval.unwrap_or(2), || {
        let mut tasks = task::load_task_records(&paths.tasks_dir())?;
        if let Some(id) = id.as_deref() {
            tasks.retain(|task| task.id == id);
        }
        task_list_watch::render_snapshot(paths, &tasks)
    })
}

/// Feed for the live `task list --watch` view. Unlike the static list it shows
/// every state (including terminal): the watch is a live monitor where seeing a
/// task reach `verified` is the point, mirroring `task watch <id>`.
fn filtered_tasks(paths: &MaestroPaths, filters: &TaskListFilters) -> Result<Vec<TaskRecord>> {
    let tasks = task::load_task_records(&paths.tasks_dir())?;
    Ok(task::filter_tasks(tasks, &task_filter(filters, true)))
}

fn doctor_tasks(paths: &MaestroPaths) -> Result<()> {
    let report = task::check_blocker_graph(&paths.tasks_dir())?;
    let rendered = task::render_report(&report);
    if report.is_ok() {
        print!("{rendered}");
        return Ok(());
    }

    for line in rendered.lines() {
        eprintln!("{line}");
    }
    bail!("task doctor found {} error(s)", report.errors.len())
}
