use std::{fs, path::Path};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::foundation::core::schema::{BACKLOG_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION};

/// Supported V1 project stack families.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StackKind {
    /// Rust project detected from `Cargo.toml`.
    Rust,
    /// TypeScript or JavaScript project detected from `package.json`.
    TypeScriptNode,
    /// Python project detected from `pyproject.toml` or `requirements.txt`.
    Python,
    /// Generic fallback when no known stack signal is present.
    Generic,
}

/// Stack detection result and default verification commands.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StackConfig {
    /// Detected stack family.
    pub kind: StackKind,
    /// Repo signals that selected this stack.
    pub detected_by: Vec<String>,
    /// Default verification commands for the stack.
    pub verify: Vec<String>,
}

/// `.maestro/harness/harness.yml` V1 configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HarnessConfig {
    /// Harness schema version.
    pub schema_version: String,
    /// Detected stack and verification defaults.
    pub stack: StackConfig,
    /// Optional recurrence-threshold surfacing policy. Missing means disabled for
    /// legacy repos so read verbs keep their old behavior until the repo opts in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escalation: Option<EscalationConfig>,
    /// Optional agent-audit cadence. Missing means no audit hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit: Option<AuditConfig>,
    /// Explicit repo-level acknowledgement that verification has no command leg.
    #[serde(default, skip_serializing_if = "is_false")]
    pub claims_only_verification: bool,
}

/// Per-repo Harness recurrence threshold policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EscalationConfig {
    /// Enable threshold-based surfacing and stricter detector guardrails.
    #[serde(default)]
    pub enabled: bool,
    /// Session/source count where a proposal becomes medium priority.
    #[serde(default = "default_warn_after")]
    pub warn_after: usize,
    /// Session/source count where a proposal becomes high priority and surfaces.
    #[serde(default = "default_act_after")]
    pub act_after: usize,
}

/// Agent-audit cadence policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AuditConfig {
    /// Number of distinct sessions after which an agent-authored repo audit is overdue.
    pub every_sessions: usize,
}

/// Runtime-normalized escalation policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscalationPolicy {
    pub enabled: bool,
    pub warn_after: usize,
    pub act_after: usize,
}

/// `.maestro/harness/backlog.yaml` V1 configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BacklogConfig {
    /// Backlog schema version.
    pub schema_version: String,
    /// Last evidence stamp used by guarded hot-verb refresh.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub evidence_stamp: String,
    /// Rule-based improver proposals. Empty at init.
    pub items: Vec<BacklogItem>,
}

/// Harness improvement proposal tracked in `.maestro/harness/backlog.yaml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BacklogItem {
    /// Stable proposal id.
    pub id: String,
    /// Stable identity `{type}:{subject}`; merge keys on this, not the mutable title.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fingerprint: String,
    /// Detection source, usually a task id, session id, or aggregate bucket.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// Proposal provenance: `detector`, `agent-audit`, or another explicit producer.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provenance: String,
    /// Agent-supplied or detector-normalized topic used for merge/measurement.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub topic: String,
    /// Rule category that produced the proposal.
    #[serde(default, rename = "type", skip_serializing_if = "String::is_empty")]
    pub item_type: String,
    /// Human-readable proposal title.
    pub title: String,
    /// Proposal priority.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub priority: String,
    /// Latest detector-computed magnitude for this proposal.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub occurrences: usize,
    /// Distinct session or source ids the detector says this proposal has hit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions_hit: Vec<String>,
    /// First time this proposal was seen by the backlog.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub first_seen: String,
    /// Most recent time this proposal was detected.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub last_seen: String,
    /// Proposal status: `proposed`, `accepted`, or `measured`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    /// Evidence snippets supporting the proposal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    /// Task spawned when this proposal was accepted (`apply`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawned_task: Option<String>,
    /// Human dismissal reason for ignored/noisy proposals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dismissal_reason: Option<String>,
    /// Append-only lifecycle log (accepted, ineffective, measured, regressed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<HistoryEntry>,
}

/// One append-only lifecycle record on a [`BacklogItem`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HistoryEntry {
    /// Outcome: `accepted`, `unapplied`, `ineffective`, `measured`, or `regressed`.
    pub result: String,
    /// Linked task for the record, when one applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Optional human note explaining manual lifecycle changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Human-facing UTC timestamp of the record.
    pub at: String,
}

/// State detectors emit notes whose silence reliably means the friction is fixed,
/// so they can be auto-measured, auto-reopened on regression, and hinted as ready.
/// All other detectors are behavioral and need a human-judgment `measure`.
pub fn is_state_detector(item_type: &str) -> bool {
    matches!(item_type, "missing_verification" | "rediscovered_decision")
}

