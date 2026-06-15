//! `maestro sync` -- resync bundled resources to this binary's shipped versions.
//!
//! Where `maestro upgrade` upgrades the binary *and* refreshes content, `sync`
//! only resyncs content (the hook recorder script and the harness protocol) to
//! whatever this binary ships. It runs the shared version-gated extraction
//! core ([`extract_all`] in `ExtractMode::Update`) directly, so it is purely
//! filesystem-bound: no network, no GitHub release lookup. Version-gated and
//! edit-preserving -- a folder whose installed version already matches is left
//! untouched; a drifted folder is backed up before overwrite.

use anyhow::{Result, bail};

use crate::domain::extraction::{
    ExtractMode, ExtractReport, FolderDecision, FolderPreview, extract_all, preview_all,
    render_preview,
};
use crate::domain::install::{
    MirrorBlockFate, MirrorBlockSync, preview_mirror_block_resync, resync_mirror_blocks,
};
use crate::domain::skills::{self, GlobalSkillsOutcome};
use crate::foundation::core::backup::backup_operation_timestamp;
use crate::foundation::core::error::MaestroError;
use crate::foundation::core::paths::{MaestroPaths, announce_repo_root, discover_repo_root};

/// Shown when `sync` runs outside an initialized project.
const NOT_INITIALIZED: &str = "no .maestro directory found here; run `maestro init` first";

/// Options for one `maestro sync` operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SyncOptions {
    /// Preview the resync without writing files.
    pub dry_run: bool,
    /// Resync the user-level Maestro global skill cache instead of repo-local resources.
    pub global_skills: bool,
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
        /// Per-file fate of the CLAUDE.md/AGENTS.md managed mirror blocks.
        mirrors: Vec<MirrorBlockSync>,
    },
    /// Sync only previewed the resync (`--dry-run`).
    DryRun {
        /// Per-folder fate of the bundled resources.
        folders: Vec<FolderPreview>,
        /// Per-file fate of the CLAUDE.md/AGENTS.md managed mirror blocks.
        mirrors: Vec<MirrorBlockSync>,
    },
    /// Sync resynced the user-level global skill cache and links.
    GlobalSkills(GlobalSkillsOutcome),
    /// Sync only previewed global skills (`--dry-run --global-skills`).
    GlobalSkillsDryRun(String),
}

/// Resync bundled resources to this binary's shipped versions. Offline and
/// edit-preserving; backs up drifted folders before overwriting them.
pub fn run(options: &SyncOptions) -> Result<SyncOutcome> {
    if options.global_skills {
        let prepared = skills::prepare_global_skills()?;
        if options.dry_run {
            return Ok(SyncOutcome::GlobalSkillsDryRun(
                skills::render_global_skills_dry_run(&prepared),
            ));
        }
        let outcome = skills::write_prepared_global_skills(prepared)?;
        return Ok(SyncOutcome::GlobalSkills(outcome));
    }

    let paths = sync_paths()?;
    announce_repo_root(paths.repo_root());

    if options.dry_run {
        let folders = preview_all(
            &paths,
            ExtractMode::Update {
                backup_timestamp: "",
            },
        )?;
        let mirrors = preview_mirror_block_resync(&paths)?;
        return Ok(SyncOutcome::DryRun { folders, mirrors });
    }

    let backup_timestamp = backup_operation_timestamp()?;
    let mode = ExtractMode::Update {
        backup_timestamp: &backup_timestamp,
    };
    // Compute the per-folder fate against the installed versions before the
    // writes change them, so the summary can report what sync did.
    let preview = preview_all(&paths, mode)?;
    let report = extract_all(&paths, mode)?;
    // The managed mirror blocks carry no version, so they resync by content:
    // a drifted block is refreshed (and backed up), a matching one untouched.
    let mirrors = resync_mirror_blocks(&paths, &backup_timestamp)?;

    Ok(SyncOutcome::Applied {
        preview,
        report,
        mirrors,
    })
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
        SyncOutcome::DryRun { folders, mirrors } => {
            let mut out = String::from("maestro sync would resync:\n");
            out.push_str(&render_preview(folders));
            let mirror_refresh = mirrors
                .iter()
                .any(|mirror| mirror.fate == MirrorBlockFate::Refresh);
            for mirror in mirrors
                .iter()
                .filter(|mirror| mirror.fate == MirrorBlockFate::Refresh)
            {
                out.push_str(&format!("  {} (mirror block)\n", mirror.relative_path));
            }
            // Pre-warn that a refresh replaces the on-disk copy (any local edits
            // included). The version gate skips matching folders, so an edited
            // but current folder is never touched; only a version-behind folder
            // refreshes, and sync backs the current copy up first (T6.s). Mirror
            // blocks resync by content, but the same back-up-first promise holds.
            if folders
                .iter()
                .any(|folder| folder.decision == FolderDecision::Refresh)
                || mirror_refresh
            {
                out.push_str(
                    "note: refresh overwrites the folder's current contents; your copy is backed up under .maestro/backups/ first\n",
                );
            }
            out
        }
        SyncOutcome::Applied {
            preview,
            report,
            mirrors,
        } => render_applied(preview, report, mirrors),
        SyncOutcome::GlobalSkills(outcome) => skills::render_global_skills_outcome(outcome),
        SyncOutcome::GlobalSkillsDryRun(rendered) => rendered.clone(),
    }
}

