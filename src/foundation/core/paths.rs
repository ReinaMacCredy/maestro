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

    /// Return the live DB-backed Maestro store.
    pub fn store_db_file(&self) -> PathBuf {
        self.maestro_dir().join("store.sqlite")
    }

    /// Return the editable workbench root for reopening finalized DB-backed cards.
    pub fn workbench_dir(&self) -> PathBuf {
        self.maestro_dir().join("workbench")
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

    /// Return the archived-cards directory (`.maestro/archive/cards`).
    ///
    /// The card-model archive sibling of `cards/`; `archive <feature>` moves the
    /// feature card and its `parent=<feature>` children here as whole directories
    /// with digest entries recorded in `INDEX.md`.
    pub fn archive_cards_dir(&self) -> PathBuf {
        self.archive_dir().join("cards")
    }

    /// Return the archive lid (`.maestro/archive/cards/INDEX.md`): one digest
    /// line per archived card, appended by the archive writers and read back
    /// by `resume`'s memory section.
    pub fn archive_index_file(&self) -> PathBuf {
        self.archive_cards_dir().join("INDEX.md")
    }

    /// Return the backup artifact directory.
    pub fn backups_dir(&self) -> PathBuf {
        self.maestro_dir().join("backups")
    }

    /// Return the local index directory (`.maestro/index`).
    ///
    /// Machine-local derived state (gitignored, like `runs/`): created on
    /// demand by the text index, never by `init`, and safe to delete --
    /// `maestro index rebuild` or the next indexed read recreates it.
    pub fn index_dir(&self) -> PathBuf {
        self.maestro_dir().join("index")
    }

    /// Return the text index file behind `list --grep` (SPEC-archive-memory-2 R6).
    pub fn text_index_file(&self) -> PathBuf {
        self.index_dir().join("text.json")
    }

    /// Return the unified grep/search index directory.
    pub fn search_index_dir(&self) -> PathBuf {
        self.index_dir().join("search")
    }

    /// Return the unified grep/search manifest file.
    pub fn search_manifest_file(&self) -> PathBuf {
        self.search_index_dir().join("manifest.json")
    }

    /// Return the Maestro-memory shard used by `maestro grep`.
    pub fn memory_shard_file(&self) -> PathBuf {
        self.search_index_dir().join("memory.shard")
    }

    /// Return the repo-source shard used by `maestro grep`.
    pub fn source_shard_file(&self) -> PathBuf {
        self.search_index_dir().join("source.shard")
    }

    /// Return the unified grep/search writer lock file.
    pub fn search_writer_lock_file(&self) -> PathBuf {
        self.search_index_dir().join("write.lock")
    }

    /// Return the install lockfile path.
    pub fn install_lock_file(&self) -> PathBuf {
        self.maestro_dir().join("install-lock.yaml")
    }

    /// Return the linked-card messaging directory (`.maestro/channels`).
    ///
    /// Machine-local conversation state (gitignored, like `runs/`): created on
    /// demand by the first `msg send`, never by `init`.
    pub fn channels_dir(&self) -> PathBuf {
        self.maestro_dir().join("channels")
    }

    /// Return the repo-local custom loop recipe directory.
    pub fn loop_recipes_dir(&self) -> PathBuf {
        self.maestro_dir().join("loop-recipes")
    }
}

