use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

/// List non-symlink child directories of `parent` with their modification
/// times. A missing `parent` yields an empty list; an unreadable modification
/// time falls back to the Unix epoch.
pub(crate) fn child_dirs(parent: &Path) -> Result<Vec<(PathBuf, SystemTime)>> {
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", parent.display()));
        }
    };
    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to list {}", parent.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        dirs.push((entry.path(), modified));
    }
    Ok(dirs)
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
