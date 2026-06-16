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
    /// Current branch name, or `None` for a detached or unborn HEAD.
    pub branch: Option<String>,
    /// Uncommitted changes under `.maestro/` (the card store).
    pub maestro_dirty: usize,
    /// Uncommitted changes outside `.maestro/` (code and everything else).
    pub code_other_dirty: usize,
}

/// Read the current Git HEAD and dirty state for the repository containing `path`.
pub fn snapshot(path: impl AsRef<Path>) -> Result<GitSnapshot> {
    let repository = discover_repository(path.as_ref())?;
    let counts = dirty_counts(&repository)?;

    Ok(GitSnapshot {
        head: head_oid(&repository)?,
        dirty: counts.maestro + counts.code_other > 0,
        branch: branch_name(&repository)?,
        maestro_dirty: counts.maestro,
        code_other_dirty: counts.code_other,
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

fn branch_name(repository: &Repository) -> Result<Option<String>> {
    match repository.head() {
        Ok(reference) if reference.is_branch() => Ok(reference.shorthand().map(str::to_string)),
        Ok(_) => Ok(None),
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => Ok(None),
        Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(error) => Err(error).context("failed to read git branch"),
    }
}

/// Uncommitted-change counts, split by whether the path is under `.maestro/`.
struct DirtyCounts {
    maestro: usize,
    code_other: usize,
}

fn dirty_counts(repository: &Repository) -> Result<DirtyCounts> {
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repository
        .statuses(Some(&mut options))
        .context("failed to read git status")?;

    let mut counts = DirtyCounts {
        maestro: 0,
        code_other: 0,
    };
    for entry in statuses.iter() {
        match entry.path() {
            Some(path) if path.starts_with(".maestro/") => counts.maestro += 1,
            _ => counts.code_other += 1,
        }
    }
    Ok(counts)
}

fn is_dirty(repository: &Repository) -> Result<bool> {
    let counts = dirty_counts(repository)?;
    Ok(counts.maestro + counts.code_other > 0)
}
