mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::Command;

use maestro::decisions::template::decision_markdown;
use maestro::foundation::core::fs::ensure_dir;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
}

fn init_git_marker(repo: &Path) {
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
}

fn stdout(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

fn assert_failure(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
}

#[test]
fn feature_guarded_lifecycle_via_cli() {
    let temp_dir = TestTempDir::new("maestro-feature-command-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let create_output = stdout(
        maestro(&["feature", "new", "Billing CSV export"], temp_dir.path()),
        &["feature", "new", "Billing CSV export"],
    );
    assert!(create_output.contains("created feature billing-csv-export"));

    let show_output = stdout(
        maestro(&["feature", "show", "billing-csv-export"], temp_dir.path()),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show_output.contains("status: proposed"));
    // `feature new` scaffolds notes.md; `show` renders it.
    assert!(show_output.contains("notes:"));
    assert!(show_output.contains("Design notes:"));

    // accept blocks on an incomplete contract, naming the gaps.
    let accept_args = ["feature", "accept", "billing-csv-export"];
    let accept_stderr = assert_failure(maestro(&accept_args, temp_dir.path()), &accept_args);
    assert!(accept_stderr.contains("acceptance"));
    assert!(accept_stderr.contains("affected_areas"));

    // author the contract, then accept freezes it.
    let set_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--acceptance",
        "exports a valid csv",
        "--area",
        "billing",
    ];
    let set_output = stdout(maestro(&set_args, temp_dir.path()), &set_args);
    assert!(set_output.contains("acceptance=1"));
    assert!(set_output.contains("areas=1"));

    // accept also requires a captured baseline (F); ship requires it proven.
    let features_dir = temp_dir.path().join(".maestro/features");
    write_baseline(&features_dir, "billing-csv-export");
    write_qa_slice(&features_dir, "billing-csv-export");

    let dry_args = ["feature", "accept", "billing-csv-export", "--dry-run"];
    let dry_output = stdout(maestro(&dry_args, temp_dir.path()), &dry_args);
    assert!(dry_output.contains("would accept"));

    let accept_output = stdout(maestro(&accept_args, temp_dir.path()), &accept_args);
    assert!(accept_output.contains("accepted billing-csv-export"));

    // start, then ship blocks while a live child task exists.
    stdout(
        maestro(&["feature", "start", "billing-csv-export"], temp_dir.path()),
        &["feature", "start", "billing-csv-export"],
    );

    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "verified");
    write_task(&tasks_dir, "task-002", "billing-csv-export", "verified");
    write_task(&tasks_dir, "task-003", "billing-csv-export", "in_progress");

    let ship_args = [
        "feature",
        "ship",
        "billing-csv-export",
        "--outcome",
        "csv export shipped",
    ];
    let ship_stderr = assert_failure(maestro(&ship_args, temp_dir.path()), &ship_args);
    assert!(ship_stderr.contains("task-003"));

    // resolve the live child, then ship succeeds.
    write_task(&tasks_dir, "task-003", "billing-csv-export", "verified");
    let ship_output = stdout(maestro(&ship_args, temp_dir.path()), &ship_args);
    assert!(ship_output.contains("shipped billing-csv-export"));

    let show_after_ship = stdout(
        maestro(&["feature", "show", "billing-csv-export"], temp_dir.path()),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show_after_ship.contains("status: shipped"));
    // ship --outcome records the one-line result; show renders it.
    assert!(show_after_ship.contains("outcome: csv export shipped"));

    // A shipped feature is terminal, so the default list hides it behind a hint.
    let list_output = stdout(
        maestro(&["feature", "list"], temp_dir.path()),
        &["feature", "list"],
    );
    assert!(!list_output.contains("billing-csv-export"));
    assert!(list_output.contains("terminal feature(s) hidden"));

    // `--all` surfaces it with its frozen status and computed counts.
    let list_all = stdout(
        maestro(&["feature", "list", "--all"], temp_dir.path()),
        &["feature", "list", "--all"],
    );
    assert!(list_all.contains("billing-csv-export"));
    assert!(list_all.contains("shipped"));
    assert!(list_all.contains("tasks=3"));
    assert!(list_all.contains("verified=3"));
    // the outcome rides the title column in `list --all`.
    assert!(list_all.contains("csv export shipped"));
}

