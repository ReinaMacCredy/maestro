//! End-to-end for the `maestro active` cross-session awareness verb. Exercises
//! only the NEW CLI surface -- column render + enrichment, the `you` marker,
//! `--all` stale filtering, and the copy-pasteable link hint with no auto-link
//! side effect (bl-001/002/003/005). The liveness model itself
//! (`src/domain/run/active.rs`) is covered by its own unit tests and is not
//! re-tested here.

mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use maestro::foundation::core::time::format_utc_seconds_rfc3339_millis;
use support::TestTempDir;

/// A repo already in card mode: `.maestro/cards/` exists so `discover_repo_root`
/// finds `.maestro/` and the card verbs apply.
fn cards_repo(name: &str) -> TestTempDir {
    let temp = TestTempDir::new(name);
    fs::create_dir_all(temp.path().join(".maestro/cards"))
        .expect("invariant: cards dir should be creatable");
    temp
}

/// Mint a card and return its id, captured from `create --id-only`.
fn create_id(repo: &Path, args: &[&str]) -> String {
    let mut full = vec!["create"];
    full.extend_from_slice(args);
    full.push("--id-only");
    run(repo, &[], &full).trim().to_string()
}

fn maestro(repo: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command
        .args(args)
        .current_dir(repo)
        .env("MAESTRO_AGENT", "codex");
    for (key, value) in env {
        command.env(key, value);
    }
    command
        .output()
        .expect("invariant: compiled maestro binary should run in integration tests")
}

fn run(repo: &Path, env: &[(&str, &str)], args: &[&str]) -> String {
    let output = maestro(repo, env, args);
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invariant: stdout should be UTF-8")
}

/// An RFC3339 millis timestamp `minutes` before the wall clock, comfortably
/// inside its liveness band so the test does not flake as the clock ticks
/// between seeding and the binary run.
fn ts_minutes_ago(minutes: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("invariant: clock is after the Unix epoch")
        .as_secs();
    format_utc_seconds_rfc3339_millis(now - minutes * 60)
}

fn skill_event(session: &str, skill: &str, ts: &str) -> String {
    format!(
        r#"{{"event_type":"skill_activation","session_id":"{session}","skill_name":"{skill}","ts":"{ts}"}}"#
    )
}

fn card_touch_event(session: &str, card: &str, ts: &str) -> String {
    format!(
        r#"{{"event_type":"card_touch","session_id":"{session}","card_id":"{card}","ts":"{ts}"}}"#
    )
}

fn stop_event(session: &str, ts: &str) -> String {
    format!(r#"{{"event_type":"Stop","session_id":"{session}","ts":"{ts}"}}"#)
}

fn seed_run(repo: &Path, session: &str, lines: &[String]) {
    let run_dir = repo.join(".maestro/runs").join(session);
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .expect("invariant: event log fixture should be writable");
}

/// Drop every run bucket so card-setup verbs (which auto-emit `card_touch`) do
/// not leave phantom sessions; the test seeds only the buckets it asserts on.
fn clear_runs(repo: &Path) {
    let runs = repo.join(".maestro/runs");
    if runs.exists() {
        fs::remove_dir_all(&runs).expect("invariant: runs dir should be removable");
    }
}

fn line_with<'a>(output: &'a str, needle: &str) -> &'a str {
    output
        .lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("expected a line containing {needle:?}\n{output}"))
}

