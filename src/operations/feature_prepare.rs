//! Feature preparation operation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::feature::{self, FeatureStatus};
use crate::domain::task::{self, BlockerTarget, TaskRecord, TaskState, TransitionDetails};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::time::nanos_since_epoch_string;

pub(crate) const AFTER_DEPENDENCY_REASON_PREFIX: &str = "after dependency:";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DraftReport {
    pub path: PathBuf,
    pub written: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareReport {
    pub feature_id: String,
    pub task_count: usize,
    pub ready_count: usize,
    pub blocked_count: usize,
    pub started: bool,
    pub remained_ready: bool,
    pub prepared: Vec<PreparedTask>,
    pub blockers: Vec<PreparedBlocker>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedTask {
    pub id: String,
    pub title: String,
    pub blocked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedBlocker {
    pub task_id: String,
    pub blocker_id: String,
    pub reason: String,
}

#[derive(Debug)]
struct PlanTask {
    local_id: Option<String>,
    title: String,
    checks: Vec<String>,
    blockers: Vec<String>,
    after: Vec<String>,
}

/// Write or point to the expected feature preparation plan file.
pub fn write_draft(paths: &MaestroPaths, feature_id: &str) -> Result<DraftReport> {
    let view = feature::show(paths, feature_id)?;
    guard_feature_can_prepare(&view.status, &view.id)?;
    let path = paths.features_dir().join(&view.id).join("prepare-draft.md");
    if path.exists() {
        return Ok(DraftReport {
            path,
            written: false,
        });
    }

    let parent = path
        .parent()
        .with_context(|| format!("failed to determine parent for {}", path.display()))?;
    ensure_dir(parent)?;
    let template = format!(
        "# Prepare plan for {}\n\n\
         ## Task T1: Scaffold project\n\
         check: package manifest exists and tests run\n\
         blocker: dependency approval required for <packages>\n\n\
         ## Task T2: Implement first behavior\n\
         after: T1\n\
         check: observable behavior passes through the real entry point\n",
        view.id
    );
    write_string_atomic(&path, &template)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(DraftReport {
        path,
        written: true,
    })
}

/// Prepare a feature implementation queue from an explicit plan file.
pub fn prepare_from_file(
    paths: &MaestroPaths,
    feature_id: &str,
    plan_path: &Path,
    actor: &str,
) -> Result<PrepareReport> {
    let view = feature::show(paths, feature_id)?;
    guard_feature_can_prepare(&view.status, &view.id)?;
    guard_no_existing_child_tasks(paths, &view.id)?;

    let contents = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read {}", plan_path.display()))?;
    let plan = parse_plan(&contents)?;
    validate_plan(&plan)?;

    let mut created = Vec::with_capacity(plan.len());
    let mut id_by_local_ref = BTreeMap::new();
    for (index, item) in plan.iter().enumerate() {
        let now = nanos_since_epoch_string();
        let task = task::create_task(
            &paths.tasks_dir(),
            &item.title,
            Some(view.id.clone()),
            None,
            None,
            item.checks.clone(),
            &now,
        )?;
        let now = nanos_since_epoch_string();
        task::transition_task(
            &paths.tasks_dir(),
            &task.id,
            TaskState::Exploring,
            actor,
            &now,
            TransitionDetails::default(),
        )?;
        let now = nanos_since_epoch_string();
        let task = task::accept_task(&paths.tasks_dir(), &task.id, actor, &now)?;
        let local_ref = item
            .local_id
            .clone()
            .unwrap_or_else(|| format!("@{}", index + 1));
        id_by_local_ref.insert(local_ref, task.id.clone());
        created.push(task);
    }

    let mut blockers = Vec::new();
    for (index, item) in plan.iter().enumerate() {
        let task_id = task_id_for_item(index, item, &id_by_local_ref)?;
        for reason in &item.blockers {
            blockers.push(block_prepared_task(
                paths,
                &task_id,
                reason,
                BlockerTarget::Human,
                actor,
            )?);
        }
        for after in &item.after {
            let predecessor = id_by_local_ref
                .get(after)
                .with_context(|| format!("after reference `{after}` was not prepared"))?;
            let reason =
                format!("{AFTER_DEPENDENCY_REASON_PREFIX} {after} ({predecessor}) verified");
            blockers.push(block_prepared_task(
                paths,
                &task_id,
                &reason,
                BlockerTarget::Task(predecessor.clone()),
                actor,
            )?);
        }
    }

    let prepared = reload_created_tasks(paths, &created)?;
    let ready_count = prepared
        .iter()
        .filter(|task| task.state == TaskState::Ready && !task::has_unresolved_blockers(task))
        .count();
    let blocked_count = prepared
        .iter()
        .filter(|task| task::has_unresolved_blockers(task))
        .count();
    let started = ready_count > 0 && view.status == FeatureStatus::Ready;
    if started {
        feature::start(paths, &view.id)?;
    }

    Ok(PrepareReport {
        feature_id: view.id,
        task_count: prepared.len(),
        ready_count,
        blocked_count,
        started,
        remained_ready: ready_count == 0 && view.status == FeatureStatus::Ready,
        prepared: prepared
            .into_iter()
            .map(|task| {
                let blocked = task::has_unresolved_blockers(&task);
                PreparedTask {
                    id: task.id,
                    title: task.title,
                    blocked,
                }
            })
            .collect(),
        blockers,
    })
}

/// Resolve prepare-generated `after:` blockers once their prerequisite task verifies.
pub(crate) fn resolve_after_dependency_blockers(
    paths: &MaestroPaths,
    verified_task_id: &str,
    actor: &str,
) -> Result<Vec<String>> {
    let entries = task::load_task_entries(&paths.tasks_dir())?;
    let mut resolved = Vec::new();
    for entry in entries {
        let blocker_ids: Vec<String> = entry
            .task
            .blockers
            .iter()
            .filter(|blocker| blocker.resolved_at.is_none())
            .filter(|blocker| blocker.reason.starts_with(AFTER_DEPENDENCY_REASON_PREFIX))
            .filter(|blocker| {
                blocker
                    .blocked_ref
                    .as_ref()
                    .map(|blocked_ref| blocked_ref.id == verified_task_id)
                    .unwrap_or(false)
            })
            .map(|blocker| blocker.id.clone())
            .collect();

        for blocker_id in blocker_ids {
            let now = nanos_since_epoch_string();
            task::unblock_task(&paths.tasks_dir(), &entry.task.id, &blocker_id, actor, &now)?;
            resolved.push(entry.task.id.clone());
        }
    }
    Ok(resolved)
}

fn guard_feature_can_prepare(status: &FeatureStatus, feature_id: &str) -> Result<()> {
    match status {
        FeatureStatus::Ready | FeatureStatus::InProgress => Ok(()),
        FeatureStatus::Proposed => {
            bail!(
                "cannot prepare {feature_id} — not accepted; run `maestro feature accept {feature_id}` first"
            )
        }
        FeatureStatus::Shipped | FeatureStatus::Cancelled => {
            bail!(
                "cannot prepare {feature_id} — terminal (status: {})",
                status.as_str()
            )
        }
    }
}

fn guard_no_existing_child_tasks(paths: &MaestroPaths, feature_id: &str) -> Result<()> {
    let existing: Vec<String> = task::load_task_records(&paths.tasks_dir())?
        .into_iter()
        .filter(|task| task.feature_id.as_deref() == Some(feature_id))
        .map(|task| task.id)
        .collect();
    if !existing.is_empty() {
        bail!(
            "cannot prepare {feature_id} — feature already has child tasks: {}\ninspect: maestro task list --feature {feature_id}",
            existing.join(", ")
        );
    }
    Ok(())
}

fn parse_plan(contents: &str) -> Result<Vec<PlanTask>> {
    let mut tasks = Vec::new();
    let mut current: Option<PlanTask> = None;
    for (line_number, line) in contents.lines().enumerate() {
        if let Some((local_id, title)) = parse_task_heading(line)? {
            if let Some(task) = current.take() {
                tasks.push(task);
            }
            current = Some(PlanTask {
                local_id,
                title,
                checks: Vec::new(),
                blockers: Vec::new(),
                after: Vec::new(),
            });
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(task) = current.as_mut() else {
            continue;
        };
        if let Some(value) = field_value(trimmed, "check") {
            push_non_empty(&mut task.checks, value, line_number + 1, "check")?;
        } else if let Some(value) = field_value(trimmed, "blocker") {
            push_non_empty(&mut task.blockers, value, line_number + 1, "blocker")?;
        } else if let Some(value) = field_value(trimmed, "after") {
            let refs = value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            if refs.is_empty() {
                bail!(
                    "line {}: after field must name at least one task ref",
                    line_number + 1
                );
            }
            task.after.extend(refs);
        }
    }
    if let Some(task) = current {
        tasks.push(task);
    }
    if tasks.is_empty() {
        bail!("prepare plan must contain at least one `## Task` section");
    }
    Ok(tasks)
}

fn parse_task_heading(line: &str) -> Result<Option<(Option<String>, String)>> {
    let trimmed = line.trim();
    let Some(rest) = trimmed
        .strip_prefix("## Task")
        .or_else(|| trimmed.strip_prefix("### Task"))
    else {
        return Ok(None);
    };
    let rest = rest.trim();
    let (local_id, title) = if let Some(title) = rest.strip_prefix(':') {
        (None, title.trim())
    } else if let Some((local_id, title)) = rest.split_once(':') {
        let local_id = local_id.trim();
        if local_id.is_empty() {
            bail!("task heading has an empty local id: {line}");
        }
        (Some(local_id.to_string()), title.trim())
    } else {
        bail!("task heading must be `## Task: <title>` or `## Task T1: <title>`: {line}");
    };
    if title.is_empty() {
        bail!("task heading title must not be empty: {line}");
    }
    Ok(Some((local_id, title.to_string())))
}

fn field_value<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let (key, value) = line.split_once(':')?;
    key.trim()
        .eq_ignore_ascii_case(field)
        .then_some(value.trim())
}

fn push_non_empty(
    values: &mut Vec<String>,
    value: &str,
    line_number: usize,
    field: &str,
) -> Result<()> {
    if value.is_empty() {
        bail!("line {line_number}: {field} field must not be empty");
    }
    values.push(value.to_string());
    Ok(())
}

fn validate_plan(plan: &[PlanTask]) -> Result<()> {
    let mut refs = BTreeSet::new();
    for item in plan {
        if item.checks.is_empty() {
            bail!("task `{}` must declare at least one `check:`", item.title);
        }
        if let Some(local_id) = item.local_id.as_deref()
            && !refs.insert(local_id.to_string())
        {
            bail!("duplicate task ref `{local_id}` in prepare plan");
        }
    }
    for item in plan {
        for after in &item.after {
            if !refs.contains(after) {
                bail!("task `{}` has unknown after ref `{after}`", item.title);
            }
        }
    }
    Ok(())
}

fn task_id_for_item(
    index: usize,
    item: &PlanTask,
    id_by_local_ref: &BTreeMap<String, String>,
) -> Result<String> {
    let local_ref = item
        .local_id
        .clone()
        .unwrap_or_else(|| format!("@{}", index + 1));
    id_by_local_ref
        .get(&local_ref)
        .cloned()
        .with_context(|| format!("prepared task `{}` is missing", item.title))
}

fn block_prepared_task(
    paths: &MaestroPaths,
    task_id: &str,
    reason: &str,
    target: BlockerTarget,
    actor: &str,
) -> Result<PreparedBlocker> {
    let now = nanos_since_epoch_string();
    let (_, blocker_id) =
        task::block_task(&paths.tasks_dir(), task_id, reason, target, actor, &now)?;
    Ok(PreparedBlocker {
        task_id: task_id.to_string(),
        blocker_id,
        reason: reason.to_string(),
    })
}

fn reload_created_tasks(paths: &MaestroPaths, created: &[TaskRecord]) -> Result<Vec<TaskRecord>> {
    created
        .iter()
        .map(|task| task::load_task_record(&paths.tasks_dir(), &task.id))
        .collect()
}
