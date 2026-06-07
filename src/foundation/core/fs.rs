use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Flush a path's parent directory entry to disk so a preceding rename or
/// hard-link is durable across a crash. A no-op when the path has no parent.
pub(crate) fn sync_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };

    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("failed to sync parent directory {}", parent.display()))
}

/// Create a directory and any missing ancestors.
pub fn ensure_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))
}

/// Create the parent directory for a file path when it has one.
pub fn ensure_parent_dir(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        ensure_dir(parent)?;
    }

    Ok(())
}

/// Build a new directory in a temp root, then publish it with one final rename.
pub fn write_new_dir_atomic<F>(
    target_dir: impl AsRef<Path>,
    temp_root: impl AsRef<Path>,
    prefix: &str,
    build: F,
) -> Result<()>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let target_dir = target_dir.as_ref();
    let temp_root = temp_root.as_ref();
    ensure_parent_dir(target_dir)?;
    ensure_dir(temp_root)?;
    if target_dir.exists() {
        bail!("directory already exists: {}", target_dir.display());
    }

    let temp_dir = match create_temp_child_dir(temp_root, prefix) {
        Ok(temp_dir) => temp_dir,
        Err(error) => {
            let _ = fs::remove_dir(temp_root);
            return Err(error);
        }
    };
    if let Err(error) = build(&temp_dir) {
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir(temp_root);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temp_dir, target_dir) {
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir(temp_root);
        return Err(error).with_context(|| {
            format!(
                "failed to publish directory {} from temp directory {}",
                target_dir.display(),
                temp_dir.display()
            )
        });
    }
    sync_parent_dir(target_dir)?;
    let _ = fs::remove_dir(temp_root);
    Ok(())
}

fn create_temp_child_dir(root: &Path, prefix: &str) -> Result<PathBuf> {
    let mut last_error = None;
    for _ in 0..16 {
        let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_dir = root.join(format!(".tmp-{prefix}-{}-{counter}", process::id()));
        match fs::create_dir(&temp_dir) {
            Ok(()) => return Ok(temp_dir),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                last_error = Some(error);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create temp directory {}", temp_dir.display())
                });
            }
        }
    }
    Err(last_error.expect("invariant: temp dir loop records collisions")).with_context(|| {
        format!(
            "failed to create unique temp directory under {}",
            root.display()
        )
    })
}

/// Create a directory symlink at `link` pointing to `target`, using the
/// platform-native symlink call.
#[cfg(unix)]
pub(crate) fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
pub(crate) fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

/// List non-symlink child directories of `parent` with their modification
/// times. A missing `parent` yields an empty list; an unreadable modification
/// time falls back to the Unix epoch.
pub(crate) fn child_dirs(parent: &Path) -> Result<Vec<(PathBuf, SystemTime)>> {
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
        dirs.push((entry.path(), modified));
    }
    Ok(dirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_case(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "maestro-fs-{name}-{}-{}",
            process::id(),
            TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn write_new_dir_atomic_publishes_only_after_build_succeeds() {
        let root = temp_case("publish");
        let target = root.join("final");
        let temp_root = root.join(".tmp");

        write_new_dir_atomic(&target, &temp_root, "case", |temp_dir| {
            fs::write(temp_dir.join("record.yaml"), "ok: true\n")?;
            Ok(())
        })
        .expect("directory publish should succeed");

        assert!(target.join("record.yaml").is_file());
        assert!(!temp_root.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn write_new_dir_atomic_cleans_temp_dir_when_build_fails() {
        let root = temp_case("fail");
        let target = root.join("final");
        let temp_root = root.join(".tmp");

        let error = write_new_dir_atomic(&target, &temp_root, "case", |temp_dir| {
            fs::write(temp_dir.join("partial"), "not ready\n")?;
            anyhow::bail!("stop before publish")
        })
        .expect_err("build failure should return an error");

        assert!(error.to_string().contains("stop before publish"));
        assert!(!target.exists());
        assert!(!temp_root.exists());
        let _ = fs::remove_dir_all(root);
    }
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
