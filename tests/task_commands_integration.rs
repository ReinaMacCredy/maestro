pub mod card_support;
mod support;

use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::Command;

use card_support::{card_dir, card_doc, card_record_path, id_by_title, task_record};
use serde_yaml::{Mapping, Value};
use support::TestTempDir;

fn maestro(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn maestro_with_env(cwd: &Path, args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command.args(args).current_dir(cwd);
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn assert_success(output: &std::process::Output, args: &[&str]) {
    assert!(
        output.status.success(),
        "maestro {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &std::process::Output, args: &[&str]) {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("invariant: stdout should be UTF-8")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("invariant: stderr should be UTF-8")
}

/// A card-mode repo: `.maestro/cards/` exists so `store_mode` resolves to Cards,
/// plus the generic claims-only harness the task verbs read for verification gating.
fn setup_repo() -> TestTempDir {
    let temp = TestTempDir::new("maestro-task-cli");
    fs::create_dir_all(temp.path().join(".maestro/cards"))
        .expect("invariant: cards directory should be creatable");
    fs::create_dir_all(temp.path().join(".maestro/harness"))
        .expect("invariant: harness directory should be creatable");
    fs::write(
        temp.path().join(".maestro/harness/harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify: []\n",
            "claims_only_verification: true\n",
        ),
    )
    .expect("invariant: harness should be writable");
    temp
}

#[test]
fn task_add_start_done_is_low_ceremony_and_verifies_simple_completion() {
    let temp = setup_repo();
    let repo = temp.path();

    let add = maestro(repo, &["task", "add", "fix typo", "--id-only"]);
    assert_success(&add, &["task", "add", "fix typo", "--id-only"]);
    let id = stdout(&add).trim().to_string();
    assert!(
        id.starts_with("task-fix-typo-"),
        "simple task uses task id prefix: {id}"
    );

    let shown = stdout(&maestro(repo, &["task", "show", &id]));
    assert!(shown.contains("state: ready"), "{shown}");

    let start = maestro_with_env(
        repo,
        &["task", "start", &id],
        &[("MAESTRO_ACTOR", "codex#s1")],
    );
    assert_success(&start, &["task", "start", &id]);
    let mine = stdout(&maestro_with_env(
        repo,
        &["task", "list", "--mine"],
        &[("MAESTRO_ACTOR", "codex#s1")],
    ));
    assert!(mine.contains(&id), "{mine}");

    let done = maestro_with_env(
        repo,
        &["task", "done", &id, "--summary", "fixed typo"],
        &[("MAESTRO_ACTOR", "codex#s1")],
    );
    assert_success(&done, &["task", "done", &id]);

    let record = task_record(repo, &id);
    assert_eq!(record["state"], Value::String("verified".to_string()));
    assert_eq!(record["verification"]["claims_only"], Value::Bool(true));
    assert_eq!(
        record["verification"]["claim_checks"][0]["source"],
        Value::String("task done".to_string())
    );
}

#[test]
fn task_done_refuses_tasks_with_explicit_verification_gates() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "create",
                "Needs proof",
                "--check",
                "observable proof exists",
            ],
        ),
        &[
            "task",
            "create",
            "Needs proof",
            "--check",
            "observable proof exists",
        ],
    );
    let id = id_by_title(repo, "Needs proof");
    for args in [
        vec!["task", "explore", id.as_str()],
        vec!["task", "accept", id.as_str()],
        vec!["task", "claim", id.as_str()],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let done = maestro(repo, &["task", "done", &id]);
    assert_failure(&done, &["task", "done", &id]);
    let message = stderr(&done);
    assert!(
        message.contains("explicit verification gate") && message.contains("maestro task complete"),
        "task done must point gated work at the proof path: {message}"
    );

    let record = task_record(repo, &id);
    assert_eq!(record["state"], Value::String("in_progress".to_string()));
}

#[test]
fn create_explore_accept_claim_complete_flow_updates_task_record() {
    let temp = setup_repo();
    let repo = temp.path();

    // The task links to a real feature; `create --feature` now rejects a dangling ref.
    assert_success(
        &maestro(repo, &["feature", "new", "Billing CSV"]),
        &["feature", "new", "Billing CSV"],
    );

    let create = maestro(
        repo,
        &[
            "task",
            "create",
            "Add CSV export",
            "--feature",
            "billing-csv",
            "--lane",
            "normal",
            "--risk",
            "high",
        ],
    );
    assert_success(
        &create,
        &[
            "task",
            "create",
            "Add CSV export",
            "--feature",
            "billing-csv",
            "--lane",
            "normal",
            "--risk",
            "high",
        ],
    );
    assert!(stdout(&create).contains("created"));
    let id = id_by_title(repo, "Add CSV export");

    for args in [
        vec!["task", "explore", id.as_str()],
        vec!["task", "accept", id.as_str()],
        vec!["task", "claim", id.as_str()],
        vec![
            "task",
            "complete",
            id.as_str(),
            "--summary",
            "done",
            "--claim",
            "implemented CSV export",
            "--proof",
            "implemented CSV export",
        ],
    ] {
        let out = maestro(repo, &args);
        assert_success(&out, &args);
    }

    let doc = task_record(repo, &id);
    assert_eq!(doc["state"], Value::String("verified".to_string()));
    assert_eq!(doc["claimed_by"], Value::String("maestro".to_string()));
    assert_eq!(doc["acceptance_locked"], Value::Bool(true));
    assert!(
        !doc.as_mapping()
            .expect("invariant: task record should be a mapping")
            .contains_key(Value::String("feature_id".to_string())),
        "feature ownership rides card.parent, not a feature_id key"
    );
    // Feature ownership is the card's flat `parent`, not a directory path.
    assert_eq!(
        card_doc(repo, &id)["parent"],
        Value::String("billing-csv".to_string()),
        "feature-owned tasks carry the feature id in card.parent"
    );
    let history = doc["state_history"]
        .as_sequence()
        .expect("invariant: state_history should be an array");
    assert_eq!(history.len(), 6);
    assert!(
        !doc["updated_at"]
            .as_str()
            .expect("invariant: updated_at should be a string")
            .is_empty()
    );
}

#[test]
fn task_complete_accepts_repeated_claims_and_proofs() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Multi-claim closeout"]),
        &["task", "create", "Multi-claim closeout"],
    );
    let id = id_by_title(repo, "Multi-claim closeout");
    for args in [
        vec!["task", "explore", id.as_str()],
        vec![
            "task",
            "set",
            id.as_str(),
            "--check",
            "evidence is complete",
        ],
        vec!["task", "accept", id.as_str()],
        vec!["task", "claim", id.as_str()],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let args = &[
        "task",
        "complete",
        id.as_str(),
        "--summary",
        "closed with separate evidence lines",
        "--claim",
        "routing line appears exactly once",
        "--proof",
        "routing line appears exactly once",
        "--claim",
        "resource guard tests passed",
        "--proof",
        "resource guard tests passed",
    ];
    let complete = maestro(repo, args);
    assert_success(&complete, args);
    assert!(
        stdout(&complete).contains(&format!("verification passed for {id}")),
        "repeatable claims must still auto-verify: {}",
        stdout(&complete)
    );

    let doc = task_record(repo, &id);
    assert_eq!(doc["state"], Value::String("verified".to_string()));
    assert_eq!(
        doc["claims"],
        Value::Sequence(vec![
            Value::String("routing line appears exactly once".to_string()),
            Value::String("resource guard tests passed".to_string()),
        ])
    );
}

