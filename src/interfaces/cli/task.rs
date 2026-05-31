use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::domain::feature::{self, FeatureStatus};
use crate::domain::task;
use crate::domain::task::{BlockerTarget, TaskRecord, TaskState, TransitionDetails};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};
use crate::foundation::core::time::nanos_since_epoch_string;
use crate::interfaces::cli::task_id::resolve_optional_task_id;
use crate::interfaces::cli::verify;
use crate::interfaces::cli::{TaskArgs, TaskCommand};
use crate::interfaces::tui::task_list_watch;

/// Execute `maestro task`.
pub fn run(args: TaskArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    ensure_dir(paths.tasks_dir())?;
    let actor = super::actor();

    match args.command {
        TaskCommand::Create {
            title,
            feature,
            lane,
            risk,
        } => create_task(&paths, &title, feature, lane, risk),
        TaskCommand::Set {
            id,
            check,
            feature,
            no_feature,
        } => set_task(&paths, &id, check, feature, no_feature, &actor),
        TaskCommand::Explore { id } => transition_task(
            &paths,
            &id,
            TaskState::Exploring,
            &actor,
            TransitionDetails::default(),
        ),
        TaskCommand::Accept { id } => accept_task(&paths, &id, &actor),
        TaskCommand::Claim { id } => claim_task(&paths, &id, &actor),
        TaskCommand::Complete { id, summary, claim } => transition_task(
            &paths,
            &id,
            TaskState::NeedsVerification,
            &actor,
            TransitionDetails {
                summary: Some(summary),
                claims: vec![claim],
                ..TransitionDetails::default()
            },
        ),
        TaskCommand::Verify { id } => {
            let id = resolve_optional_task_id(
                &paths,
                id,
                "task id is required or set MAESTRO_CURRENT_TASK",
            )?;
            verify::run_for_task(&paths, &id, &actor)
        }
        TaskCommand::Update { id, summary, claim } => {
            update_task(&paths, &id, summary, claim, &actor)
        }
        TaskCommand::Block { id, reason, by } => block_task(&paths, &id, &reason, by, &actor),
        TaskCommand::Unblock { id, blocker } => unblock_task(&paths, &id, &blocker, &actor),
        TaskCommand::Reject { id, reason } => transition_task(
            &paths,
            &id,
            TaskState::Rejected,
            &actor,
            TransitionDetails {
                summary: Some(reason),
                ..TransitionDetails::default()
            },
        ),
        TaskCommand::Abandon { id, reason } => transition_task(
            &paths,
            &id,
            TaskState::Abandoned,
            &actor,
            TransitionDetails {
                summary: Some(reason),
                ..TransitionDetails::default()
            },
        ),
        TaskCommand::Supersede { id, by, reason } => {
            supersede_task(&paths, &id, &by, &reason, &actor)
        }
        TaskCommand::Show { id } => show_task(&paths, id),
        TaskCommand::List {
            blocked,
            blocked_by,
            blocks,
            feature,
            ready,
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
                watch,
                interval,
            },
        ),
        TaskCommand::Watch { id, interval } => watch_tasks(&paths, id, interval),
        TaskCommand::Doctor => doctor_tasks(&paths),
    }
}

fn create_task(
    paths: &MaestroPaths,
    title: &str,
    feature: Option<String>,
    lane: Option<String>,
    risk: Option<String>,
) -> Result<()> {
    let now = nanos_since_epoch_string();
    let task = task::create_task(&paths.tasks_dir(), title, feature, lane, risk, &now)?;

    println!("created {}", task.id);
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
        let task = task::set_checks(&paths.tasks_dir(), id, checks)?;
        println!("updated {} checks", task.id);
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
    if let Some(current) = task.feature_id.as_deref() {
        // A dangling current link (feature unreadable) is permissive so the
        // task can be re-pointed or detached to repair it; only a resolved
        // terminal feature freezes the link as history.
        if let Some(status) = feature::show(paths, current).ok().map(|view| view.status) {
            if is_terminal_feature(&status) {
                bail!(
                    "task {id} is linked to feature {current} ({}); its link is settled history and cannot change",
                    feature::status_label(&status)
                );
            }
        }
    }
    if let Some(target) = target {
        let view = feature::show(paths, target).with_context(|| {
            format!("target feature `{target}` not found; create it with `maestro feature new`")
        })?;
        if is_terminal_feature(&view.status) {
            bail!(
                "target feature {target} is {}; tasks cannot be attached to a terminal feature",
                feature::status_label(&view.status)
            );
        }
    }
    Ok(())
}

