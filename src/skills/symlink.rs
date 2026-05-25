use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::core::fs::ensure_parent_dir;
use crate::core::paths::MaestroPaths;
use crate::install::InstallAgent;

/// Relative target used by agent skill mirrors.
pub const SKILLS_SYMLINK_TARGET: &str = "../.maestro/skills";

/// Expected skill symlink for one installed agent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkillSymlink {
    /// Repo-relative symlink path.
    pub relative_path: &'static str,
    /// Symlink target, relative to the agent config directory.
    pub target: &'static str,
}

/// Return the expected skill symlink for an agent.
pub fn skill_symlink_for_agent(agent: InstallAgent) -> SkillSymlink {
    match agent {
        InstallAgent::Claude => SkillSymlink {
            relative_path: ".claude/skills",
            target: SKILLS_SYMLINK_TARGET,
        },
        InstallAgent::Codex => SkillSymlink {
            relative_path: ".codex/skills",
            target: SKILLS_SYMLINK_TARGET,
        },
    }
}

/// Validate that the destination is absent or already the expected symlink.
pub fn validate_skill_symlink_destination(
    paths: &MaestroPaths,
    symlink: SkillSymlink,
) -> Result<()> {
    let path = managed_symlink_path(paths, symlink.relative_path)?;
    match inspect_destination(&path, symlink)? {
        DestinationState::Missing | DestinationState::ExpectedSymlink => Ok(()),
    }
}

/// Create the expected skill symlink, or no-op when it is already present.
pub fn create_skill_symlink(paths: &MaestroPaths, symlink: SkillSymlink) -> Result<()> {
    let path = managed_symlink_path(paths, symlink.relative_path)?;
    match inspect_destination(&path, symlink)? {
        DestinationState::ExpectedSymlink => return Ok(()),
        DestinationState::Missing => {}
    }

    ensure_parent_dir(&path)?;
    create_directory_symlink(Path::new(symlink.target), &path)
        .with_context(|| format!("failed to create skill symlink {}", path.display()))
}

/// Remove a skill symlink only when the live target still matches install-lock ownership.
pub fn remove_skill_symlink_if_owned(path: &Path, expected_target: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::read_link(path)
                .with_context(|| format!("failed to read symlink {}", path.display()))?;
            if target == Path::new(expected_target) {
                fs::remove_file(path)
                    .with_context(|| format!("failed to remove symlink {}", path.display()))?;
            }
            Ok(())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DestinationState {
    Missing,
    ExpectedSymlink,
}

fn inspect_destination(path: &Path, symlink: SkillSymlink) -> Result<DestinationState> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::read_link(path)
                .with_context(|| format!("failed to read symlink {}", path.display()))?;
            if target == Path::new(symlink.target) {
                return Ok(DestinationState::ExpectedSymlink);
            }
            bail!(
                "refusing to overwrite existing {} symlink: expected target {}, found {}",
                symlink.relative_path,
                symlink.target,
                target.display()
            );
        }
        Ok(_) => bail!(
            "refusing to overwrite existing {} because it is not the Maestro-managed skills symlink",
            symlink.relative_path
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(DestinationState::Missing)
        }
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn managed_symlink_path(paths: &MaestroPaths, relative_path: &str) -> Result<PathBuf> {
    let relative = Path::new(relative_path);
    reject_unsafe_relative_path(relative)?;
    reject_symlinked_parent_components(paths.repo_root(), relative)?;
    Ok(paths.repo_root().join(relative))
}

fn reject_unsafe_relative_path(relative: &Path) -> Result<()> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!(
            "skill symlink path must be repository-relative: {}",
            relative.display()
        );
    }

    Ok(())
}

fn reject_symlinked_parent_components(repo_root: &Path, relative: &Path) -> Result<()> {
    let Some(parent) = relative.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    let mut current = repo_root.to_path_buf();
    for component in parent.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!(
                    "skill symlink parent must not contain symlink components: {}",
                    current.display()
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect {}", current.display()));
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}
