use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::core::backup::backup_file_with_timestamp;
use crate::core::fs::ensure_dir;
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
}

/// A bundled skill backup created during extraction.
#[derive(Debug, Eq, PartialEq)]
pub struct SkillBackup {
    /// Bundled skill name.
    pub skill_name: String,
    /// Backup file path.
    pub path: PathBuf,
}

/// Extract all bundled skills into `.maestro/skills/`.
pub fn extract_bundled_skills(
    paths: &MaestroPaths,
    mode: ExtractMode<'_>,
) -> Result<ExtractReport> {
    ensure_dir(paths.skills_dir())?;
    let mut report = ExtractReport::default();

    for skill in bundled_skills() {
        extract_skill(paths, skill, mode, &mut report)?;
    }

    Ok(report)
}

fn extract_skill(
    paths: &MaestroPaths,
    skill: &BundledSkill,
    mode: ExtractMode<'_>,
    report: &mut ExtractReport,
) -> Result<()> {
    let path = paths.skills_dir().join(skill.name).join("SKILL.md");

    if path.exists() {
        match mode {
            ExtractMode::Merge => return Ok(()),
            ExtractMode::Force { backup_timestamp } => {
                let backup = backup_file_with_timestamp(paths, &path, "init", backup_timestamp)?;
                report.backups.push(SkillBackup {
                    skill_name: skill.name.to_string(),
                    path: backup,
                });
            }
            ExtractMode::Update { backup_timestamp } => {
                let existing = std::fs::read_to_string(&path)
                    .with_context(|| format!("failed to read bundled skill {}", path.display()))?;
                if existing != skill.contents {
                    let backup =
                        backup_file_with_timestamp(paths, &path, "update", backup_timestamp)?;
                    report.backups.push(SkillBackup {
                        skill_name: skill.name.to_string(),
                        path: backup,
                    });
                }
            }
            ExtractMode::Create => {
                bail!(
                    "{} already exists; use --merge to keep it or --force to overwrite with backup",
                    path.display()
                );
            }
        }
    }

    write_string_atomic(&path, skill.contents)
        .with_context(|| format!("failed to write bundled skill {}", path.display()))
}
