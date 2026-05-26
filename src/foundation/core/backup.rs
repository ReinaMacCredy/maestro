use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::ensure_parent_dir;
use crate::foundation::core::paths::MaestroPaths;

static BACKUP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Copy a file into `.maestro/backups/<timestamp>-<operation>/`.
pub fn backup_file(
    paths: &MaestroPaths,
    source: impl AsRef<Path>,
    operation: &str,
) -> Result<PathBuf> {
    let timestamp = backup_operation_timestamp()?;
    backup_file_with_timestamp(paths, source, operation, &timestamp)
}

/// Return a backup timestamp suitable for grouping one logical operation.
pub fn backup_operation_timestamp() -> Result<String> {
    backup_timestamp()
}

/// Copy a file into a deterministic backup location for tests and planned operations.
pub fn backup_file_with_timestamp(
    paths: &MaestroPaths,
    source: impl AsRef<Path>,
    operation: &str,
    timestamp: &str,
) -> Result<PathBuf> {
    validate_operation(operation)?;
    let source = source.as_ref();
    let repo_root = paths.repo_root().canonicalize().with_context(|| {
        format!(
            "failed to resolve repo root {}",
            paths.repo_root().display()
        )
    })?;
    let source = source
        .canonicalize()
        .with_context(|| format!("failed to resolve backup source {}", source.display()))?;
    let relative = source
        .strip_prefix(&repo_root)
        .map(Path::to_path_buf)
        .map_err(|_| MaestroError::OutsideRepository {
            path: source.to_path_buf(),
        })?;
    reject_backup_symlinks(paths)?;
    let destination = paths
        .backups_dir()
        .join(format!("{timestamp}-{operation}"))
        .join(relative);

    ensure_parent_dir(&destination)?;
    copy_without_overwrite(&source, &destination).with_context(|| {
        format!(
            "failed to back up {} to {}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(destination)
}

fn copy_without_overwrite(source: &Path, destination: &Path) -> Result<()> {
    let mut source_file =
        fs::File::open(source).with_context(|| format!("failed to open {}", source.display()))?;
    let temp_path = temp_sibling_path(destination)?;
    let mut temp_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .with_context(|| format!("failed to create {}", temp_path.display()))?;

    if let Err(error) = io::copy(&mut source_file, &mut temp_file)
        .and_then(|_| temp_file.flush())
        .and_then(|_| temp_file.sync_all())
    {
        let _ = fs::remove_file(&temp_path);
        return Err(error).with_context(|| {
            format!(
                "failed to copy {} to {}",
                source.display(),
                temp_path.display()
            )
        });
    }

    if let Err(error) = fs::hard_link(&temp_path, destination) {
        let _ = fs::remove_file(&temp_path);
        return Err(error).with_context(|| format!("failed to create {}", destination.display()));
    }
    fs::remove_file(&temp_path)
        .with_context(|| format!("failed to remove temp file {}", temp_path.display()))?;
    sync_parent_dir(destination)?;

    Ok(())
}

fn reject_backup_symlinks(paths: &MaestroPaths) -> Result<()> {
    for path in [paths.maestro_dir(), paths.backups_dir()] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(MaestroError::BackupPathContainsSymlink { path }.into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
            }
        }
    }

    Ok(())
}

fn temp_sibling_path(destination: &Path) -> Result<PathBuf> {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("path has no valid file name: {}", destination.display()))?;
    let parent = destination.parent().unwrap_or_else(|| Path::new(""));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    let counter = BACKUP_COUNTER.fetch_add(1, Ordering::Relaxed);

    Ok(parent.join(format!(".{file_name}.tmp.{nanos}.{counter}")))
}

fn sync_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };

    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("failed to sync parent directory {}", parent.display()))
}

fn backup_timestamp() -> Result<String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    let counter = BACKUP_COUNTER.fetch_add(1, Ordering::Relaxed);

    Ok(format!("{nanos}-{counter}"))
}

fn validate_operation(operation: &str) -> Result<()> {
    let is_safe = !operation.is_empty()
        && operation
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');

    if is_safe {
        Ok(())
    } else {
        Err(MaestroError::InvalidOperationName {
            operation: operation.to_string(),
        }
        .into())
    }
}
