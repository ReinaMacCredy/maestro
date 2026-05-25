use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};

use crate::commands::{TaskArgs, TaskCommand};
use crate::core::fs::ensure_dir;
use crate::core::paths::{discover_repo_root, MaestroPaths};
use crate::core::safe_write::write_string_atomic;
use crate::task::blockers::{add_blocker, has_unresolved_blockers, resolve_blocker};
use crate::task::display::{render_task, render_task_list};
use crate::task::doctor::{check_blocker_graph, load_task_records, render_report};
use crate::task::lifecycle::{transition, TransitionDetails};
use crate::task::lookup::load_task_with_snapshot as load_task_artifacts_with_snapshot;
use crate::task::template::{
    save_task_with_snapshot, write_task_artifacts, AcceptanceFile, BlockerKind, BlockerRef,
    TaskRecord, TaskState,
};
use crate::verification::verify_task::{verify_task, VerificationStatus};

/// Execute `maestro task`.
pub fn run(args: TaskArgs) -> Result<()> {
    let repo_root = discover_repo_root()?;
    let paths = MaestroPaths::new(repo_root);
    ensure_dir(paths.tasks_dir())?;
    let actor = actor();

    match args.command {
        TaskCommand::Create {
            title,
            feature,
            lane,
            risk,
        } => create_task(&paths, &title, feature, lane, risk),
        TaskCommand::Explore { id } => transition_task(
            &paths,
            &id,
            TaskState::Exploring,
            &actor,
            TransitionDetails::default(),
        ),
        TaskCommand::Accept { id } => accept_task(&paths, &id, &actor),
        TaskCommand::Claim { id } => transition_task(
            &paths,
            &id,
            TaskState::InProgress,
            &actor,
            TransitionDetails::default(),
        ),
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
        TaskCommand::Verify { id } => verify_task_command(&paths, &id, &actor),
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
        TaskCommand::Supersede { id, by, reason } => transition_task(
            &paths,
            &id,
            TaskState::Superseded,
            &actor,
            TransitionDetails {
                to: Some(by),
                summary: Some(reason),
                ..TransitionDetails::default()
            },
        ),
        TaskCommand::Show { id } => show_task(&paths, id),
        TaskCommand::List {
            blocked,
            blocked_by,
            blocks,
            feature,
            ready,
            watch,
        } => list_tasks(&paths, blocked, blocked_by, blocks, feature, ready, watch),
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
    let id = next_task_id(&paths.tasks_dir())?;
    let now = timestamp();
    let mut task = TaskRecord::draft(&id, title, &now);
    task.feature_id = feature;
    if let Some(lane) = lane {
        task.lane = Some(lane);
    }
    if let Some(risk) = risk {
        task.risk = Some(risk);
    }
    let acceptance = AcceptanceFile::new(&id, Vec::new());
    write_task_artifacts(&paths.tasks_dir(), &task, &acceptance)?;

    println!("created {id}");
    Ok(())
}

fn accept_task(paths: &MaestroPaths, id: &str, actor: &str) -> Result<()> {
    let now = timestamp();
    let (mut task, snapshot, task_dir) = load_task_with_snapshot(paths, id)?;
    task.acceptance_locked = true;
    transition(
        &mut task,
        TaskState::Ready,
        actor,
        &now,
        TransitionDetails::default(),
    )?;
    save_task_with_snapshot(&task, &snapshot)?;
    lock_acceptance(task_dir.join("acceptance.yaml"), &task.id, actor, &now)?;

    println!("accepted {}", task.id);
    Ok(())
}

fn transition_task(
    paths: &MaestroPaths,
    id: &str,
    to: TaskState,
    actor: &str,
    details: TransitionDetails,
) -> Result<()> {
    let now = timestamp();
    let (mut task, snapshot, _) = load_task_with_snapshot(paths, id)?;
    transition(&mut task, to, actor, &now, details)?;
    save_task_with_snapshot(&task, &snapshot)?;
    println!("updated {} -> {}", task.id, state_name(&task.state));
    Ok(())
}

fn block_task(
    paths: &MaestroPaths,
    id: &str,
    reason: &str,
    by: Option<String>,
    actor: &str,
) -> Result<()> {
    let now = timestamp();
    let (mut task, snapshot, _) = load_task_with_snapshot(paths, id)?;
    let blocker_id = next_blocker_id(&task);
    let (kind, blocked_ref, title) = blocker_descriptor(by);
    add_blocker(
        &mut task,
        blocker_id.clone(),
        kind,
        blocked_ref,
        title,
        reason.to_string(),
        now.clone(),
    );
    task.state_history
        .push(crate::task::template::StateHistoryEntry {
            state: task.state.clone(),
            at: now.clone(),
            by: actor.to_string(),
            to: None,
            summary: Some(format!("blocker added: {blocker_id}")),
            claims: Vec::new(),
            open_items: Vec::new(),
        });
    save_task_with_snapshot(&task, &snapshot)?;

    println!("blocked {} ({blocker_id})", task.id);
    Ok(())
}

fn unblock_task(paths: &MaestroPaths, id: &str, blocker_id: &str, actor: &str) -> Result<()> {
    let now = timestamp();
    let (mut task, snapshot, _) = load_task_with_snapshot(paths, id)?;
    resolve_blocker(&mut task, blocker_id, now.clone())?;
    task.state_history
        .push(crate::task::template::StateHistoryEntry {
            state: task.state.clone(),
            at: now,
            by: actor.to_string(),
            to: None,
            summary: Some(format!("blocker resolved: {blocker_id}")),
            claims: Vec::new(),
            open_items: Vec::new(),
        });
    save_task_with_snapshot(&task, &snapshot)?;

    println!("unblocked {} ({blocker_id})", task.id);
    Ok(())
}

fn verify_task_command(paths: &MaestroPaths, id: &str, actor: &str) -> Result<()> {
    let report = verify_task(paths, id, actor)?;
    match report.status {
        VerificationStatus::Passed => {
            println!(
                "verification passed for {} ({} claim(s), {} proof source(s))",
                report.task_id,
                report.claims.len(),
                report.proof_sources.len()
            );
            Ok(())
        }
        VerificationStatus::Failed => {
            for failure in &report.failures {
                eprintln!("verification failure: {failure}");
            }
            bail!("verification failed for {}", report.task_id)
        }
    }
}

fn show_task(paths: &MaestroPaths, id: Option<String>) -> Result<()> {
    let task_id = match id {
        Some(id) => id,
        None => std::env::var("MAESTRO_CURRENT_TASK")
            .context("task id is required or set MAESTRO_CURRENT_TASK for `maestro task show`")?,
    };
    let (task, _, _) = load_task_with_snapshot(paths, &task_id)?;
    print!("{}", render_task(&task));
    Ok(())
}

fn list_tasks(
    paths: &MaestroPaths,
    blocked: bool,
    blocked_by: Option<String>,
    blocks: Option<String>,
    feature: Option<String>,
    ready: bool,
    watch: bool,
) -> Result<()> {
    let mut tasks = load_all_tasks(&paths.tasks_dir())?;
    let task_map: HashMap<String, TaskRecord> = tasks
        .iter()
        .cloned()
        .map(|task| (task.id.clone(), task))
        .collect();

    if blocked {
        tasks.retain(has_unresolved_blockers);
    }
    if let Some(feature) = feature {
        tasks.retain(|task| task.feature_id.as_deref() == Some(feature.as_str()));
    }
    if ready {
        tasks.retain(|task| task.state == TaskState::Ready && !has_unresolved_blockers(task));
    }
    if let Some(blocked_by_id) = blocked_by {
        tasks.retain(|task| {
            task.blockers.iter().any(|blocker| {
                blocker.resolved_at.is_none()
                    && blocker
                        .blocked_ref
                        .as_ref()
                        .map(|r| r.id.as_str() == blocked_by_id.as_str())
                        .unwrap_or(false)
            })
        });
    }
    if let Some(task_id) = blocks {
        let blocking_ids = task_map
            .get(&task_id)
            .map(|task| {
                task.blockers
                    .iter()
                    .filter(|blocker| blocker.resolved_at.is_none())
                    .filter_map(|blocker| blocker.blocked_ref.as_ref())
                    .filter(|blocked_ref| blocked_ref.kind == BlockerKind::Task)
                    .map(|blocked_ref| blocked_ref.id.clone())
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        tasks.retain(|task| blocking_ids.contains(&task.id));
    }

    tasks.sort_by(|left, right| left.id.cmp(&right.id));
    print!("{}", render_task_list(&tasks));
    if watch {
        println!("watch mode is not implemented in this phase slice; rendered one snapshot");
    }
    Ok(())
}

fn doctor_tasks(paths: &MaestroPaths) -> Result<()> {
    let report = check_blocker_graph(&paths.tasks_dir())?;
    let rendered = render_report(&report);
    if report.is_ok() {
        print!("{rendered}");
        return Ok(());
    }

    for line in rendered.lines() {
        eprintln!("{line}");
    }
    bail!("task doctor found {} error(s)", report.errors.len())
}

fn lock_acceptance(path: PathBuf, task_id: &str, actor: &str, locked_at: &str) -> Result<()> {
    let acceptance = if path.exists() {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        serde_yaml::from_str::<AcceptanceFile>(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?
    } else {
        AcceptanceFile::new(task_id, Vec::new())
    };

    let locked = AcceptanceFile {
        locked_by: Some(actor.to_string()),
        locked_at: Some(locked_at.to_string()),
        ..acceptance
    };
    write_string_atomic(&path, &serde_yaml::to_string(&locked)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn load_all_tasks(tasks_dir: &Path) -> Result<Vec<TaskRecord>> {
    load_task_records(tasks_dir)
}

fn load_task_with_snapshot(
    paths: &MaestroPaths,
    id: &str,
) -> Result<(TaskRecord, crate::task::template::TaskSnapshot, PathBuf)> {
    load_task_artifacts_with_snapshot(&paths.tasks_dir(), id)
}

fn next_task_id(tasks_dir: &Path) -> Result<String> {
    let mut max = 0_u32;
    if tasks_dir.is_dir() {
        for entry in fs::read_dir(tasks_dir)
            .with_context(|| format!("failed to read {}", tasks_dir.display()))?
        {
            let entry = entry.with_context(|| format!("failed to list {}", tasks_dir.display()))?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if let Some(num) = name
                .strip_prefix("task-")
                .and_then(|rest| rest.split('-').next())
                .and_then(|value| value.parse::<u32>().ok())
            {
                max = max.max(num);
            }
        }
    }
    Ok(format!("task-{:03}", max + 1))
}

fn next_blocker_id(task: &TaskRecord) -> String {
    let max = task
        .blockers
        .iter()
        .filter_map(|blocker| blocker.id.strip_prefix("blk-"))
        .filter_map(|id| id.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("blk-{:03}", max + 1)
}

fn blocker_descriptor(by: Option<String>) -> (BlockerKind, Option<BlockerRef>, String) {
    match by {
        Some(by) if by.starts_with("task-") => (
            BlockerKind::Task,
            Some(BlockerRef {
                kind: BlockerKind::Task,
                id: by.clone(),
            }),
            format!("Blocked by {by}"),
        ),
        Some(by) if by.starts_with("decision-") => (
            BlockerKind::Decision,
            Some(BlockerRef {
                kind: BlockerKind::Decision,
                id: by.clone(),
            }),
            format!("Blocked by {by}"),
        ),
        Some(by) => (BlockerKind::External, None, format!("Blocked by {by}")),
        None => (BlockerKind::Human, None, "Manual block".to_string()),
    }
}

fn state_name(state: &TaskState) -> &'static str {
    match state {
        TaskState::Draft => "draft",
        TaskState::Exploring => "exploring",
        TaskState::Ready => "ready",
        TaskState::InProgress => "in_progress",
        TaskState::NeedsVerification => "needs_verification",
        TaskState::Verified => "verified",
        TaskState::Rejected => "rejected",
        TaskState::Abandoned => "abandoned",
        TaskState::Superseded => "superseded",
    }
}

fn actor() -> String {
    std::env::var("MAESTRO_ACTOR").unwrap_or_else(|_| "maestro".to_string())
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
