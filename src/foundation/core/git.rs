use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use git2::{Oid, Repository, StatusOptions};

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
    /// Tracked or untracked changed paths, repo-relative.
    pub dirty_paths: Vec<PathBuf>,
}

/// Read-only divergence of the current branch from the repo's shared branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchDivergence {
    /// Current branch name, or `None` for a detached HEAD.
    pub branch: Option<String>,
    /// Shared branch used as the merge-back target, such as `main`.
    pub shared_branch: String,
    /// Commits reachable from the current branch but not from the shared branch.
    pub ahead: usize,
    /// Commits reachable from the shared branch but not from the current branch.
    pub behind: usize,
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
        dirty_paths: counts.paths,
    })
}

/// Compare the current HEAD against the shared branch tip without mutating git.
///
/// `None` means no useful comparison is available: unborn/detached HEAD, no
/// local `main`/`master`, or the shared ref cannot be resolved. Callers treat
/// that as "no advisory" rather than an error.
pub fn branch_divergence(path: impl AsRef<Path>) -> Result<Option<BranchDivergence>> {
    let repository = discover_repository(path.as_ref())?;
    let branch = branch_name(&repository)?;
    let Some(head) = head_commit_oid(&repository)? else {
        return Ok(None);
    };
    let Some((shared_branch, shared)) = shared_branch_oid(&repository, branch.as_deref())? else {
        return Ok(None);
    };
    let (ahead, behind) = repository
        .graph_ahead_behind(head, shared)
        .context("failed to compare branch divergence")?;

    Ok(Some(BranchDivergence {
        branch,
        shared_branch,
        ahead,
        behind,
    }))
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

/// Return whether a local branch exists in the repository containing `path`.
pub fn local_branch_exists(path: impl AsRef<Path>, branch: &str) -> Result<bool> {
    let repository = discover_repository(path.as_ref())?;
    match repository.find_branch(branch, git2::BranchType::Local) {
        Ok(_) => Ok(true),
        Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("failed to read branch {branch}")),
    }
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

/// The Git common directory shared by every worktree of the repository
/// containing `path` (the main repo's `.git`). `commondir` resolves to the same
/// place from the main worktree or any linked one, so a lockfile placed here
/// serializes a cross-worktree resource. Canonicalized so a `/tmp` ->
/// `/private/tmp` style symlink does not split it into two paths.
pub fn common_dir(path: impl AsRef<Path>) -> Result<PathBuf> {
    let repository = discover_repository(path.as_ref())?;
    let commondir = repository.commondir();
    Ok(commondir
        .canonicalize()
        .unwrap_or_else(|_| commondir.to_path_buf()))
}

/// Every non-ignored file in the repository at `path`, as repo-relative paths,
/// sorted and de-duplicated: the tracked files (from the index) plus untracked
/// files git would not ignore. This is the `git ls-files --cached --others
/// --exclude-standard` set -- the surface a tree-wide scan (e.g. lean-debt
/// markers) should read, with build output and other ignored paths excluded for
/// free.
pub fn repo_files(path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let repository = discover_repository(path.as_ref())?;
    let mut files: BTreeSet<PathBuf> = BTreeSet::new();

    let index = repository.index().context("failed to read git index")?;
    for entry in index.iter() {
        if let Ok(path) = std::str::from_utf8(&entry.path) {
            files.insert(PathBuf::from(path));
        }
    }

    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false);
    let statuses = repository
        .statuses(Some(&mut options))
        .context("failed to read git status")?;
    for entry in statuses.iter() {
        if entry.status().contains(git2::Status::WT_NEW)
            && let Some(path) = entry.path()
        {
            files.insert(PathBuf::from(path));
        }
    }

    Ok(files.into_iter().collect())
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

fn head_commit_oid(repository: &Repository) -> Result<Option<Oid>> {
    match repository.head() {
        Ok(reference) => Ok(Some(
            reference
                .peel_to_commit()
                .context("failed to peel HEAD to commit")?
                .id(),
        )),
        Err(error) if error.code() == git2::ErrorCode::UnbornBranch => Ok(None),
        Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(error) => Err(error).context("failed to read git HEAD"),
    }
}

fn shared_branch_oid(
    repository: &Repository,
    current_branch: Option<&str>,
) -> Result<Option<(String, Oid)>> {
    for name in ["main", "master"] {
        if current_branch == Some(name) {
            let Some(oid) = head_commit_oid(repository)? else {
                return Ok(None);
            };
            return Ok(Some((name.to_string(), oid)));
        }
        let refname = format!("refs/heads/{name}");
        match repository.find_reference(&refname) {
            Ok(reference) => {
                let oid = reference
                    .peel_to_commit()
                    .with_context(|| format!("failed to peel shared branch {name} to commit"))?
                    .id();
                return Ok(Some((name.to_string(), oid)));
            }
            Err(error) if error.code() == git2::ErrorCode::NotFound => {}
            Err(error) => return Err(error).with_context(|| format!("failed to read {refname}")),
        }
    }
    Ok(None)
}

/// Uncommitted-change counts, split by whether the path is under `.maestro/`.
struct DirtyCounts {
    maestro: usize,
    code_other: usize,
    paths: Vec<PathBuf>,
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
        paths: Vec::new(),
    };
    for entry in statuses.iter() {
        match entry.path() {
            Some(path) => {
                let path = PathBuf::from(path);
                if path.starts_with(".maestro/") {
                    counts.maestro += 1;
                } else {
                    counts.code_other += 1;
                }
                counts.paths.push(path);
            }
            None => counts.code_other += 1,
        }
    }
    counts.paths.sort();
    counts.paths.dedup();
    Ok(counts)
}