#[test]
fn claim_from_draft_is_blocked_with_the_explicit_ready_path() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Direct claim task"]),
        &["task", "create", "Direct claim task"],
    );
    let id = id_by_title(repo, "Direct claim task");
    assert_success(
        &maestro(repo, &["task", "set", &id, "--check", "direct claim check"]),
        &["task", "set", &id, "--check", "direct claim check"],
    );
    let claim = maestro(repo, &["task", "claim", &id]);
    assert_failure(&claim, &["task", "claim", &id]);
    let message = stderr(&claim);
    assert!(message.contains(&format!("blocked: task {id} is not ready to claim")));
    assert!(message.contains(&format!("next: maestro task explore {id}")));

    let task = task_record(repo, &id);
    assert_eq!(task["state"], Value::String("draft".to_string()));
    assert_eq!(task["acceptance_locked"], Value::Bool(false));
}

#[test]
fn supersede_rejects_a_nonexistent_target_and_leaves_the_task_untouched() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Original task"]),
        &["task", "create", "Original task"],
    );
    let id = id_by_title(repo, "Original task");

    let args = &[
        "task",
        "supersede",
        id.as_str(),
        "--by",
        "task-999",
        "--reason",
        "replaced",
    ];
    let supersede = maestro(repo, args);
    assert_failure(&supersede, args);
    assert!(
        stderr(&supersede).contains("supersede target"),
        "supersede should reject a dangling target: {}",
        stderr(&supersede)
    );
    let task = task_record(repo, &id);
    assert_eq!(task["state"], Value::String("draft".to_string()));
}

#[test]
fn supersede_records_an_existing_target() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Old"]),
        &["task", "create", "Old"],
    );
    assert_success(
        &maestro(repo, &["task", "create", "New"]),
        &["task", "create", "New"],
    );
    let old = id_by_title(repo, "Old");
    let new = id_by_title(repo, "New");

    let args = &[
        "task",
        "supersede",
        old.as_str(),
        "--by",
        new.as_str(),
        "--reason",
        "replaced by new",
    ];
    assert_success(&maestro(repo, args), args);
    let task = task_record(repo, &old);
    assert_eq!(task["state"], Value::String("superseded".to_string()));
}

#[test]
fn claim_from_exploring_fails_with_an_actionable_message() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Exploring task"]),
        &["task", "create", "Exploring task"],
    );
    let id = id_by_title(repo, "Exploring task");
    assert_success(
        &maestro(repo, &["task", "explore", &id]),
        &["task", "explore", &id],
    );

    let claim = maestro(repo, &["task", "claim", &id]);
    assert_failure(&claim, &["task", "claim", &id]);
    let message = stderr(&claim);
    assert!(
        message.contains("exploring") && message.contains("task accept"),
        "claiming an exploring task should name the state and point at accept: {message}"
    );
}

#[test]
fn blockers_terminal_transitions_and_claim_gate_behave_as_expected() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Task A"]),
        &["task", "create", "Task A"],
    );
    let a = id_by_title(repo, "Task A");
    assert_success(
        &maestro(repo, &["task", "set", &a, "--check", "task a check"]),
        &["task", "set", &a, "--check", "task a check"],
    );
    assert_success(
        &maestro(repo, &["task", "explore", &a]),
        &["task", "explore", &a],
    );
    assert_success(
        &maestro(repo, &["task", "accept", &a]),
        &["task", "accept", &a],
    );
    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "block",
                &a,
                "--reason",
                "waiting for dependency",
                "--by",
                "task-999",
            ],
        ),
        &[
            "task",
            "block",
            &a,
            "--reason",
            "waiting for dependency",
            "--by",
            "task-999",
        ],
    );
    let claim = maestro(repo, &["task", "claim", &a]);
    assert_failure(&claim, &["task", "claim", &a]);
    assert!(stderr(&claim).contains("unresolved blockers"));

    assert_success(
        &maestro(repo, &["task", "unblock", &a, "--blocker", "blk-001"]),
        &["task", "unblock", &a, "--blocker", "blk-001"],
    );
    assert_success(
        &maestro(repo, &["task", "claim", &a]),
        &["task", "claim", &a],
    );

    assert_success(
        &maestro(repo, &["task", "create", "Task B"]),
        &["task", "create", "Task B"],
    );
    let b = id_by_title(repo, "Task B");
    assert_success(
        &maestro(repo, &["task", "reject", &b, "--reason", "invalid"]),
        &["task", "reject", &b, "--reason", "invalid"],
    );
    assert_eq!(
        task_record(repo, &b)["state"],
        Value::String("rejected".to_string())
    );

    assert_success(
        &maestro(repo, &["task", "create", "Task C"]),
        &["task", "create", "Task C"],
    );
    let c = id_by_title(repo, "Task C");
    assert_success(
        &maestro(repo, &["task", "abandon", &c, "--reason", "not needed"]),
        &["task", "abandon", &c, "--reason", "not needed"],
    );
    assert_eq!(
        task_record(repo, &c)["state"],
        Value::String("abandoned".to_string())
    );

    assert_success(
        &maestro(repo, &["task", "create", "Task D"]),
        &["task", "create", "Task D"],
    );
    assert_success(
        &maestro(repo, &["task", "create", "Task E"]),
        &["task", "create", "Task E"],
    );
    let d = id_by_title(repo, "Task D");
    let e = id_by_title(repo, "Task E");
    assert_success(
        &maestro(
            repo,
            &["task", "supersede", &d, "--by", &e, "--reason", "replaced"],
        ),
        &["task", "supersede", &d, "--by", &e, "--reason", "replaced"],
    );
    let superseded = task_record(repo, &d);
    assert_eq!(superseded["state"], Value::String("superseded".to_string()));
    let history = superseded["state_history"]
        .as_sequence()
        .expect("invariant: state_history should be present");
    let last = history
        .last()
        .expect("invariant: superseded task should have a terminal history entry");
    assert_eq!(last["to"], Value::String(e.clone()));
}

#[test]
fn show_uses_maestro_current_task_when_no_id_is_provided() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Task A"]),
        &["task", "create", "Task A"],
    );
    let id = id_by_title(repo, "Task A");

    let show = maestro_with_env(repo, &["task", "show"], &[("MAESTRO_CURRENT_TASK", &id)]);
    assert_success(&show, &["task", "show"]);
    assert!(stdout(&show).contains(&format!("id: {id}")));

    let missing = maestro(repo, &["task", "show"]);
    assert_failure(&missing, &["task", "show"]);
    assert!(stderr(&missing).contains("MAESTRO_CURRENT_TASK"));
}

#[test]
fn show_treats_empty_current_task_env_as_unset() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Task A"]),
        &["task", "create", "Task A"],
    );

    // An empty MAESTRO_CURRENT_TASK must give the "id required" remedy, not fall
    // through to a confusing "invalid task id" / "task not found".
    let show = maestro_with_env(repo, &["task", "show"], &[("MAESTRO_CURRENT_TASK", "")]);
    assert_failure(&show, &["task", "show"]);
    let err = stderr(&show);
    assert!(err.contains("task id is required"), "got: {err}");
    assert!(!err.contains("invalid task id"), "got: {err}");
}

