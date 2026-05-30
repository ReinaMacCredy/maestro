use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::domain::skills::catalog::{frontmatter_version, skills, Skill};
use crate::foundation::core::backup::backup_file_with_timestamp;
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::safe_write::{write_atomic, write_string_atomic};

/// Existing-file policy for bundled skill extraction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtractMode<'a> {
    /// Error when a bundled `SKILL.md` already exists.
    Create,
    /// Keep existing bundled `SKILL.md` files.
    Merge,
    /// Back up and overwrite existing bundled `SKILL.md` files.
    Force { backup_timestamp: &'a str },
    /// Back up edited bundled `SKILL.md` files, then overwrite with bundled contents.
    Update { backup_timestamp: &'a str },
}

/// Summary of bundled skill extraction side effects.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct ExtractReport {
    /// Backups created before overwriting edited bundled skill files.
    pub backups: Vec<SkillBackup>,
    /// Skill files written by this extraction.
    pub writes: Vec<SkillWrite>,
}

/// A bundled skill backup created during extraction.
#[derive(Debug, Eq, PartialEq)]
pub struct SkillBackup {
    /// Bundled skill name.
    pub skill_name: String,
    /// Backup file path.
    pub path: PathBuf,
}

/// A bundled skill file written during extraction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkillWrite {
    /// Bundled skill name.
    pub skill_name: String,
    /// Skill file path.
    pub path: PathBuf,
    /// Previous file contents, if the file existed before extraction.
    pub previous: Option<String>,
}

/// Extract all bundled skills into `.maestro/skills/`.
pub fn extract_skills(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<ExtractReport> {
    extract_skills_from(paths, skills(), mode)
}

/// Extract the given skills into `.maestro/skills/`.
///
/// Exposed so tests can drive the writer with a synthetic multi-file skill;
/// production callers go through [`extract_skills`] with the bundled catalog.
pub fn extract_skills_from(
    paths: &MaestroPaths,
    skills: &[Skill],
    mode: ExtractMode<'_>,
) -> Result<ExtractReport> {
    managed_path(paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    ensure_dir(paths.skills_dir())?;
    let mut report = ExtractReport::default();
    let actions = plan_skills(paths, skills, mode)?;

    apply_actions(paths, &actions, &mut report)?;

    Ok(report)
}

/// Validate bundled skill extraction without writing files.
pub fn validate_skills(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<()> {
    managed_path(paths, ".maestro", SymlinkPolicy::RejectAllComponents)?;
    plan_skills(paths, skills(), mode)?;
    Ok(())
}

fn plan_skills<'a>(
    paths: &MaestroPaths,
    skills: &'a [Skill],
    mode: ExtractMode<'a>,
) -> Result<Vec<SkillAction<'a>>> {
    let mut actions = Vec::new();
    for skill in skills {
        actions.extend(plan_skill(paths, skill, mode)?);
    }
    Ok(actions)
}

/// Roll back skill file writes recorded in an extraction report.
pub fn rollback_skill_writes(report: &ExtractReport) -> Result<()> {
    for write in report.writes.iter().rev() {
        match &write.previous {
            Some(contents) => write_string_atomic(&write.path, contents)
                .with_context(|| format!("failed to roll back {}", write.path.display()))?,
            None => match fs::remove_file(&write.path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to roll back {}", write.path.display()));
                }
            },
        }
    }

    Ok(())
}

#[derive(Debug)]
struct SkillAction<'a> {
    skill_name: &'a str,
    contents: &'a [u8],
    path: PathBuf,
    existing: Option<String>,
    backup_operation: Option<&'static str>,
    backup_timestamp: Option<&'a str>,
    write: bool,
}

#[derive(Debug)]
struct AppliedWrite {
    path: PathBuf,
    previous: Option<String>,
}

