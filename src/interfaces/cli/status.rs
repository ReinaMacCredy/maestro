use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::domain::feature::{self, FeatureRosterEntry, FeatureStatus};
use crate::domain::task::{self, TaskRecord, TaskState};
use crate::domain::{card, gate_lock};
use crate::foundation::core::paths::{MaestroPaths, discover_repo_root};
use crate::foundation::core::table;
use crate::foundation::core::time::{timestamp_nanos, utc_now_timestamp};
use crate::interfaces::cli::{
    ClaimArgs, GitReadout, NextArgs, StatusArgs, clean_worktree_note, feature_next_label,
    git_readout, merge_busy_advisory, proof_concern_line, recovery_label, render_git_line,
    stale_merge_advisory,
};
use crate::operations::harness;
use crate::operations::memory::{
    self, ApprovedMemory, MemoryReadScope, MemoryReadSurface, MemorySuggestionHint,
};

pub fn run(args: StatusArgs) -> Result<()> {
    let repo_root = match discover_repo_root() {
        Ok(repo_root) => repo_root,
        Err(_) => {
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let report = StatusReport::not_initialized(cwd, "repo root not found".to_string());
            return print_status(report, args.json);
        }
    };
    let paths = MaestroPaths::new(repo_root);
    if !paths.maestro_dir().is_dir() {
        let report = StatusReport::not_initialized(
            paths.repo_root().to_path_buf(),
            ".maestro is missing".to_string(),
        );
        return print_status(report, args.json);
    }

    let report = build_status_report(&paths)?;
    print_status(report, args.json)
}

pub fn run_task_next(paths: &MaestroPaths, json: bool) -> Result<()> {
    let report = build_task_next_report(paths)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&TaskNextJson::from(&report))?
        );
    } else {
        print_task_next(&report);
    }
    if report.next_action.is_none() && report.harness_friction.is_empty() {
        bail!("no actionable task");
    }
    Ok(())
}

pub fn run_next(args: NextArgs) -> Result<()> {
    let repo_root = match discover_repo_root() {
        Ok(repo_root) => repo_root,
        Err(_) => {
            let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let report = StatusReport::not_initialized(cwd, "repo root not found".to_string());
            return print_next_suggest(&report, "suggest", args.json);
        }
    };
    let paths = MaestroPaths::new(repo_root);
    if !paths.maestro_dir().is_dir() {
        let report = StatusReport::not_initialized(
            paths.repo_root().to_path_buf(),
            ".maestro is missing".to_string(),
        );
        return print_next_suggest(&report, "suggest", args.json);
    }

    if args.loop_mode {
        return run_next_loop(&paths, args.max_steps, args.json);
    }
    let report = build_task_next_report(&paths)?;
    if args.run {
        run_one_next_action(&paths, &report, args.json)
    } else {
        print_next_suggest(&report, "suggest", args.json)
    }
}

fn run_next_loop(paths: &MaestroPaths, max_steps: usize, json: bool) -> Result<()> {
    let max_steps = max_steps.max(1);
    let mut taken = Vec::new();
    for step in 1..=max_steps {
        let report = build_task_next_report(paths)?;
        let Some(action) = report.next_action.as_ref() else {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&NextRunJson::done("loop", taken))?
                );
            } else {
                println!("done: no actionable task");
            }
            return Ok(());
        };
        if !action.auto_safe {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&NextRunJson::blocked(
                        "loop",
                        taken,
                        action,
                        "next action is not auto-safe",
                    ))?
                );
            } else {
                println!("blocked: next action requires input or review");
                print_next_action(action);
            }
            return Ok(());
        }
        if json {
            taken.push(action.command.display.clone());
        } else {
            println!("step {step}/{max_steps}:");
        }
        execute_auto_safe_action(paths, action)?;
    }
    let report = build_task_next_report(paths)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&NextRunJson::done("loop", taken))?
        );
    } else if let Some(action) = report.next_action.as_ref() {
        println!("blocked: max steps reached");
        print_next_action(action);
    } else {
        println!("done: max steps reached and no actionable task remains");
    }
    Ok(())
}

fn run_one_next_action(paths: &MaestroPaths, report: &StatusReport, json: bool) -> Result<()> {
    let Some(action) = report.next_action.as_ref() else {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&NextRunJson::done("run", Vec::new()))?
            );
        } else {
            println!("no actionable task");
        }
        return Ok(());
    };
    if !action.auto_safe {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&NextRunJson::blocked(
                    "run",
                    Vec::new(),
                    action,
                    "next action is not auto-safe",
                ))?
            );
        } else {
            println!("blocked: next action requires input");
            print_next_action(action);
        }
        bail!("next action requires input or review");
    }
    execute_auto_safe_action(paths, action)
}

fn execute_auto_safe_action(_paths: &MaestroPaths, action: &NextAction) -> Result<()> {
    match action.kind.as_str() {
        "claim_task" => {
            let Some(task_id) = action.task_id.as_deref() else {
                bail!("claim_task action missing task id");
            };
            println!("auto-safe: {}", action.command.display);
            super::card::claim(ClaimArgs {
                id: task_id.to_string(),
            })
        }
        _ => bail!("next action {} is not auto-safe", action.kind),
    }
}

fn print_next_suggest(report: &StatusReport, mode: &str, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&NextJson::from_report(report, mode))?
        );
        return Ok(());
    }
    print_task_next(report);
    Ok(())
}

