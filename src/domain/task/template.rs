use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::safe_write::write_string_atomic;
use crate::foundation::core::schema::{ACCEPTANCE_SCHEMA_VERSION, TASK_SCHEMA_VERSION};
use crate::foundation::core::slug::slugify_ascii;

/// V1 task lifecycle states.
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

/// V1 task record stored in `task.yaml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskRecord {
    pub schema_version: String,
    pub id: String,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_id: Option<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_areas: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub open_questions: Vec<String>,
    pub state: TaskState,
    pub acceptance_locked: bool,
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
}

/// Verification proof binding stored in task.yaml.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_by_run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_contract_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptance_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks_hash: Option<String>,
}

/// Acceptance criteria stored in `acceptance.yaml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AcceptanceFile {
    pub schema_version: String,
    pub task: String,
    pub checks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_at: Option<String>,
}

/// Derived proof state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofState {
    Missing,
    Failed,
    Accepted,
    Stale,
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

impl TaskRecord {
    /// Create a draft task artifact.
    pub fn draft(id: &str, title: &str, created_at: &str) -> Self {
        let slug = slugify_ascii(title);
        Self {
            schema_version: TASK_SCHEMA_VERSION.to_string(),
            id: id.to_string(),
            slug,
            feature_id: None,
            title: title.to_string(),
            task_type: None,
            lane: Some("normal".to_string()),
            risk: Some("medium".to_string()),
            raw_request: None,
            input_type: None,
            affected_areas: Vec::new(),
            open_questions: Vec::new(),
            state: TaskState::Draft,
            acceptance_locked: false,
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
            }],
            verification: VerificationBinding::default(),
            created_at: created_at.to_string(),
            updated_at: created_at.to_string(),
        }
    }

    /// Repo-local task directory name.
    pub fn directory_name(&self) -> String {
        format!("{}-{}", self.id, self.slug)
    }
}

impl AcceptanceFile {
    /// Create an unlocked acceptance file.
    pub fn new(task_id: &str, checks: Vec<String>) -> Self {
        Self {
            schema_version: ACCEPTANCE_SCHEMA_VERSION.to_string(),
            task: task_id.to_string(),
            checks,
            locked_by: None,
            locked_at: None,
        }
    }
}

impl VerificationBinding {
    /// Compute proof state from binding plus current expected hashes.
    pub fn proof_state(
        &self,
        current_commit: Option<&str>,
        current_acceptance_hash: Option<&str>,
        current_checks_hash: Option<&str>,
        latest_failed: bool,
    ) -> ProofState {
        if latest_failed {
            return ProofState::Failed;
        }
        let Some(verified_commit) = self.verified_commit.as_deref() else {
            return ProofState::Missing;
        };
        if Some(verified_commit) != current_commit
            || self.acceptance_hash.as_deref() != current_acceptance_hash
            || self.checks_hash.as_deref() != current_checks_hash
        {
            return ProofState::Stale;
        }
        ProofState::Accepted
    }
}

/// Write task.yaml, task.md, and acceptance.yaml for a task.
pub fn write_task_artifacts(
    tasks_dir: &Path,
    task: &TaskRecord,
    acceptance: &AcceptanceFile,
) -> Result<PathBuf> {
    let task_dir = tasks_dir.join(task.directory_name());
    ensure_dir(&task_dir)?;
    write_string_atomic(task_dir.join("task.yaml"), &serde_yaml::to_string(task)?)
        .context("failed to write task.yaml")?;
    write_string_atomic(task_dir.join("task.md"), &task_markdown(task))
        .context("failed to write task.md")?;
    write_string_atomic(
        task_dir.join("acceptance.yaml"),
        &serde_yaml::to_string(acceptance)?,
    )
    .context("failed to write acceptance.yaml")?;

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
    let _lock = TaskSaveLock::acquire(&snapshot.path)?;
    let (current, _) = load_task(&snapshot.path)?;
    if current.updated_at != snapshot.updated_at {
        anyhow::bail!("task was modified, please retry");
    }
    write_string_atomic(&snapshot.path, &serde_yaml::to_string(task)?)
        .with_context(|| format!("failed to write {}", snapshot.path.display()))
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
                anyhow::bail!("task is locked, please retry")
            }
            Err(error) => Err(error)
                .with_context(|| format!("failed to create task lock {}", lock_path.display())),
        }
    }
}

impl Drop for TaskSaveLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn task_markdown(task: &TaskRecord) -> String {
    format!("# {}\n\n## Acceptance\nSee acceptance.yaml.\n", task.title)
}
