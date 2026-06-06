use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};

use crate::foundation::core::error::MaestroError;
use crate::foundation::core::fs::{ensure_parent_dir, sync_parent_dir};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::retention::prune_child_dirs;
use crate::foundation::core::safe_write::temp_sibling_path;
use crate::foundation::core::time::utc_now_filesystem_millis_timestamp;

static BACKUP_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    prune_child_dirs(&paths.backups_dir(), 3)?;

    Ok(destination)
}

fn copy_without_overwrite(source: &Path, destination: &Path) -> Result<()> {
    let mut source_file =
        fs::File::open(source).with_context(|| format!("failed to open {}", source.display()))?;
    let temp_path = temp_sibling_path(destination, "tmp")?;
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

fn backup_timestamp() -> Result<String> {
    let counter = BACKUP_COUNTER.fetch_add(1, Ordering::Relaxed);

    Ok(format!(
        "{}-{counter}",
        utc_now_filesystem_millis_timestamp()
    ))
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
