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
}

/// Extract all bundled skills into `.maestro/skills/`.
pub fn extract_bundled_skills(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<()> {
    ensure_dir(paths.skills_dir())?;

    for skill in bundled_skills() {
        extract_skill(paths, skill, mode)?;
    }

    Ok(())
}

fn extract_skill(paths: &MaestroPaths, skill: &BundledSkill, mode: ExtractMode<'_>) -> Result<()> {
    let path = paths.skills_dir().join(skill.name).join("SKILL.md");

    if path.exists() {
        match mode {
            ExtractMode::Merge => return Ok(()),
            ExtractMode::Force { backup_timestamp } => {
                backup_file_with_timestamp(paths, &path, "init", backup_timestamp)?;
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