fn build_task_next_report(paths: &MaestroPaths) -> Result<StatusReport> {
    let task_entries = task::load_task_entries(&paths.tasks_dir())?;
    let mut features = Vec::new();
    let mut unreadable_features = Vec::new();
    for entry in feature::list_tolerant_with_entries(paths, &task_entries) {
        match entry {
            FeatureRosterEntry::Loaded(view) => features.push(*view),
            FeatureRosterEntry::Unreadable {
                id,
                path,
                error,
                hint,
                typed_error,
            } => unreadable_features.push((id, path, error, hint, typed_error)),
        }
    }
    if features.is_empty()
        && let Some((_, _, error, _, typed_error)) = unreadable_features.first()
    {
        if let Some(typed_error) = typed_error.clone() {
            return Err(typed_error.into());
        }
        bail!("{error}");
    }

    let tasks: Vec<TaskRecord> = task_entries.into_iter().map(|entry| entry.task).collect();
    let live_tasks: Vec<TaskRecord> = tasks
        .iter()
        .filter(|task| task.state.is_live())
        .cloned()
        .collect();
    let mut warnings = Vec::new();
    let mut current_task = None;
    let mut current_feature = None;

    let current_task_action = match env::var("MAESTRO_CURRENT_TASK") {
        Ok(id) if !id.trim().is_empty() => match task::load_task_record(&paths.tasks_dir(), &id) {
            Ok(task) if task.state.is_live() => {
                current_task = Some(task.id.clone());
                current_feature = task.feature_id.clone();
                task_action(paths, &task)?
            }
            Ok(task) => {
                warnings.push(WarningJson {
                    code: "current_task_terminal".to_string(),
                    message: format!(
                        "MAESTRO_CURRENT_TASK={} is {}; falling back to repo queue",
                        task.id,
                        task.state.as_str()
                    ),
                });
                None
            }
            Err(_) => {
                warnings.push(WarningJson {
                    code: "current_task_missing".to_string(),
                    message: format!("MAESTRO_CURRENT_TASK={id} was not found; falling back"),
                });
                None
            }
        },
        _ => None,
    };

    let next_action = match current_task_action {
        Some(action) => Some(action),
        None => choose_next_task_action(paths, &live_tasks)?,
    };
    let proof_concern = focal_proof_concern(paths, next_action.as_ref(), &live_tasks);
    let ready_to_close_features = ready_to_close_features(&features);
    for (_, path, error, _, _) in unreadable_features {
        warnings.push(WarningJson {
            code: "feature_unreadable".to_string(),
            message: format!("{} is unreadable: {error}", path.display()),
        });
    }
    let harness_friction = harness::over_threshold_items(paths)?
        .into_iter()
        .map(HarnessFrictionJson::from)
        .collect::<Vec<_>>();
    let audit_hint = harness::audit_overdue_hint(paths)?.map(AuditHintJson::from);
    let approved_memory =
        memory::approved_memory(paths, MemoryReadSurface::Status, MemoryReadScope::default())?;
    let memory_suggestions =
        memory::suggestion_hints(paths, MemoryReadSurface::Status, MemoryReadScope::default())?;
    let sections = StatusSectionsJson {
        ready_to_close: ready_to_close_features.clone(),
    };

    Ok(StatusReport {
        schema: "maestro.status.v1".to_string(),
        status: if next_action.is_some() || !harness_friction.is_empty() || audit_hint.is_some() {
            "actionable".to_string()
        } else {
            "no_action".to_string()
        },
        repo: paths.repo_root().display().to_string(),
        current_task,
        current_feature,
        git: None,
        merge_lock_holder: None,
        close_or_verify_pending: false,
        proof_concern,
        warnings,
        next_action,
        tasks: TaskSummaryJson::default(),
        features: FeatureSummaryJson::default(),
        task_rows: Vec::new(),
        active_features: Vec::new(),
        harness_friction,
        audit_hint,
        approved_memory: approved_memory.memories,
        approved_memory_omitted: approved_memory.omitted,
        memory_suggestions: memory_suggestions.suggestions,
        memory_suggestions_omitted: memory_suggestions.omitted,
        sections,
        ready_to_close_features,
    })
}

fn print_status(report: StatusReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    if report.status == "not_initialized" {
        println!("maestro status: not initialized");
        println!("repo: {}", report.repo);
        for warning in &report.warnings {
            println!("warning: {}", warning.message);
        }
        println!("next:");
        println!("- preview setup: maestro init --dry-run");
        println!("- initialize: maestro init --yes");
        return Ok(());
    }

    if !report.warnings.is_empty() {
        for warning in &report.warnings {
            println!("warning: {}", warning.message);
        }
        println!();
    }
    println!("repo: {}", report.repo);
    println!(
        "tasks: active={} ready={} needs_verification={} blocked={} | features: active={} ready_to_close={}",
        report.tasks.active,
        report.tasks.ready,
        report.tasks.needs_verification,
        report.tasks.blocked,
        report.features.active,
        report.ready_to_close_features.len()
    );
    if let Some(git) = &report.git {
        println!("{}", render_git_line(git));
        if let Some(stale) = stale_merge_advisory(git) {
            println!("{stale}");
        }
        if report.close_or_verify_pending && git.code_other_dirty > 0 {
            println!("{}", clean_worktree_note(git.code_other_dirty));
        }
    }
    if let Some(holder) = report.merge_lock_holder.as_deref() {
        println!("{}", merge_busy_advisory(holder));
    }
    print_harness_friction(&report.harness_friction);
    print_audit_hint(report.audit_hint.as_ref());
    print_approved_memory(&report.approved_memory, report.approved_memory_omitted);
    print_memory_suggestions(
        &report.memory_suggestions,
        report.memory_suggestions_omitted,
    );
    if let Some(action) = &report.next_action {
        if action.requires_input {
            println!("template: {}", action.command.display);
        } else {
            println!("run: {}", action.command.display);
        }
        match (action.task_id.as_deref(), action.title.as_deref()) {
            (Some(id), Some(title)) => println!("task: {id}  {title}"),
            (Some(id), None) => println!("task: {id}"),
            _ => {}
        }
        if let Some(feature_id) = action.feature_id.as_deref() {
            println!("feature: {feature_id}");
        }
        let others = report.task_rows.len().saturating_sub(1);
        if others > 0 {
            println!("+{others} other active tasks: maestro task list");
        }
    } else {
        println!("no actionable tasks");
    }
    if let Some(concern) = &report.proof_concern {
        println!("{concern}");
    }
    print!("{}", active_features_block(&report.active_features));
    if !report.ready_to_close_features.is_empty() {
        println!("FEATURES READY TO CLOSE");
        let rows: Vec<Vec<String>> = report
            .ready_to_close_features
            .iter()
            .map(|feature| {
                vec![
                    feature.id.clone(),
                    format!("verified={}/{}", feature.verified, feature.total),
                    format!("template: {}", feature.next_action.command.display),
                ]
            })
            .collect();
        print!("{}", table::render_table(&[], &rows));
    }
    println!("more: maestro resume");
    Ok(())
}

