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
use crate::foundation::core::time::utc_now_timestamp;

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
    covers: Vec<String>,
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
    let first_check = view
        .acceptance
        .first()
        .map(String::as_str)
        .unwrap_or("observable behavior passes through the real entry point");
    let areas = if view.affected_areas.is_empty() {
        "unspecified".to_string()
    } else {
        view.affected_areas.join(", ")
    };
    let covers = view
        .acceptance
        .iter()
        .enumerate()
        .map(|(index, _)| feature::acceptance_id(index))
        .collect::<Vec<_>>()
        .join(", ");
    let template = format!(
        "# Prepare plan for {}\n\n\
         # Review before applying. Split this into multiple tasks only when the\n\
         # accepted contract has independent work slices; add blocker: lines only\n\
         # for real approvals or external waits.\n\
         # Affected areas: {areas}\n\n\
         ## Task T1: Implement accepted behavior\n\
         covers: {covers}\n\
         check: {first_check}\n",
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
    prepare_from_file_with_blocker(paths, feature_id, plan_path, actor, block_prepared_task)
}

fn prepare_from_file_with_blocker(
    paths: &MaestroPaths,
    feature_id: &str,
    plan_path: &Path,
    actor: &str,
    block_task_fn: fn(&MaestroPaths, &str, &str, BlockerTarget, &str) -> Result<PreparedBlocker>,
) -> Result<PrepareReport> {
    let view = feature::show(paths, feature_id)?;
    guard_feature_can_prepare(&view.status, &view.id)?;
    guard_no_existing_child_tasks(paths, &view.id)?;

    let contents = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read {}", plan_path.display()))?;
    let plan = parse_plan(&contents)?;
    validate_plan(&plan)?;

    let mut created = Vec::with_capacity(plan.len());
    let result = (|| -> Result<PrepareReport> {
        let mut id_by_local_ref = BTreeMap::new();
        for (index, item) in plan.iter().enumerate() {
            let now = utc_now_timestamp();
            let task = task::create_task(
                &paths.tasks_dir(),
                &item.title,
                task::CreateTaskOptions {
                    feature: Some(view.id.clone()),
                    covers: item.covers.clone(),
                    lane: None,
                    risk: None,
                    checks: item.checks.clone(),
                    created_at: now,
                },
            )?;
            let now = utc_now_timestamp();
            let task = task::transition_task(
                &paths.tasks_dir(),
                &task.id,
                TaskState::Exploring,
                actor,
                &now,
                TransitionDetails::default(),
            )?;
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
                blockers.push(block_task_fn(
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
                blockers.push(block_task_fn(
                    paths,
                    &task_id,
                    &reason,
                    BlockerTarget::Task(predecessor.clone()),
                    actor,
                )?);
            }
        }

        let mut accepted = Vec::with_capacity(created.len());
        for task in &created {
            let now = utc_now_timestamp();
            accepted.push(task::accept_task(
                &paths.tasks_dir(),
                &task.id,
                actor,
                &now,
            )?);
        }

        let prepared = reload_created_tasks(paths, &accepted)?;
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
            feature_id: view.id.clone(),
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
    })();

    match result {
        Ok(report) => {
            let draft_path = paths.features_dir().join(&view.id).join("prepare-draft.md");
            if same_path(plan_path, &draft_path) && draft_path.exists() {
                fs::remove_file(&draft_path)
                    .with_context(|| format!("failed to remove {}", draft_path.display()))?;
            }
            Ok(report)
        }
        Err(error) => {
            rollback_created_tasks(paths, &created).with_context(|| {
                format!("failed to roll back partial prepare after error: {error}")
            })?;
            Err(error)
        }
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.canonicalize().unwrap_or_else(|_| left.to_path_buf())
        == right.canonicalize().unwrap_or_else(|_| right.to_path_buf())
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
            let now = utc_now_timestamp();
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
    let mut in_task_plan_section = false;
    for (line_number, line) in contents.lines().enumerate() {
        if let Some((local_id, title)) = parse_task_line(line, in_task_plan_section)? {
            if let Some(task) = current.take() {
                tasks.push(task);
            }
            current = Some(PlanTask {
                local_id,
                title,
                covers: Vec::new(),
                checks: Vec::new(),
                blockers: Vec::new(),
                after: Vec::new(),
            });
            continue;
        }
        let trimmed = line.trim();
        if let Some(heading) = markdown_heading_text(trimmed) {
            in_task_plan_section = is_task_plan_section(heading);
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let Some(task) = current.as_mut() else {
            continue;
        };
        let field_line = strip_markdown_list_marker(trimmed).unwrap_or(trimmed);
        if let Some(value) = field_value(field_line, "check") {
            push_non_empty(&mut task.checks, value, line_number + 1, "check")?;
        } else if let Some(value) = field_value(field_line, "covers") {
            push_comma_values(&mut task.covers, value, line_number + 1, "covers")?;
        } else if let Some(value) = field_value(field_line, "blocker") {
            push_non_empty(&mut task.blockers, value, line_number + 1, "blocker")?;
        } else if let Some(value) = field_value(field_line, "after") {
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
        bail!(
            "prepare plan must contain at least one explicit task entry, such as `## Task T1: <title>`, `- Task T1: <title>`, or a numbered item inside a Task Plan section"
        );
    }
    Ok(tasks)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlanLineKind {
    Heading,
    ListItem,
    Plain,
}

fn parse_task_line(
    line: &str,
    in_task_plan_section: bool,
) -> Result<Option<(Option<String>, String)>> {
    let trimmed = line.trim();
    let (text, kind) = if let Some(heading) = markdown_heading_text(trimmed) {
        (heading, PlanLineKind::Heading)
    } else if let Some(item) = strip_markdown_list_marker(trimmed) {
        (item, PlanLineKind::ListItem)
    } else {
        (trimmed, PlanLineKind::Plain)
    };

    if text.is_empty() || is_task_field_line(text) {
        return Ok(None);
    }

    if let Some(rest) = task_prefix_rest(text) {
        return parse_task_rest(rest, line);
    }

    if in_task_plan_section {
        if let Some((local_id, title)) = parse_local_ref_title(text, line)? {
            return Ok(Some((Some(local_id), title)));
        }
        if kind == PlanLineKind::ListItem {
            return Ok(Some((None, non_empty_task_title(text, line)?)));
        }
    }

    Ok(None)
}

fn parse_task_rest(rest: &str, line: &str) -> Result<Option<(Option<String>, String)>> {
    let rest = rest.trim();
    if let Some(title) = rest.strip_prefix(':') {
        return Ok(Some((None, non_empty_task_title(title, line)?)));
    }
    if let Some(title) = rest.strip_prefix("- ") {
        return Ok(Some((None, non_empty_task_title(title, line)?)));
    }
    if let Some((local_id, title)) = parse_local_ref_title(rest, line)? {
        return Ok(Some((Some(local_id), title)));
    }
    if !rest.contains(':') && !rest.contains(" - ") {
        if looks_like_task_local_id(rest) {
            bail!("task heading must be `## Task: <title>` or `## Task T1: <title>`: {line}");
        }
        return Ok(None);
    }
    bail!("task heading must be `## Task: <title>` or `## Task T1: <title>`: {line}");
}

fn parse_local_ref_title(value: &str, line: &str) -> Result<Option<(String, String)>> {
    let Some((local_id, title)) = split_task_title(value) else {
        return Ok(None);
    };
    let local_id = local_id.trim();
    if local_id.is_empty() {
        bail!("task heading has an empty local id: {line}");
    }
    if !looks_like_task_local_id(local_id) {
        return Ok(None);
    }
    Ok(Some((
        local_id.to_string(),
        non_empty_task_title(title, line)?,
    )))
}

fn split_task_title(value: &str) -> Option<(&str, &str)> {
    value.split_once(':').or_else(|| value.split_once(" - "))
}

fn non_empty_task_title(value: &str, line: &str) -> Result<String> {
    let title = value.trim();
    if title.is_empty() {
        bail!("task heading title must not be empty: {line}");
    }
    Ok(title.to_string())
}

fn markdown_heading_text(line: &str) -> Option<&str> {
    let hash_count = line.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&hash_count) {
        return None;
    }
    let rest = line.get(hash_count..)?;
    if !rest.starts_with(' ') {
        return None;
    }
    Some(rest.trim())
}

fn strip_markdown_list_marker(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let rest = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "));
    if let Some(rest) = rest {
        return Some(strip_checkbox(rest).trim());
    }

    let marker_len = trimmed
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if marker_len == 0 {
        return None;
    }
    let after_digits = trimmed.get(marker_len..)?;
    let delimiter = after_digits.as_bytes().first().copied()?;
    if delimiter != b'.' && delimiter != b')' {
        return None;
    }
    let rest = after_digits.get(1..)?;
    if !rest.starts_with(' ') {
        return None;
    }
    Some(strip_checkbox(rest).trim())
}

fn strip_checkbox(value: &str) -> &str {
    let trimmed = value.trim_start();
    if let Some(rest) = trimmed
        .strip_prefix("[ ] ")
        .or_else(|| trimmed.strip_prefix("[x] "))
        .or_else(|| trimmed.strip_prefix("[X] "))
    {
        rest
    } else {
        value
    }
}

fn task_prefix_rest(value: &str) -> Option<&str> {
    let rest = value
        .strip_prefix("Task")
        .or_else(|| value.strip_prefix("task"))?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
    {
        return None;
    }
    Some(rest.trim())
}

fn is_task_plan_section(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized == "tasks"
        || normalized == "plan"
        || normalized.contains("task plan")
        || normalized.contains("task breakdown")
        || normalized.contains("implementation plan")
        || normalized.contains("implementation tasks")
        || normalized.contains("work plan")
}

fn is_task_field_line(value: &str) -> bool {
    field_value(value, "check").is_some()
        || field_value(value, "covers").is_some()
        || field_value(value, "blocker").is_some()
        || field_value(value, "after").is_some()
}

fn looks_like_task_local_id(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.chars().any(|ch| ch.is_ascii_digit())
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
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

fn push_comma_values(
    values: &mut Vec<String>,
    value: &str,
    line_number: usize,
    field: &str,
) -> Result<()> {
    let parsed = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        bail!("line {line_number}: {field} field must name at least one acceptance id");
    }
    values.extend(parsed);
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
    let now = utc_now_timestamp();
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

fn rollback_created_tasks(paths: &MaestroPaths, created: &[TaskRecord]) -> Result<()> {
    for task in created.iter().rev() {
        let task_dir = paths.cards_dir().join(&task.id);
        if task_dir.exists() {
            fs::remove_dir_all(&task_dir)
                .with_context(|| format!("failed to remove {}", task_dir.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::feature::ContractEdits;

    #[test]
    fn prepare_rolls_back_created_tasks_when_blocker_attachment_fails() {
        let root = temp_root("maestro-feature-prepare-rollback");
        let paths = MaestroPaths::new(&root);
        let feature_id = feature::create(&paths, "Rollback Prepare")
            .expect("invariant: feature should be created");
        feature::set(
            &paths,
            &feature_id,
            ContractEdits {
                acceptance: Some(vec!["rollback behavior is observable".to_string()]),
                affected_areas: Some(vec!["task".to_string()]),
                ..ContractEdits::default()
            },
        )
        .expect("invariant: feature contract should be set");
        fs::write(
            paths.cards_dir().join(&feature_id).join("qa.md"),
            "---\namend_log_position: 0\n---\n\nbaseline\n",
        )
        .expect("invariant: baseline should be writable");
        feature::accept(&paths, &feature_id, false).expect("invariant: feature should be ready");
        let plan = root.join("prepare.md");
        fs::write(
            &plan,
            concat!(
                "## Task T1: First generated task\n",
                "check: first task works\n",
                "blocker: injected failure\n",
            ),
        )
        .expect("invariant: plan should be writable");

        let error =
            prepare_from_file_with_blocker(&paths, &feature_id, &plan, "tester", fail_blocker)
                .expect_err("invariant: injected blocker failure should fail prepare");

        assert!(error.to_string().contains("injected blocker failure"));
        assert!(
            task_dirs(&paths).is_empty(),
            "created tasks should be rolled back after prepare failure"
        );
        fs::remove_dir_all(root).expect("invariant: temp root should be removable");
    }

    fn fail_blocker(
        _paths: &MaestroPaths,
        _task_id: &str,
        _reason: &str,
        _target: BlockerTarget,
        _actor: &str,
    ) -> Result<PreparedBlocker> {
        bail!("injected blocker failure")
    }

    fn task_dirs(paths: &MaestroPaths) -> Vec<String> {
        task::load_task_records(&paths.tasks_dir())
            .expect("invariant: task records should be loadable")
            .into_iter()
            .map(|task| task.id)
            .collect()
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            utc_now_timestamp()
        ));
        fs::create_dir_all(&root).expect("invariant: temp root should be creatable");
        root
    }
}
