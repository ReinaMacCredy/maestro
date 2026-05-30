use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::foundation::core::fs::{create_directory_symlink, ensure_parent_dir};
use crate::foundation::core::managed_path::{managed_path, managed_symlink_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;

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

/// Validate that the destination is absent or already the expected symlink.
pub fn validate_skill_symlink_destination(
    paths: &MaestroPaths,
    symlink: SkillSymlink,
) -> Result<()> {
    validate_canonical_skills_tree(paths)?;
    let path = managed_symlink_path(paths, symlink.relative_path)?;
    match inspect_destination(&path, symlink)? {
        DestinationState::Missing | DestinationState::ExpectedSymlink => Ok(()),
    }
}

/// Create the expected skill symlink, or no-op when it is already present.
pub fn create_skill_symlink(paths: &MaestroPaths, symlink: SkillSymlink) -> Result<()> {
    validate_canonical_skills_tree(paths)?;
    let path = managed_symlink_path(paths, symlink.relative_path)?;
    match inspect_destination(&path, symlink)? {
        DestinationState::ExpectedSymlink => return Ok(()),
        DestinationState::Missing => {}
    }

    ensure_parent_dir(&path)?;
    create_directory_symlink(Path::new(symlink.target), &path)
        .with_context(|| format!("failed to create skill symlink {}", path.display()))
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

fn validate_canonical_skills_tree(paths: &MaestroPaths) -> Result<()> {
    managed_path(paths, ".maestro/skills", SymlinkPolicy::RejectAllComponents)?;
    Ok(())
}