#[test]
fn feature_cancel_via_cli_cascades_to_live_tasks() {
    let temp_dir = TestTempDir::new("maestro-feature-cancel-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    stdout(
        maestro(&["feature", "new", "Billing CSV export"], temp_dir.path()),
        &["feature", "new", "Billing CSV export"],
    );
    let set_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--acceptance",
        "exports a valid csv",
        "--area",
        "billing",
    ];
    stdout(maestro(&set_args, temp_dir.path()), &set_args);
    write_baseline(
        &temp_dir.path().join(".maestro/features"),
        "billing-csv-export",
    );
    stdout(
        maestro(
            &["feature", "accept", "billing-csv-export"],
            temp_dir.path(),
        ),
        &["feature", "accept", "billing-csv-export"],
    );
    stdout(
        maestro(&["feature", "start", "billing-csv-export"], temp_dir.path()),
        &["feature", "start", "billing-csv-export"],
    );

    let tasks_dir = temp_dir.path().join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "in_progress");

    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope dropped",
    ];
    let cancel_output = stdout(maestro(&cancel_args, temp_dir.path()), &cancel_args);
    assert!(cancel_output.contains("cancelled billing-csv-export"));
    assert!(cancel_output.contains("task-001"));

    let show_output = stdout(
        maestro(&["feature", "show", "billing-csv-export"], temp_dir.path()),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show_output.contains("status: cancelled"));

    let task_raw = fs::read_to_string(tasks_dir.join("task-001").join("task.yaml"))
        .expect("invariant: cascaded child task should be readable");
    assert!(task_raw.contains("abandoned"));
}

