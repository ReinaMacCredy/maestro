//! Shared engine for extracting bundled resources into `.maestro/`.
//!
//! Skills, the hook recorder script, and the harness protocol each ship embedded
//! in the binary and extract on init/update through one version-gated core.
//! [`extract_all`] composes every resource family into a single operation with
//! unified rollback; each family's planner lives with its own module
//! ([`crate::domain::skills::extract`] for skills, [`hook_script`] here).

pub(crate) mod extract;
pub(crate) mod hook_script;

use anyhow::Result;

use crate::domain::skills::extract::{extract_skills, validate_skills};
use crate::foundation::core::paths::MaestroPaths;

pub use extract::{rollback_writes, ExtractMode, ExtractReport, ResourceBackup, ResourceWrite};
pub use hook_script::{extract_hook_script, extract_hook_script_from, validate_hook_script};

/// Extract every bundled resource (skills, then the hook script) into
/// `.maestro/`, merging their reports.
///
/// If a later resource fails after an earlier one has written, the earlier
/// writes are rolled back so the operation leaves no partial extraction.
pub fn extract_all(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<ExtractReport> {
    let mut report = extract_skills(paths, mode)?;
    match extract_hook_script(paths, mode) {
        Ok(hook) => {
            report.backups.extend(hook.backups);
            report.writes.extend(hook.writes);
            Ok(report)
        }
        Err(error) => {
            rollback_writes(&report)?;
            Err(error)
        }
    }
}

/// Validate extraction of every bundled resource without writing files.
pub fn validate_all(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<()> {
    validate_skills(paths, mode)?;
    validate_hook_script(paths, mode)?;
    Ok(())
}