#[test]
fn task_lookup_does_not_resolve_a_partial_id() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "First task"]),
        &["task", "create", "First task"],
    );

    // Card lookup is exact (no prefix scan / ambiguity resolution): a partial id
    // like the shared `card` stem must not resolve to the lone card; it is simply
    // not found.
    let show = maestro(repo, &["task", "show", "card"]);
    assert_failure(&show, &["task", "show", "card"]);
    assert!(
        stderr(&show).contains("task not found"),
        "a partial id must not resolve: {}",
        stderr(&show)
    );
}

#[test]
fn task_lookup_rejects_path_traversal_ids() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "First task"]),
        &["task", "create", "First task"],
    );
    let id = id_by_title(repo, "First task");

    let traversal = format!("../{id}");
    let show = maestro(repo, &["task", "show", &traversal]);
    assert_failure(&show, &["task", "show", &traversal]);
    assert!(stderr(&show).contains("invalid task id"));

    let nested = format!("{id}/sub");
    let nested_show = maestro(repo, &["task", "show", &nested]);
    assert_failure(&nested_show, &["task", "show", &nested]);
    assert!(stderr(&nested_show).contains("invalid task id"));
}

#[test]
fn list_supports_basic_output_and_requested_filters() {
    let temp = setup_repo();
    let repo = temp.path();

    // The tasks link to real features; `create --feature` now rejects a dangling ref.
    assert_success(
        &maestro(repo, &["feature", "new", "Billing CSV"]),
        &["feature", "new", "Billing CSV"],
    );
    assert_success(
        &maestro(repo, &["feature", "new", "Other"]),
        &["feature", "new", "Other"],
    );

    assert_success(
        &maestro(
            repo,
            &["task", "create", "Task A", "--feature", "billing-csv"],
        ),
        &["task", "create", "Task A", "--feature", "billing-csv"],
    );
    assert_success(
        &maestro(
            repo,
            &["task", "create", "Task B", "--feature", "billing-csv"],
        ),
        &["task", "create", "Task B", "--feature", "billing-csv"],
    );
    assert_success(
        &maestro(repo, &["task", "create", "Task C", "--feature", "other"]),
        &["task", "create", "Task C", "--feature", "other"],
    );
    let a = id_by_title(repo, "Task A");
    let b = id_by_title(repo, "Task B");
    let c = id_by_title(repo, "Task C");

    for args in [
        vec!["task", "explore", a.as_str()],
        vec!["task", "accept", a.as_str()],
        vec!["task", "explore", b.as_str()],
        vec!["task", "accept", b.as_str()],
        vec![
            "task",
            "block",
            b.as_str(),
            "--reason",
            "wait for a",
            "--by",
            a.as_str(),
        ],
    ] {
        let out = maestro(repo, &args);
        assert_success(&out, &args);
    }

    let all = maestro(repo, &["task", "list"]);
    assert_success(&all, &["task", "list"]);
    let all_out = stdout(&all);
    assert!(untabify(&all_out).contains("ID\tSTATE\tNEXT\tTITLE"));
    assert!(all_out.contains("inspect any: maestro task show <id>"));
    assert!(all_out.contains(&a));
    assert!(all_out.contains(&b));
    assert!(all_out.contains(&c));

    let ready = maestro(repo, &["task", "list", "--ready"]);
    assert_success(&ready, &["task", "list", "--ready"]);
    let ready_out = stdout(&ready);
    assert!(ready_out.contains(&a));
    assert!(!ready_out.contains(&b));
    assert!(!ready_out.contains(&c));

    let blocked = maestro(repo, &["task", "list", "--blocked"]);
    assert_success(&blocked, &["task", "list", "--blocked"]);
    let blocked_out = stdout(&blocked);
    assert!(blocked_out.contains(&b));
    assert!(!blocked_out.contains(&a));

    let blocked_by = maestro(repo, &["task", "list", "--blocked-by", &a]);
    assert_success(&blocked_by, &["task", "list", "--blocked-by", &a]);
    assert!(stdout(&blocked_by).contains(&b));

    let blocks = maestro(repo, &["task", "list", "--blocks", &b]);
    assert_success(&blocks, &["task", "list", "--blocks", &b]);
    assert!(stdout(&blocks).contains(&a));

    let feature = maestro(repo, &["task", "list", "--feature", "billing-csv"]);
    assert_success(&feature, &["task", "list", "--feature", "billing-csv"]);
    let feature_out = stdout(&feature);
    assert!(feature_out.contains(&a));
    assert!(feature_out.contains(&b));
    assert!(!feature_out.contains(&c));

    assert_success(
        &maestro(repo, &["task", "claim", &a]),
        &["task", "claim", &a],
    );
    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "update",
                &a,
                "--summary",
                "progress noted",
                "--claim",
                "partial implementation",
            ],
        ),
        &[
            "task",
            "update",
            &a,
            "--summary",
            "progress noted",
            "--claim",
            "partial implementation",
        ],
    );
    let watch = maestro(repo, &["task", "list", "--watch", "--interval", "0"]);
    assert_success(&watch, &["task", "list", "--watch", "--interval", "0"]);
    let watch_out = stdout(&watch);
    assert!(watch_out.contains("scheduler: 1 agents active"));
    // The watch groups by the feature's human title (resolved from the registry),
    // falling back to the raw id only for dangling refs — now that the feature exists.
    assert!(watch_out.contains("Billing CSV"));
    assert!(watch_out.contains("~ Task A"));
    assert!(watch_out.contains("in-progress (maestro)"));
    assert!(watch_out.contains("! Task B"));
    assert!(watch_out.contains(&format!("blocked by {a}")));

    let task_watch = maestro(repo, &["task", "watch", &a, "--interval", "0"]);
    assert_success(&task_watch, &["task", "watch", &a, "--interval", "0"]);
    let task_watch_out = stdout(&task_watch);
    assert!(task_watch_out.contains("~ Task A"));
    assert!(!task_watch_out.contains("Task B"));

    let watch_feature = maestro(
        repo,
        &[
            "task",
            "list",
            "--watch",
            "--feature",
            "billing-csv",
            "--interval",
            "0",
        ],
    );
    assert_success(
        &watch_feature,
        &[
            "task",
            "list",
            "--watch",
            "--feature",
            "billing-csv",
            "--interval",
            "0",
        ],
    );
    let watch_feature_out = stdout(&watch_feature);
    assert!(watch_feature_out.contains("~ Task A"));
    assert!(watch_feature_out.contains("! Task B"));
    assert!(!watch_feature_out.contains("Task C"));

    let snapshot = maestro(repo, &["watch", "snapshot"]);
    assert_success(&snapshot, &["watch", "snapshot"]);
    let snapshot_out = stdout(&snapshot);
    // `watch snapshot` renders the card-model board: a per-feature header with
    // the done ratio and live counts, then workable rows keyed by state glyph.
    assert!(snapshot_out.contains(
        "Billing CSV: 0/2 done (0%) | ready 0 | active 1 | needs_verification 0 | blocked 1"
    ));
    assert!(snapshot_out.contains("\u{25d0} active"));
    assert!(snapshot_out.contains("Task A"));
    assert!(snapshot_out.contains("\u{00b7} blocked"));
    assert!(snapshot_out.contains("Task B"));
    // The snapshot path never animates: with Task A active it renders the static
    // half-circle (asserted above) and none of the live-only Braille frames.
    for frame in [
        '\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}',
        '\u{2827}', '\u{2807}', '\u{280F}',
    ] {
        assert!(
            !snapshot_out.contains(frame),
            "watch snapshot must not render the live spinner frame {frame:?}:\n{snapshot_out}"
        );
    }

    // `watch snapshot <known-id>` focuses on exactly that feature.
    let focus = maestro(repo, &["watch", "snapshot", "billing-csv"]);
    assert_success(&focus, &["watch", "snapshot", "billing-csv"]);
    let focus_out = stdout(&focus);
    assert!(focus_out.contains("Billing CSV: 0/2 done (0%)"));
    assert!(
        !focus_out.contains("Other"),
        "focus must exclude other features:\n{focus_out}"
    );

    // Focusing the other feature renders only its header and rows.
    let focus_other = maestro(repo, &["watch", "snapshot", "other"]);
    assert_success(&focus_other, &["watch", "snapshot", "other"]);
    let focus_other_out = stdout(&focus_other);
    assert!(focus_other_out.contains("Other: 0/1 done (0%)"));
    assert!(focus_other_out.contains("Task C"));
    assert!(
        !focus_other_out.contains("Billing CSV"),
        "focus must exclude other features:\n{focus_other_out}"
    );

    // An unknown feature id errors with a re-list hint, never empty output.
    let unknown = maestro(repo, &["watch", "snapshot", "does-not-exist"]);
    assert!(!unknown.status.success(), "unknown focus id should error");
    let unknown_err = String::from_utf8_lossy(&unknown.stderr);
    assert!(
        unknown_err.contains("no feature 'does-not-exist'")
            && unknown_err.contains("maestro list --type feature"),
        "unknown id must point back to the feature list:\n{unknown_err}"
    );

    // Bare `maestro watch` over a pipe (non-terminal) prints one frame and exits 0.
    let bare = maestro(repo, &["watch"]);
    assert_success(&bare, &["watch"]);
    assert!(stdout(&bare).contains("Billing CSV: 0/2 done (0%)"));

    // The unknown-id error must also surface through the bare (live) path, where
    // it propagates out of the render closure rather than a direct call. The
    // command returns (does not hang) with the same re-list hint.
    let bare_unknown = maestro(repo, &["watch", "does-not-exist"]);
    assert!(
        !bare_unknown.status.success(),
        "bare unknown focus id should error"
    );
    let bare_unknown_err = String::from_utf8_lossy(&bare_unknown.stderr);
    assert!(
        bare_unknown_err.contains("no feature 'does-not-exist'")
            && bare_unknown_err.contains("maestro list --type feature"),
        "bare unknown id must point back to the feature list:\n{bare_unknown_err}"
    );
}

