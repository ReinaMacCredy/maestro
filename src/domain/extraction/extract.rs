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

use anyhow::{Context, Result, bail};

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

/// The whole-folder fate of a bundled resource, independent of write-path
/// concerns (backup operation label, timestamp). Single source of truth shared
/// by [`folder_gate`] (the write path) and [`preview_folder`] (the read-only
/// `--dry-run` path), so the two can never drift.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FolderDecision {
    /// No installed anchor: a fresh install writes every file.
    Create,
    /// Preserve every installed file (matching version or `--merge`).
    Skip,
    /// Back up edited files and overwrite (version drift, `--force`).
    Refresh,
    /// `Create` mode but the anchor already exists: a hard conflict.
    Conflict,
}

/// Decide the whole-folder fate from the installed anchor contents.
///
/// `installed_anchor` is the on-disk anchor file (e.g. `SKILL.md`) when present;
/// `shipped_anchor` is the bundled anchor contents; `read_version` extracts the
/// comparable version marker from either (frontmatter `version:` for skills and
/// harness, a `# maestro:hook-version:` comment for the hook script). Comparing
/// those versions is what lets local edits survive across updates until the
/// shipped version changes.
pub fn folder_decision(
    mode: ExtractMode<'_>,
    installed_anchor: Option<&str>,
    shipped_anchor: &str,
    read_version: impl Fn(&str) -> Option<String>,
) -> FolderDecision {
    match (installed_anchor, mode) {
        (None, _) => FolderDecision::Create,
        (Some(_), ExtractMode::Merge) => FolderDecision::Skip,
        (Some(_), ExtractMode::Force { .. }) => FolderDecision::Refresh,
        (Some(installed), ExtractMode::Update { .. }) => {
            // Version-gated: refresh only when the shipped version differs from
            // the installed one, so local edits survive across updates until the
            // shipped version changes. A missing installed version (None) differs
            // from the shipped Some(..), migrating pre-version installs.
            if read_version(installed) == read_version(shipped_anchor) {
                FolderDecision::Skip
            } else {
                FolderDecision::Refresh
            }
        }
        (Some(_), ExtractMode::Create) => FolderDecision::Conflict,
    }
}

/// Resolve the folder gate for the write path: the [`folder_decision`] plus the
/// backup operation label and timestamp that only matter when actually writing.
pub(crate) fn folder_gate<'a>(
    mode: ExtractMode<'a>,
    installed_anchor: Option<&str>,
    shipped_anchor: &str,
    read_version: impl Fn(&str) -> Option<String>,
    anchor_path: &Path,
) -> Result<FolderGate<'a>> {
    Ok(
        match folder_decision(mode, installed_anchor, shipped_anchor, read_version) {
            FolderDecision::Create => FolderGate::Create,
            FolderDecision::Skip => FolderGate::Skip,
            FolderDecision::Refresh => match mode {
                ExtractMode::Force { backup_timestamp } => FolderGate::Refresh {
                    operation: "init",
                    backup_timestamp,
                },
                ExtractMode::Update { backup_timestamp } => FolderGate::Refresh {
                    operation: "update",
                    backup_timestamp,
                },
                // folder_decision yields Refresh only in Force/Update modes.
                ExtractMode::Create | ExtractMode::Merge => {
                    unreachable!("Refresh decision arises only in Force/Update modes")
                }
            },
            FolderDecision::Conflict => bail!(
                "{} already exists; use --merge to keep it or --force to overwrite with backup",
                anchor_path.display()
            ),
        },
    )
}

/// One folder's previewed fate plus the versions that drove it, for `--dry-run`.
///
/// Granularity is whole-folder, matching [`folder_decision`]: a `Skip` means the
/// folder's version matches, not that zero files are written -- in Merge/Update
/// mode `file_action` still creates a file missing from an otherwise-current
/// folder. The preview reports the dominant per-folder decision, not a per-file
/// write count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FolderPreview {
    /// Resource name, reusing the [`ResourceWrite`] vocabulary (a skill
    /// directory name, `record.sh`, or `HARNESS.md`).
    pub name: String,
    /// The folder's fate under the previewed mode.
    pub decision: FolderDecision,
    /// Installed anchor version, if an anchor is present and carries one.
    pub installed_version: Option<String>,
    /// Bundled anchor version, if it carries one.
    pub shipped_version: Option<String>,
}

/// Compute one [`FolderPreview`] from the same inputs as [`folder_gate`],
/// without touching the filesystem beyond the `read_version` the caller already
/// performed. Drives `sync --dry-run`, `init --dry-run`, and the merge drift hint.
pub fn preview_folder(
    name: impl Into<String>,
    mode: ExtractMode<'_>,
    installed_anchor: Option<&str>,
    shipped_anchor: &str,
    read_version: impl Fn(&str) -> Option<String>,
) -> FolderPreview {
    FolderPreview {
        name: name.into(),
        decision: folder_decision(mode, installed_anchor, shipped_anchor, &read_version),
        installed_version: installed_anchor.and_then(&read_version),
        shipped_version: read_version(shipped_anchor),
    }
}