fn is_terminal_feature(status: &FeatureStatus) -> bool {
    matches!(status, FeatureStatus::Shipped | FeatureStatus::Cancelled)
}

fn accept_task(paths: &MaestroPaths, id: &str, actor: &str) -> Result<()> {
    let now = nanos_since_epoch_string();
    let task = task::accept_task(&paths.tasks_dir(), id, actor, &now)?;

    println!("accepted {}", task.id);
    Ok(())
}

fn claim_task(paths: &MaestroPaths, id: &str, actor: &str) -> Result<()> {
    let now = nanos_since_epoch_string();
    let task = task::claim_task(&paths.tasks_dir(), id, actor, &now)?;
    println!("updated {} -> {}", task.id, task.state.as_str());
    Ok(())
}

fn transition_task(
    paths: &MaestroPaths,
    id: &str,
    to: TaskState,
    actor: &str,
    details: TransitionDetails,
) -> Result<()> {
    let now = nanos_since_epoch_string();
    let task = task::transition_task(&paths.tasks_dir(), id, to, actor, &now, details)?;
    println!("updated {} -> {}", task.id, task.state.as_str());
    Ok(())
}

fn supersede_task(
    paths: &MaestroPaths,
    id: &str,
    by: &str,
    reason: &str,
    actor: &str,
) -> Result<()> {
    let now = nanos_since_epoch_string();
    let task = task::supersede_task(&paths.tasks_dir(), id, by, reason, actor, &now)?;
    println!("updated {} -> {}", task.id, task.state.as_str());
    Ok(())
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
        bail!("task update requires --summary or --claim");
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
    let task_id = match id {
        Some(id) => id,
        None => std::env::var("MAESTRO_CURRENT_TASK")
            .context("task id is required or set MAESTRO_CURRENT_TASK for `maestro task show`")?,
    };
    let task = task::load_task_record(&paths.tasks_dir(), &task_id)?;
    print!("{}", task::render_task(&task));
    Ok(())
}

struct TaskListFilters {
    blocked: bool,
    blocked_by: Option<String>,
    blocks: Option<String>,
    feature: Option<String>,
    ready: bool,
    watch: bool,
    interval: Option<u64>,
}

fn list_tasks(paths: &MaestroPaths, filters: TaskListFilters) -> Result<()> {
    if filters.watch {
        return task_list_watch::run(paths, filters.interval.unwrap_or(2), || {
            filtered_tasks(paths, &filters)
        });
    }

    let tasks = filtered_tasks(paths, &filters)?;
    print!("{}", task::render_task_list(&tasks));
    Ok(())
}

fn watch_tasks(paths: &MaestroPaths, id: Option<String>, interval: Option<u64>) -> Result<()> {
    task_list_watch::run(paths, interval.unwrap_or(2), || {
        let mut tasks = load_all_tasks(&paths.tasks_dir())?;
        if let Some(id) = id.as_deref() {
            tasks.retain(|task| task.id == id);
        }
        Ok(tasks)
    })
}

fn filtered_tasks(paths: &MaestroPaths, filters: &TaskListFilters) -> Result<Vec<TaskRecord>> {
    let tasks = load_all_tasks(&paths.tasks_dir())?;
    Ok(task::filter_tasks(
        tasks,
        &task::TaskFilter {
            ready: filters.ready,
            blocked: filters.blocked,
            blocked_by: filters.blocked_by.clone(),
            blocks: filters.blocks.clone(),
            feature_id: filters.feature.clone(),
            claimed_by: None,
        },
    ))
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

fn load_all_tasks(tasks_dir: &Path) -> Result<Vec<TaskRecord>> {
    task::load_task_records(tasks_dir)
}

fn blocker_target(by: Option<String>) -> BlockerTarget {
    match by {
        Some(by) if by.starts_with("task-") => BlockerTarget::Task(by),
        Some(by) if by.starts_with("decision-") => BlockerTarget::Decision(by),
        Some(by) => BlockerTarget::External(by),
        None => BlockerTarget::Human,
    }
}