/// Plan one action per file in `skill`. The version gate is whole-folder: the
/// installed `SKILL.md` version decides the fate of every file in the tree, so
/// local edits to any file survive until the shipped `version:` changes.
fn plan_skill<'a>(
    paths: &MaestroPaths,
    skill: &'a Skill,
    mode: ExtractMode<'a>,
) -> Result<Vec<SkillAction<'a>>> {
    let skill_md_path = skill_file_path(paths, skill.name, "SKILL.md")?;
    let installed_skill_md = read_existing(&skill_md_path)?;
    let folder_gate = folder_gate(skill, mode, installed_skill_md.as_deref(), &skill_md_path)?;

    let mut actions = Vec::with_capacity(skill.files.len());
    for file in &skill.files {
        let path = skill_file_path(paths, skill.name, file.relative_path)?;
        let existing = if file.relative_path == "SKILL.md" {
            installed_skill_md.clone()
        } else {
            read_existing(&path)?
        };
        let (write, backup_operation, backup_timestamp) = match folder_gate {
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

        actions.push(SkillAction {
            skill_name: skill.name,
            contents: file.contents,
            path,
            existing,
            backup_operation,
            backup_timestamp,
            write,
        });
    }

    Ok(actions)
}

/// Whole-folder write decision derived from the installed `SKILL.md`.
#[derive(Clone, Copy, Debug)]
enum FolderGate<'a> {
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

fn folder_gate<'a>(
    skill: &Skill,
    mode: ExtractMode<'a>,
    installed_skill_md: Option<&str>,
    skill_md_path: &std::path::Path,
) -> Result<FolderGate<'a>> {
    Ok(match (installed_skill_md, mode) {
        (None, _) => FolderGate::Create,
        (Some(_), ExtractMode::Merge) => FolderGate::Skip,
        (Some(_), ExtractMode::Force { backup_timestamp }) => FolderGate::Refresh {
            operation: "init",
            backup_timestamp,
        },
        (Some(installed), ExtractMode::Update { backup_timestamp }) => {
            // Version-gated: refresh only when the shipped version differs from
            // the installed one, so local edits survive across updates until the
            // shipped `version:` changes. A missing installed version (None)
            // differs from the shipped Some(..), migrating pre-version installs.
            if frontmatter_version(installed) == frontmatter_version(skill.skill_md()) {
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
                skill_md_path.display()
            );
        }
    })
}

fn skill_file_path(paths: &MaestroPaths, skill_name: &str, relative_path: &str) -> Result<PathBuf> {
    let relative = format!(".maestro/skills/{skill_name}/{relative_path}");
    managed_path(paths, &relative, SymlinkPolicy::RejectAllComponents)
}

fn read_existing(path: &std::path::Path) -> Result<Option<String>> {
    if path.exists() {
        Ok(Some(fs::read_to_string(path).with_context(|| {
            format!("failed to read bundled skill {}", path.display())
        })?))
    } else {
        Ok(None)
    }
}

fn apply_actions(
    paths: &MaestroPaths,
    actions: &[SkillAction<'_>],
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
                    rollback_writes(&written)?;
                    return Err(error);
                }
            };
            report.backups.push(SkillBackup {
                skill_name: action.skill_name.to_string(),
                path: backup,
            });
        }
        if let Err(error) = write_atomic(&action.path, action.contents)
            .with_context(|| format!("failed to write bundled skill {}", action.path.display()))
        {
            rollback_writes(&written)?;
            return Err(error);
        }
        written.push(AppliedWrite {
            path: action.path.clone(),
            previous: action.existing.clone(),
        });
        report.writes.push(SkillWrite {
            skill_name: action.skill_name.to_string(),
            path: action.path.clone(),
            previous: action.existing.clone(),
        });
    }

    Ok(())
}

fn rollback_writes(written: &[AppliedWrite]) -> Result<()> {
    for write in written.iter().rev() {
        match &write.previous {
            Some(contents) => write_string_atomic(&write.path, contents)
                .with_context(|| format!("failed to roll back {}", write.path.display()))?,
            None => match fs::remove_file(&write.path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to roll back {}", write.path.display()));
                }
            },
        }
    }

    Ok(())
}