#[test]
fn decision_new_list_show_auto_increment_and_preserve_template() {
    let temp_dir = TestTempDir::new("maestro-decision-command-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let decisions_dir = temp_dir.path().join(".maestro/decisions");
    ensure_dir(&decisions_dir).expect("invariant: decisions directory should be creatable");
    fs::write(
        decisions_dir.join("decision-007-existing.md"),
        decision_markdown(7, "Existing"),
    )
    .expect("invariant: seed decision should be writable");

    let title = "Use single HARNESS.md instead of three adapter files";
    let new_output = stdout(
        maestro(&["decision", "new", title], temp_dir.path()),
        &["decision", "new", title],
    );
    assert!(new_output.contains("created decision decision-008"));

    let expected_file =
        decisions_dir.join("decision-008-use-single-harness-md-instead-of-three-adapter-files.md");
    assert!(expected_file.is_file());
    assert_eq!(
        fs::read_to_string(&expected_file).expect("invariant: decision file should be readable"),
        decision_markdown(8, title)
    );

    let list_output = stdout(
        maestro(&["decision", "list"], temp_dir.path()),
        &["decision", "list"],
    );
    assert!(list_output.contains("decision-007-existing"));
    assert!(
        list_output.contains("decision-008-use-single-harness-md-instead-of-three-adapter-files")
    );

    let show_output = stdout(
        maestro(
            &[
                "decision",
                "show",
                "decision-008-use-single-harness-md-instead-of-three-adapter-files",
            ],
            temp_dir.path(),
        ),
        &[
            "decision",
            "show",
            "decision-008-use-single-harness-md-instead-of-three-adapter-files",
        ],
    );
    assert_eq!(show_output, decision_markdown(8, title));
}

#[test]
fn decision_show_rejects_path_traversal_ids() {
    let temp_dir = TestTempDir::new("maestro-decision-command-traversal-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let stderr = assert_failure(
        maestro(&["decision", "show", "../outside.md"], temp_dir.path()),
        &["decision", "show", "../outside.md"],
    );
    assert!(stderr.contains("invalid decision id"));

    let decisions_dir = temp_dir.path().join(".maestro/decisions");
    let nested_dir = decisions_dir.join("nested");
    fs::create_dir(&nested_dir).expect("invariant: nested decisions dir should be creatable");
    fs::write(nested_dir.join("secret.md"), "secret\n")
        .expect("invariant: nested decision file should be writable");
    let nested_stderr = assert_failure(
        maestro(&["decision", "show", "nested/secret.md"], temp_dir.path()),
        &["decision", "show", "nested/secret.md"],
    );
    assert!(nested_stderr.contains("invalid decision id"));

    fs::write(temp_dir.path().join("outside.md"), "secret\n")
        .expect("invariant: outside file should be writable");
    unix_fs::symlink(
        temp_dir.path().join("outside.md"),
        decisions_dir.join("decision-001-leak.md"),
    )
    .expect("invariant: decision symlink should be creatable");
    let symlink_stderr = assert_failure(
        maestro(&["decision", "show", "decision-001-leak"], temp_dir.path()),
        &["decision", "show", "decision-001-leak"],
    );
    assert!(symlink_stderr.contains("not found"));
}

#[test]
fn decision_new_rejects_an_empty_title() {
    let temp_dir = TestTempDir::new("maestro-decision-command-empty-title");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    let stderr = assert_failure(
        maestro(&["decision", "new", "   "], temp_dir.path()),
        &["decision", "new", "   "],
    );
    assert!(stderr.contains("title cannot be empty"), "{stderr}");

    // The boundary rejected it before any file was written; no husk left behind.
    let decisions_dir = temp_dir.path().join(".maestro/decisions");
    if decisions_dir.is_dir() {
        let husk = fs::read_dir(&decisions_dir)
            .expect("invariant: decisions dir should be listable")
            .count();
        assert_eq!(husk, 0, "no decision file should be written for an empty title");
    }
}

#[test]
fn feature_verbs_reject_path_traversal_ids() {
    let temp_dir = TestTempDir::new("maestro-feature-command-traversal-test");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    // Read path: `show` joins the id into `features/<id>/feature.yaml`. A `..`
    // id must be rejected before the join can escape the features dir (the
    // `load_record_at` chokepoint).
    let show_stderr = assert_failure(
        maestro(&["feature", "show", "../outside"], temp_dir.path()),
        &["feature", "show", "../outside"],
    );
    assert!(show_stderr.contains("invalid feature id"));

    // An absolute id replaces the whole path on join — a distinct failure mode
    // from `..`, both caught by the single-normal-component rule.
    let abs_stderr = assert_failure(
        maestro(&["feature", "show", "/etc/passwd"], temp_dir.path()),
        &["feature", "show", "/etc/passwd"],
    );
    assert!(abs_stderr.contains("invalid feature id"));

    // `unarchive` is the one verb that never goes through `load_record_at`; it
    // stats and renames directly, so it carries its own guard.
    let unarchive_stderr = assert_failure(
        maestro(&["feature", "unarchive", "../outside"], temp_dir.path()),
        &["feature", "unarchive", "../outside"],
    );
    assert!(unarchive_stderr.contains("invalid feature id"));
}

/// Write a minimal QA baseline (one `[bl-001]` scenario) so the accept gate's
/// baseline precondition (F) and the ship gate's coverage check are satisfiable.
fn write_baseline(features_dir: &Path, id: &str) {
    let dir = features_dir.join(id);
    ensure_dir(&dir).expect("invariant: feature directory should be creatable");
    fs::write(
        dir.join("baseline.md"),
        "---\namend_log_position: 0\n---\n\n### QA Baseline Contract\n\n- Scenario Matrix:\n  - [bl-001] csv export round-trips\n",
    )
    .expect("invariant: baseline.md should be writable");
}

/// Write a counting QA slice (scenarios + evidence) covering `[bl-001]`.
fn write_qa_slice(features_dir: &Path, id: &str) {
    let dir = features_dir.join(id);
    ensure_dir(&dir).expect("invariant: feature directory should be creatable");
    fs::write(
        dir.join("qa-slices.yaml"),
        "slices:\n  - scenarios: [\"bl-001\"]\n    evidence: [\"manual: exported csv opens in a spreadsheet\"]\n",
    )
    .expect("invariant: qa-slices.yaml should be writable");
}

fn write_task(tasks_dir: &Path, id: &str, feature_id: &str, state: &str) {
    let task_dir = tasks_dir.join(id);
    ensure_dir(&task_dir).expect("invariant: task directory should be creatable");
    // A complete TaskRecord: the cancel cascade loads and transitions the child,
    // so a projection-only stub (id/feature_id/state) fails to deserialize.
    fs::write(
        task_dir.join("task.yaml"),
        format!(
            "schema_version: maestro.task.v1\nid: {id}\nslug: {id}\nfeature_id: {feature_id}\ntitle: {id}\nstate: {state}\nacceptance_locked: false\nverification: {{}}\ncreated_at: \"1\"\nupdated_at: \"1\"\n"
        ),
    )
    .expect("invariant: task yaml should be writable");
}

/// Drive a fresh feature all the way to Shipped: new -> set -> baseline+slice ->
/// accept -> start -> one verified child -> ship.
fn ship_feature(root: &Path, title: &str, slug: &str, child: &str) {
    stdout(
        maestro(&["feature", "new", title], root),
        &["feature", "new", title],
    );
    let set_args = [
        "feature",
        "set",
        slug,
        "--acceptance",
        "works",
        "--area",
        "core",
    ];
    stdout(maestro(&set_args, root), &set_args);
    let features_dir = root.join(".maestro/features");
    write_baseline(&features_dir, slug);
    write_qa_slice(&features_dir, slug);
    stdout(
        maestro(&["feature", "accept", slug], root),
        &["feature", "accept", slug],
    );
    stdout(
        maestro(&["feature", "start", slug], root),
        &["feature", "start", slug],
    );
    write_task(&root.join(".maestro/tasks"), child, slug, "verified");
    stdout(
        maestro(&["feature", "ship", slug], root),
        &["feature", "ship", slug],
    );
}

#[test]
fn feature_create_refuses_a_slug_held_in_the_archive() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-slug");
    init_git_marker(temp_dir.path());
    stdout(
        maestro(&["init", "--yes"], temp_dir.path()),
        &["init", "--yes"],
    );

    // An archived feature still owns its slug; `create` must not reissue it (L6a).
    let archived = temp_dir.path().join(".maestro/archive/features/csv-export");
    ensure_dir(&archived).expect("invariant: archive feature dir should be creatable");
    fs::write(
        archived.join("feature.yaml"),
        "schema_version: maestro.feature.v1\nid: csv-export\ntitle: CSV Export\nstatus: shipped\ncreated_at: \"1\"\nupdated_at: \"1\"\n",
    )
    .expect("invariant: archived feature yaml should be writable");

    let args = ["feature", "new", "CSV Export"];
    let stderr = assert_failure(maestro(&args, temp_dir.path()), &args);
    assert!(stderr.contains("csv-export"));
    assert!(stderr.contains("archive"));
}