/// Infer a card's project from where it is being created, gated by the repo's
/// declared `projects:` scopes (T3). Purely lexical: `cwd` is passed in (not read
/// from the process) and matched against `repo_root` by prefix, so it is
/// unit-testable without touching disk or mutating the global cwd.
///
/// Patterns are evaluated IN ORDER; the first match's captured segment wins.
/// Exactly three declared forms are supported:
/// - `*`            -> the first path segment (`svc-pay/src` -> `svc-pay`).
/// - `prefix/*`     -> the second segment when the first equals `prefix`
///   (`services/pay/x` -> `pay`; bare `services` -> None).
/// - literal        -> the literal when it equals the first segment
///   (`fe/src` with `["fe"]` -> `fe`).
///
/// Returns `None` when: `patterns` is empty (the activation gate -- no
/// declaration means nothing is inferred), `cwd` equals `repo_root` (no relative
/// segments), `cwd` is not under `repo_root`, or no pattern matches.
pub fn infer_project(repo_root: &Path, cwd: &Path, patterns: &[String]) -> Option<String> {
    if patterns.is_empty() {
        return None;
    }
    let relative = cwd.strip_prefix(repo_root).ok()?;
    let segments: Vec<&str> = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|segment| !segment.is_empty())
        .collect();
    let first = segments.first()?;

    for pattern in patterns {
        if pattern == "*" {
            return Some((*first).to_string());
        }
        if let Some(prefix) = pattern.strip_suffix("/*") {
            if *first == prefix
                && let Some(second) = segments.get(1)
            {
                return Some((*second).to_string());
            }
            continue;
        }
        if *first == pattern.as_str() {
            return Some(pattern.clone());
        }
    }
    None
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

    #[test]
    fn infer_top_level_wildcard_captures_first_segment() {
        let root = Path::new("/repo");
        let patterns = vec!["*".to_string()];
        assert_eq!(
            infer_project(root, &root.join("svc-pay/src"), &patterns),
            Some("svc-pay".to_string())
        );
    }

    #[test]
    fn infer_top_level_wildcard_at_repo_root_is_none() {
        let root = Path::new("/repo");
        let patterns = vec!["*".to_string()];
        assert_eq!(infer_project(root, root, &patterns), None);
    }

    #[test]
    fn infer_nested_prefix_captures_second_segment() {
        let root = Path::new("/repo");
        let patterns = vec!["services/*".to_string()];
        assert_eq!(
            infer_project(root, &root.join("services/pay/x"), &patterns),
            Some("pay".to_string())
        );
    }

    #[test]
    fn infer_nested_prefix_without_second_segment_is_none() {
        let root = Path::new("/repo");
        let patterns = vec!["services/*".to_string()];
        assert_eq!(infer_project(root, &root.join("services"), &patterns), None);
    }

    #[test]
    fn infer_nested_prefix_non_matching_first_segment_is_none() {
        let root = Path::new("/repo");
        let patterns = vec!["services/*".to_string()];
        assert_eq!(infer_project(root, &root.join("fe/x"), &patterns), None);
    }

    #[test]
    fn infer_literal_matches_first_segment() {
        let root = Path::new("/repo");
        let patterns = vec!["fe".to_string(), "be".to_string()];
        assert_eq!(
            infer_project(root, &root.join("fe/src"), &patterns),
            Some("fe".to_string())
        );
    }

    #[test]
    fn infer_literal_set_ignores_unlisted_folder() {
        let root = Path::new("/repo");
        let patterns = vec!["fe".to_string(), "be".to_string()];
        assert_eq!(infer_project(root, &root.join("docs/x"), &patterns), None);
    }

    #[test]
    fn infer_without_declaration_is_always_none() {
        let root = Path::new("/repo");
        assert_eq!(infer_project(root, &root.join("svc-pay/src"), &[]), None);
    }

    #[test]
    fn infer_first_matching_pattern_wins() {
        let root = Path::new("/repo");
        // A literal `fe` ahead of the catch-all `*` must capture `fe` itself,
        // not let the wildcard win -- ordering, not specificity, decides.
        let patterns = vec!["fe".to_string(), "*".to_string()];
        assert_eq!(
            infer_project(root, &root.join("fe/deep/leaf"), &patterns),
            Some("fe".to_string())
        );
    }

    #[test]
    fn infer_captures_by_segment_index_not_depth() {
        let root = Path::new("/repo");
        // Deeper than the pattern reaches: `*` still grabs the FIRST segment,
        // never the leaf.
        let patterns = vec!["*".to_string()];
        assert_eq!(
            infer_project(root, &root.join("svc-pay/a/b/c"), &patterns),
            Some("svc-pay".to_string())
        );
    }

    #[test]
    fn infer_cwd_outside_repo_is_none() {
        let patterns = vec!["*".to_string()];
        assert_eq!(
            infer_project(Path::new("/repo"), Path::new("/other/svc-pay"), &patterns),
            None
        );
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