impl FolderDecision {
    /// Stable, machine-greppable verb for `--dry-run` output.
    pub fn verb(self) -> &'static str {
        match self {
            FolderDecision::Create => "create",
            FolderDecision::Skip => "skip",
            FolderDecision::Refresh => "refresh",
            FolderDecision::Conflict => "conflict",
        }
    }
}

/// Render previews as aligned `<verb> <name> (<detail>)` lines, one per folder.
/// The caller supplies the header (`maestro sync would refresh:` etc.); this
/// renders only the body so `init` and `sync` share one format.
pub fn render_preview(previews: &[FolderPreview]) -> String {
    let mut out = String::new();
    for preview in previews {
        let detail = match preview.decision {
            FolderDecision::Create => String::new(),
            FolderDecision::Skip => preview
                .shipped_version
                .as_deref()
                .map(|version| format!(" ({version})"))
                .unwrap_or_default(),
            FolderDecision::Refresh => {
                let from = preview
                    .installed_version
                    .as_deref()
                    .unwrap_or("unversioned");
                let to = preview.shipped_version.as_deref().unwrap_or("unversioned");
                format!(" ({from} -> {to})")
            }
            FolderDecision::Conflict => " (already exists)".to_string(),
        };
        out.push_str(&format!(
            "{:<8} {}{}\n",
            preview.decision.verb(),
            preview.name,
            detail
        ));
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Treat the whole anchor string as its version, so tests control drift by
    /// passing matching or differing strings.
    fn version(anchor: &str) -> Option<String> {
        Some(anchor.to_string())
    }

    #[test]
    fn folder_decision_covers_every_mode_and_anchor_combination() {
        // Missing anchor is always a fresh create, whatever the mode.
        assert_eq!(
            folder_decision(ExtractMode::Create, None, "v1", version),
            FolderDecision::Create
        );
        assert_eq!(
            folder_decision(
                ExtractMode::Update {
                    backup_timestamp: ""
                },
                None,
                "v1",
                version
            ),
            FolderDecision::Create
        );
        // Create mode over an existing anchor is a hard conflict.
        assert_eq!(
            folder_decision(ExtractMode::Create, Some("v1"), "v1", version),
            FolderDecision::Conflict
        );
        // Merge keeps the install regardless of version.
        assert_eq!(
            folder_decision(ExtractMode::Merge, Some("old"), "new", version),
            FolderDecision::Skip
        );
        // Force always refreshes an existing anchor.
        assert_eq!(
            folder_decision(
                ExtractMode::Force {
                    backup_timestamp: ""
                },
                Some("v1"),
                "v1",
                version
            ),
            FolderDecision::Refresh
        );
        // Update is version-gated: skip on a match, refresh on drift.
        assert_eq!(
            folder_decision(
                ExtractMode::Update {
                    backup_timestamp: ""
                },
                Some("v1"),
                "v1",
                version
            ),
            FolderDecision::Skip
        );
        assert_eq!(
            folder_decision(
                ExtractMode::Update {
                    backup_timestamp: ""
                },
                Some("v1"),
                "v2",
                version
            ),
            FolderDecision::Refresh
        );
    }

    #[test]
    fn render_preview_formats_each_decision() {
        let previews = vec![
            FolderPreview {
                name: "a".to_string(),
                decision: FolderDecision::Create,
                installed_version: None,
                shipped_version: Some("1".to_string()),
            },
            FolderPreview {
                name: "b".to_string(),
                decision: FolderDecision::Skip,
                installed_version: Some("1".to_string()),
                shipped_version: Some("1".to_string()),
            },
            FolderPreview {
                name: "c".to_string(),
                decision: FolderDecision::Refresh,
                installed_version: Some("1".to_string()),
                shipped_version: Some("2".to_string()),
            },
            FolderPreview {
                name: "d".to_string(),
                decision: FolderDecision::Conflict,
                installed_version: None,
                shipped_version: Some("1".to_string()),
            },
        ];

        assert_eq!(
            render_preview(&previews),
            "create   a\nskip     b (1)\nrefresh  c (1 -> 2)\nconflict d (already exists)\n"
        );
    }

    #[test]
    fn render_preview_marks_a_missing_installed_version_as_unversioned() {
        let previews = vec![FolderPreview {
            name: "x".to_string(),
            decision: FolderDecision::Refresh,
            installed_version: None,
            shipped_version: Some("2".to_string()),
        }];

        assert_eq!(render_preview(&previews), "refresh  x (unversioned -> 2)\n");
    }
}