/// Drive a feature to Shipped, then archive it: the feature dir + its terminal
/// child tasks leave the live scan, the QA artifacts travel inside the archived
/// dir, reads fall through (L6b), and unarchive round-trips (§5.9).
#[test]
fn feature_archive_cascades_children_with_qa_and_round_trips() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-shipped");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    stdout(
        maestro(&["feature", "new", "Billing CSV export"], root),
        &["feature", "new", "Billing CSV export"],
    );
    let set_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--acceptance",
        "exports a valid csv",
        "--area",
        "billing",
    ];
    stdout(maestro(&set_args, root), &set_args);

    let features_dir = root.join(".maestro/features");
    write_baseline(&features_dir, "billing-csv-export");
    write_qa_slice(&features_dir, "billing-csv-export");
    stdout(
        maestro(&["feature", "accept", "billing-csv-export"], root),
        &["feature", "accept", "billing-csv-export"],
    );

    // A non-terminal feature cannot be archived.
    let in_progress = ["feature", "archive", "billing-csv-export"];
    stdout(
        maestro(&["feature", "start", "billing-csv-export"], root),
        &["feature", "start", "billing-csv-export"],
    );
    let not_terminal = assert_failure(maestro(&in_progress, root), &in_progress);
    assert!(not_terminal.contains("not terminal"));

    let tasks_dir = root.join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "verified");
    write_task(&tasks_dir, "task-002", "billing-csv-export", "verified");
    stdout(
        maestro(&["feature", "ship", "billing-csv-export"], root),
        &["feature", "ship", "billing-csv-export"],
    );

    // Archive cascades the terminal children alongside the feature.
    let archive_args = ["feature", "archive", "billing-csv-export"];
    let archived = stdout(maestro(&archive_args, root), &archive_args);
    assert!(archived.contains("archived feature billing-csv-export"));
    assert!(archived.contains("task-001"));
    assert!(archived.contains("task-002"));

    // The feature dir + QA artifacts moved into the archive sibling tree.
    let archived_feature = root.join(".maestro/archive/features/billing-csv-export");
    assert!(archived_feature.join("feature.yaml").is_file());
    assert!(archived_feature.join("baseline.md").is_file());
    assert!(archived_feature.join("qa-slices.yaml").is_file());
    assert!(!features_dir.join("billing-csv-export").exists());
    assert!(root.join(".maestro/archive/tasks/task-001").is_dir());
    assert!(root.join(".maestro/archive/tasks/task-002").is_dir());
    assert!(!tasks_dir.join("task-001").exists());

    // L6b: show falls through to the archive; list --all reads it.
    let show = stdout(
        maestro(&["feature", "show", "billing-csv-export"], root),
        &["feature", "show", "billing-csv-export"],
    );
    assert!(show.contains("status: shipped"));
    assert!(show.contains("tasks_total: 2"));
    let list_all = stdout(
        maestro(&["feature", "list", "--all"], root),
        &["feature", "list", "--all"],
    );
    assert!(list_all.contains("billing-csv-export"));

    // Idempotent: re-archiving is a no-op at exit 0.
    let again = maestro(&archive_args, root);
    let again_out = stdout(again, &archive_args);
    assert!(again_out.contains("already archived"));

    // Unarchive restores the feature dir + its archived children.
    let unarchive_args = ["feature", "unarchive", "billing-csv-export"];
    let restored = stdout(maestro(&unarchive_args, root), &unarchive_args);
    assert!(restored.contains("unarchived feature billing-csv-export"));
    assert!(restored.contains("task-001"));
    assert!(
        features_dir
            .join("billing-csv-export")
            .join("baseline.md")
            .is_file()
    );
    assert!(tasks_dir.join("task-001").is_dir());
    assert!(!archived_feature.exists());
}

