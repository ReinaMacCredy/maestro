use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::foundation::core::error::MaestroError;

/// Repository-local Maestro path helpers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaestroPaths {
    repo_root: PathBuf,
}

impl MaestroPaths {
    /// Create path helpers rooted at a repository directory.
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    /// Return the repository root directory.
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Return the `.maestro` artifact directory.
    pub fn maestro_dir(&self) -> PathBuf {
        self.repo_root.join(".maestro")
    }

    /// Return the harness artifact directory.
    pub fn harness_dir(&self) -> PathBuf {
        self.maestro_dir().join("harness")
    }

    /// Return the feature artifact directory.
    pub fn features_dir(&self) -> PathBuf {
        self.maestro_dir().join("features")
    }

    /// Return the decision artifact directory.
    pub fn decisions_dir(&self) -> PathBuf {
        self.maestro_dir().join("decisions")
    }

    /// Return the bundled skills directory.
    pub fn skills_dir(&self) -> PathBuf {
        self.maestro_dir().join("skills")
    }

    /// Return the bundled hook scripts directory.
    pub fn hooks_dir(&self) -> PathBuf {
        self.maestro_dir().join("hooks")
    }

    /// Return the task artifact directory.
    pub fn tasks_dir(&self) -> PathBuf {
        self.maestro_dir().join("tasks")
    }

    /// Return the run artifact directory.
    pub fn runs_dir(&self) -> PathBuf {
        self.maestro_dir().join("runs")
    }

    /// Return the archive root, a sibling of the live `tasks`/`features` trees.
    ///
    /// Archived items move under here so the live scans skip them for free
    /// (§5.3). Created on-demand by the archive verbs, not by `init` (§5.6).
    pub fn archive_dir(&self) -> PathBuf {
        self.maestro_dir().join("archive")
    }

    /// Return the archived-tasks directory (`.maestro/archive/tasks`).
    pub fn archive_tasks_dir(&self) -> PathBuf {
        self.archive_dir().join("tasks")
    }

    /// Return the archived-features directory (`.maestro/archive/features`).
    pub fn archive_features_dir(&self) -> PathBuf {
        self.archive_dir().join("features")
    }

    /// Return the backup artifact directory.
    pub fn backups_dir(&self) -> PathBuf {
        self.maestro_dir().join("backups")
    }

    /// Return the install lockfile path.
    pub fn install_lock_file(&self) -> PathBuf {
        self.maestro_dir().join("install-lock.yaml")
    }
}

/// Discover the repository root from the current working directory.
pub fn discover_repo_root() -> Result<PathBuf> {
    let current_dir = env::current_dir().context("failed to read current working directory")?;
    discover_repo_root_from(current_dir)
}

/// Discover the nearest ancestor that contains a `.maestro` or `.git` directory.
pub fn discover_repo_root_from(start_dir: impl AsRef<Path>) -> Result<PathBuf> {
    let start_dir = start_dir.as_ref();
    let mut current = start_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve start directory {}", start_dir.display()))?;

    loop {
        if current.join(".maestro").is_dir() || current.join(".git").exists() {
            return Ok(current);
        }

        if !current.pop() {
            return Err(MaestroError::RepoRootNotFound {
                start: start_dir.to_path_buf(),
            }
            .into());
        }
    }
}

/// Announce, on stderr, the repository root a mutating command resolved to when
/// it differs from the current working directory. Run from a nested subdirectory,
/// maestro walks up to the enclosing repo and mutates it; echoing the root keeps
/// that from being a silent footgun (T5). Stays silent when run from the root, so
/// it never fires for callers (including tests) invoked at the repo top.
pub fn announce_repo_root(root: &Path) {
    let Ok(cwd) = env::current_dir() else {
        return;
    };
    let canonical = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if canonical(&cwd) != canonical(root) {
        eprintln!("operating on {}", root.display());
    }
}
