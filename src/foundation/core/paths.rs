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

    /// Return the global structured decision store.
    pub fn decisions_file(&self) -> PathBuf {
        self.maestro_dir().join("decisions.yaml")
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

    /// Return the card artifact directory (`.maestro/cards`).
    ///
    /// Each card owns a directory `cards/<id>/` holding `card.yaml`; feature
    /// cards carry `spec.md`/`notes.md` as sidecar prose. This is the single
    /// flat store that the card model folds features/tasks/harness-backlog/
    /// decisions into (SPEC-beads-model.md).
    pub fn cards_dir(&self) -> PathBuf {
        self.maestro_dir().join("cards")
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

    /// Return the archived-cards directory (`.maestro/archive/cards`).
    ///
    /// The card-model archive sibling of `cards/`; `archive <feature>` moves the
    /// feature card and its `parent=<feature>` children here as whole directories
    /// (SPEC-beads-model E4/D5).
    pub fn archive_cards_dir(&self) -> PathBuf {
        self.archive_dir().join("cards")
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
    let home_root = env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|path| path.canonicalize().ok());
    discover_repo_root_from_with_home(start_dir.as_ref(), home_root.as_deref())
}

fn discover_repo_root_from_with_home(
    start_dir: &Path,
    home_root: Option<&Path>,
) -> Result<PathBuf> {
    let mut current = start_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve start directory {}", start_dir.display()))?;
    let start = current.clone();

    loop {
        let has_repo_marker = current.join(".maestro").is_dir() || current.join(".git").exists();
        if has_repo_marker && !is_home_root_escape(&current, &start, home_root) {
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

fn is_home_root_escape(current: &Path, start: &Path, home_root: Option<&Path>) -> bool {
    home_root.is_some_and(|home| current == home && start != home)
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn discovery_skips_home_level_maestro_for_clean_child_directory() {
        let root = temp_root("maestro-home-discovery-test");
        let home = root.join("home");
        fs::create_dir_all(home.join(".maestro"))
            .expect("invariant: home-level maestro dir should be creatable");
        let project = home.join("Code/demo");
        fs::create_dir_all(&project).expect("invariant: project dir should be creatable");
        let home = home
            .canonicalize()
            .expect("invariant: home dir should canonicalize");

        let error = discover_repo_root_from_with_home(&project, Some(&home))
            .expect_err("home-level .maestro must not capture clean child directories");

        assert!(
            matches!(
                error.downcast_ref::<MaestroError>(),
                Some(MaestroError::RepoRootNotFound { .. })
            ),
            "{error:?}"
        );
        fs::remove_dir_all(root).expect("invariant: temp root should be removable");
    }

    #[test]
    fn discovery_allows_home_level_maestro_when_started_at_home() {
        let root = temp_root("maestro-home-discovery-test");
        let home = root.join("home");
        fs::create_dir_all(home.join(".maestro"))
            .expect("invariant: home-level maestro dir should be creatable");
        let home = home
            .canonicalize()
            .expect("invariant: home dir should canonicalize");

        let discovered = discover_repo_root_from_with_home(&home, Some(&home))
            .expect("home itself remains a valid explicit root");

        assert_eq!(discovered, home);
        fs::remove_dir_all(root).expect("invariant: temp root should be removable");
    }

    #[test]
    fn discovery_prefers_project_marker_before_home_marker() {
        let root = temp_root("maestro-home-discovery-test");
        let home = root.join("home");
        fs::create_dir_all(home.join(".maestro"))
            .expect("invariant: home-level maestro dir should be creatable");
        let project = home.join("Code/demo");
        fs::create_dir_all(project.join(".git"))
            .expect("invariant: project git marker should be creatable");
        let nested = project.join("src/deep");
        fs::create_dir_all(&nested).expect("invariant: nested dir should be creatable");
        let home = home
            .canonicalize()
            .expect("invariant: home dir should canonicalize");

        let discovered = discover_repo_root_from_with_home(&nested, Some(&home))
            .expect("project marker should win before home marker");

        assert_eq!(
            discovered,
            project
                .canonicalize()
                .expect("invariant: project dir should canonicalize")
        );
        fs::remove_dir_all(root).expect("invariant: temp root should be removable");
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("invariant: test clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir_all(&root).expect("invariant: temp root should be creatable");
        root
    }
}