#[test]
fn list_hides_terminal_tasks_until_all_is_passed() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Live task"]),
        &["task", "create", "Live task"],
    );
    assert_success(
        &maestro(repo, &["task", "create", "Done task"]),
        &["task", "create", "Done task"],
    );
    let live = id_by_title(repo, "Live task");
    let done = id_by_title(repo, "Done task");
    assert_success(
        &maestro(repo, &["task", "abandon", &done, "--reason", "not needed"]),
        &["task", "abandon", &done, "--reason", "not needed"],
    );

    // Default list keeps the abandoned (terminal) task off the active set and
    // reports the count behind a parser-skippable hint.
    let default = maestro(repo, &["task", "list"]);
    assert_success(&default, &["task", "list"]);
    let default_out = stdout(&default);
    assert!(default_out.contains(&live));
    assert!(!default_out.contains(&done));
    assert!(default_out.contains("# 1 terminal task(s) hidden; use --all to include"));

    // `--all` includes the terminal task and drops the hint.
    let all = maestro(repo, &["task", "list", "--all"]);
    assert_success(&all, &["task", "list", "--all"]);
    let all_out = stdout(&all);
    assert!(all_out.contains(&live));
    assert!(all_out.contains(&done));
    assert!(!all_out.contains("terminal task(s) hidden"));
}

#[test]
fn set_on_a_settled_task_refuses_the_link_change_before_writing_checks() {
    let temp = setup_repo();
    let repo = temp.path();

    // The task is created (draft, no checks) then abandoned: settled, but never
    // accepted so its acceptance stays unlocked — the state where set_checks
    // would otherwise write before set_feature's settled guard fires.
    assert_success(
        &maestro(repo, &["task", "create", "Dead end"]),
        &["task", "create", "Dead end"],
    );
    let id = id_by_title(repo, "Dead end");
    assert_success(
        &maestro(repo, &["task", "abandon", &id, "--reason", "scrapped"]),
        &["task", "abandon", &id, "--reason", "scrapped"],
    );

    // A combined `--check --feature` set must fail fast on the settled task.
    let args = &[
        "task",
        "set",
        id.as_str(),
        "--check",
        "must not persist",
        "--feature",
        "billing",
    ];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    assert!(stderr(&set).contains("settled history"));

    // The refused set wrote no check: inline acceptance carries nothing from it.
    let raw = fs::read_to_string(card_record_path(repo, &id))
        .expect("invariant: card record should be readable");
    assert!(
        !raw.contains("must not persist"),
        "a refused set must not persist its checks: {raw}"
    );
}

#[test]
fn set_check_rejects_an_empty_value_so_it_cannot_satisfy_the_acceptance_gate() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Empty-check probe"]),
        &["task", "create", "Empty-check probe"],
    );
    let id = id_by_title(repo, "Empty-check probe");

    // A `--check ''` whose value is empty must be refused: stored verbatim it
    // would have list length 1 and so satisfy the standalone >=1-check
    // acceptance gate while carrying no contract.
    let args = &["task", "set", id.as_str(), "--check", ""];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    assert!(stderr(&set).contains("check cannot be empty"));

    // The refused set wrote nothing, so the standalone-checks gate still
    // refuses accept — the empty check never satisfies it.
    assert_success(
        &maestro(repo, &["task", "explore", &id]),
        &["task", "explore", &id],
    );
    let accept = maestro(repo, &["task", "accept", &id]);
    assert_failure(&accept, &["task", "accept", &id]);
    assert!(stderr(&accept).contains("has no checks"));
}

#[test]
fn accept_on_a_terminal_task_reports_the_terminal_state_not_a_dead_end_add_check_remedy() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Doomed standalone"]),
        &["task", "create", "Doomed standalone"],
    );
    let id = id_by_title(repo, "Doomed standalone");
    assert_success(
        &maestro(repo, &["task", "reject", &id, "--reason", "out of scope"]),
        &["task", "reject", &id, "--reason", "out of scope"],
    );

    // The task is terminal (rejected) and has no checks. accept must surface the
    // real, actionable blocker -- a terminal task cannot transition -- not the
    // add-check remedy, which is a dead end: adding a check still cannot move a
    // terminal task to ready, so the state gate must be evaluated before the
    // content gate.
    let accept = maestro(repo, &["task", "accept", &id]);
    assert_failure(&accept, &["task", "accept", &id]);
    let message = stderr(&accept);
    assert!(
        message.contains("terminal state"),
        "expected the terminal-state error, got: {message}"
    );
    assert!(
        !message.contains("has no checks"),
        "accept on a terminal task must not hand the dead-end add-check remedy: {message}"
    );
}