#[test]
fn active_lists_live_sessions_with_enriched_rows_and_you_marker() {
    // bl-001: one row per session, each carrying mode, bound card title +
    // status, type-aware progress, last action, age, and presence; the running
    // session is marked `you`.
    let temp = cards_repo("active-bl001");
    let repo = temp.path();

    run(repo, &[], &["create", "-t", "feature", "Peer topic"]);
    let task_one = create_id(repo, &["-t", "task", "Task one", "--parent", "peer-topic"]);
    create_id(repo, &["-t", "task", "Task two", "--parent", "peer-topic"]);
    run(repo, &[], &["update", &task_one, "--status", "verified"]);
    clear_runs(repo);

    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "peer-sess",
        &[
            skill_event("peer-sess", "maestro-card", &recent),
            card_touch_event("peer-sess", "peer-topic", &recent),
        ],
    );
    seed_run(
        repo,
        "you-sess",
        &[skill_event(
            "you-sess",
            "maestro-design",
            &ts_minutes_ago(2),
        )],
    );

    let out = run(repo, &[("MAESTRO_SESSION_ID", "you-sess")], &["active"]);

    assert!(out.contains("peer-sess"), "peer row present\n{out}");
    assert!(out.contains("you-sess"), "running row present\n{out}");

    let peer = line_with(&out, "peer-sess");
    assert!(
        peer.contains("card"),
        "peer mode (maestro-card -> card)\n{out}"
    );
    assert!(peer.contains("Peer topic"), "bound card title\n{out}");
    assert!(peer.contains("1/2 tasks"), "type-aware progress\n{out}");
    assert!(
        peer.contains("[working]"),
        "recent non-Stop -> working\n{out}"
    );

    let you = line_with(&out, "you-sess");
    assert!(
        you.contains("design"),
        "running mode (maestro-design)\n{out}"
    );
    assert!(you.contains("you"), "running session marked you\n{out}");
}

#[test]
fn all_reveals_stale_sessions_hidden_by_default() {
    // bl-002: a session whose latest event is beyond the window is absent
    // without `--all`, present and tagged `[stale Nm]` with it.
    let temp = cards_repo("active-bl002");
    let repo = temp.path();
    clear_runs(repo);

    seed_run(
        repo,
        "fresh-sess",
        &[skill_event(
            "fresh-sess",
            "maestro-card",
            &ts_minutes_ago(1),
        )],
    );
    seed_run(
        repo,
        "stale-sess",
        &[skill_event(
            "stale-sess",
            "maestro-card",
            &ts_minutes_ago(40),
        )],
    );

    let default = run(repo, &[], &["active"]);
    assert!(default.contains("fresh-sess"), "fresh row shown\n{default}");
    assert!(
        !default.contains("stale-sess"),
        "stale row hidden by default\n{default}"
    );

    let all = run(repo, &[], &["active", "--all"]);
    assert!(all.contains("fresh-sess"), "fresh row still shown\n{all}");
    assert!(
        all.contains("stale-sess"),
        "stale row revealed by --all\n{all}"
    );
    assert!(
        line_with(&all, "stale-sess").contains("[stale"),
        "stale row tagged [stale Nm]\n{all}"
    );
}

#[test]
fn recent_stop_reads_as_waiting_not_excluded() {
    // bl-003: a session whose latest event is a recent Stop is present and
    // labelled `[waiting]`, not filtered out.
    let temp = cards_repo("active-bl003");
    let repo = temp.path();
    clear_runs(repo);

    seed_run(
        repo,
        "stop-sess",
        &[
            skill_event("stop-sess", "maestro-design", &ts_minutes_ago(5)),
            stop_event("stop-sess", &ts_minutes_ago(2)),
        ],
    );

    let out = run(repo, &[], &["active"]);
    assert!(
        out.contains("stop-sess"),
        "stopped session not excluded\n{out}"
    );
    assert!(
        line_with(&out, "stop-sess").contains("[waiting]"),
        "recent Stop -> waiting\n{out}"
    );
}

#[test]
fn prints_copy_pasteable_link_hint_and_creates_no_edge() {
    // bl-005: output carries a `maestro link add <your-card> <their-card>`
    // template referencing the peer's card id; running `active` creates no
    // related edge as a side effect.
    let temp = cards_repo("active-bl005");
    let repo = temp.path();

    run(repo, &[], &["create", "-t", "feature", "Peer topic"]);
    clear_runs(repo);

    seed_run(
        repo,
        "peer-sess",
        &[card_touch_event(
            "peer-sess",
            "peer-topic",
            &ts_minutes_ago(1),
        )],
    );

    // Running session has no bucket yet (active as a first step), so `<your-card>`
    // stays a literal placeholder.
    let out = run(repo, &[("MAESTRO_SESSION_ID", "you-sess")], &["active"]);
    assert!(out.contains("maestro link add"), "link hint present\n{out}");
    assert!(
        out.contains("peer-topic"),
        "hint names the peer card\n{out}"
    );
    assert!(
        out.contains("<your-card>"),
        "no bound card yet -> literal placeholder\n{out}"
    );

    let show = run(repo, &[], &["show", "peer-topic"]);
    assert!(
        !show.contains("related"),
        "active must not auto-create a related edge\n{show}"
    );
}
