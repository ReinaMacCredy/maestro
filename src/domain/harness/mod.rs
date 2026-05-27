pub mod backlog;
pub mod schema;
pub mod templates;

use std::path::PathBuf;

use anyhow::{bail, Result};

use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;

pub use schema::{BacklogConfig, BacklogItem, HarnessConfig, StackConfig, StackKind};

/// Return the repository-local Harness protocol file path.
pub fn harness_protocol_path(paths: &MaestroPaths) -> PathBuf {
    paths.harness_dir().join("HARNESS.md")
}

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
