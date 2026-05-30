//! Shared, anchor-agnostic engine for extracting bundled resources into
//! `.maestro/` under a version gate.
//!
//! Skills, the hook recorder script, and the harness protocol all ship embedded
//! in the binary and extract to `.maestro/` on init/update. They share one
//! policy: a *folder gate* keyed on an anchor file's version decides whether to
//! skip (preserving local edits) or back up and overwrite. This module owns that
//! policy plus the write/rollback mechanics; each resource family supplies its
//! own planner that names the anchor file and the version reader.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::foundation::core::backup::backup_file_with_timestamp;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::{restore_or_remove, write_atomic};

/// Existing-file policy for bundled resource extraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtractMode<'a> {
    /// Error when a bundled anchor file already exists.
    Create,
    /// Keep existing bundled files.
    Merge,
    /// Back up and overwrite existing bundled files.
    Force { backup_timestamp: &'a str },
    /// Back up edited bundled files, then overwrite with bundled contents.
    Update { backup_timestamp: &'a str },
}

/// Summary of bundled resource extraction side effects.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct ExtractReport {
    /// Backups created before overwriting edited bundled files.
    pub backups: Vec<ResourceBackup>,
    /// Files written by this extraction.
    pub writes: Vec<ResourceWrite>,
}

/// A bundled resource backup created during extraction.
#[derive(Debug, Eq, PartialEq)]
pub struct ResourceBackup {
    /// Bundled resource name (e.g. a skill directory name or `record.sh`).
    pub name: String,
    /// Backup file path.
    pub path: PathBuf,
}

/// A bundled resource file written during extraction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceWrite {
    /// Bundled resource name.
    pub name: String,
    /// Written file path.
    pub path: PathBuf,
    /// Previous file contents, if the file existed before extraction.
    pub previous: Option<String>,
}

/// Roll back resource file writes recorded in an extraction report.
pub fn rollback_writes(report: &ExtractReport) -> Result<()> {
    for write in report.writes.iter().rev() {
        restore_or_remove(
            &write.path,
            write.previous.as_deref(),
            || format!("failed to roll back {}", write.path.display()),
            || format!("failed to roll back {}", write.path.display()),
        )?;
    }

    Ok(())
}

/// One planned write for a single bundled file.
#[derive(Debug)]
pub(crate) struct Action<'a> {
    pub(crate) name: &'a str,
    pub(crate) contents: &'a [u8],
    pub(crate) path: PathBuf,
    pub(crate) existing: Option<String>,
    pub(crate) backup_operation: Option<&'static str>,
    pub(crate) backup_timestamp: Option<&'a str>,
    pub(crate) write: bool,
}

#[derive(Debug)]
struct AppliedWrite {
    path: PathBuf,
    previous: Option<String>,
}

/// Whole-folder write decision derived from the installed anchor file.
#[derive(Clone, Copy, Debug)]
pub(crate) enum FolderGate<'a> {
    /// Fresh install: write missing files, reject existing ones.
    Create,
    /// Preserve every installed file (matching version or `--merge`).
    Skip,
    /// Back up and overwrite every installed file, write missing ones.
    Refresh {
        operation: &'static str,
        backup_timestamp: &'a str,
    },
}

