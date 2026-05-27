use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::foundation::core::error::MaestroError;
use crate::foundation::core::managed_path::{managed_path, SymlinkPolicy};
use crate::foundation::core::paths::MaestroPaths;

use super::event::logical_session_id_from_run_path;

/// Managed Run event log read model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunEventLog {
    path: PathBuf,
    session_id: String,
}

impl RunEventLog {
    fn new(path: PathBuf) -> Self {
        let session_id = logical_session_id_from_run_path(&path);
        Self { path, session_id }
    }

    /// Path to the managed `events.jsonl` file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Logical session id represented by this run log.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// List managed Run event logs.
pub fn managed_event_logs(paths: &MaestroPaths) -> Result<Vec<RunEventLog>> {
    managed_event_files(paths).map(|files| files.into_iter().map(RunEventLog::new).collect())
}

/// List all managed `.maestro/runs/**/events.jsonl` files.
pub(crate) fn managed_event_files(paths: &MaestroPaths) -> Result<Vec<PathBuf>> {
    managed_run_files(paths, "events.jsonl")
}

/// List all managed `.maestro/runs/**/run_evidence.yaml` files.
pub(crate) fn managed_run_evidence_files(paths: &MaestroPaths) -> Result<Vec<PathBuf>> {
    managed_run_files(paths, "run_evidence.yaml")
}

fn managed_run_files(paths: &MaestroPaths, file_name: &str) -> Result<Vec<PathBuf>> {
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
    run_files_under(&runs_dir, file_name)
}

fn run_files_under(runs_dir: &Path, file_name: &str) -> Result<Vec<PathBuf>> {
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
    collect_run_files(runs_dir, &root, file_name, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_run_files(
    dir: &Path,
    root: &Path,
    file_name: &str,
    files: &mut Vec<PathBuf>,
) -> Result<()> {
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
                    collect_run_files(&path, root, file_name, files)?;
                } else if file_type.is_file()
                    && path.file_name().and_then(|name| name.to_str()) == Some(file_name)
                    && is_inside_canonical_root(&path, root)?
                {
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
