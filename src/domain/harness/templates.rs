use anyhow::Result;

use crate::domain::harness::schema::{BacklogConfig, HarnessConfig};

/// `HARNESS.md` content installed by `maestro init`.
pub const HARNESS_MD: &str = include_str!("../../../embedded/harness/HARNESS.md");

/// Static break-glass runbook installed at `.maestro/RECOVERY.md`.
pub const RECOVERY_MD: &str = include_str!("../../../embedded/harness/RECOVERY.md");

/// Serialize the default harness config.
pub fn harness_yml(config: &HarnessConfig) -> Result<String> {
    Ok(serde_yaml::to_string(config)?)
}

/// Serialize the default empty harness backlog.
pub fn backlog_yaml() -> Result<String> {
    Ok(serde_yaml::to_string(&BacklogConfig::empty())?)
}
