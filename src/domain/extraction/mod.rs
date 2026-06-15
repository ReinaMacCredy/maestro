//! Shared engine for extracting bundled resources into `.maestro/`.
//!
//! The hook recorder script and the harness protocol each ship embedded in the
//! binary and extract on init/update through one version-gated core.
//! [`extract_all`] composes every resource family into a single operation with
//! unified rollback; each family's planner lives with its own module
//! ([`hook_script`] here, [`crate::domain::harness::extract`] for the harness
//! protocol). Skills are not extracted per repo; they are served from the
//! global `~/.maestro/skills` cache (see [`crate::domain::skills`]). The code
//! playbook is likewise served from the binary (`maestro playbook`, see
//! [`crate::domain::playbook`]); [`extract_all`] only removes the obsolete
//! `.maestro/playbook/` folder a pre-1.16 install may have left.

pub(crate) mod extract;
pub(crate) mod hook_script;

use anyhow::{Context, Result};

use crate::domain::harness::extract::{extract_harness, preview_harness, validate_harness};
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
use crate::foundation::core::paths::MaestroPaths;

pub use extract::{
    ExtractMode, ExtractReport, FolderDecision, FolderPreview, ResourceBackup, ResourceWrite,
    folder_decision, preview_folder, render_preview, rollback_writes,
};
pub use hook_script::{
    ensure_hook_script_exists, extract_hook_script, extract_hook_script_from, preview_hook_script,
    validate_hook_script,
};

/// Extract every bundled resource (the hook script, then the harness protocol)
/// into `.maestro/`, merging their reports. First removes the obsolete
/// `.maestro/playbook/` folder, now served from the binary.
///
/// If a later resource fails after an earlier one has written, the earlier
/// writes are rolled back so the operation leaves no partial extraction.
pub fn extract_all(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<ExtractReport> {
    remove_obsolete_playbook_folder(paths)?;
    let mut report = ExtractReport::default();
    match extract_into(&mut report, paths, mode) {
        Ok(()) => Ok(report),
        Err(error) => {
            rollback_writes(&report)?;
            Err(error)
        }
    }
}

/// Run every resource extractor in turn, merging each report into `report` so a
/// failure leaves the accumulated writes available for rollback by the caller.
fn extract_into(
    report: &mut ExtractReport,
    paths: &MaestroPaths,
    mode: ExtractMode<'_>,
) -> Result<()> {
    merge(report, extract_hook_script(paths, mode)?);
    merge(report, extract_harness(paths, mode)?);
    Ok(())
}

fn merge(report: &mut ExtractReport, mut other: ExtractReport) {
    report.backups.append(&mut other.backups);
    report.writes.append(&mut other.writes);
}

/// Validate extraction of every bundled resource without writing files.
pub fn validate_all(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<()> {
    validate_hook_script(paths, mode)?;
    validate_harness(paths, mode)?;
    Ok(())
}

/// Preview the whole-folder fate of every bundled resource (the hook script,
/// then the harness) under `mode`, without writing files. Mirrors
/// [`validate_all`]; drives `--dry-run` and the merge drift hint.
pub fn preview_all(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<Vec<FolderPreview>> {
    let mut previews = preview_hook_script(paths, mode)?;
    previews.extend(preview_harness(paths, mode)?);
    Ok(previews)
}

/// Remove the pre-1.16 `.maestro/playbook/` folder if a prior install extracted
/// one. The code playbook is served from the binary now (`maestro playbook`),
/// so the per-repo copy is obsolete; it is deleted outright with no backup --
/// the guides are vendored/embedded, not local work worth preserving.
/// Idempotent: a fresh repo has nothing to remove. `managed_path` rejects a
/// symlinked component so the removal can never escape `.maestro/`.
fn remove_obsolete_playbook_folder(paths: &MaestroPaths) -> Result<()> {
    let dir = managed_path(paths, ".maestro/playbook", SymlinkPolicy::RejectAllComponents)?;
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to remove obsolete {}", dir.display()))
        }
    }
}
