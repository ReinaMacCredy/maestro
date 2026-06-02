use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

use crate::foundation::core::error::MaestroError;
use crate::foundation::core::paths::MaestroPaths;

/// Return a managed repo-local path after validating path traversal and symlink components.
pub fn managed_path(
    paths: &MaestroPaths,
    relative_path: &str,
    policy: SymlinkPolicy,
) -> Result<PathBuf> {
    let relative = Path::new(relative_path);
    reject_unsafe_relative_path(relative)?;
    match policy {
        SymlinkPolicy::RejectAllComponents => {
            reject_symlinked_path_components(paths.repo_root(), relative)?;
        }
        SymlinkPolicy::RejectParentComponents => {
            reject_symlinked_parent_components(paths.repo_root(), relative)?;
        }
    }
    Ok(paths.repo_root().join(relative))
}

/// Managed path for a symlink leaf: rejects symlinks in the parent components
/// but allows the leaf itself to be the Maestro-managed symlink.
pub(crate) fn managed_symlink_path(paths: &MaestroPaths, relative_path: &str) -> Result<PathBuf> {
    managed_path(paths, relative_path, SymlinkPolicy::RejectParentComponents)
}

/// Which components of a managed path may be symlinks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymlinkPolicy {
    /// Reject symlinks in every existing path component.
    RejectAllComponents,
    /// Reject symlinks only in parent components, allowing the leaf itself to be a symlink.
    RejectParentComponents,
}

fn reject_unsafe_relative_path(relative: &Path) -> Result<()> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MaestroError::OutsideRepository {
            path: relative.to_path_buf(),
        }
        .into());
    }

    Ok(())
}

fn reject_symlinked_path_components(repo_root: &Path, relative: &Path) -> Result<()> {
    let mut current = repo_root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(MaestroError::ManagedPathContainsSymlink { path: current }.into());
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

fn reject_symlinked_parent_components(repo_root: &Path, relative: &Path) -> Result<()> {
    let Some(parent) = relative.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    reject_symlinked_path_components(repo_root, parent)
}
