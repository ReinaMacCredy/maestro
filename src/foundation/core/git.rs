use std::path::Path;

use anyhow::{Context, Result};
use git2::{Repository, StatusOptions};

/// Current Git state needed by proof freshness and migration checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitSnapshot {
    /// Current HEAD object id, or `None` for an unborn repository.
    pub head: Option<String>,
    /// Whether tracked or untracked worktree changes are present.
    pub dirty: bool,
}

/// Read the current Git HEAD and dirty state for the repository containing `path`.
pub fn snapshot(path: impl AsRef<Path>) -> Result<GitSnapshot> {
    let repository = discover_repository(path.as_ref())?;

    Ok(GitSnapshot {
        head: head_oid(&repository)?,
        dirty: is_dirty(&repository)?,
    })
}

/// Return the current Git HEAD object id.
pub fn head(path: impl AsRef<Path>) -> Result<Option<String>> {
    let repository = discover_repository(path.as_ref())?;

    head_oid(&repository)
}

/// Return whether the repository containing `path` has tracked or untracked changes.
pub fn dirty(path: impl AsRef<Path>) -> Result<bool> {
    let repository = discover_repository(path.as_ref())?;

    is_dirty(&repository)
}

fn discover_repository(path: &Path) -> Result<Repository> {
    Repository::discover(path)
        .with_context(|| format!("failed to discover git repository from {}", path.display()))
}

fn head_oid(repository: &Repository) -> Result<Option<String>> {
    match repository.head() {
        Ok(reference) => Ok(reference.target().map(|oid| oid.to_string())),
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => Ok(None),
        Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(error) => Err(error).context("failed to read git HEAD"),
    }
}

fn is_dirty(repository: &Repository) -> Result<bool> {
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repository
        .statuses(Some(&mut options))
        .context("failed to read git status")?;

    Ok(!statuses.is_empty())
}
