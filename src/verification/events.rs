use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::core::error::MaestroError;
use crate::core::managed_path::{managed_path, SymlinkPolicy};
use crate::core::paths::MaestroPaths;

/// List all managed `.maestro/runs/**/events.jsonl` files.
pub fn managed_event_files(paths: &MaestroPaths) -> Result<Vec<PathBuf>> {
    let runs_dir = match managed_path(paths, ".maestro/runs", SymlinkPolicy::RejectAllComponents) {
        Ok(path) => path,
        Err(error)
            if matches!(
                error.downcast_ref::<MaestroError>(),
                Some(MaestroError::ManagedPathContainsSymlink { .. })
            ) =>
        {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    event_files_under(&runs_dir)
}

/// List all `.maestro/runs/**/events.jsonl` files.
pub fn event_files_under(runs_dir: &Path) -> Result<Vec<PathBuf>> {
    match fs::symlink_metadata(runs_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => return Ok(Vec::new()),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", runs_dir.display()));
        }
    }
    let root = match fs::canonicalize(runs_dir) {
        Ok(root) => root,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to resolve {}", runs_dir.display()));
        }
    };
    let mut files = Vec::new();
    collect_files(runs_dir, &root, &mut files)?;
    files.retain(|path| path.file_name().and_then(|name| name.to_str()) == Some("events.jsonl"));
    files.sort();
    Ok(files)
}

fn collect_files(dir: &Path, root: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !is_inside_canonical_root(dir, root)? {
        return Ok(());
    }
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.with_context(|| format!("failed to list {}", dir.display()))?;
                let path = entry.path();
                let file_type = entry
                    .file_type()
                    .with_context(|| format!("failed to inspect {}", path.display()))?;
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    collect_files(&path, root, files)?;
                } else if file_type.is_file() && is_inside_canonical_root(&path, root)? {
                    files.push(path);
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", dir.display())),
    }
}

fn is_inside_canonical_root(path: &Path, root: &Path) -> Result<bool> {
    match fs::canonicalize(path) {
        Ok(canonical) => Ok(canonical.starts_with(root)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to resolve {}", path.display())),
    }
}
