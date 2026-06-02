use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::domain::decisions;
use crate::domain::task::lookup::task_yaml_path_for_entry;
use crate::domain::task::template::{BlockerKind, TaskRecord, load_task};

/// Result of scanning task blocker references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskDoctorReport {
    pub tasks_scanned: usize,
    pub errors: Vec<String>,
}

/// Task record plus its artifact directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskEntry {
    pub task: TaskRecord,
    pub task_dir: std::path::PathBuf,
}

impl TaskDoctorReport {
    /// Whether the scan found no task graph errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Load all task records under `.maestro/tasks`.
pub fn load_task_records(tasks_dir: &Path) -> Result<Vec<TaskRecord>> {
    let mut tasks = load_task_entries(tasks_dir)?
        .into_iter()
        .map(|entry| entry.task)
        .collect::<Vec<_>>();
    tasks.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(tasks)
}

/// Load all task records with their directories under `.maestro/tasks`.
pub fn load_task_entries(tasks_dir: &Path) -> Result<Vec<TaskEntry>> {
    if !tasks_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(tasks_dir)
        .with_context(|| format!("failed to read {}", tasks_dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to list {}", tasks_dir.display()))?;
        if let Some(task_path) = task_yaml_path_for_entry(&entry)? {
            let (task, _) = load_task(&task_path)?;
            entries.push(TaskEntry {
                task,
                task_dir: entry.path(),
            });
        }
    }
    entries.sort_by(|left, right| left.task.id.cmp(&right.task.id));
    Ok(entries)
}

/// Check unresolved task blocker references for missing nodes, self-blocks, and cycles.
pub fn check_blocker_graph(tasks_dir: &Path) -> Result<TaskDoctorReport> {
    let tasks = load_task_records(tasks_dir)?;
    let by_id: HashMap<String, TaskRecord> = tasks
        .iter()
        .cloned()
        .map(|task| (task.id.clone(), task))
        .collect();
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    let mut errors = Vec::new();

    // Decision blockers point at `.maestro/decisions`, the sibling of tasks_dir
    // under `.maestro` (both are MaestroPaths children); resolving refs there
    // surfaces a dangling `--by decision-NNN` like a missing task ref (T4).
    let decisions_dir = tasks_dir.parent().map(|maestro| maestro.join("decisions"));

    for task in &tasks {
        for blocker in task
            .blockers
            .iter()
            .filter(|blocker| blocker.resolved_at.is_none())
        {
            let Some(blocked_ref) = blocker.blocked_ref.as_ref() else {
                continue;
            };
            match blocked_ref.kind {
                // External and human blockers are free-form by design and cannot be validated.
                BlockerKind::External | BlockerKind::Human => continue,
                BlockerKind::Decision => {
                    if let Some(decisions_dir) = decisions_dir.as_deref()
                        && decisions::resolve_decision_path(decisions_dir, &blocked_ref.id).is_err()
                    {
                        errors.push(format!(
                            "{} has blocker {} referencing missing decision {}",
                            task.id, blocker.id, blocked_ref.id
                        ));
                    }
                    continue;
                }
                BlockerKind::Task => {}
            }

            if blocked_ref.id == task.id {
                errors.push(format!(
                    "{} has self-blocking blocker {}",
                    task.id, blocker.id
                ));
            }
            if !by_id.contains_key(&blocked_ref.id) {
                errors.push(format!(
                    "{} has blocker {} referencing missing task {}",
                    task.id, blocker.id, blocked_ref.id
                ));
            }
            edges
                .entry(task.id.clone())
                .or_default()
                .push(blocked_ref.id.clone());
        }
    }

    let mut reported_cycles = HashSet::new();
    for task_id in edges.keys() {
        let mut path = Vec::new();
        visit_task_blockers(
            task_id,
            &edges,
            &mut path,
            &mut reported_cycles,
            &mut errors,
        );
    }

    errors.sort();
    errors.dedup();
    Ok(TaskDoctorReport {
        tasks_scanned: tasks.len(),
        errors,
    })
}

/// Render a task doctor report for CLI output.
pub fn render_report(report: &TaskDoctorReport) -> String {
    if report.is_ok() {
        return format!("task doctor: ok ({} tasks scanned)\n", report.tasks_scanned);
    }

    let mut out = String::new();
    for error in &report.errors {
        out.push_str(&format!("error: {error}\n"));
    }
    out.push_str(&format!(
        "task doctor found {} error(s)\n",
        report.errors.len()
    ));
    out.push_str(
        "fix: clear a blocker with `maestro task unblock <id> --blocker <blocker-id>`; \
         a terminal task can instead be archived to drop it from the graph\n",
    );
    out
}

fn visit_task_blockers(
    task_id: &str,
    edges: &HashMap<String, Vec<String>>,
    path: &mut Vec<String>,
    reported_cycles: &mut HashSet<String>,
    errors: &mut Vec<String>,
) {
    if let Some(position) = path.iter().position(|entry| entry == task_id) {
        let mut cycle = path[position..].to_vec();
        cycle.push(task_id.to_string());
        let key = normalized_cycle_key(&cycle);
        if reported_cycles.insert(key) {
            errors.push(format!("blocker cycle detected: {}", cycle.join(" -> ")));
        }
        return;
    }

    path.push(task_id.to_string());
    if let Some(blocked_by) = edges.get(task_id) {
        for next in blocked_by {
            visit_task_blockers(next, edges, path, reported_cycles, errors);
        }
    }
    path.pop();
}

fn normalized_cycle_key(cycle: &[String]) -> String {
    let mut nodes = cycle
        .iter()
        .take(cycle.len().saturating_sub(1))
        .cloned()
        .collect::<Vec<_>>();
    nodes.sort();
    nodes.join("|")
}
