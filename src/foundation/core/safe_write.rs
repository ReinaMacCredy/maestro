use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};

use crate::foundation::core::fs::{ensure_parent_dir, sync_parent_dir};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Atomically write UTF-8 text by replacing the target with a sibling temp file.
pub fn write_string_atomic(path: impl AsRef<Path>, contents: &str) -> Result<()> {
    write_atomic(path, contents.as_bytes())
}

/// Atomically write bytes by replacing the target with a sibling temp file.
pub fn write_atomic(path: impl AsRef<Path>, contents: &[u8]) -> Result<()> {
    let path = path.as_ref();
    ensure_parent_dir(path)?;

    let temp_path = create_temp_sibling(path, contents)?;

    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error).with_context(|| {
            format!(
                "failed to replace {} with temp file {}",
                path.display(),
                temp_path.display()
            )
        });
    }
    sync_parent_dir(path)?;

    Ok(())
}

fn create_temp_sibling(path: &Path, contents: &[u8]) -> Result<PathBuf> {
    let mut last_error = None;

    for _ in 0..16 {
        let temp_path = temp_sibling_path(path)?;
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
                    let _ = fs::remove_file(&temp_path);
                    return Err(error).with_context(|| {
                        format!("failed to write temp file {}", temp_path.display())
                    });
                }

                return Ok(temp_path);
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                last_error = Some(error);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create temp file {}", temp_path.display())
                });
            }
        }
    }

    match last_error {
        Some(error) => Err(error).context("failed to allocate unique temp file after 16 attempts"),
        None => bail!("failed to allocate unique temp file"),
    }
}

fn temp_sibling_path(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("path has no valid file name: {}", path.display()))?;
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

    Ok(parent.join(format!(
        ".{file_name}.tmp.{}.{}.{}",
        process::id(),
        timestamp,
        counter
    )))
}
