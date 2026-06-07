use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::foundation::core::fs::write_new_dir_atomic;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::TASK_SCHEMA_VERSION;
use crate::foundation::core::slug::slugify_ascii;

/// Task lifecycle states.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Draft,
    Exploring,
    Ready,
    InProgress,
    NeedsVerification,
    Verified,
    Rejected,
    Abandoned,
    Superseded,
}

impl TaskState {
    /// Canonical lifecycle label, identical to the serde `snake_case` wire form.
    /// Used for CLI/TUI output and as a metric bucket key, so these strings are a
    /// contract and must not drift.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Exploring => "exploring",
            Self::Ready => "ready",
            Self::InProgress => "in_progress",
            Self::NeedsVerification => "needs_verification",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
            Self::Abandoned => "abandoned",
            Self::Superseded => "superseded",
        }
    }

    /// Whether the task is still in flight (not done).
    ///
    /// Live states are shown by default in listings and block a feature from
    /// shipping; the four done states (`Verified` is done) are hidden behind
    /// `--all`. Matched exhaustively so a new variant forces this to be
    /// reconsidered rather than silently defaulting.
    pub fn is_live(&self) -> bool {
        match self {
            Self::Draft
            | Self::Exploring
            | Self::Ready
            | Self::InProgress
            | Self::NeedsVerification => true,
            Self::Verified | Self::Rejected | Self::Abandoned | Self::Superseded => false,
        }
    }
}

/// Task record stored in `task.yaml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskRecord {
    pub schema_version: String,
    pub id: String,
    #[serde(default, skip)]
    pub feature_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub covers: Vec<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_request: Option<String>,
    pub state: TaskState,
    pub acceptance_locked: bool,
    #[serde(default)]
    pub acceptance: AcceptanceFile,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<Blocker>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_history: Vec<StateHistoryEntry>,
    pub verification: VerificationBinding,
    pub created_at: String,
    pub updated_at: String,
}

/// Blocker overlay metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Blocker {
    pub id: String,
    pub kind: BlockerKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_ref: Option<BlockerRef>,
    pub title: String,
    pub reason: String,
    pub source: BlockerSource,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
}

/// Blocker kind.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerKind {
    Task,
    External,
    Human,
    Decision,
}

/// Blocker source.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerSource {
    Command,
    HookAdapter,
    Migration,
}

/// Reference from a blocker to another object.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BlockerRef {
    pub kind: BlockerKind,
    pub id: String,
}

/// One irreversible task transition history entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StateHistoryEntry {
    pub state: TaskState,
    pub at: String,
    pub by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_items: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeats: Option<usize>,
}

/// Verification proof binding stored in task.yaml.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<VerificationStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_by_run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claim_checks: Vec<ClaimCheckReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<VerificationCommandReceipt>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub claims_only: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proof_sources: Vec<ProofSourceReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClaimCheckReceipt {
    pub claim: String,
    pub matched: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationCommandReceipt {
    pub cmd: String,
    pub exit_code: i32,
    pub duration_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProofSourceReceipt {
    pub kind: String,
    pub path: String,
    pub hash: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Acceptance criteria stored inside `task.yaml`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AcceptanceFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_at: Option<String>,
}

/// Optimistic-concurrency snapshot for a loaded task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSnapshot {
    pub path: PathBuf,
    pub updated_at: String,
}

struct TaskSaveLock {
    path: PathBuf,
}

/// Typed optimistic-save failure from Task-owned persistence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TaskSaveError {
    Modified,
    Locked,
}

impl TaskRecord {
    /// Create a draft task artifact.
    pub fn draft(id: &str, title: &str, created_at: &str) -> Self {
        Self {
            schema_version: TASK_SCHEMA_VERSION.to_string(),
            id: id.to_string(),
            feature_id: None,
            covers: Vec::new(),
            title: title.to_string(),
            lane: Some("normal".to_string()),
            risk: Some("medium".to_string()),
            raw_request: None,
            state: TaskState::Draft,
            acceptance_locked: false,
            acceptance: AcceptanceFile::new(id, Vec::new()),
            claims: Vec::new(),
            claimed_by: None,
            claimed_at: None,
            blockers: Vec::new(),
            state_history: vec![StateHistoryEntry {
                state: TaskState::Draft,
                at: created_at.to_string(),
                by: "maestro".to_string(),
                to: None,
                summary: None,
                claims: Vec::new(),
                open_items: Vec::new(),
                repeats: None,
            }],
            verification: VerificationBinding::default(),
            created_at: created_at.to_string(),
            updated_at: created_at.to_string(),
        }
    }

    /// Repo-local task directory name.
    pub fn directory_name(&self) -> String {
        format!("{}-{}", self.id, slugify_ascii(&self.title))
    }
}

impl AcceptanceFile {
    /// Create an unlocked acceptance file.
    pub fn new(_task_id: &str, checks: Vec<String>) -> Self {
        Self {
            checks,
            locked_by: None,
            locked_at: None,
        }
    }
}

