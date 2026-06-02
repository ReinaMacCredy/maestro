use std::path::Path;

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
}

/// `.maestro/harness/backlog.yaml` V1 configuration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BacklogConfig {
    /// Backlog schema version.
    pub schema_version: String,
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
    /// Rule category that produced the proposal.
    #[serde(default, rename = "type", skip_serializing_if = "String::is_empty")]
    pub item_type: String,
    /// Human-readable proposal title.
    pub title: String,
    /// Proposal priority.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub priority: String,
    /// Proposal status: `proposed`, `accepted`, or `measured`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    /// Evidence snippets supporting the proposal.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    /// Task spawned when this proposal was accepted (`apply`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawned_task: Option<String>,
    /// Append-only lifecycle log (accepted, ineffective, measured, regressed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<HistoryEntry>,
}

/// One append-only lifecycle record on a [`BacklogItem`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HistoryEntry {
    /// Outcome: `accepted`, `ineffective`, `measured`, or `regressed`.
    pub result: String,
    /// Linked task for the record, when one applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
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
        }
    }
}

impl BacklogConfig {
    /// Return an empty V1 backlog.
    pub fn empty() -> Self {
        Self {
            schema_version: BACKLOG_SCHEMA_VERSION.to_string(),
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
            verify: vec!["python -m mypy".to_string(), "python -m pytest".to_string()],
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
