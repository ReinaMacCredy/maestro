use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use crate::domain::run;
use crate::domain::task::{self, TaskEntry, TaskState};
use crate::foundation::core::paths::MaestroPaths;

pub use crate::domain::run::{RunEvidenceLoad, RunEvidenceRecord};

/// Computed metrics rendered by `maestro metrics summary`.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricsSummary {
    pub task_counts: BTreeMap<String, usize>,
    pub average_time_to_verify_seconds: Option<u64>,
    pub agents: Vec<AgentSummary>,
    pub interventions_per_task: f64,
    pub skipped_run_evidence: usize,
}

/// Per-agent metrics derived from run evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentSummary {
    pub agent: String,
    pub tasks: usize,
    pub average_duration_seconds: Option<u64>,
}

/// Build metrics by reading current task and run artifacts on demand.
pub fn summarize(paths: &MaestroPaths) -> Result<MetricsSummary> {
    let tasks = task::load_task_entries(&paths.tasks_dir())?;
    summarize_task_entries(paths, &tasks)
}

pub(crate) fn summarize_task_entries(
    paths: &MaestroPaths,
    tasks: &[TaskEntry],
) -> Result<MetricsSummary> {
    let evidence = load_run_evidence(paths)?;
    let mut task_counts = BTreeMap::<String, usize>::new();
    let mut verify_durations = Vec::new();

    for entry in tasks {
        *task_counts
            .entry(entry.task.state.as_str().to_string())
            .or_default() += 1;
        if entry.task.state == TaskState::Verified {
            if let Some(seconds) = task::verification_duration_seconds(
                &entry.task.created_at,
                &entry.task.verification,
            ) {
                verify_durations.push(seconds);
            }
        }
    }

    let mut by_agent = BTreeMap::<String, AgentAccumulator>::new();
    let total_interventions = evidence
        .records
        .iter()
        .map(|run| run.human_interventions)
        .sum::<u64>();
    for run in &evidence.records {
        let agent = run.agent.as_deref().unwrap_or("<unknown>").to_string();
        let accumulator = by_agent.entry(agent).or_default();
        if let Some(task_id) = run.task_id.as_deref() {
            accumulator.tasks.insert(task_id.to_string());
        }
        if let Some(duration) = run.duration_seconds {
            accumulator.durations.push(duration);
        }
    }

    let agents = by_agent
        .into_iter()
        .map(|(agent, accumulator)| AgentSummary {
            agent,
            tasks: accumulator.tasks.len(),
            average_duration_seconds: average(&accumulator.durations),
        })
        .collect();

    let task_total = tasks.len();
    let interventions_per_task = if task_total == 0 {
        0.0
    } else {
        total_interventions as f64 / task_total as f64
    };

    Ok(MetricsSummary {
        task_counts,
        average_time_to_verify_seconds: average(&verify_durations),
        agents,
        interventions_per_task,
        skipped_run_evidence: evidence.skipped,
    })
}

/// Load valid managed run evidence records.
pub fn load_run_evidence(paths: &MaestroPaths) -> Result<RunEvidenceLoad> {
    run::load_run_evidence(paths)
}

/// Render metrics in the human-readable summary format from the spec.
pub fn render_summary(summary: &MetricsSummary) -> String {
    let total = summary.task_counts.values().sum::<usize>();
    let verified = count(&summary.task_counts, "verified");
    let needs_verification = count(&summary.task_counts, "needs_verification");
    let in_progress = count(&summary.task_counts, "in_progress");
    let mut out = String::new();

    out.push_str(&format!(
        "Tasks: {total} ({verified} verified, {needs_verification} needs_verification, {in_progress} in_progress)\n"
    ));
    out.push_str(&format!(
        "Avg time-to-verify: {}\n",
        format_minutes(summary.average_time_to_verify_seconds)
    ));
    out.push_str("Agents:\n");
    if summary.agents.is_empty() {
        out.push_str("  <none>: 0 tasks, n/a avg\n");
    } else {
        for agent in &summary.agents {
            out.push_str(&format!(
                "  {}: {} tasks, {} avg\n",
                agent.agent,
                agent.tasks,
                format_minutes(agent.average_duration_seconds)
            ));
        }
    }
    out.push_str(&format!(
        "Interventions: {:.1} per task\n",
        summary.interventions_per_task
    ));
    if summary.skipped_run_evidence > 0 {
        out.push_str(&format!(
            "Skipped run evidence: {}\n",
            summary.skipped_run_evidence
        ));
    }
    out
}

/// Return per-task verification durations, grouped by task id.
pub fn task_verification_durations(paths: &MaestroPaths) -> Result<BTreeMap<String, u64>> {
    let tasks = task::load_task_entries(&paths.tasks_dir())?;
    Ok(task::task_verification_durations(&tasks))
}

#[derive(Default)]
struct AgentAccumulator {
    tasks: BTreeSet<String>,
    durations: Vec<u64>,
}

fn average(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<u64>() / values.len() as u64)
}

fn count(counts: &BTreeMap<String, usize>, state: &str) -> usize {
    counts.get(state).copied().unwrap_or(0)
}

fn format_minutes(seconds: Option<u64>) -> String {
    match seconds {
        Some(seconds) => format!("{} min", seconds / 60),
        None => "n/a".to_string(),
    }
}