#[test]
fn set_check_rejects_a_terminal_task_whose_checks_are_settled_history() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Doomed"]),
        &["task", "create", "Doomed"],
    );
    let id = id_by_title(repo, "Doomed");
    assert_success(
        &maestro(repo, &["task", "reject", &id, "--reason", "out of scope"]),
        &["task", "reject", &id, "--reason", "out of scope"],
    );

    // A rejected task is terminal but never accepted (acceptance_locked is false),
    // so it slips past the lock guard. Editing its checks must still be refused --
    // they are settled history.
    let args = &["task", "set", id.as_str(), "--check", "too late"];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    let message = stderr(&set);
    assert!(
        message.contains("settled history"),
        "expected the terminal settled-history guard, got: {message}"
    );
}

#[test]
fn set_check_on_a_previously_accepted_terminal_task_reports_settled_history_not_the_lock() {
    let temp = setup_repo();
    let repo = temp.path();

    // Drive the task to accepted (acceptance_locked = true), then reject it: it is
    // now terminal AND acceptance-locked. Editing its checks must report the
    // terminal settled-history reason, not "acceptance is locked ... after accept",
    // which would falsely imply the block is tied to a still-active accepted
    // contract. The terminal guard must be evaluated before the lock guard.
    assert_success(
        &maestro(repo, &["task", "create", "Was accepted"]),
        &["task", "create", "Was accepted"],
    );
    let id = id_by_title(repo, "Was accepted");
    for args in [
        vec!["task", "explore", id.as_str()],
        vec!["task", "set", id.as_str(), "--check", "build passes"],
        vec!["task", "accept", id.as_str()],
        vec!["task", "reject", id.as_str(), "--reason", "out of scope"],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let args = &["task", "set", id.as_str(), "--check", "too late"];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    let message = stderr(&set);
    assert!(
        message.contains("settled history"),
        "expected the terminal settled-history guard, got: {message}"
    );
    assert!(
        !message.contains("acceptance is locked"),
        "a terminal task must not report the acceptance lock (the terminal reason is the accurate one): {message}"
    );
}

#[test]
fn set_check_honors_an_on_disk_acceptance_lock_even_when_the_task_snapshot_is_stale() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Race probe"]),
        &["task", "create", "Race probe"],
    );
    let id = id_by_title(repo, "Race probe");

    // Simulate a partially-written inline contract lock: the task stays draft
    // (`acceptance_locked = false`), but the nested acceptance record (under the
    // card's folded `extra`) is frozen.
    let card_path = card_record_path(repo, &id);
    let mut doc: Value = serde_yaml::from_str(
        &fs::read_to_string(&card_path).expect("invariant: card record should be readable"),
    )
    .expect("invariant: card record should parse");
    let mut acceptance = Mapping::new();
    acceptance.insert(
        Value::String("locked_by".to_string()),
        Value::String("maestro".to_string()),
    );
    acceptance.insert(
        Value::String("locked_at".to_string()),
        Value::String("now".to_string()),
    );
    doc.as_mapping_mut()
        .expect("invariant: card.yaml should be a mapping")
        .get_mut(Value::String("extra".to_string()))
        .expect("invariant: a task card carries a folded `extra` record")
        .as_mapping_mut()
        .expect("invariant: card extra should be a mapping")
        .insert(
            Value::String("acceptance".to_string()),
            Value::Mapping(acceptance),
        );
    fs::write(
        &card_path,
        serde_yaml::to_string(&doc).expect("invariant: card yaml should serialize"),
    )
    .expect("invariant: card.yaml should be writable");

    let args = &[
        "task",
        "set",
        id.as_str(),
        "--check",
        "must not clobber the frozen contract",
    ];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    assert!(
        stderr(&set).contains("acceptance is locked"),
        "set_checks must refuse to overwrite a contract already frozen on disk: {}",
        stderr(&set)
    );

    // The refused set left the frozen contract intact (no clobber).
    let raw = fs::read_to_string(&card_path).expect("invariant: card.yaml should be readable");
    assert!(
        raw.contains("locked_by: maestro") && !raw.contains("must not clobber"),
        "the frozen contract must survive the refused set: {raw}"
    );
}

#[test]
fn complete_on_a_pre_claim_task_points_at_claim_not_a_dead_end() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Ship it"]),
        &["task", "create", "Ship it"],
    );
    let id = id_by_title(repo, "Ship it");
    for args in [
        vec!["task", "explore", id.as_str()],
        vec!["task", "set", id.as_str(), "--check", "build passes"],
        vec!["task", "accept", id.as_str()],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    // The task is ready but never claimed. Completing it must point at `claim` (the
    // get-to-in_progress verb), not the generic "cannot transition" dead end.
    let complete_args = &[
        "task",
        "complete",
        id.as_str(),
        "--summary",
        "did it",
        "--claim",
        "build passes",
    ];
    let complete = maestro(repo, complete_args);
    assert_failure(&complete, complete_args);
    let message = stderr(&complete);
    assert!(
        message.contains(&format!("maestro task claim {id}")),
        "expected the claim remedy, got: {message}"
    );
    assert!(
        !message.contains("cannot transition"),
        "expected the actionable claim remedy, not the generic catch-all: {message}"
    );
}

#[test]
fn task_create_rejects_an_empty_or_whitespace_title() {
    let temp = setup_repo();
    let repo = temp.path();

    // Sibling create verbs (feature new / decision new) reject a blank title;
    // task create must too, instead of writing a task with a meaningless label.
    for title in ["", "   "] {
        let create = maestro(repo, &["task", "create", title]);
        assert_failure(&create, &["task", "create", title]);
        assert!(
            stderr(&create).contains("title must not be empty"),
            "unexpected error for {title:?}: {}",
            stderr(&create)
        );
    }
}

#[test]
fn task_block_rejects_an_empty_or_whitespace_reason() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "blocked"]),
        &["task", "create", "blocked"],
    );
    let id = id_by_title(repo, "blocked");
    // The sibling claim/check/complete verbs all reject a blank value; block must
    // too, rather than persist a dangling-colon blank-reason blocker.
    for reason in ["", "   "] {
        let block = maestro(
            repo,
            &["task", "block", &id, "--reason", reason, "--by", "task-002"],
        );
        assert_failure(&block, &["task", "block", "--reason", reason]);
        assert!(
            stderr(&block).contains("`--reason` must not be empty"),
            "unexpected error for {reason:?}: {}",
            stderr(&block)
        );
    }
}

