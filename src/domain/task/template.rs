use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::domain::card::store::ResolvedCard;
use crate::domain::task::cards;
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
    /// closing; the four done states (`Verified` is done) are hidden behind
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
    /// Project/service scope carried on the underlying card base (T4). A
    /// read-time projection like `feature_id`: never in `task.yaml`, populated by
    /// the card fold, fully skipped on serialize/deserialize.
    #[serde(default, skip)]
    pub project: Option<String>,
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
    /// Optional per-task narrow falsifier. When set, `task verify` runs ONLY this
    /// command for the slice instead of the repo-global `stack.verify`. It is
    /// authored config (not a verification result), so it lives here on the task
    /// rather than in the rebuilt-every-verify `verification` binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verify_command: Option<String>,
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

/// Optimistic-concurrency snapshot for a loaded task. The snapshot is the CAS
/// basis the matching save checks: the card store's raw-string CAS is the
/// guard, and the resolved card carries the home the save writes back to. The
/// save seam takes no `paths`, so the snapshot is the natural carrier of both
/// the write target and the proof.
#[derive(Clone, Debug, PartialEq)]
pub enum TaskSnapshot {
    Card(Box<ResolvedCard>),
}

impl TaskRecord {
    /// Create a draft task artifact.
    pub fn draft(id: &str, title: &str, created_at: &str) -> Self {
        Self {
            schema_version: TASK_SCHEMA_VERSION.to_string(),
            id: id.to_string(),
            feature_id: None,
            project: None,
            covers: Vec::new(),
            title: title.to_string(),
            lane: Some("normal".to_string()),
            risk: Some("medium".to_string()),
            raw_request: None,
            state: TaskState::Draft,
            acceptance_locked: false,
            acceptance: AcceptanceFile::new(id, Vec::new()),
            claims: Vec::new(),
            verify_command: None,
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

/// Save a task against its load-time snapshot. The card store's raw-string
/// compare-and-set is the whole guard -- no lock or reload here.
pub fn save_task_with_snapshot(task: &TaskRecord, snapshot: &TaskSnapshot) -> Result<()> {
    match snapshot {
        TaskSnapshot::Card(resolved) => cards::save(task, resolved),
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