fn print_task_next(report: &StatusReport) {
    if !report.warnings.is_empty() {
        for warning in &report.warnings {
            println!("warning: {}", warning.message);
        }
        println!();
    }
    print_harness_friction(&report.harness_friction);
    print_audit_hint(report.audit_hint.as_ref());
    if let Some(action) = &report.next_action {
        print_next_action(action);
        if let Some(concern) = &report.proof_concern {
            println!("{concern}");
        }
        return;
    }
    println!("no actionable task");
    if !report.ready_to_close_features.is_empty() {
        println!("broader repo action available: ready_to_close_features");
        println!("check broader repo status: maestro status");
    }
}

pub(crate) fn print_harness_friction_epilogue(paths: &MaestroPaths) -> Result<()> {
    let items = harness::over_threshold_items(paths)?
        .into_iter()
        .map(HarnessFrictionJson::from)
        .collect::<Vec<_>>();
    print_harness_friction(&items);
    Ok(())
}

fn print_harness_friction(items: &[HarnessFrictionJson]) {
    if items.is_empty() {
        return;
    }
    println!("HARNESS FRICTION");
    for item in items {
        println!("! friction {} over threshold", item.id);
        println!("  seen: {}x/{}s", item.occurrences, item.sessions);
        println!("  title: {}", item.title);
        println!("  apply: maestro harness apply {}", item.id);
        println!(
            "  dismiss: maestro harness dismiss {} --reason \"<why>\"",
            item.id
        );
    }
}

fn print_audit_hint(hint: Option<&AuditHintJson>) {
    let Some(hint) = hint else {
        return;
    };
    println!("AUDIT");
    println!(
        "! repo audit overdue: {} session(s) since last audit (threshold {})",
        hint.sessions_since_audit, hint.every_sessions
    );
    println!("  skill: maestro-audit");
    println!(
        "  propose: maestro harness propose --title \"<finding>\" --evidence \"<evidence>\" --topic <slug>"
    );
}

fn print_approved_memory(memories: &[ApprovedMemory], omitted: usize) {
    if memories.is_empty() {
        return;
    }
    println!("APPROVED MEMORY");
    for memory in memories {
        println!(
            "{}. {} scope={} risk={} {}",
            memory.rank,
            memory.id,
            memory.scope_kind.as_str(),
            memory.risk.as_str(),
            memory.summary
        );
        println!("   show: {}", memory.show_command);
    }
    if omitted > 0 {
        println!("... {omitted} omitted; search with `maestro memory search <query>`");
    }
}

fn print_memory_suggestions(suggestions: &[MemorySuggestionHint], omitted: usize) {
    if suggestions.is_empty() {
        return;
    }
    println!("MEMORY SUGGESTIONS");
    for suggestion in suggestions {
        println!(
            "{}. {} sources={} {}",
            suggestion.rank, suggestion.id, suggestion.source_count, suggestion.summary
        );
        println!("   create: {}", suggestion.create_command);
        println!("   dismiss: {}", suggestion.dismiss_command);
    }
    if omitted > 0 {
        println!("... {omitted} omitted; inspect with `maestro memory suggest list --all`");
    }
}

