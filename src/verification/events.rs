use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// List all `.maestro/runs/**/events.jsonl` files.
pub fn event_files_under(runs_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(runs_dir, &mut files)?;
    files.retain(|path| path.file_name().and_then(|name| name.to_str()) == Some("events.jsonl"));
    files.sort();
    Ok(files)
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
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
                    collect_files(&path, files)?;
                } else if file_type.is_file() {
                    files.push(path);
                }
            }
            Ok(())
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", dir.display())),
    }
}