fn is_dirty(repository: &Repository) -> Result<bool> {
    let counts = dirty_counts(repository)?;
    Ok(counts.maestro + counts.code_other > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{RepositoryInitOptions, Signature};
    use std::collections::BTreeSet;
    use std::env;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_base(label: &str) -> PathBuf {
        let pid = std::process::id();
        let nonce = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("maestro-wt-{label}-{pid}-{nonce}"));
        fs::create_dir_all(&dir).expect("temp base dir is creatable");
        dir
    }

    fn init_main_repo(path: &Path) -> Repository {
        let mut options = RepositoryInitOptions::new();
        options.initial_head("main");
        Repository::init_opts(path, &options).expect("init main repo")
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

    fn commit_file(repository: &Repository, path: &str, contents: &str, message: &str) -> Oid {
        let workdir = repository.workdir().expect("workdir repo");
        let full_path = workdir.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        fs::write(&full_path, contents).expect("write fixture file");

        let mut index = repository.index().expect("index");
        index
            .add_path(Path::new(path))
            .expect("fixture path added to index");
        index.write().expect("index written");
        let tree_oid = index.write_tree().expect("tree");
        let tree = repository.find_tree(tree_oid).expect("tree");
        let signature = Signature::now("t", "t@example.com").expect("signature");
        let parents = match repository.head() {
            Ok(reference) => vec![
                reference
                    .peel_to_commit()
                    .expect("HEAD peels to parent commit"),
            ],
            Err(error) if error.code() == git2::ErrorCode::UnbornBranch => Vec::new(),
            Err(error) if error.code() == git2::ErrorCode::NotFound => Vec::new(),
            Err(error) => panic!("unexpected HEAD error: {error}"),
        };
        let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();
        repository
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )
            .expect("commit file")
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
        init_main_repo(&main);

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
        let repository = init_main_repo(&main);
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
            canon_set(&from_linked),
            expected,
            "linked view sees the same set regardless of invoking worktree"
        );
        // The main worktree is listed first so the union has a stable anchor.
        assert_eq!(
            from_main.first().map(|root| root.canonicalize().unwrap()),
            Some(main.canonicalize().expect("canon main")),
        );
    }

    #[test]
    fn branch_divergence_reports_shared_branch_moving_under_a_slice() {
        let base = unique_base("diverged");
        let main = base.join("main");
        fs::create_dir_all(&main).expect("main dir");
        let repository = init_main_repo(&main);
        let base_oid = commit_file(&repository, "base.txt", "base", "base");
        let base_commit = repository.find_commit(base_oid).expect("base commit");
        repository
            .branch("slice", &base_commit, false)
            .expect("slice branch");
        drop(base_commit);

        commit_file(&repository, "main.txt", "main", "main moves");
        repository.set_head("refs/heads/slice").expect("head slice");

        let divergence = branch_divergence(&main)
            .expect("divergence reads")
            .expect("shared branch exists");
        assert_eq!(divergence.branch.as_deref(), Some("slice"));
        assert_eq!(divergence.shared_branch, "main");
        assert_eq!(divergence.ahead, 0);
        assert_eq!(divergence.behind, 1);
    }

    #[test]
    fn branch_divergence_is_zero_on_shared_branch() {
        let base = unique_base("current");
        let main = base.join("main");
        fs::create_dir_all(&main).expect("main dir");
        let repository = init_main_repo(&main);
        commit_file(&repository, "base.txt", "base", "base");

        let divergence = branch_divergence(&main)
            .expect("divergence reads")
            .expect("shared branch exists");
        assert_eq!(divergence.branch.as_deref(), Some("main"));
        assert_eq!(divergence.shared_branch, "main");
        assert_eq!(divergence.ahead, 0);
        assert_eq!(divergence.behind, 0);
    }
}