fn print_next_action(action: &NextAction) {
    if action.requires_input {
        println!("template: {}", action.command.display);
    } else {
        println!("run: {}", action.command.display);
    }
    if !action.command.requires_input.is_empty() {
        println!("required input:");
        for input in &action.command.requires_input {
            println!("- {}: {}", input.name, input.description);
        }
    }
    if let Some(task_id) = action.task_id.as_deref() {
        println!("task: {task_id}");
    }
    if let Some(feature_id) = action.feature_id.as_deref() {
        println!("feature: {feature_id}");
    }
    if let Some(title) = action.title.as_deref() {
        println!("title: {title}");
    }
    println!("reason: {}", action.reason);
    if let Some(inspect) = action.inspect.as_deref() {
        println!("inspect: {inspect}");
    }
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

/// The concern-only proof line for the focal (next-action) task, shared by both
/// the `status` and `task next` builders so the surfaces never diverge.
fn focal_proof_concern(
    paths: &MaestroPaths,
    next_action: Option<&NextAction>,
    live_tasks: &[TaskRecord],
) -> Option<String> {
    let task_id = next_action?.task_id.as_deref()?;
    let task = live_tasks.iter().find(|task| task.id == task_id)?;
    proof_concern_line(paths, task)
}

fn build_status_report(paths: &MaestroPaths) -> Result<StatusReport> {
    // One task scan feeds both the report and the per-feature counts inside the
    // roster (list_tolerant would otherwise re-scan the same cards).
    let task_entries = task::load_task_entries(&paths.tasks_dir())?;
    let mut features = Vec::new();
    let mut unreadable_features = Vec::new();
    for entry in feature::list_tolerant_with_entries(paths, &task_entries) {
        match entry {
            FeatureRosterEntry::Loaded(view) => features.push(*view),
            FeatureRosterEntry::Unreadable {
                id,
                path,
                error,
                hint,
                typed_error,
            } => unreadable_features.push((id, path, error, hint, typed_error)),
        }
    }
    if features.is_empty()
        && let Some((_, _, error, _, typed_error)) = unreadable_features.first()
    {
        if let Some(typed_error) = typed_error.clone() {
            return Err(typed_error.into());
        }
        bail!("{error}");
    }
    let mut warnings = Vec::new();
    let mut current_task = None;
    let mut current_feature = None;

    let tasks: Vec<TaskRecord> = task_entries.into_iter().map(|entry| entry.task).collect();
    let live_tasks: Vec<TaskRecord> = tasks
        .iter()
        .filter(|task| task.state.is_live())
        .cloned()
        .collect();
    // The summary counts come from the card graph, not the legacy `TaskRecord`
    // projection, so they read the same buckets the `maestro watch` board does
    // (the projection counted a card-model `blocks` dep as unblocked and any
    // open card as `active`). Rows and next-action still ride the records.
    let summary_cards = card::query::scan(paths)?;
    let blocked_ids: BTreeSet<String> = card::query::blocked(&summary_cards)
        .into_iter()
        .map(|card| card.id.clone())
        .collect();

    let current_task_action = match env::var("MAESTRO_CURRENT_TASK") {
        Ok(id) if !id.trim().is_empty() => match task::load_task_record(&paths.tasks_dir(), &id) {
            Ok(task) if task.state.is_live() => {
                current_task = Some(task.id.clone());
                current_feature = task.feature_id.clone();
                task_action(paths, &task)?
            }
            Ok(task) => {
                warnings.push(WarningJson {
                    code: "current_task_terminal".to_string(),
                    message: format!(
                        "MAESTRO_CURRENT_TASK={} is {}; falling back to repo queue",
                        task.id,
                        task.state.as_str()
                    ),
                });
                None
            }
            Err(_) => {
                warnings.push(WarningJson {
                    code: "current_task_missing".to_string(),
                    message: format!("MAESTRO_CURRENT_TASK={id} was not found; falling back"),
                });
                None
            }
        },
        _ => None,
    };

    let mut rows = Vec::new();
    for task in &live_tasks {
        rows.push(TaskRowJson {
            id: task.id.clone(),
            state: task_state_label(task),
            title: task.title.clone(),
            next: compact_next(paths, task)?,
            inspect: format!("maestro task show {}", task.id),
            project: task.project.clone(),
        });
    }

    let next_action = match current_task_action {
        Some(action) => Some(action),
        None => choose_next_task_action(paths, &live_tasks)?,
    };
    let proof_concern = focal_proof_concern(paths, next_action.as_ref(), &live_tasks);
    let ready_to_close_features = ready_to_close_features(&features);
    let now_nanos = timestamp_nanos(&utc_now_timestamp()).unwrap_or(0);
    let mut active_features = active_feature_rows(paths, &features, now_nanos);
    for (id, path, error, hint, _) in unreadable_features {
        warnings.push(WarningJson {
            code: "feature_unreadable".to_string(),
            message: format!("{} is unreadable: {error}", path.display()),
        });
        active_features.push(FeatureRowJson {
            id: id.clone(),
            state: "unreadable".to_string(),
            title: error,
            next: recovery_label(hint.as_deref()),
            inspect: format!("maestro feature spec {id}"),
            project: None,
            stale_proposed: false,
        });
    }
    let harness_friction = harness::over_threshold_items(paths)?
        .into_iter()
        .map(HarnessFrictionJson::from)
        .collect::<Vec<_>>();
    let audit_hint = harness::audit_overdue_hint(paths)?.map(AuditHintJson::from);
    let approved_memory =
        memory::approved_memory(paths, MemoryReadSurface::Status, MemoryReadScope::default())?;
    let memory_suggestions =
        memory::suggestion_hints(paths, MemoryReadSurface::Status, MemoryReadScope::default())?;
    let sections = StatusSectionsJson {
        ready_to_close: ready_to_close_features.clone(),
    };
    let git = git_readout(paths);
    let merge_lock_holder = gate_lock::merge_holder(paths);
    // The next verb is close/verify-shaped when the chosen task action is a proof
    // or completion step, or a feature is ready to close (`feature_close` never
    // appears as a task `next_action.kind`; it lives in ready_to_close_features).
    let close_or_verify_pending = next_action
        .as_ref()
        .is_some_and(|action| matches!(action.kind.as_str(), "complete_task" | "proof_recovery"))
        || !ready_to_close_features.is_empty();

    Ok(StatusReport {
        schema: "maestro.status.v1".to_string(),
        status: if next_action.is_some() || !harness_friction.is_empty() || audit_hint.is_some() {
            "actionable".to_string()
        } else {
            "no_action".to_string()
        },
        repo: paths.repo_root().display().to_string(),
        current_task,
        current_feature,
        git,
        merge_lock_holder,
        close_or_verify_pending,
        proof_concern,
        warnings,
        next_action,
        tasks: TaskSummaryJson::from_cards(&summary_cards, &blocked_ids),
        features: FeatureSummaryJson::from_features(&features),
        task_rows: rows,
        active_features,
        harness_friction,
        audit_hint,
        approved_memory: approved_memory.memories,
        approved_memory_omitted: approved_memory.omitted,
        memory_suggestions: memory_suggestions.suggestions,
        memory_suggestions_omitted: memory_suggestions.omitted,
        sections,
        ready_to_close_features,
    })
}

fn choose_next_task_action(
    paths: &MaestroPaths,
    tasks: &[TaskRecord],
) -> Result<Option<NextAction>> {
    for state in [
        TaskState::NeedsVerification,
        TaskState::Ready,
        TaskState::InProgress,
        TaskState::Draft,
        TaskState::Exploring,
    ] {
        if let Some(action) = tasks
            .iter()
            .filter(|task| task.state == state)
            .find_map(|task| task_action(paths, task).transpose())
            .transpose()?
        {
            return Ok(Some(action));
        }
    }
    if let Some(action) = tasks
        .iter()
        .find(|task| task::has_unresolved_blockers(task))
        .map(blocked_action)
    {
        return Ok(Some(action));
    }
    Ok(None)
}

fn task_action(paths: &MaestroPaths, task: &TaskRecord) -> Result<Option<NextAction>> {
    if task::has_unresolved_blockers(task) {
        return Ok(Some(blocked_action(task)));
    }
    let checks = task::load_task_checks(&paths.tasks_dir(), task).unwrap_or_default();
    let has_verify_contract = task.feature_id.is_some() || !checks.is_empty();
    let action = match task.state {
        TaskState::NeedsVerification => NextAction::task(
            "proof_recovery",
            task,
            runnable_command(["maestro", "task", "verify", task.id.as_str()]),
            "verification needs proof; re-verify",
        ),
        TaskState::Ready => NextAction::task(
            "claim_task",
            task,
            runnable_command(["maestro", "task", "claim", task.id.as_str()]),
            "ready task is unclaimed",
        ),
        TaskState::InProgress => NextAction::task(
            "complete_task",
            task,
            task_complete_template(&task.id),
            "claimed task needs completion proof",
        ),
        TaskState::Draft if !has_verify_contract => NextAction::task(
            "add_task_check",
            task,
            task_check_template(&task.id),
            "standalone task needs a verify+ check",
        ),
        TaskState::Draft => NextAction::task(
            "explore_task",
            task,
            runnable_command(["maestro", "task", "explore", task.id.as_str()]),
            "draft task has a verify+ path",
        ),
        TaskState::Exploring if !has_verify_contract => NextAction::task(
            "add_task_check",
            task,
            task_check_template(&task.id),
            "standalone task needs a verify+ check before accept",
        ),
        TaskState::Exploring => NextAction::task(
            "accept_task",
            task,
            runnable_command(["maestro", "task", "accept", task.id.as_str()]),
            "explored task can lock acceptance",
        ),
        TaskState::Verified
        | TaskState::Rejected
        | TaskState::Abandoned
        | TaskState::Superseded => {
            return Ok(None);
        }
    };
    Ok(Some(action))
}

fn blocked_action(task: &TaskRecord) -> NextAction {
    NextAction::task(
        "inspect_blocker",
        task,
        runnable_command(["maestro", "task", "show", task.id.as_str()]),
        "task has unresolved blockers",
    )
}

fn compact_next(paths: &MaestroPaths, task: &TaskRecord) -> Result<String> {
    Ok(task_action(paths, task)?
        .map(|action| action.kind)
        .unwrap_or_else(|| "status".to_string()))
}

fn task_state_label(task: &TaskRecord) -> String {
    if task::has_unresolved_blockers(task) {
        format!("{} / blocked", task.state.as_str())
    } else {
        task.state.as_str().to_string()
    }
}

fn ready_to_close_features(features: &[feature::FeatureView]) -> Vec<ReadyFeatureJson> {
    features
        .iter()
        .filter(|view| view.status == FeatureStatus::InProgress)
        .filter(|view| view.counts.total > 0 && view.counts.total == view.counts.verified)
        .map(|view| ReadyFeatureJson {
            id: view.id.clone(),
            feature_id: view.id.clone(),
            title: view.title.clone(),
            total: view.counts.total,
            verified: view.counts.verified,
            next_action: NextAction::feature_close(view),
        })
        .collect()
}

fn active_feature_rows(
    paths: &MaestroPaths,
    features: &[feature::FeatureView],
    now_nanos: i128,
) -> Vec<FeatureRowJson> {
    features
        .iter()
        .filter(|view| !view.status.is_terminal())
        .map(|view| FeatureRowJson {
            id: view.id.clone(),
            state: feature::status_label(&view.status).to_string(),
            title: view.title.clone(),
            next: feature_next_label(paths, view),
            inspect: format!("maestro feature show {}", view.id),
            project: view.project.clone(),
            stale_proposed: feature::is_stale_proposed(&view.status, &view.updated_at, now_nanos),
        })
        .collect()
}

/// Render the ACTIVE FEATURES block: a table of the features still worth a row,
/// then a one-line collapse plus the fixed retire reminder for any stale
/// proposed features. Stale proposed features stay in `rows` (and in
/// `status --json`) but never render as their own row -- `feature list --all`
/// is where they are reviewed. Returns the empty string when there is nothing
/// to show.
fn active_features_block(rows: &[FeatureRowJson]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let visible: Vec<&FeatureRowJson> = rows.iter().filter(|row| !row.stale_proposed).collect();
    let stale_count = rows.len() - visible.len();
    let mut out = String::from("ACTIVE FEATURES\n");
    if !visible.is_empty() {
        let table_rows: Vec<Vec<String>> = visible
            .iter()
            .map(|row| {
                vec![
                    row.id.clone(),
                    row.state.clone(),
                    row.next.clone(),
                    row.title.clone(),
                ]
            })
            .collect();
        out.push_str(&table::render_table(
            &["FEATURE", "STATE", "NEXT", "TITLE"],
            &table_rows,
        ));
        out.push_str("inspect any: maestro feature show <id>\n");
    }
    if stale_count > 0 {
        out.push_str(&format!(
            "({stale_count} proposed stale hidden; feature list --all to review)\n"
        ));
        out.push_str(feature::RETIRE_REMINDER);
        out.push('\n');
    }
    out
}

#[derive(Clone, Debug, Serialize)]
struct StatusReport {
    schema: String,
    status: String,
    repo: String,
    current_task: Option<String>,
    current_feature: Option<String>,
    /// Working-tree git readout; `None` when not a git repository or on the
    /// `task next` surface, which does not render it.
    #[serde(skip_serializing_if = "Option::is_none")]
    git: Option<GitReadout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge_lock_holder: Option<String>,
    /// Whether the next verb is close/verify-shaped; drives the clean-worktree
    /// note. Render-only, not part of the serialized contract.
    #[serde(skip)]
    close_or_verify_pending: bool,
    /// Concern-only proof repair line for the focal (next-action) task;
    /// render-only, not part of the serialized contract. `None` when the proof
    /// needs no action.
    #[serde(skip)]
    proof_concern: Option<String>,
    warnings: Vec<WarningJson>,
    next_action: Option<NextAction>,
    tasks: TaskSummaryJson,
    features: FeatureSummaryJson,
    task_rows: Vec<TaskRowJson>,
    active_features: Vec<FeatureRowJson>,
    harness_friction: Vec<HarnessFrictionJson>,
    audit_hint: Option<AuditHintJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    approved_memory: Vec<ApprovedMemory>,
    #[serde(skip_serializing_if = "is_zero")]
    approved_memory_omitted: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    memory_suggestions: Vec<MemorySuggestionHint>,
    #[serde(skip_serializing_if = "is_zero")]
    memory_suggestions_omitted: usize,
    sections: StatusSectionsJson,
    ready_to_close_features: Vec<ReadyFeatureJson>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct StatusSectionsJson {
    ready_to_close: Vec<ReadyFeatureJson>,
}

impl StatusReport {
    fn not_initialized(repo: PathBuf, reason: String) -> Self {
        Self {
            schema: "maestro.status.v1".to_string(),
            status: "not_initialized".to_string(),
            repo: repo.display().to_string(),
            current_task: None,
            current_feature: None,
            git: None,
            merge_lock_holder: None,
            close_or_verify_pending: false,
            proof_concern: None,
            warnings: vec![WarningJson {
                code: "not_initialized".to_string(),
                message: reason,
            }],
            next_action: Some(NextAction::repo(
                "init_maestro",
                runnable_command(["maestro", "init", "--yes"]),
                "maestro is not initialized in this repo",
            )),
            tasks: TaskSummaryJson::default(),
            features: FeatureSummaryJson::default(),
            task_rows: Vec::new(),
            active_features: Vec::new(),
            harness_friction: Vec::new(),
            audit_hint: None,
            approved_memory: Vec::new(),
            approved_memory_omitted: 0,
            memory_suggestions: Vec::new(),
            memory_suggestions_omitted: 0,
            sections: StatusSectionsJson::default(),
            ready_to_close_features: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct NextJson {
    schema: String,
    mode: String,
    status: String,
    repo: String,
    warnings: Vec<WarningJson>,
    next_action: Option<NextAction>,
    harness_friction: Vec<HarnessFrictionJson>,
    audit_hint: Option<AuditHintJson>,
    ready_to_close_features: Vec<ReadyFeatureJson>,
}

impl NextJson {
    fn from_report(report: &StatusReport, mode: &str) -> Self {
        Self {
            schema: "maestro.next.v1".to_string(),
            mode: mode.to_string(),
            status: report.status.clone(),
            repo: report.repo.clone(),
            warnings: report.warnings.clone(),
            next_action: report.next_action.clone(),
            harness_friction: report.harness_friction.clone(),
            audit_hint: report.audit_hint.clone(),
            ready_to_close_features: report.ready_to_close_features.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct NextRunJson {
    schema: String,
    mode: String,
    status: String,
    actions_taken: Vec<String>,
    blocker: Option<String>,
    next_action: Option<NextAction>,
}

impl NextRunJson {
    fn done(mode: &str, actions_taken: Vec<String>) -> Self {
        Self {
            schema: "maestro.next.v1".to_string(),
            mode: mode.to_string(),
            status: "done".to_string(),
            actions_taken,
            blocker: None,
            next_action: None,
        }
    }

    fn blocked(mode: &str, actions_taken: Vec<String>, action: &NextAction, blocker: &str) -> Self {
        Self {
            schema: "maestro.next.v1".to_string(),
            mode: mode.to_string(),
            status: "blocked".to_string(),
            actions_taken,
            blocker: Some(blocker.to_string()),
            next_action: Some(action.clone()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct TaskNextJson {
    schema: String,
    status: String,
    next_action: Option<NextAction>,
    harness_friction: Vec<HarnessFrictionJson>,
    audit_hint: Option<AuditHintJson>,
    broader_actions: Vec<BroaderActionJson>,
    warnings: Vec<WarningJson>,
    summary: String,
}

#[derive(Clone, Debug, Serialize)]
struct BroaderActionJson {
    kind: String,
    feature_id: String,
    summary: String,
    inspect: String,
}

impl From<&ReadyFeatureJson> for BroaderActionJson {
    fn from(feature: &ReadyFeatureJson) -> Self {
        Self {
            kind: "feature_ready_to_close".to_string(),
            feature_id: feature.id.clone(),
            summary: "ready-to-close feature available in status".to_string(),
            inspect: format!("maestro feature show {}", feature.id),
        }
    }
}

impl From<&StatusReport> for TaskNextJson {
    fn from(report: &StatusReport) -> Self {
        let warnings = report.warnings.clone();
        let broader_actions = if report.next_action.is_none()
            && report.harness_friction.is_empty()
            && report.audit_hint.is_none()
        {
            report
                .ready_to_close_features
                .iter()
                .map(BroaderActionJson::from)
                .collect()
        } else {
            Vec::new()
        };
        Self {
            schema: "maestro.task_next.v1".to_string(),
            status: if report.next_action.is_some()
                || !report.harness_friction.is_empty()
                || report.audit_hint.is_some()
            {
                "actionable".to_string()
            } else {
                "no_action".to_string()
            },
            next_action: report.next_action.clone(),
            harness_friction: report.harness_friction.clone(),
            audit_hint: report.audit_hint.clone(),
            broader_actions,
            warnings,
            summary: if report.next_action.is_some()
                || !report.harness_friction.is_empty()
                || report.audit_hint.is_some()
            {
                "task action available".to_string()
            } else {
                "no actionable tasks".to_string()
            },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct AuditHintJson {
    sessions_since_audit: usize,
    every_sessions: usize,
    skill: String,
}

impl From<harness::AuditHint> for AuditHintJson {
    fn from(hint: harness::AuditHint) -> Self {
        Self {
            sessions_since_audit: hint.sessions_since_audit,
            every_sessions: hint.every_sessions,
            skill: "maestro-audit".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct HarnessFrictionJson {
    id: String,
    item_type: String,
    title: String,
    priority: String,
    occurrences: usize,
    sessions: usize,
}

impl From<harness::OverThresholdItem> for HarnessFrictionJson {
    fn from(item: harness::OverThresholdItem) -> Self {
        Self {
            id: item.id,
            item_type: item.item_type,
            title: item.title,
            priority: item.priority,
            occurrences: item.occurrences,
            sessions: item.sessions,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct NextAction {
    kind: String,
    scope: String,
    task_id: Option<String>,
    feature_id: Option<String>,
    title: Option<String>,
    command: CommandJson,
    auto_safe: bool,
    runnable: bool,
    requires_input: bool,
    reason: String,
    inspect: Option<String>,
}

impl NextAction {
    fn task(kind: &str, task: &TaskRecord, command: CommandJson, reason: &str) -> Self {
        let runnable = command.argv.is_some();
        let requires_input = !command.requires_input.is_empty();
        Self {
            kind: kind.to_string(),
            scope: "task".to_string(),
            task_id: Some(task.id.clone()),
            feature_id: task.feature_id.clone(),
            title: Some(task.title.clone()),
            command,
            auto_safe: kind == "claim_task",
            runnable,
            requires_input,
            reason: reason.to_string(),
            inspect: Some(format!("maestro task show {}", task.id)),
        }
    }

    fn repo(kind: &str, command: CommandJson, reason: &str) -> Self {
        let runnable = command.argv.is_some();
        let requires_input = !command.requires_input.is_empty();
        Self {
            kind: kind.to_string(),
            scope: "repo".to_string(),
            task_id: None,
            feature_id: None,
            title: None,
            command,
            auto_safe: false,
            runnable,
            requires_input,
            reason: reason.to_string(),
            inspect: None,
        }
    }

    fn feature_close(view: &feature::FeatureView) -> Self {
        let command = feature_close_template(&view.id);
        let runnable = command.argv.is_some();
        let requires_input = !command.requires_input.is_empty();
        Self {
            kind: "feature_close".to_string(),
            scope: "feature".to_string(),
            task_id: None,
            feature_id: Some(view.id.clone()),
            title: Some(view.title.clone()),
            command,
            auto_safe: false,
            runnable,
            requires_input,
            reason: "feature has no open child task work".to_string(),
            inspect: Some(format!("maestro feature show {}", view.id)),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct CommandJson {
    display: String,
    argv: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    argv_template: Option<Vec<String>>,
    requires_input: Vec<RequiredInputJson>,
}

#[derive(Clone, Debug, Serialize)]
struct RequiredInputJson {
    name: String,
    flag: String,
    placeholder: String,
    description: String,
}

fn runnable_command<const N: usize>(parts: [&str; N]) -> CommandJson {
    CommandJson {
        display: parts.join(" "),
        argv: Some(parts.iter().map(|part| (*part).to_string()).collect()),
        argv_template: None,
        requires_input: Vec::new(),
    }
}

fn task_check_template(task_id: &str) -> CommandJson {
    template_command(
        format!("maestro task set {task_id} --check \"<observable result>\""),
        vec![
            "maestro",
            "task",
            "set",
            task_id,
            "--check",
            "<observable result>",
        ],
        vec![required_input(
            "observable_result",
            "--check",
            "<observable result>",
            "observable acceptance check text",
        )],
    )
}

fn task_complete_template(task_id: &str) -> CommandJson {
    template_command(
        format!(
            "maestro task complete {task_id} --summary \"<summary>\" --claim \"<claim>\" --proof \"<observed evidence>\""
        ),
        vec![
            "maestro",
            "task",
            "complete",
            task_id,
            "--summary",
            "<summary>",
            "--claim",
            "<claim>",
            "--proof",
            "<observed evidence>",
        ],
        vec![
            required_input("summary", "--summary", "<summary>", "what changed"),
            required_input("claim", "--claim", "<claim>", "observable completion claim"),
            required_input(
                "proof",
                "--proof",
                "<observed evidence>",
                "observed proof text",
            ),
        ],
    )
}

fn feature_close_template(feature_id: &str) -> CommandJson {
    template_command(
        format!("maestro feature close {feature_id} --outcome \"<outcome>\""),
        vec![
            "maestro",
            "feature",
            "close",
            feature_id,
            "--outcome",
            "<outcome>",
        ],
        vec![required_input(
            "outcome",
            "--outcome",
            "<outcome>",
            "closing outcome text",
        )],
    )
}

fn template_command(
    display: String,
    argv_template: Vec<&str>,
    inputs: Vec<RequiredInputJson>,
) -> CommandJson {
    CommandJson {
        display,
        argv: None,
        argv_template: Some(
            argv_template
                .into_iter()
                .map(|part| part.to_string())
                .collect(),
        ),
        requires_input: inputs,
    }
}

fn required_input(
    name: &str,
    flag: &str,
    placeholder: &str,
    description: &str,
) -> RequiredInputJson {
    RequiredInputJson {
        name: name.to_string(),
        flag: flag.to_string(),
        placeholder: placeholder.to_string(),
        description: description.to_string(),
    }
}

#[derive(Clone, Debug, Serialize)]
struct WarningJson {
    code: String,
    message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
struct TaskSummaryJson {
    total: usize,
    active: usize,
    ready: usize,
    needs_verification: usize,
    blocked: usize,
    verified: usize,
}

impl TaskSummaryJson {
    /// Tally the open buckets from the card graph through the same
    /// [`query::classify`] the `maestro watch` board uses, so the two never
    /// disagree on `active`/`ready`/`needs_verification`/`blocked`. `total` and
    /// `verified` are repo-wide over workable cards (not the board's
    /// open-feature subset), so they are not expected to match a board frame.
    fn from_cards(cards: &[card::schema::Card], blocked_ids: &BTreeSet<String>) -> Self {
        let mut summary = Self::default();
        for card in cards.iter().filter(|card| card.card_type.workable()) {
            summary.total += 1;
            if card.status == "verified" {
                summary.verified += 1;
            }
        }
        let counts = card::query::RowStateCounts::from_cards(cards.iter(), blocked_ids);
        summary.active = counts.active;
        summary.ready = counts.ready;
        summary.needs_verification = counts.needs_verification;
        summary.blocked = counts.blocked;
        summary
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct FeatureSummaryJson {
    total: usize,
    active: usize,
    closed: usize,
    cancelled: usize,
}

impl FeatureSummaryJson {
    fn from_features(features: &[feature::FeatureView]) -> Self {
        Self {
            total: features.len(),
            active: features
                .iter()
                .filter(|view| !view.status.is_terminal())
                .count(),
            closed: features
                .iter()
                .filter(|view| view.status == FeatureStatus::Closed)
                .count(),
            cancelled: features
                .iter()
                .filter(|view| view.status == FeatureStatus::Cancelled)
                .count(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct TaskRowJson {
    id: String,
    state: String,
    title: String,
    next: String,
    inspect: String,
    project: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct FeatureRowJson {
    id: String,
    state: String,
    title: String,
    next: String,
    inspect: String,
    project: Option<String>,
    /// Render-only: a stale proposed feature is collapsed out of the human
    /// table. Skipped from JSON so `status --json` still carries every row.
    #[serde(skip)]
    stale_proposed: bool,
}

#[derive(Clone, Debug, Serialize)]
struct ReadyFeatureJson {
    id: String,
    feature_id: String,
    title: String,
    total: usize,
    verified: usize,
    next_action: NextAction,
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-06-21T00:00:00.000Z";

    fn now() -> i128 {
        timestamp_nanos(NOW).expect("fixed now parses")
    }

    fn proposed_view(id: &str, updated_at: &str) -> feature::FeatureView {
        feature::FeatureView {
            id: id.to_string(),
            title: format!("{id} title"),
            status: FeatureStatus::Proposed,
            counts: feature::query::FeatureTaskCounts::default(),
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
            description: None,
            raw_request: None,
            input_type: None,
            acceptance: Vec::new(),
            acceptance_coverage: None,
            affected_areas: Vec::new(),
            non_goals: Vec::new(),
            open_questions: Vec::new(),
            outcome: None,
            cancel_reason: None,
            qa_none_reason: None,
            notes: None,
            project: None,
        }
    }

    fn test_paths() -> MaestroPaths {
        MaestroPaths::new(PathBuf::from("."))
    }

    #[test]
    fn injected_now_collapses_stale_proposed_keeps_fresh() {
        let views = vec![
            proposed_view("stale-old", "2026-06-01T00:00:00.000Z"), // 20d -> stale
            proposed_view("fresh-new", "2026-06-20T00:00:00.000Z"), // 1d  -> fresh
        ];
        let paths = test_paths();
        let rows = active_feature_rows(&paths, &views, now());
        assert_eq!(
            rows.len(),
            2,
            "render-only collapse keeps every row in the data"
        );
        let block = active_features_block(&rows);
        assert!(
            block.contains("fresh-new"),
            "fresh proposed renders as a row"
        );
        assert!(
            !block.contains("stale-old"),
            "stale proposed is collapsed out"
        );
        assert!(block.contains("(1 proposed stale hidden; feature list --all to review)"));
        assert!(block.contains(feature::RETIRE_REMINDER));
        assert!(block.contains("inspect any: maestro feature show <id>"));
    }

    #[test]
    fn no_stale_block_carries_no_collapse_or_reminder() {
        let views = vec![proposed_view("fresh", "2026-06-20T00:00:00.000Z")];
        let paths = test_paths();
        let block = active_features_block(&active_feature_rows(&paths, &views, now()));
        assert!(block.contains("fresh"));
        assert!(!block.contains("proposed stale hidden"));
        assert!(!block.contains(feature::RETIRE_REMINDER));
    }

    #[test]
    fn all_stale_block_collapses_with_no_table_or_inspect_line() {
        let views = vec![
            proposed_view("alpha", "2026-06-01T00:00:00.000Z"),
            proposed_view("beta", "2026-05-20T00:00:00.000Z"),
        ];
        let paths = test_paths();
        let block = active_features_block(&active_feature_rows(&paths, &views, now()));
        assert!(block.contains("(2 proposed stale hidden; feature list --all to review)"));
        assert!(block.contains(feature::RETIRE_REMINDER));
        assert!(
            !block.contains("inspect any"),
            "no visible rows -> no inspect line"
        );
        assert!(!block.contains("alpha title") && !block.contains("beta title"));
    }

    #[test]
    fn retire_reminder_is_one_const_line_regardless_of_stale_count() {
        // ac-4: the reminder is a single const, byte-identical no matter how
        // many proposed features are collapsed -- it never moves into a loop.
        let one = vec![proposed_view("a", "2026-06-01T00:00:00.000Z")];
        let many: Vec<feature::FeatureView> = (0..5)
            .map(|i| proposed_view(&format!("f{i}"), "2026-06-01T00:00:00.000Z"))
            .collect();
        let paths = test_paths();
        let block_one = active_features_block(&active_feature_rows(&paths, &one, now()));
        let block_many = active_features_block(&active_feature_rows(&paths, &many, now()));
        assert_eq!(block_one.matches(feature::RETIRE_REMINDER).count(), 1);
        assert_eq!(block_many.matches(feature::RETIRE_REMINDER).count(), 1);
    }
}
