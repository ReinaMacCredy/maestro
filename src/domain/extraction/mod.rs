//! Shared engine for extracting bundled resources into `.maestro/`.
//!
//! The hook recorder script and the harness protocol each ship embedded in the
//! binary and extract on init/update through one version-gated core.
//! [`extract_all`] composes every resource family into a single operation with
//! unified rollback; each family's planner lives with its own module
//! (`hook_script` here, [`crate::domain::harness::extract`] for the harness
//! protocol). Skills are not extracted per repo; they are served from the
//! global `~/.maestro/skills` cache (see [`crate::domain::skills`]). The code
//! playbook is likewise served from the binary (`maestro playbook`, see
//! [`crate::domain::playbook`]); [`extract_all`] can remove the obsolete
//! `.maestro/playbook/` folder a pre-1.16 install may have left, but only through
//! visible backup-first refresh modes.

pub(crate) mod extract;
pub(crate) mod hook_script;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::domain::harness::extract::{extract_harness, preview_harness, validate_harness};
use crate::foundation::core::error::MaestroError;
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
/// into `.maestro/`, merging their reports. Removes the obsolete
/// `.maestro/playbook/` folder only after the normal extraction path succeeds.
///
/// If a later resource fails after an earlier one has written, the earlier
/// writes are rolled back so the operation leaves no partial extraction.
pub fn extract_all(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<ExtractReport> {
    let mut report = ExtractReport::default();
    match extract_into(&mut report, paths, mode)
        .and_then(|_| remove_obsolete_playbook_folder(paths, mode, &mut report))
    {
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
    validate_obsolete_playbook_folder(paths, mode)?;
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
    if let Some(preview) = preview_obsolete_playbook_folder(paths, mode)? {
        previews.push(preview);
    }
    Ok(previews)
}

/// Remove the pre-1.16 `.maestro/playbook/` folder if a prior install extracted
/// one. The code playbook is served from the binary now (`maestro playbook`),
/// so the per-repo copy is obsolete. Refresh modes back the folder up before
/// removal; merge preserves it; create conflicts if it already exists.
fn remove_obsolete_playbook_folder(
    paths: &MaestroPaths,
    mode: ExtractMode<'_>,
    report: &mut ExtractReport,
) -> Result<()> {
    let dir = managed_path(
        paths,
        ".maestro/playbook",
        SymlinkPolicy::RejectAllComponents,
    )?;
    if !dir.exists() {
        return Ok(());
    }
    let Some((operation, backup_timestamp)) = cleanup_policy(mode, &dir)? else {
        return Ok(());
    };
    let backup = move_directory_to_backup(paths, &dir, operation, backup_timestamp)?;
    report.backups.push(ResourceBackup {
        name: "playbook".to_string(),
        path: backup,
    });
    Ok(())
}

fn validate_obsolete_playbook_folder(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<()> {
    let dir = managed_path(
        paths,
        ".maestro/playbook",
        SymlinkPolicy::RejectAllComponents,
    )?;
    if dir.exists() {
        let _ = cleanup_policy(mode, &dir)?;
    }
    Ok(())
}

fn preview_obsolete_playbook_folder(
    paths: &MaestroPaths,
    mode: ExtractMode<'_>,
) -> Result<Option<FolderPreview>> {
    let dir = managed_path(
        paths,
        ".maestro/playbook",
        SymlinkPolicy::RejectAllComponents,
    )?;
    if !dir.exists() {
        return Ok(None);
    }
    let decision = match mode {
        ExtractMode::Create => FolderDecision::Conflict,
        ExtractMode::Merge => FolderDecision::Skip,
        ExtractMode::Force { .. } | ExtractMode::Update { .. } => FolderDecision::Refresh,
    };
    Ok(Some(FolderPreview {
        name: "playbook".to_string(),
        decision,
        installed_version: Some("obsolete".to_string()),
        shipped_version: None,
    }))
}

fn cleanup_policy<'a>(
    mode: ExtractMode<'a>,
    dir: &Path,
) -> Result<Option<(&'static str, &'a str)>> {
    match mode {
        ExtractMode::Create => bail!(
            "{} already exists; use --merge to keep it or --force to remove it with backup",
            dir.display()
        ),
        ExtractMode::Merge => Ok(None),
        ExtractMode::Force { backup_timestamp } => Ok(Some(("init", backup_timestamp))),
        ExtractMode::Update { backup_timestamp } => Ok(Some(("update", backup_timestamp))),
    }
}

fn move_directory_to_backup(
    paths: &MaestroPaths,
    source: &Path,
    operation: &str,
    timestamp: &str,
) -> Result<PathBuf> {
    let source = source
        .canonicalize()
        .with_context(|| format!("failed to resolve backup source {}", source.display()))?;
    let repo_root = paths.repo_root().canonicalize().with_context(|| {
        format!(
            "failed to resolve repo root {}",
            paths.repo_root().display()
        )
    })?;
    let relative =
        source
            .strip_prefix(&repo_root)
            .map_err(|_| MaestroError::OutsideRepository {
                path: source.clone(),
            })?;
    let destination = paths
        .backups_dir()
        .join(format!("{timestamp}-{operation}"))
        .join(relative);
    reject_tree_symlinks(&source)?;
    if destination.exists() {
        bail!(
            "backup destination already exists: {}",
            destination.display()
        );
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::rename(&source, &destination).with_context(|| {
        format!(
            "failed to move obsolete {} to backup {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(destination)
}

fn reject_tree_symlinks(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(MaestroError::BackupPathContainsSymlink {
            path: path.to_path_buf(),
        }
        .into());
    }
    if metadata.is_dir() {
        for entry in
            fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))?
        {
            let entry = entry?;
            reject_tree_symlinks(&entry.path())?;
        }
    }
    Ok(())
}