/// Write task.yaml and task.md for a task.
pub fn write_task_artifacts(
    tasks_dir: &Path,
    task: &TaskRecord,
    acceptance: &AcceptanceFile,
) -> Result<PathBuf> {
    let task_dir = tasks_dir.join(task.directory_name());
    let mut record = task.clone();
    record.acceptance = acceptance.clone();
    let temp_root = tasks_dir.parent().unwrap_or(tasks_dir).join(".tmp-create");
    write_new_dir_atomic(&task_dir, temp_root, "task", |temp_dir| {
        write_string_atomic(temp_dir.join("task.md"), &task_markdown(&record))
            .context("failed to write task.md")?;
        write_string_atomic(temp_dir.join("task.yaml"), &serde_yaml::to_string(&record)?)
            .context("failed to write task.yaml")?;
        Ok(())
    })?;

    Ok(task_dir)
}

/// Load a task and return its optimistic concurrency snapshot.
pub fn load_task(path: &Path) -> Result<(TaskRecord, TaskSnapshot)> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let task: TaskRecord = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let snapshot = TaskSnapshot {
        path: path.to_path_buf(),
        updated_at: task.updated_at.clone(),
    };

    Ok((task, snapshot))
}

/// Save a task only if `updated_at` still matches the loaded snapshot.
pub fn save_task_with_snapshot(task: &TaskRecord, snapshot: &TaskSnapshot) -> Result<()> {
    save_task_with_snapshot_after(task, snapshot, || Ok(NoopSaveTaskHook))
}

/// Save a task under its optimistic-concurrency lock after a caller-owned pre-save step.
pub(crate) fn save_task_with_snapshot_after<F, C>(
    task: &TaskRecord,
    snapshot: &TaskSnapshot,
    before_save: F,
) -> Result<()>
where
    F: FnOnce() -> Result<C>,
    C: SaveTaskHook,
{
    let _lock = TaskSaveLock::acquire(&snapshot.path)?;
    let (current, _) = load_task(&snapshot.path)?;
    if current.updated_at != snapshot.updated_at {
        return Err(TaskSaveError::Modified.into());
    }
    let serialized = serde_yaml::to_string(task)?;
    let mut hook = before_save()?;
    let write_result = write_string_atomic(&snapshot.path, &serialized)
        .with_context(|| format!("failed to write {}", snapshot.path.display()));
    match write_result {
        Ok(()) => {
            hook.commit();
            Ok(())
        }
        Err(write_error) => {
            if let Err(rollback_error) = hook.rollback() {
                return Err(write_error).context(format!(
                    "failed to roll back caller-owned pre-save step: {rollback_error}"
                ));
            }
            Err(write_error)
        }
    }
}

pub(crate) trait SaveTaskHook {
    fn commit(self);

    fn rollback(&mut self) -> Result<()>;
}

struct NoopSaveTaskHook;

impl SaveTaskHook for NoopSaveTaskHook {
    fn commit(self) {}

    fn rollback(&mut self) -> Result<()> {
        Ok(())
    }
}

impl TaskSaveLock {
    fn acquire(task_path: &Path) -> Result<Self> {
        let lock_path = task_path.with_extension("yaml.lock");
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(_) => Ok(Self { path: lock_path }),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                Err(TaskSaveError::Locked.into())
            }
            Err(error) => Err(error)
                .with_context(|| format!("failed to create task lock {}", lock_path.display())),
        }
    }
}

impl fmt::Display for TaskSaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskSaveError::Modified => formatter.write_str("task was modified, please retry"),
            TaskSaveError::Locked => formatter.write_str("task is locked, please retry"),
        }
    }
}

impl Error for TaskSaveError {}

impl Drop for TaskSaveLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Render the Task-owned `task.md` companion artifact.
pub fn task_markdown(task: &TaskRecord) -> String {
    let mut out = format!("# {}\n\n## Acceptance\n", task.title);
    if !task.covers.is_empty() {
        out.push_str(&format!("Covers: {}\n\n", task.covers.join(", ")));
    }
    if task.acceptance.checks.is_empty() {
        out.push_str("- none\n");
    } else {
        for check in &task.acceptance.checks {
            out.push_str(&format!("- {check}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `as_str` doubles as the serde wire form, CLI/TUI label, and metric bucket
    /// key, so the two representations must never drift. Lock every variant.
    #[test]
    fn as_str_matches_serde_wire_form() {
        let all = [
            TaskState::Draft,
            TaskState::Exploring,
            TaskState::Ready,
            TaskState::InProgress,
            TaskState::NeedsVerification,
            TaskState::Verified,
            TaskState::Rejected,
            TaskState::Abandoned,
            TaskState::Superseded,
        ];
        for state in all {
            let json = serde_json::to_string(&state).expect("invariant: TaskState serializes");
            let wire = json.trim_matches('"');
            assert_eq!(
                state.as_str(),
                wire,
                "as_str() drifted from serde wire form for {state:?}"
            );
        }
    }
}
