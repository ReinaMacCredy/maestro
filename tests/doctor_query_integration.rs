mod card_support;
mod support;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use card_support::{card_record_path, id_by_title};
use serde_yaml::Value as YamlValue;
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

fn setup_repo(prefix: &str) -> TestTempDir {
    let temp = TestTempDir::new(prefix);
    fs::create_dir(temp.path().join(".git")).expect("invariant: .git marker should be creatable");
    assert_success(
        &maestro(temp.path(), &["init", "--yes"]),
        &["init", "--yes"],
    );
    assert_success(
        &maestro(temp.path(), &["harness", "set", "--claims-only"]),
        &["harness", "set", "--claims-only"],
    );
    temp
}

fn run_success(repo: &Path, args: &[&str]) -> String {
    let output = maestro(repo, args);
    assert_success(&output, args);
    stdout(&output)
}

/// Drive a feature + one verified task with proof, returning the minted task
/// card id. Card-mode `task create` mints an opaque `card-XXXXXX` id, so the
/// id is recovered by the unique title and the later verbs address it directly.
fn create_verified_task_with_proof(repo: &Path) -> String {
    assert_success(
        &maestro(repo, &["feature", "new", "Billing CSV export"]),
        &["feature", "new", "Billing CSV export"],
    );
    let create = [
        "task",
        "create",
        "Add CSV export",
        "--feature",
        "billing-csv-export",
    ];
    assert_success(&maestro(repo, &create), &create);
    let task_id = id_by_title(repo, "Add CSV export");
    for args in [
        vec!["task", "explore", &task_id],
        vec!["task", "accept", &task_id],
        vec!["task", "claim", &task_id],
        vec![
            "task",
            "complete",
            &task_id,
            "--summary",
            "done",
            "--claim",
            "implemented CSV export",
            "--proof",
            "implemented CSV export",
        ],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let run_dir = repo.join(".maestro/runs/run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        "{\"kind\":\"UserPromptSubmit\",\"message\":\"actually, check the blocker graph\"}\n",
    )
    .expect("invariant: events should be writable");
    task_id
}

#[test]
fn doctor_outside_repo_reports_not_initialized_instead_of_discovery_error() {
    let temp = TestTempDir::new("maestro-doctor-rootless");

    let doctor = maestro(temp.path(), &["doctor"]);

    assert_failure(&doctor, &["doctor"]);
    let err = stderr(&doctor);
    assert!(
        err.contains("is not initialized for Maestro"),
        "rootless doctor should explain the missing setup:\n{err}"
    );
    assert!(
        err.contains("maestro init --yes"),
        "rootless doctor should include the repair command:\n{err}"
    );
    assert!(
        !err.contains("failed to discover repository root"),
        "rootless doctor should not leak raw repo discovery:\n{err}"
    );
    assert!(
        !temp.path().join(".maestro").exists(),
        "doctor must not scaffold .maestro"
    );
}

#[test]
fn doctor_reports_ok_for_initialized_phase_three_artifacts() {
    let temp = setup_repo("maestro-doctor-ok");
    let repo = temp.path();
    create_verified_task_with_proof(repo);

    let doctor = maestro_with_env(repo, &["doctor"], &[("MAESTRO_AGENT", "codex")]);
    assert_success(&doctor, &["doctor"]);
    let out = stdout(&doctor);

    assert!(out.contains("check harness: ok"));
    // The harness ok line prints a stable schema descriptor like its sibling checks
    // (counts/states), not the machine's absolute harness.yml path.
    assert!(
        out.contains("check harness: ok (schema "),
        "harness ok should print a schema descriptor, not a path: {out}"
    );
    assert!(
        !out.contains(&repo.display().to_string()),
        "doctor must not leak the absolute repo path on a healthy repo: {out}"
    );
    assert!(out.contains("check features: ok"));
    assert!(out.contains("check backlog: ok"));
    assert!(out.contains("check task-blockers: ok"));
    assert!(out.contains("doctor: ok"));
    assert!(out.contains("next: maestro install --agent codex"));
    assert!(out.contains("then: maestro status"));

    let claude_doctor = maestro_with_env(repo, &["doctor"], &[("MAESTRO_AGENT", "claude")]);
    assert_success(&claude_doctor, &["doctor"]);
    let claude_out = stdout(&claude_doctor);
    assert!(
        claude_out.contains("next: maestro install --agent claude"),
        "{claude_out}"
    );
}

#[test]
fn doctor_words_missing_resources_uniformly_with_the_init_merge_repair() {
    // R31: a deleted harness.yml must read "is missing" (like the dir checks),
    // carry the working `init --merge` repair, and never leak the internal
    // "failed to read" io phrasing -- one vocabulary across the missing-resource class.
    //
    // Card-mode change: feature cards live in the flat store (`check features`
    // always reads cards) and the backlog has no file of its own -- items live as
    // idea cards (D7). So harness.yml is the last member of the missing-resource
    // class, and the uniform repair hint fires exactly once.
    let temp = setup_repo("maestro-doctor-missing");
    let repo = temp.path();
    fs::remove_file(repo.join(".maestro/harness/harness.yml"))
        .expect("invariant: scaffolded harness.yml should exist");

    let doctor = maestro(repo, &["doctor"]);
    assert_failure(&doctor, &["doctor"]);
    let err = stderr(&doctor);

    assert!(
        !err.contains("failed to read"),
        "missing resources must not leak the io phrasing:\n{err}"
    );
    assert!(
        err.contains("harness.yml"),
        "doctor should name harness.yml as missing:\n{err}"
    );
    assert_eq!(
        err.matches("is missing; run `maestro init --merge` to repair")
            .count(),
        1,
        "every missing resource should carry the uniform repair hint:\n{err}"
    );
}

#[test]
fn doctor_collects_a_corrupt_task_error_without_aborting_the_rest_of_the_report() {
    // S2-3: a single malformed task card used to abort the whole report via `?`,
    // suppressing every other check. It must now be collected like the other
    // corrupt-artifact diagnostics, and surface its full serde cause.
    //
    // Card-mode shape: the card.yaml envelope itself stays parseable so the rest of
    // the scan keeps running (an unparseable card.yaml drops the `check features`
    // line); the folded `extra` task record is type-mismatched (`state` should be a
    // string, here a sequence), which yields the `invalid type` serde cause.
    let temp = setup_repo("maestro-doctor-corrupt-task");
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["task", "create", "probe"]),
        &["task", "create", "probe"],
    );

    let task_id = id_by_title(repo, "probe");
    let card_yaml = card_record_path(repo, &task_id);
    fs::write(
        &card_yaml,
        format!(
            "schema_version: maestro.card.v1\nid: {task_id}\ntype: task\ntitle: probe\nstatus: draft\ncreated_at: \"1\"\nupdated_at: \"1\"\nextra:\n  state:\n    - oops\n"
        ),
    )
    .expect("invariant: card record should be writable");

    let doctor = maestro(repo, &["doctor"]);
    assert_failure(&doctor, &["doctor"]);
    // The report was not aborted: the other checks still ran and printed.
    assert!(
        stdout(&doctor).contains("check harness: ok"),
        "a corrupt task must not suppress the other doctor checks:\n{}",
        stdout(&doctor)
    );
    // The corrupt-task error carries its full parse cause, not a bare "failed to parse".
    assert!(
        stderr(&doctor).contains("invalid type"),
        "the corrupt-task error should carry its serde cause:\n{}",
        stderr(&doctor)
    );
}

