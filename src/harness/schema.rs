use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::schema::{BACKLOG_SCHEMA_VERSION, HARNESS_SCHEMA_VERSION};

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

/// Placeholder type for future harness improver proposals.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BacklogItem {
    /// Stable proposal id.
    pub id: String,
    /// Human-readable proposal title.
    pub title: String,
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