impl HarnessConfig {
    /// Build a default harness config for `repo_root`.
    pub fn detect(repo_root: &Path) -> Self {
        Self {
            schema_version: HARNESS_SCHEMA_VERSION.to_string(),
            stack: detect_stack(repo_root),
            escalation: Some(EscalationConfig::enabled_default()),
            audit: None,
            claims_only_verification: false,
        }
    }

    pub fn escalation_policy(&self) -> EscalationPolicy {
        match &self.escalation {
            Some(config) if config.enabled => config.policy(),
            _ => EscalationPolicy::disabled(),
        }
    }
}

impl BacklogConfig {
    /// Return an empty V1 backlog.
    pub fn empty() -> Self {
        Self {
            schema_version: BACKLOG_SCHEMA_VERSION.to_string(),
            evidence_stamp: String::new(),
            items: Vec::new(),
        }
    }

    /// Find a backlog item by id.
    pub fn find(&self, id: &str) -> Result<&BacklogItem> {
        self.items
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(|| anyhow!("backlog item not found: {id}"))
    }

    /// Find a backlog item by id for mutation.
    pub fn find_mut(&mut self, id: &str) -> Result<&mut BacklogItem> {
        self.items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| anyhow!("backlog item not found: {id}"))
    }
}

impl EscalationConfig {
    pub fn enabled_default() -> Self {
        Self {
            enabled: true,
            warn_after: default_warn_after(),
            act_after: default_act_after(),
        }
    }

    fn policy(&self) -> EscalationPolicy {
        let warn_after = self.warn_after.max(1);
        let act_after = self.act_after.max(warn_after);
        EscalationPolicy {
            enabled: true,
            warn_after,
            act_after,
        }
    }
}

impl EscalationPolicy {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            warn_after: default_warn_after(),
            act_after: default_act_after(),
        }
    }

    pub fn priority_for(&self, sessions_hit: usize, fallback: &str) -> String {
        if sessions_hit == 0 {
            return if fallback.is_empty() {
                "medium".to_string()
            } else {
                fallback.to_string()
            };
        }
        if !self.enabled {
            return if fallback.is_empty() {
                "medium".to_string()
            } else {
                fallback.to_string()
            };
        }
        if sessions_hit >= self.act_after {
            "high".to_string()
        } else if sessions_hit >= self.warn_after {
            "medium".to_string()
        } else {
            "low".to_string()
        }
    }

    pub fn over_threshold(&self, sessions_hit: usize) -> bool {
        self.enabled && sessions_hit >= self.act_after
    }
}

fn default_warn_after() -> usize {
    2
}

fn default_act_after() -> usize {
    3
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Detect the project stack from repo-local files.
pub fn detect_stack(repo_root: &Path) -> StackConfig {
    if repo_root.join("Cargo.toml").is_file() {
        return StackConfig {
            kind: StackKind::Rust,
            detected_by: vec!["Cargo.toml".to_string()],
            verify: vec![
                "cargo build".to_string(),
                "cargo test".to_string(),
                "cargo clippy -- -D warnings".to_string(),
            ],
        };
    }

    if repo_root.join("package.json").is_file() {
        return StackConfig {
            kind: StackKind::TypeScriptNode,
            detected_by: vec!["package.json".to_string()],
            verify: vec![
                "bun run lint".to_string(),
                "bun run typecheck".to_string(),
                "bun test".to_string(),
            ],
        };
    }

    let mut python_signals = Vec::new();
    if repo_root.join("pyproject.toml").is_file() {
        python_signals.push("pyproject.toml".to_string());
    }
    if repo_root.join("requirements.txt").is_file() {
        python_signals.push("requirements.txt".to_string());
    }
    if !python_signals.is_empty() {
        return StackConfig {
            kind: StackKind::Python,
            detected_by: python_signals,
            verify: detect_python_verify(repo_root),
        };
    }

    let verify = if repo_root.join("Makefile").is_file() {
        vec!["make test".to_string()]
    } else {
        Vec::new()
    };
    StackConfig {
        kind: StackKind::Generic,
        detected_by: Vec::new(),
        verify,
    }
}

fn detect_python_verify(repo_root: &Path) -> Vec<String> {
    if uses_pytest(repo_root) {
        return vec!["python -m pytest".to_string()];
    }
    if repo_root.join("tests").is_dir() {
        return vec!["python -m unittest discover -s tests".to_string()];
    }
    vec!["python -m unittest discover".to_string()]
}

fn uses_pytest(repo_root: &Path) -> bool {
    repo_root.join("pytest.ini").is_file()
        || repo_root.join("conftest.py").is_file()
        || repo_root.join("tests/conftest.py").is_file()
        || file_contains(repo_root.join("pyproject.toml").as_path(), "pytest")
        || file_contains(repo_root.join("requirements.txt").as_path(), "pytest")
}

fn file_contains(path: &Path, needle: &str) -> bool {
    fs::read_to_string(path).is_ok_and(|contents| contents.contains(needle))
}
