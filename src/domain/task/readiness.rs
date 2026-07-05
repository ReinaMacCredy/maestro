use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::Serialize;

use crate::domain::task::blockers::has_unresolved_blockers;
use crate::domain::task::doctor;
use crate::domain::task::template::{TaskRecord, TaskState};
use crate::foundation::core::paths::MaestroPaths;

pub const READY_SCHEMA_V2: &str = "maestro.ready.v2";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReadinessFilter {
    pub project: Option<String>,
    pub feature: Option<String>,
    pub blocked_next_limit: usize,
    pub include_projected_waves: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ReadyProjection {
    pub version: u8,
    pub schema: String,
    pub parallel_wave: Vec<ReadyTaskRow>,
    pub serial_gates: Vec<ReadyTaskRow>,
    pub blocked_next: Vec<ReadyTaskRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projected_waves: Vec<ProjectedWave>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
    #[serde(skip)]
    pub blocked_next_hidden: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProjectedWave {
    pub index: usize,
    pub parallel_wave: Vec<ReadyTaskRow>,
    pub serial_gates: Vec<ReadyTaskRow>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReadyTaskRow {
    pub id: String,
    pub title: String,
    pub lane: String,
    pub gate: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gate_kind: Option<String>,
    pub execution_mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remaining_blockers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<ReadyCommand>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReadyCommand {
    pub display: String,
    pub argv: Vec<String>,
}

pub fn projection(paths: &MaestroPaths, filter: ReadinessFilter) -> Result<ReadyProjection> {
    let tasks = doctor::load_task_records(&paths.tasks_dir())?;
    Ok(projection_from_records(&tasks, filter))
}

pub fn projection_from_records(tasks: &[TaskRecord], filter: ReadinessFilter) -> ReadyProjection {
    let mut tasks = tasks.to_vec();
    tasks.retain(|task| {
        filter
            .project
            .as_deref()
            .is_none_or(|project| task.project.as_deref() == Some(project))
            && filter
                .feature
                .as_deref()
                .is_none_or(|feature| task.feature_id.as_deref() == Some(feature))
    });
    let mut diagnostics = Vec::new();
    let mut rows = classify_rows(&tasks, &mut diagnostics);
    rows.parallel_wave.sort_by(compare_ready_rows);
    rows.serial_gates.sort_by(compare_ready_rows);
    rows.blocked_next.sort_by(compare_blocked_rows);
    let hidden = rows
        .blocked_next
        .len()
        .saturating_sub(filter.blocked_next_limit);
    if filter.blocked_next_limit > 0 {
        rows.blocked_next.truncate(filter.blocked_next_limit);
    }
    let projected_waves = if filter.include_projected_waves {
        project_future_waves(&tasks, &mut diagnostics)
    } else {
        Vec::new()
    };
    ReadyProjection {
        version: 1,
        schema: READY_SCHEMA_V2.to_string(),
        parallel_wave: rows.parallel_wave,
        serial_gates: rows.serial_gates,
        blocked_next: rows.blocked_next,
        projected_waves,
        diagnostics,
        blocked_next_hidden: hidden,
    }
}

pub fn remaining_start_blockers(paths: &MaestroPaths, task: &TaskRecord) -> Result<Vec<String>> {
    let tasks = doctor::load_task_records(&paths.tasks_dir())?;
    Ok(remaining_start_blockers_from_records(task, &tasks))
}

pub fn remaining_start_blockers_from_records(
    task: &TaskRecord,
    tasks: &[TaskRecord],
) -> Vec<String> {
    let task_map: BTreeMap<&str, &TaskRecord> =
        tasks.iter().map(|task| (task.id.as_str(), task)).collect();
    let mut diagnostics = Vec::new();
    remaining_blockers(task, &task_map, &mut diagnostics)
}

#[derive(Default)]
struct ClassifiedRows {
    parallel_wave: Vec<ReadyTaskRow>,
    serial_gates: Vec<ReadyTaskRow>,
    blocked_next: Vec<ReadyTaskRow>,
}

fn classify_rows(tasks: &[TaskRecord], diagnostics: &mut Vec<String>) -> ClassifiedRows {
    let task_map: BTreeMap<&str, &TaskRecord> =
        tasks.iter().map(|task| (task.id.as_str(), task)).collect();
    let mut rows = ClassifiedRows::default();
    for task in tasks {
        if task.state != TaskState::Ready {
            continue;
        }
        let remaining = remaining_blockers(task, &task_map, diagnostics);
        if remaining.is_empty() {
            if task.gate {
                rows.serial_gates.push(row_for_task(task, Vec::new(), true));
            } else {
                rows.parallel_wave
                    .push(row_for_task(task, Vec::new(), false));
            }
        } else {
            rows.blocked_next
                .push(row_for_task(task, remaining, task.gate));
        }
    }
    rows
}

fn remaining_blockers(
    task: &TaskRecord,
    task_map: &BTreeMap<&str, &TaskRecord>,
    diagnostics: &mut Vec<String>,
) -> Vec<String> {
    remaining_blockers_with(task, task_map, diagnostics, |_, dependency| {
        dependency.is_some_and(|dependency| dependency.state == TaskState::Verified)
    })
}

fn remaining_blockers_with(
    task: &TaskRecord,
    task_map: &BTreeMap<&str, &TaskRecord>,
    diagnostics: &mut Vec<String>,
    mut is_satisfied: impl FnMut(&str, Option<&TaskRecord>) -> bool,
) -> Vec<String> {
    let mut remaining = Vec::new();
    if has_unresolved_blockers(task) {
        remaining.push("impediment blockers".to_string());
    }
    for blocked_by in &task.blocked_by {
        let dependency = task_map.get(blocked_by.as_str()).copied();
        if is_satisfied(blocked_by, dependency) {
            continue;
        }
        push_unsatisfied_dependency(
            task,
            blocked_by,
            dependency,
            "blocked_by",
            diagnostics,
            &mut remaining,
        );
    }
    let lane = lane_for_task(task);
    let mut predecessor: Option<&TaskRecord> = None;
    for dependency in task_map.values().copied() {
        if dependency.id == task.id
            || lane_for_task(dependency) != lane
            || !precedes_in_same_serial_sequence(dependency, task)
            || is_satisfied(dependency.id.as_str(), Some(dependency))
        {
            continue;
        }
        let replace =
            predecessor.is_none_or(|current| precedes_in_same_serial_sequence(current, dependency));
        if replace {
            predecessor = Some(dependency);
        }
    }
    if let Some(dependency) = predecessor {
        push_unsatisfied_dependency(
            task,
            &dependency.id,
            Some(dependency),
            "same-lane predecessor",
            diagnostics,
            &mut remaining,
        );
    }
    remaining.sort();
    remaining.dedup();
    remaining
}

fn push_unsatisfied_dependency(
    task: &TaskRecord,
    dependency_ref: &str,
    dependency: Option<&TaskRecord>,
    relation: &str,
    diagnostics: &mut Vec<String>,
    remaining: &mut Vec<String>,
) {
    match dependency {
        Some(dep)
            if matches!(
                dep.state,
                TaskState::Rejected | TaskState::Abandoned | TaskState::Superseded
            ) =>
        {
            diagnostics.push(format!(
                "{} {relation} {} is terminal {}",
                task.id,
                dep.id,
                dep.state.as_str()
            ));
            remaining.push(dep.id.clone());
        }
        Some(dep) => remaining.push(dep.id.clone()),
        None => {
            diagnostics.push(format!(
                "{} {relation} missing task {}",
                task.id, dependency_ref
            ));
            remaining.push(dependency_ref.to_string());
        }
    }
}

fn row_for_task(task: &TaskRecord, remaining_blockers: Vec<String>, serial: bool) -> ReadyTaskRow {
    let lane = lane_for_task(task).to_string();
    let execution_mode = if serial || task.gate {
        "serial".to_string()
    } else {
        "parallel".to_string()
    };
    let command = if remaining_blockers.is_empty() {
        Some(start_command(&task.id))
    } else {
        None
    };
    ReadyTaskRow {
        id: task.id.clone(),
        title: task.title.clone(),
        lane,
        gate: task.gate,
        gate_kind: task.gate_kind.clone(),
        execution_mode,
        blocked_by: task.blocked_by.clone(),
        remaining_blockers,
        command,
    }
}

fn lane_for_task(task: &TaskRecord) -> &str {
    task.lane.as_deref().unwrap_or("general")
}

fn precedes_in_same_serial_sequence(candidate: &TaskRecord, task: &TaskRecord) -> bool {
    match (candidate.order, task.order) {
        (Some(candidate_order), Some(task_order)) => candidate_order < task_order,
        (None, None) if candidate.progress_backed && task.progress_backed => {
            (&candidate.created_at, &candidate.title, &candidate.id)
                < (&task.created_at, &task.title, &task.id)
        }
        _ => false,
    }
}

fn start_command(task_id: &str) -> ReadyCommand {
    ReadyCommand {
        display: format!("maestro task start {task_id}"),
        argv: vec![
            "maestro".to_string(),
            "task".to_string(),
            "start".to_string(),
            task_id.to_string(),
        ],
    }
}

fn project_future_waves(tasks: &[TaskRecord], diagnostics: &mut Vec<String>) -> Vec<ProjectedWave> {
    let task_map: BTreeMap<&str, &TaskRecord> =
        tasks.iter().map(|task| (task.id.as_str(), task)).collect();
    let mut satisfied: BTreeSet<String> = tasks
        .iter()
        .filter(|task| task.state == TaskState::Verified)
        .map(|task| task.id.clone())
        .collect();
    let mut pending: Vec<&TaskRecord> = tasks
        .iter()
        .filter(|task| task.state == TaskState::Ready)
        .collect();
    let mut waves = Vec::new();
    while !pending.is_empty() {
        let mut ready = Vec::new();
        let mut still_pending = Vec::new();
        for task in pending {
            let unresolved =
                remaining_blockers_for_projection(task, &task_map, &satisfied, diagnostics);
            if unresolved.is_empty() {
                ready.push(task);
            } else {
                still_pending.push(task);
            }
        }
        if ready.is_empty() {
            break;
        }
        let mut parallel_wave = Vec::new();
        let mut serial_gates = Vec::new();
        for task in ready {
            satisfied.insert(task.id.clone());
            if task.gate {
                serial_gates.push(row_for_task(task, Vec::new(), true));
            } else {
                parallel_wave.push(row_for_task(task, Vec::new(), false));
            }
        }
        parallel_wave.sort_by(compare_ready_rows);
        serial_gates.sort_by(compare_ready_rows);
        waves.push(ProjectedWave {
            index: waves.len() + 1,
            parallel_wave,
            serial_gates,
        });
        pending = still_pending;
    }
    waves
}

fn remaining_blockers_for_projection(
    task: &TaskRecord,
    task_map: &BTreeMap<&str, &TaskRecord>,
    satisfied: &BTreeSet<String>,
    diagnostics: &mut Vec<String>,
) -> Vec<String> {
    remaining_blockers_with(task, task_map, diagnostics, |blocked_by, _| {
        satisfied.contains(blocked_by)
    })
}

fn compare_ready_rows(left: &ReadyTaskRow, right: &ReadyTaskRow) -> std::cmp::Ordering {
    lane_rank(&left.lane)
        .cmp(&lane_rank(&right.lane))
        .then_with(|| left.lane.cmp(&right.lane))
        .then_with(|| left.title.cmp(&right.title))
        .then_with(|| left.id.cmp(&right.id))
}

fn compare_blocked_rows(left: &ReadyTaskRow, right: &ReadyTaskRow) -> std::cmp::Ordering {
    left.remaining_blockers
        .len()
        .cmp(&right.remaining_blockers.len())
        .then_with(|| compare_ready_rows(left, right))
}

fn lane_rank(lane: &str) -> usize {
    match lane {
        "frontend" => 0,
        "backend" => 1,
        "tests" => 2,
        "docs" => 3,
        "integration" => 4,
        "ship" => 5,
        "general" | "normal" => 6,
        _ => 100,
    }
}
