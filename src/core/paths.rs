use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::core::error::MaestroError;

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

    /// Return the backup artifact directory.
    pub fn backups_dir(&self) -> PathBuf {
        self.maestro_dir().join("backups")
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
