//! Shared engine for extracting bundled resources into `.maestro/`.
//!
//! Skills, the hook recorder script, and the harness protocol each ship embedded
//! in the binary and extract on init/update through one version-gated core.
//! [`extract_all`] composes every resource family into a single operation with
//! unified rollback; each family's planner lives with its own module
//! ([`crate::domain::skills::extract`] for skills, [`hook_script`] here, and
//! [`crate::domain::harness::extract`] for the harness protocol).

pub(crate) mod extract;
pub(crate) mod hook_script;

use anyhow::Result;

use crate::domain::harness::extract::{extract_harness, preview_harness, validate_harness};
use crate::domain::skills::extract::{extract_skills, preview_skills, validate_skills};
use crate::foundation::core::paths::MaestroPaths;

pub use extract::{
    folder_decision, preview_folder, render_preview, rollback_writes, ExtractMode, ExtractReport,
    FolderDecision, FolderPreview, ResourceBackup, ResourceWrite,
};
pub use hook_script::{
    ensure_hook_script_exists, extract_hook_script, extract_hook_script_from, preview_hook_script,
    validate_hook_script,
};

/// Extract every bundled resource (skills, the hook script, then the harness
/// protocol) into `.maestro/`, merging their reports.
///
/// If a later resource fails after an earlier one has written, the earlier
/// writes are rolled back so the operation leaves no partial extraction.
pub fn extract_all(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<ExtractReport> {
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
    merge(report, extract_skills(paths, mode)?);
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
    validate_skills(paths, mode)?;
    validate_hook_script(paths, mode)?;
    validate_harness(paths, mode)?;
    Ok(())
}

/// Preview the whole-folder fate of every bundled resource (skills, the hook
/// script, then the harness) under `mode`, without writing files. Mirrors
/// [`validate_all`]; drives `--dry-run` and the merge drift hint.
pub fn preview_all(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<Vec<FolderPreview>> {
    let mut previews = preview_skills(paths, mode)?;
    previews.extend(preview_hook_script(paths, mode)?);
    previews.extend(preview_harness(paths, mode)?);
    Ok(previews)
}