/// The straggler-sweep case: a child a live task still references is skipped (not
/// blocked) while the feature + other children archive; clearing the reference
/// and re-running sweeps the straggler even though the feature dir already moved.
#[test]
fn feature_archive_skips_a_referenced_child_then_sweeps_it_on_rerun() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-straggler");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    stdout(
        maestro(&["feature", "new", "Billing CSV export"], root),
        &["feature", "new", "Billing CSV export"],
    );
    let set_args = [
        "feature",
        "set",
        "billing-csv-export",
        "--acceptance",
        "exports a valid csv",
        "--area",
        "billing",
    ];
    stdout(maestro(&set_args, root), &set_args);
    write_baseline(&root.join(".maestro/features"), "billing-csv-export");
    stdout(
        maestro(&["feature", "accept", "billing-csv-export"], root),
        &["feature", "accept", "billing-csv-export"],
    );
    stdout(
        maestro(&["feature", "start", "billing-csv-export"], root),
        &["feature", "start", "billing-csv-export"],
    );

    // Three live children; cancel abandons them so the feature is terminal with
    // terminal children (cheaper than the ship gate, exercises the same cascade).
    let tasks_dir = root.join(".maestro/tasks");
    write_task(&tasks_dir, "task-001", "billing-csv-export", "in_progress");
    write_task(&tasks_dir, "task-002", "billing-csv-export", "in_progress");
    write_task(&tasks_dir, "task-003", "billing-csv-export", "in_progress");
    let cancel_args = [
        "feature",
        "cancel",
        "billing-csv-export",
        "--reason",
        "scope dropped",
    ];
    stdout(maestro(&cancel_args, root), &cancel_args);

    // A live task (task-004) blocked by the terminal child task-002 entangles it.
    stdout(
        maestro(&["task", "create", "Holder"], root),
        &["task", "create", "Holder"],
    );
    let block_args = [
        "task", "block", "task-004", "--reason", "needs 2", "--by", "task-002",
    ];
    stdout(maestro(&block_args, root), &block_args);

    // Archive: task-002 is skipped (live-referenced), the feature + task-001/003 move.
    let archive_args = ["feature", "archive", "billing-csv-export"];
    let first = stdout(maestro(&archive_args, root), &archive_args);
    assert!(first.contains("skipped"));
    assert!(first.contains("task-002"));
    assert!(first.contains("task-004"));
    assert!(
        root.join(".maestro/archive/features/billing-csv-export/feature.yaml")
            .is_file()
    );
    assert!(root.join(".maestro/archive/tasks/task-001").is_dir());
    assert!(root.join(".maestro/archive/tasks/task-003").is_dir());
    // The straggler stays in the live tree.
    assert!(tasks_dir.join("task-002").is_dir());
    assert!(!root.join(".maestro/archive/tasks/task-002").exists());

    // Clear the reference, then re-run: task-002 sweeps even though the feature
    // dir already moved (the child sweep is unconditional).
    let unblock_args = ["task", "unblock", "task-004", "--blocker", "blk-001"];
    stdout(maestro(&unblock_args, root), &unblock_args);
    let swept = stdout(maestro(&archive_args, root), &archive_args);
    assert!(swept.contains("already archived"));
    assert!(swept.contains("task-002"));
    assert!(root.join(".maestro/archive/tasks/task-002").is_dir());
    assert!(!tasks_dir.join("task-002").exists());
}