#[test]
fn doctor_reports_an_unparseable_envelope_once_and_keeps_every_check() {
    // The card-aware doctor walks the store once; a card.yaml that fails to
    // parse has an unknowable type, so it must surface as ONE central error --
    // not once per per-type scan -- and the typed check lines (features,
    // backlog, decisions, task-blockers) must still print from the loadable
    // cards.
    let temp = setup_repo("maestro-doctor-corrupt-envelope");
    let repo = temp.path();
    assert_success(
        &maestro(repo, &["feature", "new", "Billing CSV"]),
        &["feature", "new", "Billing CSV"],
    );

    let broken = repo.join(".maestro/cards/broken");
    fs::create_dir_all(&broken).expect("invariant: card dir should be creatable");
    fs::write(broken.join("card.yaml"), "type: [").expect("invariant: card.yaml writable");

    let doctor = maestro(repo, &["doctor"]);
    assert_failure(&doctor, &["doctor"]);
    let out = stdout(&doctor);
    for check in [
        "check features: ok (1 feature(s))",
        "check backlog: ok",
        "check decisions: ok",
        "check task-blockers: ok",
    ] {
        assert!(
            out.contains(check),
            "{check} must survive a corrupt envelope:\n{out}"
        );
    }
    let err = stderr(&doctor);
    assert_eq!(
        err.matches("failed to parse").count(),
        1,
        "the corrupt envelope is reported exactly once:\n{err}"
    );
}

