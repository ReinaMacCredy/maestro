mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use git2::{BranchType, IndexAddOption, Repository, Signature};
use serde_yaml::Value as YamlValue;
use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn assert_success(output: &std::process::Output, args: &[&str]) {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        stdout(output),
        stderr(output)
    );
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("invariant: stdout should be UTF-8")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("invariant: stderr should be UTF-8")
}

fn commit_worktree(repository: &Repository, message: &str) {
    let mut index = repository.index().expect("invariant: git index readable");
    index
        .add_all(["*"].iter(), IndexAddOption::DEFAULT, None)
        .expect("invariant: git index add should succeed");
    index
        .write()
        .expect("invariant: git index write should succeed");
    let tree_id = index
        .write_tree()
        .expect("invariant: git tree write should succeed");
    let tree = repository
        .find_tree(tree_id)
        .expect("invariant: git tree should exist");
    let signature = Signature::now("Maestro Test", "maestro@example.test")
        .expect("invariant: git signature should be constructable");
    let parent = repository
        .head()
        .ok()
        .and_then(|head| head.target())
        .and_then(|oid| repository.find_commit(oid).ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .expect("invariant: git commit should succeed");
}

fn git_worktree_list(repo: &Path) -> String {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo)
        .output()
        .expect("invariant: git should be runnable");
    assert!(
        output.status.success(),
        "git worktree list failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    stdout(&output)
}

fn setup_repo() -> (TestTempDir, Repository) {
    let temp = TestTempDir::new("maestro-worktree-ledger-cli");
    let repository = Repository::init(temp.path()).expect("invariant: git repo should initialize");
    fs::write(temp.path().join("seed.txt"), "seed\n").expect("invariant: seed writable");
    commit_worktree(&repository, "seed");
    let init = maestro(temp.path(), &["init", "--yes"]);
    assert_success(&init, &["init", "--yes"]);
    (temp, repository)
}

#[test]
fn worktree_record_verbs_update_ledger_without_running_git() {
    let (temp, repository) = setup_repo();
    let feature = maestro(
        temp.path(),
        &["feature", "new", "Worktree ledger", "--id-only"],
    );
    assert_success(
        &feature,
        &["feature", "new", "Worktree ledger", "--id-only"],
    );
    let feature_id = stdout(&feature).trim().to_string();
    let head = repository
        .head()
        .expect("invariant: HEAD should exist")
        .target()
        .expect("invariant: HEAD should point at a commit")
        .to_string();
    let branch = "codex/passive-ledger";
    let lane_path = ".maestro/worktree/passive-ledger";
    let before_worktrees = git_worktree_list(temp.path());

    let plan = maestro(
        temp.path(),
        &[
            "worktree",
            "plan",
            &feature_id,
            "--slug",
            "passive-ledger",
            "--branch",
            branch,
            "--path",
            lane_path,
            "--base",
            &head,
        ],
    );
    assert_success(&plan, &["worktree", "plan"]);
    assert!(
        repository.find_branch(branch, BranchType::Local).is_err(),
        "plan must not create the worker branch"
    );
    assert_eq!(
        git_worktree_list(temp.path()),
        before_worktrees,
        "plan must not create a git worktree"
    );

    let release_after_plan = maestro(
        temp.path(),
        &[
            "active",
            "release",
            &feature_id,
            "--reason",
            "interrupted-plan-check",
        ],
    );
    assert_success(&release_after_plan, &["active", "release"]);
    let recovery_status = maestro(temp.path(), &["status"]);
    assert_success(&recovery_status, &["status"]);
    let recovery_status = stdout(&recovery_status);
    assert!(
        recovery_status.contains("WORKTREE RECOVERY"),
        "{recovery_status}"
    );
    assert!(
        recovery_status.contains("branch_reserved_path_missing"),
        "{recovery_status}"
    );
    assert!(
        recovery_status.contains("git worktree add -b"),
        "{recovery_status}"
    );
    assert!(
        !recovery_status.contains("max worktree") && !recovery_status.contains("maximum worktree"),
        "{recovery_status}"
    );

    let lane_created = maestro(
        temp.path(),
        &[
            "worktree",
            "mark",
            &feature_id,
            "--slug",
            "passive-ledger",
            "--lane-created",
        ],
    );
    assert_success(&lane_created, &["worktree", "mark", "--lane-created"]);
    let merged = maestro(
        temp.path(),
        &[
            "worktree",
            "mark",
            &feature_id,
            "--slug",
            "passive-ledger",
            "--merged-back",
            "--commit",
            &head,
        ],
    );
    assert_success(&merged, &["worktree", "mark", "--merged-back"]);
    let verified = maestro(
        temp.path(),
        &[
            "worktree",
            "mark",
            &feature_id,
            "--slug",
            "passive-ledger",
            "--verified",
            "--commit",
            &head,
        ],
    );
    assert_success(&verified, &["worktree", "mark", "--verified"]);

    let head_commit = repository
        .head()
        .expect("invariant: HEAD should exist")
        .peel_to_commit()
        .expect("invariant: HEAD should peel to commit");
    repository
        .branch(branch, &head_commit, false)
        .expect("invariant: manual branch creation should succeed");
    let child = maestro(
        temp.path(),
        &[
            "task",
            "create",
            "Worker cleanup guard",
            "--feature",
            &feature_id,
            "--id-only",
        ],
    );
    assert_success(&child, &["task", "create"]);
    let child_id = stdout(&child).trim().to_string();
    let claim_child = maestro(temp.path(), &["card", "claim", &child_id]);
    assert_success(&claim_child, &["card", "claim"]);
    let active_status = maestro(temp.path(), &["status"]);
    assert_success(&active_status, &["status"]);
    let active_status = stdout(&active_status);
    assert!(
        !active_status.contains("cleanup_due"),
        "active ownership must gate cleanup_due:\n{active_status}"
    );
    let release = maestro(
        temp.path(),
        &["active", "release", &child_id, "--reason", "cleanup-ready"],
    );
    assert_success(&release, &["active", "release"]);

    let status = maestro(temp.path(), &["status"]);
    assert_success(&status, &["status"]);
    let status = stdout(&status);
    assert!(status.contains("WORKTREE RECOVERY"), "{status}");
    assert!(status.contains(&feature_id), "{status}");
    assert!(status.contains("cleanup_due"), "{status}");
    assert!(status.contains("git worktree remove"), "{status}");
    assert!(
        status.contains("maestro worktree cleanup-record"),
        "{status}"
    );

    let show = maestro(temp.path(), &["feature", "show", &feature_id]);
    assert_success(&show, &["feature", "show"]);
    let show = stdout(&show);
    assert!(show.contains("worktrees:"), "{show}");
    assert!(show.contains("state: cleanup_due"), "{show}");
    assert!(show.contains("branch_exists: true"), "{show}");
    assert!(show.contains("path_exists: false"), "{show}");

    let finalize = maestro(temp.path(), &["feature", "finalize", &feature_id]);
    assert_success(&finalize, &["feature", "finalize"]);
    let handoff = fs::read_to_string(
        temp.path()
            .join(".maestro/cards")
            .join(&feature_id)
            .join("handoff.md"),
    )
    .expect("handoff should exist");
    assert!(handoff.contains("## Worktree Ledger"), "{handoff}");
    assert!(
        handoff.contains("- Lane `passive-ledger`: `cleanup_due`"),
        "{handoff}"
    );
    assert!(
        handoff.contains("- Worktree ledger: `.maestro/cards/"),
        "{handoff}"
    );

    let cleanup = maestro(
        temp.path(),
        &[
            "worktree",
            "cleanup-record",
            &feature_id,
            "--slug",
            "passive-ledger",
            "--removed-path",
            lane_path,
            "--deleted-branch",
            branch,
            "--pruned",
        ],
    );
    assert_success(&cleanup, &["worktree", "cleanup-record"]);
    assert!(
        repository.find_branch(branch, BranchType::Local).is_ok(),
        "cleanup-record must not delete the worker branch"
    );
    assert_eq!(
        git_worktree_list(temp.path()),
        before_worktrees,
        "record verbs must not add, remove, or prune git worktrees"
    );

    let show_complete = maestro(temp.path(), &["feature", "show", &feature_id]);
    assert_success(&show_complete, &["feature", "show"]);
    let show_complete = stdout(&show_complete);
    assert!(
        show_complete.contains("state: cleanup_complete"),
        "{show_complete}"
    );
    assert!(
        show_complete.contains("cleanup_receipts:"),
        "{show_complete}"
    );
    assert!(
        show_complete.contains("pruned_stale_metadata: true"),
        "{show_complete}"
    );
    let status_after_cleanup = maestro(temp.path(), &["status"]);
    assert_success(&status_after_cleanup, &["status"]);
    let status_after_cleanup = stdout(&status_after_cleanup);
    assert!(
        !status_after_cleanup.contains("WORKTREE RECOVERY"),
        "cleanup_complete must not keep prompting cleanup:\n{status_after_cleanup}"
    );

    let ledger_path = temp
        .path()
        .join(".maestro/cards")
        .join(&feature_id)
        .join("worktree.yml");
    let ledger: YamlValue =
        serde_yaml::from_str(&fs::read_to_string(ledger_path).expect("ledger should exist"))
            .expect("ledger should parse");
    assert_eq!(ledger["lanes"][0]["intent"]["slug"], "passive-ledger");
    assert_eq!(ledger["lanes"][0]["milestones"]["merged_back_commit"], head);
    assert_eq!(ledger["lanes"][0]["milestones"]["verified_commit"], head);
    assert_eq!(
        ledger["lanes"][0]["cleanup_receipts"][0]["deleted_branch"],
        branch
    );
}
