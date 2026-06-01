use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};

/// Flush a path's parent directory entry to disk so a preceding rename or
/// hard-link is durable across a crash. A no-op when the path has no parent.
pub(crate) fn sync_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };

    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("failed to sync parent directory {}", parent.display()))
}

/// Create a directory and any missing ancestors.
pub fn ensure_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))
}

/// Create the parent directory for a file path when it has one.
pub fn ensure_parent_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        ensure_dir(parent)?;
    }

    Ok(())
}

/// Create a directory symlink at `link` pointing to `target`, using the
/// platform-native symlink call.
#[cfg(unix)]
pub(crate) fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
pub(crate) fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

/// Read a UTF-8 file if it exists.
pub fn read_to_string_if_exists(path: impl AsRef<Path>) -> Result<Option<String>> {
    let path = path.as_ref();

    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read file {}", path.display())),
    }
}