#[test]
fn doctor_and_task_doctor_fail_on_bad_blocker_graph() {
    let temp = setup_repo("maestro-doctor-bad-blockers");
    let repo = temp.path();

    assert_success(
        &maestro(repo, &["task", "create", "Self blocked task"]),
        &["task", "create", "Self blocked task"],
    );
    let task_id = id_by_title(repo, "Self blocked task");
    for args in [
        vec!["task", "set", &task_id, "--check", "self check"],
        vec!["task", "explore", &task_id],
        vec!["task", "accept", &task_id],
        vec![
            "task",
            "block",
            &task_id,
            "--reason",
            "waiting for itself",
            "--by",
            &task_id,
        ],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let doctor = maestro(repo, &["doctor"]);
    assert_failure(&doctor, &["doctor"]);
    assert!(stderr(&doctor).contains("self-blocking blocker"));

    let task_doctor = maestro(repo, &["task", "doctor"]);
    assert_failure(&task_doctor, &["task", "doctor"]);
    assert!(stderr(&task_doctor).contains("self-blocking blocker"));
    // The report names the remedy so doctor does not just say "wrong" without "what now".
    assert!(stderr(&task_doctor).contains("maestro task unblock"));
    assert!(stderr(&task_doctor).contains("can instead be archived"));
}

#[test]
fn doctor_fails_on_blocker_cycles() {
    let temp = setup_repo("maestro-doctor-blocker-cycle");
    let repo = temp.path();

    for args in [
        vec!["task", "create", "Task A"],
        vec!["task", "create", "Task B"],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }
    let task_a = id_by_title(repo, "Task A");
    let task_b = id_by_title(repo, "Task B");
    for args in [
        vec!["task", "set", &task_a, "--check", "task a check"],
        vec!["task", "set", &task_b, "--check", "task b check"],
        vec!["task", "explore", &task_a],
        vec!["task", "accept", &task_a],
        vec!["task", "explore", &task_b],
        vec!["task", "accept", &task_b],
        vec![
            "task",
            "block",
            &task_a,
            "--reason",
            "wait for B",
            "--by",
            &task_b,
        ],
        vec![
            "task",
            "block",
            &task_b,
            "--reason",
            "wait for A",
            "--by",
            &task_a,
        ],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }

    let doctor = maestro(repo, &["doctor"]);
    assert_failure(&doctor, &["doctor"]);
    assert!(stderr(&doctor).contains("blocker cycle detected"));

    let task_doctor = maestro(repo, &["task", "doctor"]);
    assert_failure(&task_doctor, &["task", "doctor"]);
    assert!(stderr(&task_doctor).contains("blocker cycle detected"));
}

#[test]
fn doctor_and_task_doctor_flag_a_dangling_decision_blocker() {
    let temp = setup_repo("maestro-doctor-dangling-decision");
    let repo = temp.path();

    // Decisions mint opaque card ids now; recover the real id by title.
    run_success(repo, &["decision", "new", "Adopt CSV schema"]);
    let decision_id = id_by_title(repo, "Adopt CSV schema");
    for args in [
        vec!["task", "create", "Valid decision blocker"],
        vec!["task", "create", "Dangling decision blocker"],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }
    let valid_task = id_by_title(repo, "Valid decision blocker");
    let dangling_task = id_by_title(repo, "Dangling decision blocker");
    let block_valid = [
        "task",
        "block",
        &valid_task,
        "--reason",
        "needs ADR",
        "--by",
        &decision_id,
    ];
    assert_success(&maestro(repo, &block_valid), &block_valid);

    // A resolvable decision blocker does not trip the doctor.
    assert_success(&maestro(repo, &["task", "doctor"]), &["task", "doctor"]);

    let block_dangling = [
        "task",
        "block",
        &dangling_task,
        "--reason",
        "needs ADR",
        "--by",
        "decision-999",
    ];
    assert_success(&maestro(repo, &block_dangling), &block_dangling);

    let task_doctor = maestro(repo, &["task", "doctor"]);
    assert_failure(&task_doctor, &["task", "doctor"]);
    assert!(
        stderr(&task_doctor).contains("referencing missing decision decision-999"),
        "{}",
        stderr(&task_doctor)
    );

    let doctor = maestro(repo, &["doctor"]);
    assert_failure(&doctor, &["doctor"]);
    assert!(stderr(&doctor).contains("referencing missing decision"));
}

#[test]
fn doctor_flags_a_deleted_installed_mirror() {
    let temp = setup_repo("maestro-doctor-install-integrity");
    let repo = temp.path();
    let home = TestTempDir::new("maestro-doctor-install-home");
    let home_var = home.path().to_string_lossy().into_owned();
    let envs = [("HOME", home_var.as_str())];

    // Init'd but no agent installed: no install check, doctor stays ok.
    let pre = maestro_with_env(repo, &["doctor"], &envs);
    assert_success(&pre, &["doctor"]);
    assert!(!stdout(&pre).contains("check install"));

    assert_success(
        &maestro_with_env(repo, &["install", "--agent", "claude"], &envs),
        &["install", "--agent", "claude"],
    );
    let installed = maestro_with_env(repo, &["doctor"], &envs);
    assert_success(&installed, &["doctor"]);
    assert!(stdout(&installed).contains("check install: ok"));

    // Deleting an owned mirror is caught.
    fs::remove_file(repo.join("CLAUDE.md"))
        .expect("invariant: installed CLAUDE.md should be removable");
    let broken = maestro_with_env(repo, &["doctor"], &envs);
    assert_failure(&broken, &["doctor"]);
    assert!(
        stderr(&broken).contains("mirror is missing or broken"),
        "{}",
        stderr(&broken)
    );

    // Re-installing repairs it.
    assert_success(
        &maestro_with_env(repo, &["install", "--agent", "claude"], &envs),
        &["install", "--agent", "claude"],
    );
    assert_success(&maestro_with_env(repo, &["doctor"], &envs), &["doctor"]);
}

#[test]
fn doctor_counts_real_decisions_and_skips_symlinked_entries() {
    let temp = setup_repo("maestro-doctor-decision-symlink");
    let repo = temp.path();

    run_success(repo, &["decision", "new", "Use the domain decision reader"]);

    // Doctor enumerates structured decisions (decision-typed cards) plus frozen
    // legacy markdown through the domain reader. A symlinked legacy decision-*.md
    // must not inflate the legacy count. Card-mode `init` scaffolds no decisions
    // dir, so the test plants the legacy markdown root itself.
    let decisions_dir = repo.join(".maestro/decisions");
    fs::create_dir_all(&decisions_dir).expect("invariant: decisions dir should be creatable");
    let real = decisions_dir.join("decision-007-legacy.md");
    fs::write(&real, "# decision-007: Legacy\n\n## Status\nAccepted\n")
        .expect("invariant: legacy decision should be writable");
    unix_fs::symlink(&real, decisions_dir.join("decision-008-symlinked.md"))
        .expect("invariant: symlink should be creatable in test");

    let doctor = maestro(repo, &["doctor"]);
    let out = stdout(&doctor);
    assert!(
        out.contains("check decisions: ok (1 structured decision(s), 1 legacy file(s))"),
        "doctor should count the structured decision and only the real legacy file:\n{out}"
    );
}

#[test]
fn doctor_warns_on_dangling_structured_decision_refs_without_failing() {
    let temp = setup_repo("maestro-doctor-dangling-decision-refs");
    let repo = temp.path();

    run_success(repo, &["feature", "new", "Decision Ref Integrity"]);
    run_success(
        repo,
        &[
            "decision",
            "new",
            "Choose storage",
            "--feature",
            "decision-ref-integrity",
        ],
    );
    let decision_id = id_by_title(repo, "Choose storage");
    run_success(
        repo,
        &[
            "decision",
            "lock",
            &decision_id,
            "--decision",
            "Use feature-local stores",
            "--rejected",
            "global only: less local",
        ],
    );
    // Locking stamps a `<id> locked` pointer into the feature card's notes.md. In
    // card mode the decision IS a card -- an entry in the feature container's
    // decisions.yaml -- so dangling that note ref means deleting the container
    // file; the feature card's notes.md then references a missing decision and
    // the dangling-ref gate warns (without failing).
    fs::remove_file(repo.join(".maestro/cards/decision-ref-integrity/decisions.yaml"))
        .expect("invariant: decision container file should be removable");

    let doctor = maestro(repo, &["doctor"]);
    assert_success(&doctor, &["doctor"]);
    let out = stdout(&doctor);
    assert!(out.contains("warning:"), "{out}");
    assert!(
        out.contains(&format!(
            "notes.md references missing decision {decision_id}"
        )),
        "{out}"
    );
    assert!(out.contains("fix:"), "{out}");
}

#[test]
fn doctor_resolves_a_structured_ref_against_a_non_canonical_stored_id() {
    // The note ref normalizes to decision-001 form; the resolvable-id set must
    // normalize stored ids the same way, or an externally non-canonical id like
    // decision-1 yields a false "missing decision" warning. Regression for the
    // store-branch normalize asymmetry in resolvable_decision_ids.
    //
    // Card mode never mints a `decision-NNN`-form id (creation is `card-<hash>`), so
    // the only way to drive this exact normalize asymmetry against the card store is
    // to plant both halves: a decision-typed card whose stored id is the
    // non-canonical `decision-1`, and a feature card whose notes.md carries the
    // canonical `decision-001 locked` pointer.
    let temp = setup_repo("maestro-doctor-noncanonical-id");
    let repo = temp.path();

    run_success(repo, &["feature", "new", "Normalize Check"]);

    let decision_dir = repo.join(".maestro/cards/decision-1");
    fs::create_dir_all(&decision_dir).expect("invariant: decision card dir should be creatable");
    fs::write(
        decision_dir.join("card.yaml"),
        "schema_version: maestro.card.v1\nid: decision-1\ntype: decision\ntitle: Pick storage\nstatus: locked\ncreated_at: \"1\"\nupdated_at: \"1\"\nextra:\n  id: decision-1\n  title: Pick storage\n  status: locked\n  created_at: \"1\"\n  updated_at: \"1\"\n",
    )
    .expect("invariant: non-canonical decision card should be writable");
    fs::write(
        repo.join(".maestro/cards/normalize-check/notes.md"),
        "# Normalize Check\n\n2026-06-08  decision-001 locked -- Pick storage\n",
    )
    .expect("invariant: feature notes.md should be writable");

    let doctor = maestro(repo, &["doctor"]);
    assert_success(&doctor, &["doctor"]);
    let out = stdout(&doctor);
    assert!(
        !out.contains("references missing decision decision-001"),
        "a non-canonical stored id should still satisfy the normalized note ref: {out}"
    );
}

#[test]
fn doctor_warns_on_dangling_supersedes_but_ignores_prose_mentions() {
    let temp = setup_repo("maestro-doctor-dangling-supersedes");
    let repo = temp.path();

    run_success(repo, &["feature", "new", "Supersede Integrity"]);
    run_success(repo, &["decision", "new", "Global decision"]);
    // The decision is a card -- the sole entry in the root decisions.yaml; its
    // folded record carries the supersedes list. Append a dangling
    // `supersedes: decision-999` under the entry's `extra` (the last block, so
    // the deeper indent nests cleanly) to drive the dangling-supersedes gate.
    let decision_card = repo.join(".maestro/cards/decisions.yaml");
    let mut yaml =
        fs::read_to_string(&decision_card).expect("invariant: decision card should be readable");
    yaml.push_str("    supersedes:\n    - decision-999\n");
    fs::write(&decision_card, yaml).expect("invariant: decision card should be writable");
    // notes.md is the file the dangling-ref scan actually reads. Seed the feature
    // card's notes.md with both a structured dangling ref (which MUST be flagged --
    // proving the file is scanned) and a bare prose mention on a non-structured line
    // (which must be ignored by the `locked`/`superseded` line gate). Writing
    // decision-998 to an unscanned file like spec.md would make the prose assertion
    // vacuous.
    fs::write(
        repo.join(".maestro/cards/supersede-integrity/notes.md"),
        "2026-06-08  decision-997 locked -- never recorded\n\
         Background: decision-998 came up in discussion but is prose, not a reference.\n",
    )
    .expect("invariant: notes.md should be writable");

    let doctor = maestro(repo, &["doctor"]);
    assert_success(&doctor, &["doctor"]);
    let out = stdout(&doctor);
    assert!(
        out.contains("superseding missing decision decision-999"),
        "{out}"
    );
    // The structured dangling ref is caught, so notes.md is genuinely scanned ...
    assert!(
        out.contains("references missing decision decision-997"),
        "{out}"
    );
    // ... yet the prose mention of decision-998 on a non-structured line is not.
    assert!(!out.contains("decision-998"), "{out}");
}

#[test]
fn doctor_warns_on_a_dual_home_card_without_failing() {
    let temp = setup_repo("maestro-doctor-dual-home");
    let repo = temp.path();

    run_success(repo, &["feature", "new", "Dual Home"]);
    run_success(
        repo,
        &["decision", "new", "Pick storage", "--feature", "dual-home"],
    );
    let decision_id = id_by_title(repo, "Pick storage");
    // Plant the flat leaf copy a crash-interrupted container fold leaves
    // beside the entry.
    let flat_dir = repo.join(".maestro/cards").join(&decision_id);
    fs::create_dir_all(&flat_dir).expect("invariant: flat card dir should be creatable");
    fs::write(
        flat_dir.join("card.yaml"),
        format!(
            "schema_version: maestro.card.v1\nid: {decision_id}\ntype: decision\ntitle: Pick storage\nstatus: open\ncreated_at: \"1\"\nupdated_at: \"1\"\n"
        ),
    )
    .expect("invariant: flat card copy should be writable");

    let doctor = maestro(repo, &["doctor"]);
    assert_success(&doctor, &["doctor"]);
    let out = stdout(&doctor);
    assert!(
        out.contains(&format!("card {decision_id} exists at 2 homes")),
        "{out}"
    );
    assert!(out.contains("doctor: ok"), "{out}");
}

#[test]
fn doctor_warns_on_recordless_live_task_and_feature_dirs_without_failing() {
    let temp = setup_repo("maestro-doctor-recordless-dirs");
    let repo = temp.path();
    let task_dir = repo.join(".maestro/tasks/task-999-aborted-create");
    let feature_dir = repo.join(".maestro/features/ghost-feature");
    fs::create_dir_all(&task_dir).expect("invariant: task dir should be creatable");
    fs::create_dir_all(&feature_dir).expect("invariant: feature dir should be creatable");

    let doctor = maestro(repo, &["doctor"]);
    assert_success(&doctor, &["doctor"]);
    let out = stdout(&doctor);

    assert!(
        out.contains("warning: .maestro/tasks/task-999-aborted-create has no task.yaml"),
        "{out}"
    );
    assert!(
        out.contains("remove it: rm -r .maestro/tasks/task-999-aborted-create"),
        "{out}"
    );
    assert!(
        out.contains("warning: .maestro/features/ghost-feature has no feature.yaml"),
        "{out}"
    );
    assert!(
        out.contains("remove it: rm -r .maestro/features/ghost-feature"),
        "{out}"
    );
    assert!(out.contains("doctor: ok"), "{out}");
}

#[test]
fn query_backlog_reports_empty_state_when_no_idea_cards_exist() {
    // R29/R22: the backlog has no file of its own (D7) -- a repo with no idea
    // cards must read as an empty backlog, not leak a raw ENOENT + absolute path.
    let temp = setup_repo("maestro-query-backlog-absent");
    let repo = temp.path();

    let backlog = run_success(repo, &["query", "backlog"]);
    assert!(
        backlog.contains("no backlog items found"),
        "empty backlog should report the empty state, got:\n{backlog}"
    );
    assert!(
        !backlog.contains("failed to read") && !backlog.contains("os error"),
        "empty backlog must not leak a raw io error:\n{backlog}"
    );
}

#[test]
fn query_views_scan_current_artifacts_without_writing_cache_files() {
    let temp = setup_repo("maestro-query-views");
    let repo = temp.path();
    let task_id = create_verified_task_with_proof(repo);

    run_success(repo, &["decision", "new", "Use computed query views"]);
    let decision_id = id_by_title(repo, "Use computed query views");
    // The backlog now reads `idea` cards (the harness backlog folded into the flat
    // store), not `backlog.yaml#items`, so seed a backlog item as an idea card.
    run_success(
        repo,
        &["create", "-t", "idea", "Add query regression coverage"],
    );

    let before = maestro_files(repo);

    let decisions = run_success(repo, &["query", "decisions"]);
    assert!(untabify(&decisions).contains(&format!("{decision_id}\topen\tglobal")));
    assert!(decisions.contains("Use computed query views"));

    let backlog = run_success(repo, &["query", "backlog"]);
    assert!(backlog.contains("Add query regression coverage"));

    let matrix = run_success(repo, &["query", "matrix"]);
    assert!(matrix.contains("billing-csv-export"));
    assert!(matrix.contains(&task_id));
    assert!(matrix.contains("verified"));
    assert!(matrix.contains("accepted"));

    let friction = run_success(repo, &["query", "friction"]);
    assert!(friction.contains("FRICTION"));
    assert!(friction.contains("events: 2"));
    assert!(friction.contains("corrections: 1"));

    let proof = run_success(repo, &["query", "proof", &task_id]);
    assert!(proof.contains(&format!("proof {task_id}: accepted")));
    assert!(proof.contains("task.yaml#verification"));

    let after = maestro_files(repo);
    assert_eq!(before, after);
    assert!(!repo.join(".maestro/cache").exists());
    assert!(!repo.join(".maestro/tmp").exists());

    // The task record is folded under the card's `extra`; changing its acceptance
    // contract re-derives a contract hash that no longer matches the recorded
    // verification, so the matrix proof column reads `stale`.
    let card_path = card_record_path(repo, &task_id);
    let mut card: YamlValue = serde_yaml::from_str(
        &fs::read_to_string(&card_path).expect("invariant: card.yaml should be readable"),
    )
    .expect("invariant: card.yaml should parse");
    card["extra"]["acceptance"]["checks"] = YamlValue::Sequence(vec![YamlValue::String(
        "changed query matrix proof binding".to_string(),
    )]);
    fs::write(
        &card_path,
        serde_yaml::to_string(&card).expect("invariant: card.yaml should serialize"),
    )
    .expect("invariant: card.yaml should be writable for stale proof setup");
    let before_stale = maestro_files(repo);
    let stale_matrix = run_success(repo, &["query", "matrix"]);
    assert!(stale_matrix.contains("stale"));
    assert_eq!(before_stale, maestro_files(repo));
}

#[test]
fn query_friction_ignores_bad_json_and_symlinked_run_dirs() {
    let temp = setup_repo("maestro-query-friction-bad-events");
    let repo = temp.path();
    let runs_dir = repo.join(".maestro/runs");
    let run_dir = runs_dir.join("run-001");
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            "{\"kind\":\"UserPromptSubmit\",\"message\":\"actually retry\"}\n",
            "not json\n"
        ),
    )
    .expect("invariant: events should be writable");
    let bad_run_dir = runs_dir.join("run-002");
    fs::create_dir_all(&bad_run_dir).expect("invariant: bad run dir should be creatable");
    fs::write(bad_run_dir.join("events.jsonl"), [0xff, b'\n'])
        .expect("invariant: bad events should be writable");
    unix_fs::symlink(&runs_dir, runs_dir.join("loop"))
        .expect("invariant: symlink should be creatable on unix test host");

    let friction = run_success(repo, &["query", "friction"]);
    assert!(friction.contains("events: 1"));
    assert!(friction.contains("corrections: 1"));
}

