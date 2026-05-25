use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::core::backup::backup_file_with_timestamp;
use crate::core::fs::ensure_dir;
use crate::core::managed_path::{managed_path, SymlinkPolicy};
use crate::core::paths::MaestroPaths;
use crate::core::safe_write::write_string_atomic;
use crate::skills::bundled::{bundled_skills, BundledSkill};

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
pub fn extract_bundled_skills(
    paths: &MaestroPaths,
    mode: ExtractMode<'_>,
) -> Result<ExtractReport> {
    ensure_dir(paths.skills_dir())?;
    let mut report = ExtractReport::default();
    let actions = bundled_skills()
        .iter()
        .map(|skill| plan_skill(paths, skill, mode))
        .collect::<Result<Vec<_>>>()?;

    apply_actions(paths, &actions, &mut report)?;

    Ok(report)
}

/// Roll back skill file writes recorded in an extraction report.
pub fn rollback_bundled_skill_writes(report: &ExtractReport) -> Result<()> {
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
    skill: &'a BundledSkill,
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

fn plan_skill<'a>(
    paths: &MaestroPaths,
    skill: &'a BundledSkill,
    mode: ExtractMode<'a>,
) -> Result<SkillAction<'a>> {
    let relative_path = format!(".maestro/skills/{}/SKILL.md", skill.name);
    let path = managed_path(paths, &relative_path, SymlinkPolicy::RejectAllComponents)?;
    let existing = if path.exists() {
        Some(
            fs::read_to_string(&path)
                .with_context(|| format!("failed to read bundled skill {}", path.display()))?,
        )
    } else {
        None
    };

    let (write, backup_operation, backup_timestamp) = match (&existing, mode) {
        (None, _) => (true, None, None),
        (Some(_), ExtractMode::Merge) => (false, None, None),
        (Some(_), ExtractMode::Force { backup_timestamp }) => {
            (true, Some("init"), Some(backup_timestamp))
        }
        (Some(existing), ExtractMode::Update { backup_timestamp }) => {
            if existing == skill.contents {
                (false, None, None)
            } else {
                (true, Some("update"), Some(backup_timestamp))
            }
        }
        (Some(_), ExtractMode::Create) => {
            bail!(
                "{} already exists; use --merge to keep it or --force to overwrite with backup",
                path.display()
            );
        }
    };

    Ok(SkillAction {
        skill,
        path,
        existing,
        backup_operation,
        backup_timestamp,
        write,
    })
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
                skill_name: action.skill.name.to_string(),
                path: backup,
            });
        }
        if let Err(error) = write_string_atomic(&action.path, action.skill.contents)
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
            skill_name: action.skill.name.to_string(),
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
