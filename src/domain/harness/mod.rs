pub mod backlog;
pub mod extract;
pub mod schema;
pub mod templates;

use anyhow::{Result, bail};

use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;

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