#[test]
fn task_reject_abandon_supersede_reject_an_empty_or_whitespace_reason() {
    let temp = setup_repo();
    let repo = temp.path();

    // `block --reason` already guards blank; reject/abandon/supersede are its
    // missed peers -- terminal, audited transitions where a blank reason would
    // leave a permanent, un-amendable record with no explanation. The guard fires
    // before any state change, so the draft tasks survive both iterations.
    for args in [
        vec!["task", "create", "reject target"],
        vec!["task", "create", "abandon target"],
        vec!["task", "create", "supersede target"],
        vec!["task", "create", "supersede by"],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }
    let reject_id = id_by_title(repo, "reject target");
    let abandon_id = id_by_title(repo, "abandon target");
    let supersede_id = id_by_title(repo, "supersede target");
    let supersede_by = id_by_title(repo, "supersede by");

    for reason in ["", "   "] {
        let reject = maestro(repo, &["task", "reject", &reject_id, "--reason", reason]);
        assert_failure(&reject, &["task", "reject", "--reason", reason]);
        assert!(
            stderr(&reject).contains("needs an audited reason")
                && stderr(&reject).contains("reason: --reason is empty"),
            "reject {reason:?}: {}",
            stderr(&reject)
        );

        let abandon = maestro(repo, &["task", "abandon", &abandon_id, "--reason", reason]);
        assert_failure(&abandon, &["task", "abandon", "--reason", reason]);
        assert!(
            stderr(&abandon).contains("needs an audited reason")
                && stderr(&abandon).contains("reason: --reason is empty"),
            "abandon {reason:?}: {}",
            stderr(&abandon)
        );

        let supersede = maestro(
            repo,
            &[
                "task",
                "supersede",
                &supersede_id,
                "--by",
                &supersede_by,
                "--reason",
                reason,
            ],
        );
        assert_failure(&supersede, &["task", "supersede", "--reason", reason]);
        assert!(
            stderr(&supersede).contains("needs an audited reason")
                && stderr(&supersede).contains("reason: --reason is empty"),
            "supersede {reason:?}: {}",
            stderr(&supersede)
        );
    }
}

#[test]
fn task_update_with_no_fields_shows_worked_examples_like_task_set() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "needs an update"]),
        &["task", "create", "needs an update"],
    );
    let id = id_by_title(repo, "needs an update");

    // `task set` teaches the exact invocation on its no-args error; `task update`,
    // its sibling, must too rather than dead-end with a bare one-liner.
    let update = maestro(repo, &["task", "update", &id]);
    assert_failure(&update, &["task", "update", &id]);
    let message = stderr(&update);
    assert!(
        message.contains(&format!("maestro task update {id} --summary")),
        "expected a worked --summary example: {message}"
    );
    assert!(
        message.contains(&format!("maestro task update {id} --claim")),
        "expected a worked --claim example: {message}"
    );
}

#[test]
fn event_create_rejects_an_empty_or_whitespace_claim() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "proofed"]),
        &["task", "create", "proofed"],
    );
    let id = id_by_title(repo, "proofed");
    // `task complete --claim ""`/`task update --claim ""` are both refused; the
    // event verb that records the same proof artifact must not accept a blank one.
    for claim in ["", "   "] {
        let event = maestro(
            repo,
            &["event", "create", "--task-id", &id, "--claim", claim],
        );
        assert_failure(&event, &["event", "create", "--claim", claim]);
        assert!(
            stderr(&event).contains("`--claim` must not be empty"),
            "unexpected error for {claim:?}: {}",
            stderr(&event)
        );
    }
}

#[test]
fn task_update_rejects_an_empty_claim_so_no_blank_proof_is_recorded() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Empty-claim probe"]),
        &["task", "create", "Empty-claim probe"],
    );
    let id = id_by_title(repo, "Empty-claim probe");
    assert_success(
        &maestro(repo, &["task", "set", &id, "--check", "builds"]),
        &["task", "set", &id, "--check", "builds"],
    );
    assert_success(
        &maestro(repo, &["task", "explore", &id]),
        &["task", "explore", &id],
    );
    assert_success(
        &maestro(repo, &["task", "accept", &id]),
        &["task", "accept", &id],
    );
    assert_success(
        &maestro(repo, &["task", "claim", &id]),
        &["task", "claim", &id],
    );

    let history_len = |repo: &Path| {
        task_record(repo, &id)["state_history"]
            .as_sequence()
            .expect("invariant: state_history should be an array")
            .len()
    };
    let before = history_len(repo);

    // A `--claim ''` is meaningless: a claim is the proof a later `task verify`
    // checks against, so a blank one must be refused and nothing recorded.
    let args = &["task", "update", id.as_str(), "--claim", ""];
    let update = maestro(repo, args);
    assert_failure(&update, args);
    assert!(stderr(&update).contains("`--claim` must not be empty"));

    // The refused update appended no history entry.
    assert_eq!(history_len(repo), before);
}

#[test]
fn task_update_and_verify_refuse_terminal_tasks_without_mutation() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Verified terminal probe"]),
        &["task", "create", "Verified terminal probe"],
    );
    let verified_id = id_by_title(repo, "Verified terminal probe");
    for args in [
        vec![
            "task",
            "set",
            verified_id.as_str(),
            "--check",
            "done proof exists",
        ],
        vec!["task", "explore", verified_id.as_str()],
        vec!["task", "accept", verified_id.as_str()],
        vec!["task", "claim", verified_id.as_str()],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }
    let complete_args = &[
        "task",
        "complete",
        verified_id.as_str(),
        "--summary",
        "done",
        "--claim",
        "done proof exists",
        "--proof",
        "done proof exists",
    ];
    assert_success(&maestro(repo, complete_args), complete_args);

    assert_success(
        &maestro(repo, &["task", "create", "Rejected terminal probe"]),
        &["task", "create", "Rejected terminal probe"],
    );
    let rejected_id = id_by_title(repo, "Rejected terminal probe");
    assert_success(
        &maestro(
            repo,
            &[
                "task",
                "reject",
                rejected_id.as_str(),
                "--reason",
                "not worth doing",
            ],
        ),
        &[
            "task",
            "reject",
            rejected_id.as_str(),
            "--reason",
            "not worth doing",
        ],
    );

    for (id, state) in [
        (verified_id.as_str(), "verified"),
        (rejected_id.as_str(), "rejected"),
    ] {
        let before = fs::read_to_string(card_record_path(repo, id))
            .expect("invariant: task card should be readable before refused commands");

        let update_args = &[
            "task",
            "update",
            id,
            "--summary",
            "late summary",
            "--claim",
            "late claim",
        ];
        let update = maestro(repo, update_args);
        assert_failure(&update, update_args);
        let update_err = stderr(&update);
        assert!(
            update_err.contains(&format!("cannot update task {id}")),
            "{update_err}"
        );
        assert!(
            update_err.contains(&format!("done (state: {state})")),
            "{update_err}"
        );
        assert_eq!(
            fs::read_to_string(card_record_path(repo, id))
                .expect("invariant: task card should remain readable"),
            before
        );

        let verify_args = &["task", "verify", id];
        let verify = maestro(repo, verify_args);
        assert_failure(&verify, verify_args);
        let verify_err = stderr(&verify);
        assert!(
            verify_err.contains(&format!("cannot verify task {id}")),
            "{verify_err}"
        );
        assert!(
            verify_err.contains(&format!("state is {state}")),
            "{verify_err}"
        );
        assert!(
            verify_err.contains("expected needs_verification"),
            "{verify_err}"
        );
        assert_eq!(
            fs::read_to_string(card_record_path(repo, id))
                .expect("invariant: task card should remain readable"),
            before
        );
    }
}

#[test]
fn task_block_is_refused_on_a_done_task_so_no_open_blocker_is_baked_in() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Abandoned probe"]),
        &["task", "create", "Abandoned probe"],
    );
    let id = id_by_title(repo, "Abandoned probe");
    assert_success(
        &maestro(repo, &["task", "abandon", &id, "--reason", "scrapped"]),
        &["task", "abandon", &id, "--reason", "scrapped"],
    );

    // Block alone must not bypass the terminal guard the 5 sibling verbs honor:
    // a finished task cannot take an open blocker (e.g. "abandoned / blocked").
    let args = &[
        "task",
        "block",
        id.as_str(),
        "--reason",
        "needs dep",
        "--by",
        "task-002",
    ];
    let block = maestro(repo, args);
    assert_failure(&block, args);
    assert!(stderr(&block).contains(&format!("cannot block {id} — done")));

    // No blocker was written onto the done task.
    let doc = task_record(repo, &id);
    let blockers = doc["blockers"].as_sequence();
    assert!(
        blockers.map(|b| b.is_empty()).unwrap_or(true),
        "a refused block must not persist a blocker: {doc:?}"
    );
}