#[cfg(unix)]
#[test]
fn query_friction_ignores_events_when_maestro_root_is_symlinked() {
    let temp = TestTempDir::new("maestro-query-symlinked-root");
    let repo = temp.path();
    fs::create_dir(repo.join(".git")).expect("invariant: .git marker should be creatable");
    let external = TestTempDir::new("maestro-query-external-root");
    let external_run = external.path().join("runs/run-001");
    fs::create_dir_all(&external_run).expect("invariant: external run dir should be creatable");
    fs::write(
        external_run.join("events.jsonl"),
        "{\"kind\":\"UserPromptSubmit\",\"message\":\"actually retry\"}\n",
    )
    .expect("invariant: external event should be writable");
    unix_fs::symlink(external.path(), repo.join(".maestro"))
        .expect("invariant: symlinked .maestro root should be creatable");

    let friction = run_success(repo, &["query", "friction"]);

    assert!(friction.contains("friction: no events found"));
}

#[test]
fn query_matrix_ignores_symlinked_task_dirs() {
    let temp = setup_repo("maestro-query-symlinked-task");
    let repo = temp.path();
    let external_dir = repo.join("external-task");
    fs::create_dir(&external_dir).expect("invariant: external task dir should be creatable");
    fs::write(
        external_dir.join("task.yaml"),
        "schema_version: maestro.task.v1\nid: task-999\nslug: forged\nfeature_id: forged\nstate: verified\ntitle: Forged task\nacceptance_locked: true\nverification: {}\ncreated_at: now\nupdated_at: now\n",
    )
    .expect("invariant: forged task yaml should be writable");
    fs::create_dir_all(repo.join(".maestro/tasks"))
        .expect("invariant: tasks dir should be creatable");
    unix_fs::symlink(&external_dir, repo.join(".maestro/tasks/task-999-forged"))
        .expect("invariant: task symlink should be creatable");

    let matrix = run_success(repo, &["query", "matrix"]);
    assert!(!matrix.contains("task-999"));
    assert!(!matrix.contains("forged"));
}

