use std::path::PathBuf;

use anyhow::{bail, Result};

use crate::domain::extraction::extract::{
    apply_actions, folder_gate, read_existing, Action, FolderGate,
};
use crate::domain::skills::catalog::{frontmatter_version, skills, Skill};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;

pub use crate::domain::extraction::extract::{ExtractMode, ExtractReport};

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
) -> Result<Vec<Action<'a>>> {
    let mut actions = Vec::new();
    for skill in skills {
        actions.extend(plan_skill(paths, skill, mode)?);
    }
    Ok(actions)
}

/// Plan one action per file in `skill`. The version gate is whole-folder: the
/// installed `SKILL.md` version decides the fate of every file in the tree, so
/// local edits to any file survive until the shipped `version:` changes.
fn plan_skill<'a>(
    paths: &MaestroPaths,
    skill: &'a Skill,
    mode: ExtractMode<'a>,
) -> Result<Vec<Action<'a>>> {
    let skill_md_path = skill_file_path(paths, skill.name, "SKILL.md")?;
    let installed_skill_md = read_existing(&skill_md_path)?;
    let folder_gate = folder_gate(
        mode,
        installed_skill_md.as_deref(),
        skill.skill_md(),
        frontmatter_version,
        &skill_md_path,
    )?;

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

        actions.push(Action {
            name: skill.name,
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

fn skill_file_path(paths: &MaestroPaths, skill_name: &str, relative_path: &str) -> Result<PathBuf> {
    let relative = format!(".maestro/skills/{skill_name}/{relative_path}");
    managed_path(paths, &relative, SymlinkPolicy::RejectAllComponents)
}