#[test]
fn task_supersede_by_itself_is_refused_so_no_self_reference_is_recorded() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Self-supersede probe"]),
        &["task", "create", "Self-supersede probe"],
    );
    let id = id_by_title(repo, "Self-supersede probe");

    // `--by` naming the task itself would record a corrupt superseded_by: self.
    let args = &[
        "task",
        "supersede",
        id.as_str(),
        "--by",
        id.as_str(),
        "--reason",
        "oops",
    ];
    let supersede = maestro(repo, args);
    assert_failure(&supersede, args);
    assert!(stderr(&supersede).contains(&format!("cannot supersede {id} by itself")));

    // The task stays in its prior state with no superseded_by ref.
    let doc = task_record(repo, &id);
    assert_eq!(doc["state"], Value::String("draft".to_string()));
    assert!(doc.get("superseded_by").is_none() || doc["superseded_by"].is_null());
}

#[test]
fn task_unblock_is_refused_on_an_already_resolved_blocker() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Double-unblock probe"]),
        &["task", "create", "Double-unblock probe"],
    );
    let id = id_by_title(repo, "Double-unblock probe");
    assert_success(
        &maestro(
            repo,
            &[
                "task", "block", &id, "--reason", "waiting", "--by", "task-999",
            ],
        ),
        &[
            "task", "block", &id, "--reason", "waiting", "--by", "task-999",
        ],
    );
    assert_success(
        &maestro(repo, &["task", "unblock", &id, "--blocker", "blk-001"]),
        &["task", "unblock", &id, "--blocker", "blk-001"],
    );

    // Capture the resolved state after the first (legitimate) unblock.
    let after_first = task_record(repo, &id);
    let resolved_at = after_first["blockers"][0]["resolved_at"]
        .as_str()
        .expect("invariant: first unblock should set resolved_at")
        .to_string();
    let history_len = after_first["state_history"]
        .as_sequence()
        .expect("invariant: state_history should be an array")
        .len();

    // A second unblock of the same blocker must be refused, not silently
    // overwrite the original resolved_at or append a duplicate history entry.
    let args = &["task", "unblock", id.as_str(), "--blocker", "blk-001"];
    let second = maestro(repo, args);
    assert_failure(&second, args);
    assert!(stderr(&second).contains("blocker blk-001 is already resolved"));

    let after_second = task_record(repo, &id);
    assert_eq!(
        after_second["blockers"][0]["resolved_at"].as_str(),
        Some(resolved_at.as_str()),
        "the original resolved_at must be preserved"
    );
    assert_eq!(
        after_second["state_history"]
            .as_sequence()
            .expect("invariant: state_history should be an array")
            .len(),
        history_len,
        "a refused unblock must not append history"
    );
}

#[test]
fn read_verbs_do_not_scaffold_the_cards_dir_but_create_still_does() {
    // R30: a pure inspect (`task list`/`task doctor`) must leave disk untouched,
    // matching feature/decision/query; only a mutator (`create`) may scaffold.
    // Bespoke setup WITHOUT `.maestro/cards` so the scaffold is observable; a
    // harness yaml is enough for the repo root to be discovered.
    let temp = TestTempDir::new("maestro-task-cli-scaffold");
    let repo = temp.path();
    fs::create_dir_all(repo.join(".maestro/harness"))
        .expect("invariant: harness directory should be creatable");
    fs::write(
        repo.join(".maestro/harness/harness.yml"),
        concat!(
            "schema_version: maestro.harness.v1\n",
            "stack:\n",
            "  kind: generic\n",
            "  detected_by: []\n",
            "  verify: []\n",
            "claims_only_verification: true\n",
        ),
    )
    .expect("invariant: harness should be writable");

    let cards_dir = repo.join(".maestro/cards");
    assert!(!cards_dir.exists(), "setup must start without a cards dir");

    let list = maestro(repo, &["task", "list"]);
    assert_success(&list, &["task", "list"]);
    assert!(stdout(&list).contains("no tasks found"));
    assert!(
        !cards_dir.exists(),
        "`task list` must not scaffold .maestro/cards"
    );

    let doctor = maestro(repo, &["task", "doctor"]);
    assert_success(&doctor, &["task", "doctor"]);
    // The surviving doctor-ok behavior from the retired sequential-minter test:
    // a clean repo reports ok (and the read verb still does not scaffold).
    assert!(
        stdout(&doctor).contains("task doctor: ok"),
        "{}",
        stdout(&doctor)
    );
    assert!(
        !cards_dir.exists(),
        "`task doctor` must not scaffold .maestro/cards"
    );

    let create = maestro(repo, &["task", "create", "first task"]);
    assert_success(&create, &["task", "create"]);
    assert!(
        cards_dir.exists(),
        "`task create` must still create .maestro/cards on first write"
    );
}

#[test]
fn forward_verbs_on_a_verified_task_point_at_a_follow_up_not_a_bare_dead_end() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Done deal"]),
        &["task", "create", "Done deal"],
    );
    let id = id_by_title(repo, "Done deal");
    for args in [
        vec!["task", "set", id.as_str(), "--check", "build passes"],
        vec!["task", "explore", id.as_str()],
        vec!["task", "accept", id.as_str()],
        vec!["task", "claim", id.as_str()],
        vec![
            "task",
            "complete",
            id.as_str(),
            "--summary",
            "did it",
            "--claim",
            "build passes",
            "--proof",
            "build passes",
        ],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    // Verified is a settled success terminus; a forward verb (claim/complete) means
    // new work, so the error must point at a follow-up task, not the bare
    // "cannot transition" catch-all dead end.
    for verb in [
        vec!["task", "claim", id.as_str()],
        vec![
            "task",
            "complete",
            id.as_str(),
            "--summary",
            "more",
            "--claim",
            "x",
        ],
    ] {
        let out = maestro(repo, &verb);
        assert_failure(&out, &verb);
        let message = stderr(&out);
        assert!(
            message.contains("maestro task create"),
            "expected the follow-up remedy for {verb:?}: {message}"
        );
        assert!(
            !message.contains("cannot transition"),
            "must not be the bare catch-all for {verb:?}: {message}"
        );
    }
}

#[test]
fn task_show_rejects_a_symlinked_card_dir() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "First task"]),
        &["task", "create", "First task"],
    );
    let id = id_by_title(repo, "First task");

    // Move the card dir out of the store and replace it with a symlink. A single
    // card load must refuse to follow the symlinked dir (the single-load mirror of
    // the bulk-scan symlink skip), so `task show` reports not-found rather than
    // reading a record from outside the store.
    let card_dir = card_dir(repo, &id);
    let external = repo.join("external-card");
    fs::rename(&card_dir, &external).expect("invariant: card dir should be movable");
    unix_fs::symlink(&external, &card_dir).expect("invariant: symlink should be creatable");

    let show = maestro(repo, &["task", "show", &id]);
    assert_failure(&show, &["task", "show", &id]);
    assert!(
        stderr(&show).contains("task not found"),
        "a symlinked card dir must not resolve: {}",
        stderr(&show)
    );
}

