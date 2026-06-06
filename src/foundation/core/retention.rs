use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use anyhow::{Context, Result};

use crate::foundation::core::fs::child_dirs;

/// Keep only the newest managed child directories under `parent`.
pub fn prune_child_dirs(parent: &Path, keep: usize) -> Result<usize> {
    if keep == 0 {
        return Ok(0);
    }
    let mut dirs = child_dirs(parent)?;
    if dirs.len() <= keep {
        return Ok(0);
    }
    dirs.sort_by(|(left_path, left_modified), (right_path, right_modified)| {
        left_modified
            .cmp(right_modified)
            .then_with(|| left_path.cmp(right_path))
    });
    let remove_count = dirs.len() - keep;
    for (path, _) in dirs.into_iter().take(remove_count) {
        match fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("failed to prune {}", path.display()));
            }
        }
    }
    Ok(remove_count)
}