/// `feature archive --shipped` archives every shipped feature (each cascading its
/// children) and leaves non-shipped features in the live tree.
#[test]
fn feature_archive_shipped_sweeps_only_shipped_features() {
    let temp_dir = TestTempDir::new("maestro-feature-archive-bulk");
    let root = temp_dir.path();
    init_git_marker(root);
    stdout(maestro(&["init", "--yes"], root), &["init", "--yes"]);

    // Two shipped features (each with a verified child) + one still in progress.
    ship_feature(root, "Alpha export", "alpha-export", "task-001");
    ship_feature(root, "Beta export", "beta-export", "task-002");

    stdout(
        maestro(&["feature", "new", "Gamma export"], root),
        &["feature", "new", "Gamma export"],
    );
    let set_gamma = [
        "feature",
        "set",
        "gamma-export",
        "--acceptance",
        "works",
        "--area",
        "core",
    ];
    stdout(maestro(&set_gamma, root), &set_gamma);
    write_baseline(&root.join(".maestro/features"), "gamma-export");
    stdout(
        maestro(&["feature", "accept", "gamma-export"], root),
        &["feature", "accept", "gamma-export"],
    );
    stdout(
        maestro(&["feature", "start", "gamma-export"], root),
        &["feature", "start", "gamma-export"],
    );

    // --shipped archives both shipped features and their children; gamma stays live.
    let bulk = ["feature", "archive", "--shipped"];
    let out = stdout(maestro(&bulk, root), &bulk);
    assert!(out.contains("archived feature alpha-export"));
    assert!(out.contains("archived feature beta-export"));
    assert!(out.contains("archived 2 of 2 shipped feature(s)"));

    let features_dir = root.join(".maestro/features");
    assert!(root.join(".maestro/archive/features/alpha-export").is_dir());
    assert!(root.join(".maestro/archive/features/beta-export").is_dir());
    assert!(root.join(".maestro/archive/tasks/task-001").is_dir());
    assert!(root.join(".maestro/archive/tasks/task-002").is_dir());
    // The in-progress feature is untouched and stays in the live tree.
    assert!(
        features_dir
            .join("gamma-export")
            .join("feature.yaml")
            .is_file()
    );
    assert!(!root.join(".maestro/archive/features/gamma-export").exists());

    // Idempotent: no shipped features remain live.
    let again = stdout(maestro(&bulk, root), &bulk);
    assert!(again.contains("no shipped features to archive"));

    // A feature id and --shipped are mutually exclusive.
    let both = ["feature", "archive", "alpha-export", "--shipped"];
    let err = assert_failure(maestro(&both, root), &both);
    assert!(err.contains("not both"));
}
