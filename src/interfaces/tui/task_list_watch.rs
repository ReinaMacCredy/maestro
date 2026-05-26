use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::feature::schema::FeatureRegistry;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::FEATURE_SCHEMA_VERSION;
use crate::task::blockers::has_unresolved_blockers;
use crate::task::template::{TaskRecord, TaskState};
use crate::verification::stale::stale_reasons;
use crate::verification::verify_task::{
    freshness_inputs_for_task, read_report, VerificationStatus,
};

/// Run the polling task status screen.
pub fn run<F>(paths: &MaestroPaths, interval_seconds: u64, load_tasks: F) -> Result<()>
where
    F: Fn() -> Result<Vec<TaskRecord>>,
{
    let interval = normalized_interval(interval_seconds);
    if !io::stdout().is_terminal() {
        let initial_tasks = load_tasks()?;
        print!("{}", render_snapshot(paths, &initial_tasks)?);
        return Ok(());
    }

    loop {
        let tasks = load_tasks()?;
        print!("\x1b[2J\x1b[H{}", render_snapshot(paths, &tasks)?);
        io::stdout()
            .flush()
            .context("failed to flush watch output")?;
        thread::sleep(Duration::from_secs(interval));
    }
}

/// Render one sandcastle-style task status snapshot.
pub fn render_snapshot(paths: &MaestroPaths, tasks: &[TaskRecord]) -> Result<String> {
    let features = load_feature_titles(paths)?;
    let active_agents = active_agents(tasks);
    let mut groups = BTreeMap::<String, Vec<&TaskRecord>>::new();
    for task in tasks {
        let group = task
            .feature_id
            .as_ref()
            .and_then(|id| features.get(id).cloned().or_else(|| Some(id.clone())))
            .unwrap_or_else(|| "unassigned".to_string());
        groups.entry(group).or_default().push(task);
    }

    let mut out = String::new();
    out.push_str(&format!(
        "scheduler: {} agents active\n\n",
        active_agents.len()
    ));
    if groups.is_empty() {
        out.push_str("unassigned\n  . no tasks\n");
        return Ok(out);
    }

    for (group, mut group_tasks) in groups {
        group_tasks.sort_by(|left, right| left.id.cmp(&right.id));
        out.push_str(&format!("{group}\n"));
        for task in group_tasks {
            out.push_str(&format!("  {} {}\n", task_icon(task), task.title));
            out.push_str(&format!("    {}\n", task_substatus(paths, task)?));
        }
        out.push('\n');
    }
    Ok(out)
}

fn load_feature_titles(paths: &MaestroPaths) -> Result<BTreeMap<String, String>> {
    let path = paths.features_dir().join("features.yaml");
    if !path.is_file() {
        return Ok(BTreeMap::new());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let registry: FeatureRegistry = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if registry.schema_version != FEATURE_SCHEMA_VERSION {
        return Ok(BTreeMap::new());
    }
    Ok(registry
        .features
        .into_iter()
        .map(|feature| (feature.id, feature.title))
        .collect())
}

fn active_agents(tasks: &[TaskRecord]) -> BTreeSet<String> {
    tasks
        .iter()
        .filter(|task| task.state == TaskState::InProgress)
        .filter_map(|task| task.claimed_by.clone())
        .collect()
}

fn task_icon(task: &TaskRecord) -> &'static str {
    if has_unresolved_blockers(task) {
        return "!";
    }
    match task.state {
        TaskState::InProgress => "~",
        TaskState::NeedsVerification => "?",
        TaskState::Verified => "+",
        TaskState::Draft | TaskState::Exploring | TaskState::Ready => ".",
        TaskState::Rejected | TaskState::Abandoned | TaskState::Superseded => "x",
    }
}

fn task_substatus(paths: &MaestroPaths, task: &TaskRecord) -> Result<String> {
    if let Some(blocker) = task
        .blockers
        .iter()
        .find(|blocker| blocker.resolved_at.is_none())
    {
        let blocker_label = blocker
            .blocked_ref
            .as_ref()
            .map(|blocked_ref| blocked_ref.id.as_str())
            .unwrap_or(blocker.title.as_str());
        return Ok(format!("blocked by {blocker_label}"));
    }
    if task.state == TaskState::InProgress {
        return Ok(format!(
            "in-progress ({})",
            task.claimed_by.as_deref().unwrap_or("unclaimed")
        ));
    }
    if task.state == TaskState::NeedsVerification {
        let latest_failed = latest_report_failed(paths, task)?;
        return Ok(if latest_failed {
            "needs_verification (last verify failed)".to_string()
        } else {
            "needs_verification".to_string()
        });
    }
    if task.state == TaskState::Verified {
        return verified_substatus(paths, task);
    }
    Ok(state_label(&task.state).to_string())
}

fn latest_report_failed(paths: &MaestroPaths, task: &TaskRecord) -> Result<bool> {
    let Some(task_dir) = task_dir(paths, task) else {
        return Ok(false);
    };
    Ok(read_report(&task_dir)?
        .map(|report| report.status == VerificationStatus::Failed)
        .unwrap_or(false))
}

fn verified_substatus(paths: &MaestroPaths, task: &TaskRecord) -> Result<String> {
    let Some(task_dir) = task_dir(paths, task) else {
        return Ok("verified".to_string());
    };
    let Some(report) = read_report(&task_dir)? else {
        return Ok("verified".to_string());
    };
    if report.status == VerificationStatus::Failed {
        return Ok("verified / failed".to_string());
    }
    let current = freshness_inputs_for_task(
        task,
        &task_dir,
        crate::foundation::core::git::head(paths.repo_root()).unwrap_or(None),
    )?;
    if stale_reasons(&current, &report.freshness).is_empty() {
        Ok("verified".to_string())
    } else {
        Ok("verified / stale (HEAD changed after proof)".to_string())
    }
}

fn task_dir(paths: &MaestroPaths, task: &TaskRecord) -> Option<std::path::PathBuf> {
    let dir = paths.tasks_dir().join(task.directory_name());
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

fn state_label(state: &TaskState) -> &'static str {
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

fn normalized_interval(seconds: u64) -> u64 {
    seconds.max(1)
}

#[cfg(test)]
mod tests {
    use super::normalized_interval;

    #[test]
    fn interval_clamps_below_one_second() {
        assert_eq!(normalized_interval(0), 1);
        assert_eq!(normalized_interval(1), 1);
        assert_eq!(normalized_interval(2), 2);
    }
}
