use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::core::error::MaestroError;
use crate::core::managed_path::{managed_path, SymlinkPolicy};
use crate::core::paths::MaestroPaths;
use crate::core::schema::RUN_EVIDENCE_SCHEMA_VERSION;
use crate::core::time::parse_utc_timestamp;
use crate::task::doctor::load_task_entries;
use crate::task::template::{TaskState, VerificationBinding};

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

/// Run evidence subset used by metrics and improver rules.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RunEvidenceRecord {
    pub schema_version: String,
    #[serde(default)]
    pub session_id: String,
    pub agent: Option<String>,
    pub task_id: Option<String>,
    pub duration_seconds: Option<u64>,
    #[serde(default)]
    pub human_interventions: u64,
}

/// Build metrics by reading current task and run artifacts on demand.
pub fn summarize(paths: &MaestroPaths) -> Result<MetricsSummary> {
    let tasks = load_task_entries(&paths.tasks_dir())?;
    let evidence = load_run_evidence(paths)?;
    let mut task_counts = BTreeMap::<String, usize>::new();
    let mut verify_durations = Vec::new();

    for entry in &tasks {
        *task_counts
            .entry(task_state_label(&entry.task.state).to_string())
            .or_default() += 1;
        if entry.task.state == TaskState::Verified {
            if let Some(seconds) =
                verification_duration_seconds(&entry.task.created_at, &entry.task.verification)
            {
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
    let mut records = Vec::new();
    let mut skipped = 0;
    for path in managed_run_evidence_files(paths)? {
        let Ok(raw) = fs::read_to_string(&path) else {
            skipped += 1;
            continue;
        };
        let Ok(record) = serde_yaml::from_str::<RunEvidenceRecord>(&raw) else {
            skipped += 1;
            continue;
        };
        if record.schema_version == RUN_EVIDENCE_SCHEMA_VERSION {
            records.push(record);
        } else {
            skipped += 1;
        }
    }
    Ok(RunEvidenceLoad { records, skipped })
}

/// Best-effort run evidence load result.
#[derive(Clone, Debug, PartialEq)]
pub struct RunEvidenceLoad {
    pub records: Vec<RunEvidenceRecord>,
    pub skipped: usize,
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
    let mut durations = BTreeMap::new();
    for entry in load_task_entries(&paths.tasks_dir())? {
        if let Some(seconds) =
            verification_duration_seconds(&entry.task.created_at, &entry.task.verification)
        {
            durations.insert(entry.task.id, seconds);
        }
    }
    Ok(durations)
}

fn managed_run_evidence_files(paths: &MaestroPaths) -> Result<Vec<PathBuf>> {
    let runs_dir = match managed_path(paths, ".maestro/runs", SymlinkPolicy::RejectAllComponents) {
        Ok(path) => path,
        Err(error)
            if matches!(
                error.downcast_ref::<MaestroError>(),
                Some(MaestroError::ManagedPathContainsSymlink { .. })
            ) =>
        {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    run_evidence_files_under(&runs_dir)
}

fn run_evidence_files_under(runs_dir: &Path) -> Result<Vec<PathBuf>> {
    match fs::symlink_metadata(runs_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => return Ok(Vec::new()),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", runs_dir.display()));
        }
    }
    let root = match fs::canonicalize(runs_dir) {
        Ok(root) => root,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to resolve {}", runs_dir.display()));
        }
    };
    let mut files = Vec::new();
    collect_run_evidence_files(runs_dir, &root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_run_evidence_files(dir: &Path, root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !is_inside_canonical_root(dir, root)? {
        return Ok(());
    }
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.with_context(|| format!("failed to list {}", dir.display()))?;
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .with_context(|| format!("failed to inspect {}", path.display()))?;
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    collect_run_evidence_files(&path, root, files)?;
                } else if file_type.is_file()
                    && path.file_name().and_then(|name| name.to_str()) == Some("run_evidence.yaml")
                    && is_inside_canonical_root(&path, root)?
                {
                    files.push(path);
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", dir.display())),
    }
}

fn is_inside_canonical_root(path: &Path, root: &Path) -> Result<bool> {
    match fs::canonicalize(path) {
        Ok(canonical) => Ok(canonical.starts_with(root)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to resolve {}", path.display())),
    }
}

#[derive(Default)]
struct AgentAccumulator {
    tasks: BTreeSet<String>,
    durations: Vec<u64>,
}

fn verification_duration_seconds(
    created_at: &str,
    verification: &VerificationBinding,
) -> Option<u64> {
    let start = parse_timestamp_seconds(created_at)?;
    let end = parse_timestamp_seconds(verification.verified_at.as_deref()?)?;
    end.checked_sub(start)
}

fn parse_timestamp_seconds(value: &str) -> Option<u64> {
    if value.chars().all(|character| character.is_ascii_digit()) {
        return value.parse().ok();
    }
    let parsed = parse_utc_timestamp(value)?;
    if parsed.nanos_since_epoch < 0 {
        return None;
    }
    Some((parsed.nanos_since_epoch / 1_000_000_000) as u64)
}

fn average(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<u64>() / values.len() as u64)
}

fn task_state_label(state: &TaskState) -> &'static str {
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

fn count(counts: &BTreeMap<String, usize>, state: &str) -> usize {
    counts.get(state).copied().unwrap_or(0)
}

fn format_minutes(seconds: Option<u64>) -> String {
    match seconds {
        Some(seconds) => format!("{} min", seconds / 60),
        None => "n/a".to_string(),
    }
}
