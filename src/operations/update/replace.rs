use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use super::{
    update_request, BinaryReplacer, BinaryStatus, ChecksumVerifier, DownloadedBinary, ReleaseInfo,
    UpdateDownloader, UpdateOptions, UpdateUnavailable,
};
use crate::foundation::core::fs::ensure_parent_dir;

static REPLACE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Replacer that writes a sibling temp file and renames it over the executable.
#[derive(Debug, Default)]
pub struct AtomicBinaryReplacer;

impl BinaryReplacer for AtomicBinaryReplacer {
    fn replace(&self, current: &Path, candidate: &Path) -> Result<()> {
        replace_binary_atomic(current, candidate)
    }
}

pub(super) enum PreparedBinary {
    UpToDate {
        release: ReleaseInfo,
    },
    Skipped {
        reason: UpdateUnavailable,
    },
    Candidate {
        path: PathBuf,
        work_dir: PathBuf,
        release: Option<ReleaseInfo>,
    },
}

pub(super) fn prepare_binary_update(
    options: &UpdateOptions<'_>,
    downloader: &dyn UpdateDownloader,
    verifier: &dyn ChecksumVerifier,
) -> Result<PreparedBinary> {
    let request = update_request(options);
    let work_dir = request.work_dir.clone();
    let candidate = match downloader.download(&request) {
        Ok(DownloadedBinary::Available { path, release }) => (path, release),
        Ok(DownloadedBinary::UpToDate(release)) => {
            cleanup_work_dir(&work_dir);
            return Ok(PreparedBinary::UpToDate { release });
        }
        Ok(DownloadedBinary::Unavailable(reason)) => {
            cleanup_work_dir(&work_dir);
            return Ok(PreparedBinary::Skipped { reason });
        }
        Err(error) => {
            cleanup_work_dir(&work_dir);
            return Err(error);
        }
    };

    if let Err(error) = verifier.verify(&candidate.0) {
        cleanup_work_dir(&work_dir);
        return Err(error);
    }

    Ok(PreparedBinary::Candidate {
        path: candidate.0,
        work_dir,
        release: candidate.1,
    })
}

pub(super) fn replace_prepared_binary(
    options: &UpdateOptions<'_>,
    replacer: &dyn BinaryReplacer,
    binary: PreparedBinary,
) -> Result<BinaryStatus> {
    match binary {
        PreparedBinary::UpToDate { release } => Ok(BinaryStatus::UpToDate { release }),
        PreparedBinary::Skipped { reason } => Ok(BinaryStatus::Skipped { reason }),
        PreparedBinary::Candidate {
            path,
            work_dir,
            release,
        } => {
            let result =
                replacer
                    .replace(options.executable_path, &path)
                    .map(|()| BinaryStatus::Replaced {
                        path: options.executable_path.to_path_buf(),
                        release,
                    });
            cleanup_work_dir(&work_dir);
            result
        }
    }
}

fn cleanup_work_dir(work_dir: &Path) {
    let _ = fs::remove_dir_all(work_dir);
}

pub(super) fn cleanup_prepared_binary(binary: &PreparedBinary) {
    if let PreparedBinary::Candidate { work_dir, .. } = binary {
        cleanup_work_dir(work_dir);
    }
}

pub(super) fn prepared_release(binary: &PreparedBinary) -> Option<ReleaseInfo> {
    match binary {
        PreparedBinary::UpToDate { release } => Some(release.clone()),
        PreparedBinary::Skipped { .. } => None,
        PreparedBinary::Candidate { release, .. } => release.clone(),
    }
}

fn replace_binary_atomic(current: &Path, candidate: &Path) -> Result<()> {
    ensure_parent_dir(current)?;

    let permissions = fs::metadata(current)
        .or_else(|_| fs::metadata(candidate))
        .with_context(|| {
            format!(
                "failed to inspect update replacement permissions for {}",
                candidate.display()
            )
        })?
        .permissions();
    let temp_path = temp_sibling_path(current)?;

    if let Err(error) = copy_candidate_to_temp(candidate, &temp_path, permissions) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temp_path, current) {
        let _ = fs::remove_file(&temp_path);
        return Err(error).with_context(|| {
            format!(
                "failed to replace {} with update candidate {}",
                current.display(),
                temp_path.display()
            )
        });
    }
    let _ = sync_parent_dir(current);

    Ok(())
}

fn copy_candidate_to_temp(
    candidate: &Path,
    temp_path: &Path,
    permissions: fs::Permissions,
) -> Result<()> {
    let mut source = File::open(candidate)
        .with_context(|| format!("failed to open update candidate {}", candidate.display()))?;
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)
        .with_context(|| format!("failed to create temp update file {}", temp_path.display()))?;

    std::io::copy(&mut source, &mut temp)
        .and_then(|_| temp.flush())
        .and_then(|_| temp.sync_all())
        .with_context(|| {
            format!(
                "failed to copy update candidate {} to {}",
                candidate.display(),
                temp_path.display()
            )
        })?;
    fs::set_permissions(temp_path, permissions)
        .with_context(|| format!("failed to set permissions on {}", temp_path.display()))
}

fn temp_sibling_path(path: &Path) -> Result<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("path has no valid file name: {}", path.display()))?;
    let parent = match path.parent() {
        Some(parent) => parent,
        None => Path::new(""),
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    let counter = REPLACE_COUNTER.fetch_add(1, Ordering::Relaxed);

    Ok(parent.join(format!(
        ".{file_name}.update.{}.{}.{}",
        process::id(),
        timestamp,
        counter
    )))
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
