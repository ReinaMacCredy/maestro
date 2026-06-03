use anyhow::{Context, Result, bail};

use crate::domain::feature;
use crate::domain::proof;
use crate::domain::task;
use crate::domain::task::{BlockerTarget, TaskRecord, TaskState, TransitionDetails};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::time::nanos_since_epoch_string;
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
    // `create`, ensures `.maestro/tasks` itself via write_task_artifacts; every other
    // mutator loads an existing task, and archive/unarchive ensure their own targets.
    let actor = super::actor();

    match args.command {
        TaskCommand::Create {
            title,
            feature,
            lane,
            risk,
            check,
        } => create_task(&paths, &title, feature, lane, risk, check),
        TaskCommand::Set {
            id,
            check,
            feature,
            no_feature,
        } => set_task(&paths, &id, check, feature, no_feature, &actor),
        TaskCommand::Explore { id } => explore_task(&paths, &id, &actor),
        TaskCommand::Accept { id } => accept_task(&paths, &id, &actor),
        TaskCommand::Claim { id } => claim_task(&paths, &id, &actor),
        TaskCommand::Complete {
            id,
            summary,
            claim,
            proof,
        } => {
            if claim.trim().is_empty() {
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
        TaskCommand::Doctor => doctor_tasks(&paths),
        TaskCommand::Archive { id, dry_run } => {
            let note = match task::archive_task(
                &paths.tasks_dir(),
                &paths.archive_tasks_dir(),
                &id,
                dry_run,
            ) {
                Ok(note) => note,
                Err(error) => bail!("{}", task_archive_error_message(&id, &error.to_string())),
            };
            print_task_archive_note(&id, &note);
            Ok(())
        }
        TaskCommand::Unarchive { id } => {
            let note =
                match task::unarchive_task(&paths.tasks_dir(), &paths.archive_tasks_dir(), &id) {
                    Ok(note) => note,
                    Err(error) => {
                        bail!("{}", task_unarchive_error_message(&id, &error.to_string()))
                    }
                };
            print_task_unarchive_note(&id, &note);
            Ok(())
        }
    }
}

fn create_task(
    paths: &MaestroPaths,
    title: &str,
    feature: Option<String>,
    lane: Option<String>,
    risk: Option<String>,
    checks: Vec<String>,
) -> Result<()> {
    if let Some(target) = feature.as_deref() {
        guard_feature_target(paths, target)?;
    }
    let now = nanos_since_epoch_string();
    let task = task::create_task(&paths.tasks_dir(), title, feature, lane, risk, checks, &now)?;

    let checks = task::load_task_checks(&paths.tasks_dir(), &task)?;
    print_task_create_handoff(&task, &checks);
    Ok(())
}

fn set_task(
    paths: &MaestroPaths,
    id: &str,
    checks: Vec<String>,
    feature: Option<String>,
    no_feature: bool,
    actor: &str,
) -> Result<()> {
    let changing_feature = feature.is_some() || no_feature;
    if checks.is_empty() && !changing_feature {
        bail!(
            "task set requires --check, --feature, or --no-feature\n  maestro task set {id} --check \"...\"\n  maestro task set {id} --feature <feature-id>\n  maestro task set {id} --no-feature"
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

    if changing_feature {
        let now = nanos_since_epoch_string();
        let target = if no_feature { None } else { feature };
        let task = task::set_feature(&paths.tasks_dir(), id, target, actor, &now)?;
        match &task.feature_id {
            Some(feature_id) => println!("updated {} -> feature {feature_id}", task.id),
            None => println!("updated {} -> no feature", task.id),
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
    let now = nanos_since_epoch_string();
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
    let now = nanos_since_epoch_string();
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
    claim: String,
    proof_text: Option<String>,
    actor: &str,
) -> Result<()> {
    let task = transition_task_record(
        paths,
        id,
        TaskState::NeedsVerification,
        actor,
        TransitionDetails {
            summary: Some(summary),
            claims: vec![claim],
            ..TransitionDetails::default()
        },
    )?;
    println!("completed {} -> {}", task.id, task.state.as_str());
    if let Some(proof_text) = proof_text {
        if proof_text.trim().is_empty() {
            bail!("`--proof` must not be empty; pass observed evidence text");
        }
        proof::record_claim(
            paths,
            "task-complete",
            &task.id,
            Some(proof_text.clone()),
            None,
            Vec::new(),
        )?;
        println!("auto: recorded task_proof event");
        println!("  proof: {proof_text}");
    }
    println!("auto: maestro task verify {}", task.id);
    match verify::run_for_task(paths, &task.id, actor) {
        Ok(()) => Ok(()),
        Err(error) => {
            eprintln!("task remains: needs_verification");
            eprintln!("next: maestro query proof {}", task.id);
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
    let now = nanos_since_epoch_string();
    task::transition_task(&paths.tasks_dir(), id, to, actor, &now, details)
}

fn supersede_task(
    paths: &MaestroPaths,
    id: &str,
    by: &str,
    reason: &str,
    actor: &str,
) -> Result<()> {
    let now = nanos_since_epoch_string();
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
            println!("  feature gate: qa-baseline + qa-slice at feature accept/ship");
        }
        return;
    }

    if task.feature_id.is_some() {
        println!("verify+ inherited from feature:");
        println!("  task check: optional for feature-linked tasks");
        println!("  feature gate: qa-baseline + qa-slice at feature accept/ship");
    } else {
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
            "blocked: {id} is already terminal\nstate: {}\ninspect: maestro task show {id}\nnext: maestro status\noptional: maestro task archive {id}",
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
    println!("optional: maestro task archive {}", task.id);
}

fn terminal_verb(task: &TaskRecord) -> &'static str {
    match task.state {
        TaskState::Rejected => "rejected",
        TaskState::Abandoned => "abandoned",
        TaskState::Superseded => "superseded",
        _ => "closed",
    }
}

fn print_task_archive_note(id: &str, note: &str) {
    if note.starts_with("would archive ") {
        println!("dry-run: would archive {id} (live -> archive)");
        println!("archive receipt preview:");
        println!("  live path: .maestro/tasks/<task-dir>");
        println!("  archive path: .maestro/archive/tasks/<task-dir>");
        println!("writes: none");
        println!("run: maestro task archive {id}");
    } else if note.starts_with("already archived: ") {
        println!("unchanged: {id} already archived");
        println!("inspect: maestro task show {id}");
        println!("next: maestro status");
        println!("restore: maestro task unarchive {id}");
    } else if note.starts_with("archived ") {
        println!("archived {id} (live -> archive)");
        println!("archive receipt:");
        println!("  archive path: .maestro/archive/tasks/<task-dir>");
        println!("inspect: maestro task show {id}");
        println!("next: maestro status");
        println!("restore: maestro task unarchive {id}");
    } else {
        println!("{note}");
    }
}

fn print_task_unarchive_note(id: &str, note: &str) {
    if note.starts_with("already live: ") {
        println!("unchanged: {id} already live");
        println!("inspect: maestro task show {id}");
        println!("next: maestro status");
        println!("archive: maestro task archive {id}");
    } else if note.starts_with("unarchived ") {
        println!("unarchived {id} (archive -> live)");
        println!("restore receipt:");
        println!("  live path: .maestro/tasks/<task-dir>");
        println!("inspect: maestro task show {id}");
        println!("next: maestro status");
        println!("archive again: maestro task archive {id}");
    } else {
        println!("{note}");
    }
}

fn task_archive_error_message(id: &str, error: &str) -> String {
    if error.contains("not done") {
        return format!(
            "blocked: task is not done\n\
             task: {id}\n\
             reason: {error}\n\
             inspect: maestro task show {id}\n\
             finish first: maestro task complete {id} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\"\n\
             or close: maestro task reject {id} --reason \"<reason>\""
        );
    }
    if error.contains("blocked by it") {
        return format!(
            "blocked: live task still references this task\n\
             task: {id}\n\
             reason: {error}\n\
             inspect: maestro task show {id}\n\
             fix: clear the live blocker named above\n\
             retry: maestro task archive {id}"
        );
    }
    if error.contains("task not found") {
        return format!(
            "blocked: task not found\n\
             task: {id}\n\
             next: maestro task list --all"
        );
    }
    if error.contains("archived copy already exists") {
        return format!(
            "blocked: archived copy already exists\n\
             task: {id}\n\
             inspect: maestro task show {id}\n\
             next: maestro task list --all"
        );
    }
    error.to_string()
}

fn task_unarchive_error_message(id: &str, error: &str) -> String {
    if error.contains("archived task not found") {
        return format!(
            "blocked: archived task not found\n\
             task: {id}\n\
             next: maestro task list --all"
        );
    }
    if error.contains("live task already occupies") {
        return format!(
            "blocked: live task already occupies this id\n\
             task: {id}\n\
             inspect live: maestro task show {id}\n\
             archive live first: maestro task archive {id}\n\
             retry: maestro task unarchive {id}"
        );
    }
    error.to_string()
}

fn block_task(
    paths: &MaestroPaths,
    id: &str,
    reason: &str,
    by: Option<String>,
    actor: &str,
) -> Result<()> {
    let now = nanos_since_epoch_string();
    let target = blocker_target(by);
    let (task, blocker_id) = task::block_task(&paths.tasks_dir(), id, reason, target, actor, &now)?;

    println!("blocked {} ({blocker_id})", task.id);
    Ok(())
}

fn unblock_task(paths: &MaestroPaths, id: &str, blocker_id: &str, actor: &str) -> Result<()> {
    let now = nanos_since_epoch_string();
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
    let now = nanos_since_epoch_string();
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
    // L6b: reads cross the boundary — fall through to the archive so a
    // historical reference to an archived task still renders. Track which tree
    // resolved so the acceptance checks load from the same place.
    let (task, tasks_dir, archived) = match task::load_task_record(&paths.tasks_dir(), &task_id) {
        Ok(task) => (task, paths.tasks_dir(), false),
        Err(live_err) => {
            let archive_dir = paths.archive_tasks_dir();
            let task = task::load_task_record(&archive_dir, &task_id).map_err(|_| live_err)?;
            (task, archive_dir, true)
        }
    };
    let checks = task::load_task_checks(&tasks_dir, &task)?;
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
        return task_list_watch::run(paths, filters.interval.unwrap_or(2), || {
            filtered_tasks(paths, &filters)
        });
    }

    // Bare list scans the live tree only (P2 hot path); `--all` also reads the
    // archive (§5.4 / §5.7b), so the hidden-count hint stays live-tree only.
    let mut all_tasks = task::load_task_records(&paths.tasks_dir())?;
    let mut archived_ids = std::collections::BTreeSet::new();
    if filters.all {
        let archived = task::load_task_records(&paths.archive_tasks_dir())?;
        archived_ids.extend(archived.iter().map(|t| t.id.clone()));
        all_tasks.extend(archived);
    }
    let shown = task::filter_tasks(all_tasks.clone(), &task_filter(&filters, filters.all));
    if shown.is_empty() {
        // Match `harness list` / `decision list`: an empty result says so
        // instead of leaving a bare header (T8).
        println!("no tasks found");
    } else {
        let missing_verify_contract_ids =
            missing_verify_contract_ids(paths, &shown, &archived_ids)?;
        print!(
            "{}",
            task::render_task_list_with_missing_checks(
                &shown,
                &archived_ids,
                &missing_verify_contract_ids,
            )
        );
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

fn missing_verify_contract_ids(
    paths: &MaestroPaths,
    tasks: &[TaskRecord],
    archived_ids: &std::collections::BTreeSet<String>,
) -> Result<std::collections::BTreeSet<String>> {
    let mut missing = std::collections::BTreeSet::new();
    for task in tasks {
        if task.feature_id.is_some()
            || !matches!(task.state, TaskState::Draft | TaskState::Exploring)
        {
            continue;
        }
        let tasks_dir = if archived_ids.contains(&task.id) {
            paths.archive_tasks_dir()
        } else {
            paths.tasks_dir()
        };
        if task::load_task_checks(&tasks_dir, task)?.is_empty() {
            missing.insert(task.id.clone());
        }
    }
    Ok(missing)
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
    task_list_watch::run(paths, interval.unwrap_or(2), || {
        let mut tasks = task::load_task_records(&paths.tasks_dir())?;
        if let Some(id) = id.as_deref() {
            tasks.retain(|task| task.id == id);
        }
        Ok(tasks)
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

fn blocker_target(by: Option<String>) -> BlockerTarget {
    match by {
        Some(by) if by.starts_with("task-") => BlockerTarget::Task(by),
        Some(by) if by.starts_with("decision-") => BlockerTarget::Decision(by),
        Some(by) => BlockerTarget::External(by),
        None => BlockerTarget::Human,
    }
}