#[test]
fn query_proof_reads_an_archived_task_through_the_archive_fallback() {
    let temp = setup_repo("maestro-query-proof-archived");
    let repo = temp.path();

    // A terminal, archived task still owns its proof; `query proof` is a read and
    // must fall through to the archive tree instead of erroring "not found".
    //
    // Card mode: per-task archive was retired (SPEC E4); archiving is now a
    // feature-level cascade that moves the feature container -- its pooled child
    // tasks ride inside -- to `.maestro/archive/cards/<feature>/`. Drive a
    // verified child task, close it, terminal-ize the feature through its gated
    // verb (a generic `update --status` on a feature is refused, SPEC E3), then
    // `archive <feature>` so the task record lands in the archive tree -- and
    // `query proof` must read it through the archive fallback.
    let task_id = create_verified_task_with_proof(repo);
    for args in [
        vec!["task", "verify", &task_id],
        vec!["close", &task_id],
        vec![
            "feature",
            "cancel",
            "billing-csv-export",
            "--reason",
            "test fixture: terminal-ize for archive",
        ],
        vec!["archive", "billing-csv-export"],
    ] {
        assert_success(&maestro(repo, &args), &args);
    }
    assert!(
        repo.join(".maestro/archive/cards/billing-csv-export/tasks")
            .join(&task_id)
            .join("task.yaml")
            .is_file(),
        "the pooled task record moved to the archive tree inside its feature container"
    );
    assert!(!repo.join(".maestro/cards/billing-csv-export").exists());

    let proof = run_success(repo, &["query", "proof", &task_id]);
    assert!(proof.contains(&format!("proof {task_id}:")));
}

