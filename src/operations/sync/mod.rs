//! `maestro sync` -- resync bundled resources to this binary's shipped versions.
//!
//! Where `maestro update` upgrades the binary *and* refreshes content, `sync`
//! only resyncs content (skills, the hook recorder script, the harness protocol)
//! to whatever this binary ships. It runs the shared version-gated extraction
//! core ([`extract_all`] in `ExtractMode::Update`) directly, so it is purely
//! filesystem-bound: no network, no GitHub release lookup. Version-gated and
//! edit-preserving -- a folder whose installed version already matches is left
//! untouched; a drifted folder is backed up before overwrite.

use anyhow::{bail, Result};

use crate::domain::extraction::{
    extract_all, preview_all, render_preview, ExtractMode, ExtractReport, FolderDecision,
    FolderPreview,
};
use crate::foundation::core::backup::backup_operation_timestamp;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::paths::{discover_repo_root, MaestroPaths};

/// Shown when `sync` runs outside an initialized project.
const NOT_INITIALIZED: &str = "no .maestro directory found here; run `maestro init` first";

/// Options for one `maestro sync` operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SyncOptions {
    /// Preview the resync without writing files.
    pub dry_run: bool,
}

/// Result of a sync operation.
#[derive(Debug)]
pub enum SyncOutcome {
    /// Sync resynced resources (or found every folder already current).
    Applied {
        /// Per-folder fate computed before writing, for the summary.
        preview: Vec<FolderPreview>,
        /// The writes and backups sync performed.
        report: ExtractReport,
    },
    /// Sync only previewed the resync (`--dry-run`).
    DryRun(Vec<FolderPreview>),
}

/// Resync bundled resources to this binary's shipped versions. Offline and
/// edit-preserving; backs up drifted folders before overwriting them.
pub fn run(options: &SyncOptions) -> Result<SyncOutcome> {
    let paths = sync_paths()?;

    if options.dry_run {
        let preview = preview_all(&paths, ExtractMode::Update { backup_timestamp: "" })?;
        return Ok(SyncOutcome::DryRun(preview));
    }

    let backup_timestamp = backup_operation_timestamp()?;
    let mode = ExtractMode::Update {
        backup_timestamp: &backup_timestamp,
    };
    // Compute the per-folder fate against the installed versions before the
    // writes change them, so the summary can report what sync did.
    let preview = preview_all(&paths, mode)?;
    let report = extract_all(&paths, mode)?;

    Ok(SyncOutcome::Applied { preview, report })
}

/// Resolve the project paths, failing fast with the `maestro init` hint when no
/// initialized `.maestro` is reachable.
fn sync_paths() -> Result<MaestroPaths> {
    let repo_root = match discover_repo_root() {
        Ok(repo_root) => repo_root,
        Err(error)
            if matches!(
                error.downcast_ref::<MaestroError>(),
                Some(MaestroError::RepoRootNotFound { .. })
            ) =>
        {
            bail!(NOT_INITIALIZED);
        }
        Err(error) => return Err(error),
    };

    let paths = MaestroPaths::new(repo_root);
    if !paths.maestro_dir().is_dir() {
        bail!(NOT_INITIALIZED);
    }

    Ok(paths)
}

/// Render a sync outcome as plain, machine-useful text.
pub fn render(outcome: &SyncOutcome) -> String {
    match outcome {
        SyncOutcome::DryRun(preview) => {
            let mut out = String::from("maestro sync would resync:\n");
            out.push_str(&render_preview(preview));
            out
        }
        SyncOutcome::Applied { preview, report } => render_applied(preview, report),
    }
}

/// Render the applied summary: the folders that changed (if any), then a
/// one-line count, then any edited-file backups. The counts come from the
/// folder-level preview, matching what `--dry-run` would have shown.
fn render_applied(preview: &[FolderPreview], report: &ExtractReport) -> String {
    let refreshed = count_decision(preview, FolderDecision::Refresh);
    let created = count_decision(preview, FolderDecision::Create);
    let current = count_decision(preview, FolderDecision::Skip);

    let changed: Vec<FolderPreview> = preview
        .iter()
        .filter(|folder| {
            matches!(
                folder.decision,
                FolderDecision::Refresh | FolderDecision::Create
            )
        })
        .cloned()
        .collect();

    let mut out = String::new();
    if !changed.is_empty() {
        out.push_str("maestro sync resynced:\n");
        out.push_str(&render_preview(&changed));
    }
    out.push_str(&format!(
        "synced: {refreshed} refreshed, {created} created, {current} already current\n"
    ));

    if !report.backups.is_empty() {
        out.push_str("edited files backed up:\n");
        for backup in &report.backups {
            out.push_str(&format!("{} -> {}\n", backup.name, backup.path.display()));
        }
    }

    out
}

fn count_decision(preview: &[FolderPreview], decision: FolderDecision) -> usize {
    preview
        .iter()
        .filter(|folder| folder.decision == decision)
        .count()
}
