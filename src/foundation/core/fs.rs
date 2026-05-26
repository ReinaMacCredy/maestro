use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};

/// Create a directory and any missing ancestors.
pub fn ensure_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))
}

/// Create the parent directory for a file path when it has one.
pub fn ensure_parent_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            ensure_dir(parent)?;
        }
    }

    Ok(())
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