#[test]
fn query_proof_honors_maestro_current_task_like_task_show() {
    let temp = setup_repo("maestro-query-proof-env");
    let repo = temp.path();
    let task_id = create_verified_task_with_proof(repo);

    // With no positional id, `query proof` reads MAESTRO_CURRENT_TASK (strict, no
    // single-task auto-detect), mirroring the sibling read view `task show`.
    let from_env = maestro_with_env(
        repo,
        &["query", "proof"],
        &[("MAESTRO_CURRENT_TASK", task_id.as_str())],
    );
    assert_success(&from_env, &["query", "proof"]);
    assert!(stdout(&from_env).contains(&format!("proof {task_id}: accepted")));

    // An empty env value gives the "id required or set MAESTRO_CURRENT_TASK"
    // remedy, not a fall-through.
    let blank = maestro_with_env(repo, &["query", "proof"], &[("MAESTRO_CURRENT_TASK", "")]);
    assert_failure(&blank, &["query", "proof"]);
    assert!(stderr(&blank).contains("MAESTRO_CURRENT_TASK"));
}

#[test]
fn query_matrix_reports_an_empty_state_line_when_no_features_or_tasks_exist() {
    let temp = setup_repo("maestro-query-matrix-empty");
    let repo = temp.path();

    let matrix = run_success(repo, &["query", "matrix"]);
    assert!(matrix.contains("no features or tasks found"));
    assert!(!matrix.contains("FEATURE"));
}

