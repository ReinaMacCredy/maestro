use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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

/// Every worktree's working directory for the repository containing `path`:
/// the main worktree first, then each linked worktree that `git worktree add`
/// created, canonicalized and de-duplicated. The set is identical regardless of
/// which worktree invokes it, so a read-only cross-worktree union (active, msg)
/// sees the same topology from anywhere. maestro never creates, adds, or removes
/// a worktree; this only reads the topology the user already made.
pub fn worktree_roots(path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let repository = discover_repository(path.as_ref())?;

    // The main worktree's workdir is the parent of the common git dir
    // (`<root>/.git`), which resolves to the same path from the main worktree or
    // any linked one. For a lone repo with no linked worktrees, fall back to the
    // opened workdir (there is only one, so consistency is trivial).
    let commondir = repository.commondir();
    let mut main = if commondir.file_name().and_then(|name| name.to_str()) == Some(".git") {
        commondir.parent().map(Path::to_path_buf)
    } else {
        None
    };

    let mut linked: Vec<PathBuf> = Vec::new();
    if let Ok(names) = repository.worktrees() {
        for name in names.iter().flatten() {
            if let Ok(worktree) = repository.find_worktree(name) {
                linked.push(worktree.path().to_path_buf());
            }
        }
    }
    linked.sort();

    if main.is_none() && linked.is_empty() {
        main = repository.workdir().map(Path::to_path_buf);
    }

    // Canonicalize so a /tmp -> /private/tmp style symlink does not split one
    // worktree into two roots; keep the raw path when a worktree dir is gone so
    // a pruned-but-registered worktree still appears (the reader tolerates it).
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut roots: Vec<PathBuf> = Vec::new();
    for root in main.into_iter().chain(linked) {
        let canonical = root.canonicalize().unwrap_or(root);
        if seen.insert(canonical.clone()) {
            roots.push(canonical);
        }
    }
    Ok(roots)
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

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
    use std::collections::BTreeSet;
    use std::env;
    use std::fs;

    fn unique_base(label: &str) -> PathBuf {
        let pid = std::process::id();
        let counter = std::sync::atomic::AtomicU64::new(0);
        let nonce = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("maestro-wt-{label}-{pid}-{nonce}"));
        fs::create_dir_all(&dir).expect("temp base dir is creatable");
        dir
    }

    fn commit_repo(repository: &Repository) {
        let signature = Signature::now("t", "t@example.com").expect("signature");
        let tree_oid = repository
            .index()
            .expect("index")
            .write_tree()
            .expect("write tree");
        let tree = repository.find_tree(tree_oid).expect("tree");
        repository
            .commit(Some("HEAD"), &signature, &signature, "init", &tree, &[])
            .expect("commit");
    }

    fn canon_set(roots: &[PathBuf]) -> BTreeSet<PathBuf> {
        roots
            .iter()
            .map(|root| root.canonicalize().unwrap_or_else(|_| root.clone()))
            .collect()
    }

    #[test]
    fn lone_repo_returns_only_its_own_workdir() {
        let base = unique_base("lone");
        let main = base.join("main");
        fs::create_dir_all(&main).expect("main dir");
        Repository::init(&main).expect("init");

        let roots = worktree_roots(&main).expect("roots");
        assert_eq!(
            canon_set(&roots),
            BTreeSet::from([main.canonicalize().expect("canon main")]),
        );
    }

    #[test]
    fn enumerates_main_plus_linked_worktree_identically_from_either() {
        let base = unique_base("union");
        let main = base.join("main");
        fs::create_dir_all(&main).expect("main dir");
        let repository = Repository::init(&main).expect("init");
        commit_repo(&repository);

        let linked = base.join("wt-oauth");
        repository
            .worktree("wt-oauth", &linked, None)
            .expect("create linked worktree");

        let expected = BTreeSet::from([
            main.canonicalize().expect("canon main"),
            linked.canonicalize().expect("canon linked"),
        ]);

        let from_main = worktree_roots(&main).expect("roots from main");
        let from_linked = worktree_roots(&linked).expect("roots from linked");

        assert_eq!(canon_set(&from_main), expected, "main view sees both");
        assert_eq!(
            canon_set(&from_linked), expected,
            "linked view sees the same set regardless of invoking worktree"
        );
        // The main worktree is listed first so the union has a stable anchor.
        assert_eq!(
            from_main.first().map(|root| root.canonicalize().unwrap()),
            Some(main.canonicalize().expect("canon main")),
        );
    }
}
