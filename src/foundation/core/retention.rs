use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};

/// Keep only the newest managed child directories under `parent`.
pub fn prune_child_dirs(parent: &Path, keep: usize) -> Result<usize> {
    if keep == 0 {
        return Ok(0);
    }
    let mut dirs = child_dirs(parent)?;
    if dirs.len() <= keep {
        return Ok(0);
    }
    dirs.sort_by(|left, right| {
        left.modified
            .cmp(&right.modified)
            .then_with(|| left.path.cmp(&right.path))
    });
    let remove_count = dirs.len() - keep;
    for entry in dirs.into_iter().take(remove_count) {
        match fs::remove_dir_all(&entry.path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to prune {}", entry.path.display()));
            }
        }
    }
    Ok(remove_count)
}

#[derive(Clone, Debug)]
struct ChildDir {
    path: PathBuf,
    modified: SystemTime,
}

fn child_dirs(parent: &Path) -> Result<Vec<ChildDir>> {
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
        dirs.push(ChildDir {
            path: entry.path(),
            modified,
        });
    }
    Ok(dirs)
}
