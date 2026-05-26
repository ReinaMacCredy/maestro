use anyhow::Result;

use crate::domain::harness::schema::{BacklogConfig, HarnessConfig};
use crate::foundation::core::schema::FEATURE_SCHEMA_VERSION;

/// `HARNESS.md` content installed by `maestro init`.
pub const HARNESS_MD: &str = include_str!("../../../resources/harness/HARNESS.md");

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
