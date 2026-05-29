use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::domain::proof;
use crate::domain::task;
use crate::feature::schema::FeatureRegistry;
use crate::foundation::core::git;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::schema::{classify, Compat, FEATURE_SCHEMA_VERSION};

/// Run the polling task status screen.
pub fn run<F>(paths: &MaestroPaths, interval_seconds: u64, load_tasks: F) -> Result<()>
where
    F: Fn() -> Result<Vec<task::TaskRecord>>,
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
pub fn render_snapshot(paths: &MaestroPaths, tasks: &[task::TaskRecord]) -> Result<String> {
    let features = load_feature_titles(paths)?;
    let current_commit = git::head(paths.repo_root()).unwrap_or(None);
    let active_agents = active_agents(tasks);
    let mut groups = BTreeMap::<String, Vec<&task::TaskRecord>>::new();
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
            out.push_str(&format!(
                "    {}\n",
                task_substatus(paths, task, current_commit.clone())?
            ));
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
    if classify(&registry.schema_version, FEATURE_SCHEMA_VERSION) != Compat::Exact {
        return Ok(BTreeMap::new());
    }
    Ok(registry
        .features
        .into_iter()
        .map(|feature| (feature.id, feature.title))
        .collect())
}

fn active_agents(tasks: &[task::TaskRecord]) -> BTreeSet<String> {
    tasks
        .iter()
        .filter(|task| task.state == task::TaskState::InProgress)
        .filter_map(|task| task.claimed_by.clone())
        .collect()
}

fn task_icon(task: &task::TaskRecord) -> &'static str {
    if task::has_unresolved_blockers(task) {
        return "!";
    }
    match task.state {
        task::TaskState::InProgress => "~",
        task::TaskState::NeedsVerification => "?",
        task::TaskState::Verified => "+",
        task::TaskState::Draft | task::TaskState::Exploring | task::TaskState::Ready => ".",
        task::TaskState::Rejected | task::TaskState::Abandoned | task::TaskState::Superseded => "x",
    }
}

fn task_substatus(
    paths: &MaestroPaths,
    task: &task::TaskRecord,
    current_commit: Option<String>,
) -> Result<String> {
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
    if task.state == task::TaskState::InProgress {
        return Ok(format!(
            "in-progress ({})",
            task.claimed_by.as_deref().unwrap_or("unclaimed")
        ));
    }
    if task.state == task::TaskState::NeedsVerification {
        return needs_verification_substatus(paths, task, current_commit);
    }
    if task.state == task::TaskState::Verified {
        return verified_substatus(paths, task, current_commit);
    }
    Ok(state_label(&task.state).to_string())
}

fn needs_verification_substatus(
    paths: &MaestroPaths,
    task: &task::TaskRecord,
    current_commit: Option<String>,
) -> Result<String> {
    let Some(task_dir) = task_dir(paths, task) else {
        return Ok("needs_verification".to_string());
    };
    let _ = current_commit;
    let kind = proof::needs_verification_proof_status_kind_for_task(task, &task_dir)?;
    match kind {
        proof::ProofStatusKind::Failed => Ok("needs_verification (last verify failed)".to_string()),
        proof::ProofStatusKind::Unapplied => Ok("needs_verification / unapplied".to_string()),
        proof::ProofStatusKind::Missing
        | proof::ProofStatusKind::Accepted
        | proof::ProofStatusKind::Stale => Ok("needs_verification".to_string()),
    }
}

fn verified_substatus(
    paths: &MaestroPaths,
    task: &task::TaskRecord,
    current_commit: Option<String>,
) -> Result<String> {
    let Some(task_dir) = task_dir(paths, task) else {
        return Ok("verified".to_string());
    };
    let kind = proof::proof_status_kind_for_task(paths, task, &task_dir, current_commit)?;
    match kind {
        proof::ProofStatusKind::Missing | proof::ProofStatusKind::Accepted => {
            Ok("verified".to_string())
        }
        proof::ProofStatusKind::Failed => Ok("verified / failed".to_string()),
        proof::ProofStatusKind::Stale => {
            Ok("verified / stale (HEAD changed after proof)".to_string())
        }
        proof::ProofStatusKind::Unapplied => Ok("verified / unapplied".to_string()),
    }
}

fn task_dir(paths: &MaestroPaths, task: &task::TaskRecord) -> Option<std::path::PathBuf> {
    let dir = paths.tasks_dir().join(task.directory_name());
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

fn state_label(state: &task::TaskState) -> &'static str {
    match state {
        task::TaskState::Draft => "draft",
        task::TaskState::Exploring => "exploring",
        task::TaskState::Ready => "ready",
        task::TaskState::InProgress => "in_progress",
        task::TaskState::NeedsVerification => "needs_verification",
        task::TaskState::Verified => "verified",
        task::TaskState::Rejected => "rejected",
        task::TaskState::Abandoned => "abandoned",
        task::TaskState::Superseded => "superseded",
    }
}

