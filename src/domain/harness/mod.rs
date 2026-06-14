pub mod backlog;
pub(crate) mod cards;
pub mod extract;
pub mod schema;
pub mod templates;

use anyhow::{Context, Result, bail};

use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::write_string_atomic;

pub use schema::{
    BacklogConfig, BacklogItem, EscalationConfig, EscalationPolicy, HarnessConfig, HistoryEntry,
    StackConfig, StackKind, is_state_detector,
};

/// Require the Harness protocol file that install-managed pointers reference.
pub fn ensure_harness_protocol_exists(paths: &MaestroPaths) -> Result<()> {
    let path = managed_path(
        paths,
        ".maestro/harness/HARNESS.md",
        SymlinkPolicy::RejectAllComponents,
    )?;
    if path.is_file() {
        return Ok(());
    }

    bail!(
        "Maestro harness is not initialized: {} is missing; run `maestro init` first",
        path.display()
    )
}

/// Explicitly accept claims-only task verification for repos with no verify commands.
pub fn set_claims_only_verification(paths: &MaestroPaths) -> Result<()> {
    let path = managed_path(
        paths,
        ".maestro/harness/harness.yml",
        SymlinkPolicy::RejectAllComponents,
    )?;
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read harness config {}", path.display()))?;
    let mut config: HarnessConfig = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    config.claims_only_verification = true;
    write_string_atomic(&path, &serde_yaml::to_string(&config)?)
        .with_context(|| format!("failed to write harness config {}", path.display()))
}