/// Render the applied summary: the folders that changed (if any), then a
/// one-line count, then any edited-file backups. The counts come from the
/// folder-level preview, matching what `--dry-run` would have shown.
fn render_applied(
    preview: &[FolderPreview],
    report: &ExtractReport,
    mirrors: &[MirrorBlockSync],
) -> String {
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

    let refreshed_mirrors: Vec<&MirrorBlockSync> = mirrors
        .iter()
        .filter(|mirror| mirror.fate == MirrorBlockFate::Refresh)
        .collect();

    let mut out = String::new();
    if !changed.is_empty() {
        out.push_str("maestro sync resynced:\n");
        out.push_str(&render_preview(&changed));
    }
    out.push_str(&format!(
        "synced: {refreshed} refreshed, {created} created, {current} already current\n"
    ));

    if !refreshed_mirrors.is_empty() {
        out.push_str("mirror blocks resynced:\n");
        for mirror in &refreshed_mirrors {
            out.push_str(&format!("  {}\n", mirror.relative_path));
        }
    }

    let mut backups: Vec<(String, String)> = report
        .backups
        .iter()
        .map(|backup| (backup.name.clone(), backup.path.display().to_string()))
        .collect();
    for mirror in &refreshed_mirrors {
        if let Some(backup_path) = &mirror.backup_path {
            backups.push((
                mirror.relative_path.clone(),
                backup_path.display().to_string(),
            ));
        }
    }
    if !backups.is_empty() {
        out.push_str("edited files backed up:\n");
        for (name, path) in backups {
            out.push_str(&format!("{name} -> {path}\n"));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str, decision: FolderDecision) -> FolderPreview {
        FolderPreview {
            name: name.to_string(),
            decision,
            installed_version: Some("1".to_string()),
            shipped_version: Some("2".to_string()),
        }
    }

    fn mirror(relative_path: &str, fate: MirrorBlockFate) -> MirrorBlockSync {
        MirrorBlockSync {
            relative_path: relative_path.to_string(),
            fate,
            backup_path: None,
        }
    }

    #[test]
    fn dry_run_warns_before_a_refresh_overwrites() {
        let outcome = SyncOutcome::DryRun {
            folders: vec![
                folder("a", FolderDecision::Skip),
                folder("b", FolderDecision::Refresh),
            ],
            mirrors: vec![],
        };
        let rendered = render(&outcome);
        assert!(
            rendered.contains("backed up under .maestro/backups/"),
            "a refresh-bound dry-run should pre-warn about the overwrite: {rendered}"
        );
    }

    #[test]
    fn dry_run_is_quiet_when_nothing_refreshes() {
        let outcome = SyncOutcome::DryRun {
            folders: vec![folder("a", FolderDecision::Skip)],
            mirrors: vec![mirror("CLAUDE.md", MirrorBlockFate::Current)],
        };
        assert!(!render(&outcome).contains("backed up under"));
    }

    #[test]
    fn dry_run_lists_and_warns_for_a_drifted_mirror_block() {
        let outcome = SyncOutcome::DryRun {
            folders: vec![folder("a", FolderDecision::Skip)],
            mirrors: vec![
                mirror("CLAUDE.md", MirrorBlockFate::Refresh),
                mirror("AGENTS.md", MirrorBlockFate::Unmanaged),
            ],
        };
        let rendered = render(&outcome);
        assert!(rendered.contains("CLAUDE.md (mirror block)"), "{rendered}");
        assert!(
            rendered.contains("backed up under .maestro/backups/"),
            "a drifted mirror block alone should still pre-warn: {rendered}"
        );
        assert!(
            !rendered.contains("AGENTS.md"),
            "an unmanaged mirror file is never mentioned: {rendered}"
        );
    }

    #[test]
    fn applied_reports_refreshed_mirror_blocks_with_backup() {
        let outcome = SyncOutcome::Applied {
            preview: vec![folder("a", FolderDecision::Skip)],
            report: ExtractReport::default(),
            mirrors: vec![MirrorBlockSync {
                relative_path: "CLAUDE.md".to_string(),
                fate: MirrorBlockFate::Refresh,
                backup_path: Some(std::path::PathBuf::from(
                    ".maestro/backups/x-sync/CLAUDE.md",
                )),
            }],
        };
        let rendered = render(&outcome);
        assert!(rendered.contains("mirror blocks resynced:"), "{rendered}");
        assert!(rendered.contains("CLAUDE.md"), "{rendered}");
        assert!(
            rendered.contains("CLAUDE.md -> .maestro/backups/x-sync/CLAUDE.md"),
            "the refreshed mirror's backup is listed: {rendered}"
        );
    }
}