fn normalized_interval(seconds: u64) -> u64 {
    seconds.max(1)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{normalized_interval, render_snapshot};
    use crate::domain::task::{AcceptanceFile, AppliedVerificationReceipt, TaskRecord, TaskState};
    use crate::foundation::core::paths::MaestroPaths;

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn interval_clamps_below_one_second() {
        assert_eq!(normalized_interval(0), 1);
        assert_eq!(normalized_interval(1), 1);
        assert_eq!(normalized_interval(2), 2);
    }

    #[test]
    fn render_snapshot_marks_verified_task_with_unapplied_proof() {
        let temp = TestTempDir::new("maestro-task-list-watch-unapplied");
        let paths = MaestroPaths::new(temp.path().to_path_buf());
        let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        task.state = TaskState::Verified;
        let task_dir = paths.tasks_dir().join(task.directory_name());
        fs::create_dir_all(&task_dir).expect("invariant: task dir should be creatable");
        fs::write(
            task_dir.join("acceptance.yaml"),
            serde_yaml::to_string(&AcceptanceFile::new("task-001", Vec::new()))
                .expect("invariant: acceptance should serialize"),
        )
        .expect("invariant: acceptance should be writable");
        fs::write(
            task_dir.join("verification.json"),
            serde_json::json!({
                "schema_version": "maestro.verification.v1",
                "task_id": "task-001",
                "task_snapshot": { "updated_at": "t0" },
                "status": "passed",
                "verified_at": "t1",
                "task_contract_hash": "old-task",
                "acceptance_hash": "old-acceptance",
                "checks_hash": "old-checks",
                "claims": [],
                "proof_sources": []
            })
            .to_string(),
        )
        .expect("invariant: verification should be writable");

        let output = render_snapshot(&paths, &[task]).expect("invariant: snapshot should render");

        assert!(output.contains("Add CSV export"));
        assert!(output.contains("verified / unapplied"));
    }

    #[test]
    fn render_snapshot_marks_needs_verification_task_with_unapplied_proof() {
        let temp = TestTempDir::new("maestro-task-list-watch-needs-unapplied");
        let paths = MaestroPaths::new(temp.path().to_path_buf());
        let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        task.state = TaskState::NeedsVerification;
        let task_dir = paths.tasks_dir().join(task.directory_name());
        fs::create_dir_all(&task_dir).expect("invariant: task dir should be creatable");
        fs::write(
            task_dir.join("acceptance.yaml"),
            serde_yaml::to_string(&AcceptanceFile::new("task-001", Vec::new()))
                .expect("invariant: acceptance should serialize"),
        )
        .expect("invariant: acceptance should be writable");
        fs::write(
            task_dir.join("verification.json"),
            serde_json::json!({
                "schema_version": "maestro.verification.v1",
                "task_id": "task-001",
                "task_snapshot": { "updated_at": "t0" },
                "status": "failed",
                "verified_at": "t1",
                "task_contract_hash": "old-task",
                "acceptance_hash": "old-acceptance",
                "checks_hash": "old-checks",
                "claims": [],
                "proof_sources": [],
                "failures": ["missing proof"]
            })
            .to_string(),
        )
        .expect("invariant: verification should be writable");

        let output = render_snapshot(&paths, &[task]).expect("invariant: snapshot should render");

        assert!(output.contains("Add CSV export"));
        assert!(output.contains("needs_verification / unapplied"));
    }

    #[test]
    fn render_snapshot_marks_needs_verification_task_with_applied_failed_proof() {
        let temp = TestTempDir::new("maestro-task-list-watch-failed-applied");
        let paths = MaestroPaths::new(temp.path().to_path_buf());
        let mut task = TaskRecord::draft("task-001", "Add CSV export", "t0");
        task.state = TaskState::NeedsVerification;
        task.verification.applied_report = Some(AppliedVerificationReceipt {
            task_snapshot_updated_at: "t0".to_string(),
            verified_at: "t1".to_string(),
            attempt_id: None,
        });
        let task_dir = paths.tasks_dir().join(task.directory_name());
        fs::create_dir_all(&task_dir).expect("invariant: task dir should be creatable");
        fs::write(
            task_dir.join("verification.json"),
            serde_json::json!({
                "schema_version": "maestro.verification.v1",
                "task_id": "task-001",
                "task_snapshot": { "updated_at": "t0" },
                "status": "failed",
                "verified_at": "t1",
                "task_contract_hash": "old-task",
                "acceptance_hash": "old-acceptance",
                "checks_hash": "old-checks",
                "claims": [],
                "proof_sources": [],
                "failures": ["missing proof"]
            })
            .to_string(),
        )
        .expect("invariant: verification should be writable");

        let output = render_snapshot(&paths, &[task]).expect("invariant: snapshot should render");

        assert!(output.contains("Add CSV export"));
        assert!(output.contains("needs_verification (last verify failed)"));
    }

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("invariant: system clock should be after the Unix epoch")
                .as_nanos();
            let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("{prefix}-{}-{timestamp}-{counter}", process::id()));
            fs::create_dir(&path).expect("invariant: unique temp dir should be creatable");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
