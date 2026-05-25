use anyhow::Result;

use crate::core::schema::FEATURE_SCHEMA_VERSION;
use crate::harness::schema::{BacklogConfig, HarnessConfig};

/// `HARNESS.md` content installed by `maestro init`.
pub const HARNESS_MD: &str = r#"# Maestro Harness Protocol

You are an agent (Claude, Codex, or future) working in a repo that
uses Maestro. Follow these rules.

## Shared protocol (all agents)
1. Read MAESTRO_CURRENT_TASK env or `maestro task show` to know which task you're on.
2. Read acceptance.yaml - those criteria are locked.
3. Use the skills active for this task.
4. Run `maestro task verify` when implementation is complete.
5. Hooks already write evidence to .maestro/runs/<session_id>/events.jsonl

## If you are Claude Code
- You can use @file imports.
- Hooks fire: SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop.

## If you are Codex CLI
- Don't use @file imports.
- Read .maestro/tasks/<current-id>/ explicitly with your file-read tool.
"#;

/// Serialize the default harness config.
pub fn harness_yml(config: &HarnessConfig) -> Result<String> {
    Ok(serde_yaml::to_string(config)?)
}

/// Serialize the default empty harness backlog.
pub fn backlog_yaml() -> Result<String> {
    Ok(serde_yaml::to_string(&BacklogConfig::empty())?)
}

/// Return the empty feature registry created at init.
pub fn features_yaml() -> String {
    format!("schema_version: {FEATURE_SCHEMA_VERSION}\nfeatures: []\n")
}