/// Decide the whole-folder fate from the installed anchor contents.
///
/// `installed_anchor` is the on-disk anchor file (e.g. `SKILL.md`) when present;
/// `shipped_anchor` is the bundled anchor contents; `read_version` extracts the
/// comparable version marker from either (frontmatter `version:` for skills and
/// harness, a `# maestro:hook-version:` comment for the hook script). Comparing
/// those versions is what lets local edits survive across updates until the
/// shipped version changes.
pub(crate) fn folder_gate<'a>(
    mode: ExtractMode<'a>,
    installed_anchor: Option<&str>,
    shipped_anchor: &str,
    read_version: impl Fn(&str) -> Option<String>,
    anchor_path: &Path,
) -> Result<FolderGate<'a>> {
    Ok(match (installed_anchor, mode) {
        (None, _) => FolderGate::Create,
        (Some(_), ExtractMode::Merge) => FolderGate::Skip,
        (Some(_), ExtractMode::Force { backup_timestamp }) => FolderGate::Refresh {
            operation: "init",
            backup_timestamp,
        },
        (Some(installed), ExtractMode::Update { backup_timestamp }) => {
            // Version-gated: refresh only when the shipped version differs from
            // the installed one, so local edits survive across updates until the
            // shipped version changes. A missing installed version (None) differs
            // from the shipped Some(..), migrating pre-version installs.
            if read_version(installed) == read_version(shipped_anchor) {
                FolderGate::Skip
            } else {
                FolderGate::Refresh {
                    operation: "update",
                    backup_timestamp,
                }
            }
        }
        (Some(_), ExtractMode::Create) => {
            bail!(
                "{} already exists; use --merge to keep it or --force to overwrite with backup",
                anchor_path.display()
            );
        }
    })
}

/// Resolve one file's write decision from a folder gate into a planned action.
///
/// Shared by every resource planner: skills call it per tree file, while the
/// hook script and harness call it once. In `Create` mode an existing file is a
/// hard error; otherwise the gate decides whether to skip, write, or back up
/// the edited file before overwriting.
pub(crate) fn file_action<'a>(
    name: &'a str,
    contents: &'a [u8],
    path: PathBuf,
    existing: Option<String>,
    gate: FolderGate<'a>,
) -> Result<Action<'a>> {
    let (write, backup_operation, backup_timestamp) = match gate {
        FolderGate::Create => match existing {
            Some(_) => bail!(
                "{} already exists; use --merge to keep it or --force to overwrite with backup",
                path.display()
            ),
            None => (true, None, None),
        },
        FolderGate::Skip => (existing.is_none(), None, None),
        FolderGate::Refresh {
            operation,
            backup_timestamp,
        } => match existing {
            Some(_) => (true, Some(operation), Some(backup_timestamp)),
            None => (true, None, None),
        },
    };

    Ok(Action {
        name,
        contents,
        path,
        existing,
        backup_operation,
        backup_timestamp,
        write,
    })
}

/// Read an existing file's contents, returning `None` when it is absent.
pub(crate) fn read_existing(path: &Path) -> Result<Option<String>> {
    if path.exists() {
        Ok(Some(fs::read_to_string(path).with_context(|| {
            format!("failed to read bundled resource {}", path.display())
        })?))
    } else {
        Ok(None)
    }
}

/// Apply planned writes, backing up edited files first and rolling back on error.
pub(crate) fn apply_actions(
    paths: &MaestroPaths,
    actions: &[Action<'_>],
    report: &mut ExtractReport,
) -> Result<()> {
    let mut written = Vec::new();

    for action in actions {
        if !action.write {
            continue;
        }
        if let (Some(operation), Some(timestamp)) =
            (action.backup_operation, action.backup_timestamp)
        {
            let backup = match backup_file_with_timestamp(paths, &action.path, operation, timestamp)
            {
                Ok(backup) => backup,
                Err(error) => {
                    rollback_applied_writes(&written)?;
                    return Err(error);
                }
            };
            report.backups.push(ResourceBackup {
                name: action.name.to_string(),
                path: backup,
            });
        }
        if let Err(error) = write_atomic(&action.path, action.contents)
            .with_context(|| format!("failed to write bundled resource {}", action.path.display()))
        {
            rollback_applied_writes(&written)?;
            return Err(error);
        }
        written.push(AppliedWrite {
            path: action.path.clone(),
            previous: action.existing.clone(),
        });
        report.writes.push(ResourceWrite {
            name: action.name.to_string(),
            path: action.path.clone(),
            previous: action.existing.clone(),
        });
    }

    Ok(())
}

fn rollback_applied_writes(written: &[AppliedWrite]) -> Result<()> {
    for write in written.iter().rev() {
        restore_or_remove(
            &write.path,
            write.previous.as_deref(),
            || format!("failed to roll back {}", write.path.display()),
            || format!("failed to roll back {}", write.path.display()),
        )?;
    }

    Ok(())
}