fn maestro_files(repo: &Path) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    collect_files(&repo.join(".maestro"), repo, &mut files);
    files
}

fn collect_files(dir: &Path, repo: &Path, files: &mut BTreeSet<PathBuf>) {
    for entry in fs::read_dir(dir).expect("invariant: directory should be readable") {
        let entry = entry.expect("invariant: directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, repo, files);
        } else if path.is_file() {
            files.insert(
                path.strip_prefix(repo)
                    .expect("invariant: path should be under repo")
                    .to_path_buf(),
            );
        }
    }
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

/// SPEC-archive-memory-2 R7: `query graph <id>` walks the typed edges
/// (parent/blocks/supersedes) two hops as a tree; `--dot` exports Graphviz DOT
/// -- the whole web bare, one connected component with an id. Decision
/// supersession (kept in the record, not a dep) rides as a supersedes edge,
/// and a swept target renders as `[archived]` so the lineage survives R2.
#[test]
fn query_graph_walks_typed_edges_two_hops_and_exports_dot() {
    let temp = setup_repo("maestro-query-graph");
    let repo = temp.path();

    // Web: feature <- child task <- blocker <- hop-3 blocker (a 3-hop chain
    // from the feature), plus a disconnected decision pair (new supersedes old).
    run_success(repo, &["feature", "new", "Billing CSV export"]);
    run_success(
        repo,
        &[
            "create",
            "-t",
            "task",
            "Export rows",
            "--parent",
            "billing-csv-export",
        ],
    );
    let child = id_by_title(repo, "Export rows");
    run_success(repo, &["task", "create", "Schema freeze"]);
    let blocker = id_by_title(repo, "Schema freeze");
    run_success(repo, &["task", "create", "Vendor signoff"]);
    let hop3 = id_by_title(repo, "Vendor signoff");
    run_success(
        repo,
        &[
            "task",
            "block",
            &child,
            "--reason",
            "schema first",
            "--by",
            &blocker,
        ],
    );
    run_success(
        repo,
        &[
            "task",
            "block",
            &blocker,
            "--reason",
            "vendor first",
            "--by",
            &hop3,
        ],
    );

    run_success(repo, &["decision", "new", "Tabs or spaces"]);
    let old_rule = id_by_title(repo, "Tabs or spaces");
    run_success(
        repo,
        &[
            "decision",
            "lock",
            &old_rule,
            "--decision",
            "tabs",
            "--rejected",
            "spaces: drift",
        ],
    );
    run_success(repo, &["decision", "new", "Spaces after all"]);
    let new_rule = id_by_title(repo, "Spaces after all");
    run_success(
        repo,
        &[
            "decision",
            "lock",
            &new_rule,
            "--decision",
            "spaces",
            "--rejected",
            "tabs: rendering drift",
            "--supersedes",
            &old_rule,
        ],
    );

    // Tree from the feature: the child is hop 1, its blocker hop 2; the hop-3
    // blocker is beyond the two-hop horizon and stays out.
    let tree = run_success(repo, &["query", "graph", "billing-csv-export"]);
    assert!(
        tree.starts_with("billing-csv-export (feature, "),
        "the root line names the card:\n{tree}"
    );
    assert!(
        tree.contains(&format!("- child: {child} (task, ")),
        "hop 1 shows the child task:\n{tree}"
    );
    assert!(
        tree.contains(&format!("  - blocked-by: {blocker} (task, ")),
        "hop 2 shows the child's blocker indented:\n{tree}"
    );
    assert!(
        !tree.contains(&hop3),
        "hop 3 stays beyond the tree horizon:\n{tree}"
    );

    // From the child both directions are one hop: its parent and its blocker.
    let from_child = run_success(repo, &["query", "graph", &child]);
    assert!(
        from_child.contains("- parent: billing-csv-export (feature, "),
        "the parent edge reads from the child's side:\n{from_child}"
    );
    assert!(
        from_child.contains(&format!("- blocked-by: {blocker} (task, ")),
        "the blocker is one hop from the child:\n{from_child}"
    );
    assert!(
        from_child.contains(&format!("  - blocked-by: {hop3} (task, ")),
        "hop 2 from the child reaches the vendor blocker:\n{from_child}"
    );

    // The decision pair: supersession reads from the record, not a dep.
    let rules = run_success(repo, &["query", "graph", &new_rule]);
    assert!(
        rules.contains(&format!(
            "- supersedes: {old_rule} (decision, superseded) Tabs or spaces"
        )),
        "the kept rule points at the rule it replaced:\n{rules}"
    );

    // Whole-web DOT: every node and labeled edge, ready for rendering.
    let dot = run_success(repo, &["query", "graph", "--dot"]);
    assert!(dot.starts_with("digraph cards {"), "{dot}");
    assert!(
        dot.contains("\"billing-csv-export\" [label=\"billing-csv-export\\nfeature:"),
        "nodes carry id/type:status/title labels:\n{dot}"
    );
    assert!(
        dot.contains(&format!(
            "\"{child}\" -> \"billing-csv-export\" [label=\"parent\"];"
        )),
        "the parent edge is exported:\n{dot}"
    );
    assert!(
        dot.contains(&format!(
            "\"{child}\" -> \"{blocker}\" [label=\"blocked-by\"];"
        )),
        "the blocks edge is exported from the blocked side:\n{dot}"
    );
    assert!(
        dot.contains(&format!(
            "\"{new_rule}\" -> \"{old_rule}\" [label=\"supersedes\"];"
        )),
        "the supersession edge is exported:\n{dot}"
    );

    // Component DOT: the task web only, the decision pair stays out.
    let component = run_success(repo, &["query", "graph", &child, "--dot"]);
    assert!(component.contains(&hop3), "{component}");
    assert!(
        !component.contains(&new_rule),
        "the disconnected decision pair stays out of the component:\n{component}"
    );

    // R2 tie-in: sweeping the superseded rule turns the live edge into an
    // [archived] marker, so the lineage stays visible after the sweep.
    run_success(repo, &["archive", "--loose"]);
    let after_sweep = run_success(repo, &["query", "graph", &new_rule]);
    assert!(
        after_sweep.contains(&format!("- supersedes: {old_rule} [archived]")),
        "a swept supersession target is marked archived:\n{after_sweep}"
    );

    // Guard rails: bare graph errors with the two shapes; an unknown id names
    // the recall surfaces.
    let bare = maestro(repo, &["query", "graph"]);
    assert_failure(&bare, &["query", "graph"]);
    assert!(stderr(&bare).contains("provide a card id or --dot"));
    let missing = maestro(repo, &["query", "graph", "nope-404"]);
    assert_failure(&missing, &["query", "graph", "nope-404"]);
    assert!(stderr(&missing).contains("no card nope-404 in the live store"));
}
