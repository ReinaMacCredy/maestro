use std::path::PathBuf;

use anyhow::Result;

use crate::domain::extraction::extract::{
    Action, FolderGate, FolderPreview, apply_actions, file_action, folder_gate, preview_folder,
    read_existing,
};
use crate::domain::skills::catalog::{Skill, frontmatter_version, skills};
use crate::foundation::core::fs::ensure_dir;
use crate::foundation::core::managed_path::{SymlinkPolicy, managed_path};
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

/// Preview each bundled skill's whole-folder fate without writing files.
pub fn preview_skills(paths: &MaestroPaths, mode: ExtractMode<'_>) -> Result<Vec<FolderPreview>> {
    let mut previews = Vec::new();
    for skill in skills() {
        let skill_md_path = skill_file_path(paths, skill.name, "SKILL.md")?;
        let installed = read_existing(&skill_md_path)?;
        previews.push(preview_folder(
            skill.name,
            mode,
            installed.as_deref(),
            skill.skill_md(),
            frontmatter_version,
        ));
    }
    Ok(previews)
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

    let mut files = Vec::with_capacity(skill.files.len());
    for file in &skill.files {
        let path = skill_file_path(paths, skill.name, file.relative_path)?;
        let existing = if file.relative_path == "SKILL.md" {
            installed_skill_md.clone()
        } else {
            read_existing(&path)?
        };
        files.push((file, path, existing));
    }

    let mut gate = folder_gate(
        mode,
        installed_skill_md.as_deref(),
        skill.skill_md(),
        frontmatter_version,
        &skill_md_path,
    )?;
    // A missing anchor with surviving tree files is a partial install, not a
    // fresh one, and there is no installed version left to gate a refresh.
    // Create's reject-existing rule only fits true `init`; in Merge/Update the
    // survivors may carry local edits, so restore what is missing and preserve
    // them, while Force keeps its back-up-and-overwrite promise.
    if installed_skill_md.is_none()
        && files.iter().any(|(_, _, existing)| existing.is_some())
        && matches!(gate, FolderGate::Create)
    {
        gate = match mode {
            ExtractMode::Create => gate,
            ExtractMode::Merge | ExtractMode::Update { .. } => FolderGate::Skip,
            ExtractMode::Force { backup_timestamp } => FolderGate::Refresh {
                operation: "init",
                backup_timestamp,
            },
        };
    }

    let mut actions = Vec::with_capacity(files.len());
    for (file, path, existing) in files {
        actions.push(file_action(
            skill.name,
            file.contents,
            path,
            existing,
            gate,
        )?);
    }

    Ok(actions)
}

fn skill_file_path(paths: &MaestroPaths, skill_name: &str, relative_path: &str) -> Result<PathBuf> {
    let relative = format!(".maestro/skills/{skill_name}/{relative_path}");
    managed_path(paths, &relative, SymlinkPolicy::RejectAllComponents)
}