#[test]
fn task_archive_and_unarchive_redirect_to_the_feature_cascade() {
    let temp = setup_repo();
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "Archive me"]),
        &["task", "create", "Archive me"],
    );
    let id = id_by_title(repo, "Archive me");

    // Per-task archive was retired (SPEC E4: archive is a feature-level cascade).
    // `task archive`/`unarchive` on an existing card must emit the guiding redirect
    // (close the task / archive the whole feature), never the legacy "task not
    // found" dead-end -- the card still exists.
    for verb in ["archive", "unarchive"] {
        let out = maestro(repo, &["task", verb, &id]);
        assert_failure(&out, &["task", verb, &id]);
        let message = stderr(&out);
        assert!(
            message.contains("per-task archive removed"),
            "`task {verb}` must redirect: {message}"
        );
        assert!(
            message.contains(&format!("maestro card close {id}"))
                && message.contains("maestro card archive <feature>"),
            "`task {verb}` must point at close + the feature cascade: {message}"
        );
        assert!(
            !message.contains("task not found"),
            "`task {verb}` must not dead-end on an existing card: {message}"
        );
    }
}

#[test]
fn task_verb_on_a_below_floor_payload_points_at_migrate_v2() {
    let temp = setup_repo();
    let repo = temp.path();

    // A valid card envelope whose folded `extra` carries the legacy
    // `maestro.task.v1` stamp AND a v1 shape (no `acceptance_locked` /
    // `verification`, which v2 requires). The schema gate must classify the
    // stamp BEFORE the typed parse: the agent gets the explicit migrate route
    // from the task schema pack, never a raw YAML parse error.
    let dir = repo.join(".maestro/cards/task-legacy");
    fs::create_dir_all(&dir).expect("invariant: legacy card dir should be creatable");
    fs::write(
        dir.join("card.yaml"),
        concat!(
            "schema_version: maestro.card.v1\n",
            "id: task-legacy\n",
            "type: task\n",
            "title: Legacy payload\n",
            "status: ready\n",
            "created_at: \"1\"\n",
            "updated_at: \"1\"\n",
            "extra:\n",
            "  schema_version: maestro.task.v1\n",
            "  slug: legacy-payload\n",
        ),
    )
    .expect("invariant: legacy card should be writable");

    let explore = maestro(repo, &["task", "explore", "task-legacy"]);
    assert_failure(&explore, &["task", "explore", "task-legacy"]);
    let message = stderr(&explore);
    assert!(message.contains("schema mismatch"), "{message}");
    assert!(message.contains("maestro.task.v1"), "{message}");
    assert!(
        message.contains("fix: run maestro migrate-v2"),
        "the refusal must carry the pack's migrate route: {message}"
    );
    assert!(
        !message.contains("failed to parse"),
        "the gate must fire before the typed parse: {message}"
    );
}

#[test]
fn unknown_fields_survive_a_typed_verb_and_surface_in_doctor() {
    let temp = setup_repo();
    let repo = temp.path();

    // A current-version task card carrying two fields this binary does not
    // declare: one top-level (`future_top`) and one inside the extra payload
    // (`future_extra`). D6.6: a typed verb's save must round-trip both instead
    // of silently dropping them, and `doctor` must name them.
    let dir = repo.join(".maestro/cards/task-future");
    fs::create_dir_all(&dir).expect("invariant: card dir should be creatable");
    fs::write(
        dir.join("card.yaml"),
        concat!(
            "schema_version: maestro.card.v1\n",
            "id: task-future\n",
            "type: task\n",
            "title: Future payload\n",
            "status: draft\n",
            "created_at: \"1\"\n",
            "updated_at: \"1\"\n",
            "future_top: kept\n",
            "extra:\n",
            "  schema_version: maestro.task.v2\n",
            "  state: draft\n",
            "  acceptance_locked: false\n",
            "  verification: {}\n",
            "  future_extra: from-a-newer-maestro\n",
        ),
    )
    .expect("invariant: card should be writable");

    let doctor = maestro(repo, &["doctor"]);
    assert_success(&doctor, &["doctor"]);
    let report = stdout(&doctor);
    assert!(
        report.contains("future_top") && report.contains("extra.future_extra"),
        "doctor must name the unknown fields: {report}"
    );

    assert_success(
        &maestro(repo, &["task", "explore", "task-future"]),
        &["task", "explore", "task-future"],
    );
    let saved = fs::read_to_string(dir.join("card.yaml"))
        .expect("invariant: card should be readable after the verb");
    assert!(
        saved.contains("future_top: kept"),
        "the unknown top-level key must survive the typed save: {saved}"
    );
    assert!(
        saved.contains("future_extra: from-a-newer-maestro"),
        "the unknown extra key must survive the typed save: {saved}"
    );
    assert!(
        saved.contains("state: exploring"),
        "the verb itself must have taken effect: {saved}"
    );
}

/// Collapse aligned-table padding (runs of 2+ spaces) back to tabs so cell
/// assertions stay width-independent.
fn untabify(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            line.split("  ")
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn set_verify_command_persists_then_clears_on_a_live_task() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Slice with narrow falsifier"]),
        &["task", "create", "Slice with narrow falsifier"],
    );
    let id = id_by_title(repo, "Slice with narrow falsifier");

    let set = maestro(
        repo,
        &[
            "task",
            "set",
            id.as_str(),
            "--verify-command",
            "cargo test --test resources_version_guard",
        ],
    );
    assert_success(
        &set,
        &["task", "set", id.as_str(), "--verify-command", "..."],
    );
    assert!(
        stdout(&set).contains("not stack.verify"),
        "set should explain the falsifier replaces stack.verify: {}",
        stdout(&set)
    );
    let task = task_record(repo, &id);
    assert_eq!(
        task["verify_command"],
        Value::String("cargo test --test resources_version_guard".to_string()),
        "the per-task verify command must persist into the task record"
    );

    let clear = maestro(
        repo,
        &["task", "set", id.as_str(), "--clear-verify-command"],
    );
    assert_success(
        &clear,
        &["task", "set", id.as_str(), "--clear-verify-command"],
    );
    let raw = fs::read_to_string(card_record_path(repo, &id))
        .expect("invariant: the card record should be readable");
    assert!(
        !raw.contains("verify_command"),
        "a cleared verify command must be omitted from the record (skip_serializing_if None): {raw}"
    );
}

#[test]
fn set_verify_command_refuses_on_a_settled_task() {
    let temp = setup_repo();
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Settled slice"]),
        &["task", "create", "Settled slice"],
    );
    let id = id_by_title(repo, "Settled slice");
    assert_success(
        &maestro(
            repo,
            &["task", "abandon", id.as_str(), "--reason", "scrapped"],
        ),
        &["task", "abandon", id.as_str(), "--reason", "scrapped"],
    );

    let args = &["task", "set", id.as_str(), "--verify-command", "cargo test"];
    let set = maestro(repo, args);
    assert_failure(&set, args);
    assert!(
        stderr(&set).contains("settled history"),
        "a settled task must refuse a verify-command change: {}",
        stderr(&set)
    );
}
