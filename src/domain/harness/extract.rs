//! Extraction of the bundled harness protocol into `.maestro/harness/`.
//!
//! `HARNESS.md` is the agent-facing protocol Maestro ships. It rides the shared
//! version-gated extraction core alongside skills and the hook script: a
//! `version:` field in its Markdown frontmatter plays the same role `SKILL.md`'s
//! frontmatter plays for skills, so local edits survive `maestro update` until
//! the shipped version changes. Harness and skills share the Markdown `version:`
//! convention, so the harness gate reuses the skills frontmatter reader.

use std::path::PathBuf;

use anyhow::Result;

use crate::domain::extraction::extract::{
    apply_actions, file_action, folder_gate, preview_folder, read_existing, Action, FolderPreview,
};
use crate::domain::harness::templates::HARNESS_MD;
use crate::domain::skills::catalog::frontmatter_version;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;

pub use crate::domain::extraction::extract::{ExtractMode, ExtractReport};

/// Report name and on-disk filename for the bundled harness protocol.
const HARNESS_MD_NAME: &str = "HARNESS.md";

/// Extract the bundled harness protocol into `.maestro/harness/`.
pub fn extract_harness(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<ExtractReport> {
    extract_harness_from(paths, HARNESS_MD, mode)
}

/// Extract a harness protocol with explicit contents into `.maestro/harness/`.
///
/// Exposed so tests can drive the writer with a chosen version; production
/// callers go through [`extract_harness`] with the bundled protocol.
pub fn extract_harness_from(
    paths: &MaestroPaths,
    contents: &str,
    mode: ExtractMode<'_>,
) -> Result<ExtractReport> {
    managed_path(paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    ensure_dir(paths.harness_dir())?;
    let mut report = ExtractReport::default();
    let actions = plan_harness(paths, contents, mode)?;

    apply_actions(paths, &actions, &mut report)?;

    Ok(report)
}

/// Validate bundled harness extraction without writing files.
pub fn validate_harness(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<()> {
    managed_path(paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    plan_harness(paths, HARNESS_MD, mode)?;
    Ok(())
}

/// Preview the harness protocol's fate without writing files.
pub fn preview_harness(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<Vec<FolderPreview>> {
    let path = harness_file_path(paths, HARNESS_MD_NAME)?;
    let existing = read_existing(&path)?;
    Ok(vec![preview_folder(
        HARNESS_MD_NAME,
        mode,
        existing.as_deref(),
        HARNESS_MD,
        frontmatter_version,
    )])
}

/// Plan the single-file harness write. The version gate keys on the installed
/// protocol's frontmatter `version:`, mirroring the whole-folder skill gate for
/// a one-file resource.
fn plan_harness<'a>(
    paths: &MaestroPaths,
    contents: &'a str,
    mode: ExtractMode<'a>,
) -> Result<Vec<Action<'a>>> {
    let path = harness_file_path(paths, HARNESS_MD_NAME)?;
    let existing = read_existing(&path)?;
    let gate = folder_gate(
        mode,
        existing.as_deref(),
        contents,
        frontmatter_version,
        &path,
    )?;

    Ok(vec![file_action(
        HARNESS_MD_NAME,
        contents.as_bytes(),
        path,
        existing,
        gate,
    )?])
}

fn harness_file_path(paths: &MaestroPaths, file_name: &str) -> Result<PathBuf> {
    let relative = format!(".maestro/harness/{file_name}");
    managed_path(paths, &relative, SymlinkPolicy::RejectAllComponents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_harness_declares_a_frontmatter_version() {
        assert_eq!(frontmatter_version(HARNESS_MD).as_deref(), Some("1.1.0"));
    }
}
