//! End-to-end for the `maestro active` cross-session awareness verb. Exercises
//! only the NEW CLI surface -- column render + enrichment, relation/ownership
//! labels, `--all` stale filtering, and the copy-pasteable link hint with no
//! auto-link side effect (bl-001/002/003/005). The liveness model itself
//! (`src/domain/run/active.rs`) is covered by its own unit tests and is not
//! re-tested here.

pub mod card_support;
mod support;
mod witness_support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use card_support::cards_repo;
use maestro::foundation::core::paths::MaestroPaths;
use maestro::foundation::core::schema::SESSION_ACTIVITY_SCHEMA_VERSION;
use maestro::foundation::core::time::format_utc_seconds_rfc3339_millis;
use serde_json::Value;
use witness_support::write_valid_witness;

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

fn run_output(repo: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let output = maestro(repo, env, args);
    assert!(
        output.status.success(),
        "maestro {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn run_failure(repo: &Path, env: &[(&str, &str)], args: &[&str]) -> String {
    let output = maestro(repo, env, args);
    assert!(
        !output.status.success(),
        "maestro {args:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
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

fn skill_runtime_event(session: &str, skill: &str, runtime: &str, ts: &str) -> String {
    format!(
        r#"{{"event_type":"skill_activation","session_id":"{session}","skill_name":"{skill}","agent_runtime":"{runtime}","ts":"{ts}"}}"#
    )
}

fn card_touch_event(session: &str, card: &str, ts: &str) -> String {
    format!(
        r#"{{"event_type":"card_touch","session_id":"{session}","card_id":"{card}","ts":"{ts}"}}"#
    )
}

fn ownership_acquire_event(session: &str, card: &str, ts: &str) -> String {
    format!(
        r#"{{"event_type":"ownership_acquire","session_id":"{session}","card_id":"{card}","ts":"{ts}"}}"#
    )
}

fn ownership_release_event(session: &str, card: &str, status: &str, ts: &str) -> String {
    format!(
        r#"{{"event_type":"ownership_release","session_id":"{session}","card_id":"{card}","status":"{status}","ts":"{ts}"}}"#
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

fn seed_activity(repo: &Path, session: &str, lines: &[String]) {
    let run_dir = repo.join(".maestro/runs").join(session);
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("activity.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .expect("invariant: activity log fixture should be writable");
}

/// Drop every run bucket so card-setup verbs (which auto-emit `card_touch`) do
/// not leave phantom sessions; the test seeds only the buckets it asserts on.
fn clear_runs(repo: &Path) {
    let runs = repo.join(".maestro/runs");
    if runs.exists() {
        fs::remove_dir_all(&runs).expect("invariant: runs dir should be removable");
    }
}

fn age_session(repo: &Path, session: &str, minutes: u64) {
    let path = repo
        .join(".maestro/runs")
        .join(session)
        .join("events.jsonl");
    let ts = ts_minutes_ago(minutes);
    let aged = fs::read_to_string(&path)
        .expect("invariant: session event log should exist")
        .lines()
        .map(|line| {
            let mut value: Value =
                serde_json::from_str(line).expect("invariant: event log line should be JSON");
            value["ts"] = Value::String(ts.clone());
            serde_json::to_string(&value).expect("invariant: event log line should serialize")
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, format!("{aged}\n")).expect("invariant: aged event log should be writable");
}

fn run_log(repo: &Path, session: &str) -> String {
    fs::read_to_string(
        repo.join(".maestro/runs")
            .join(session)
            .join("events.jsonl"),
    )
    .expect("invariant: session event log should exist")
}

fn line_with<'a>(output: &'a str, needle: &str) -> &'a str {
    output
        .lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("expected a line containing {needle:?}\n{output}"))
}

fn occurrence_count(output: &str, needle: &str) -> usize {
    output.matches(needle).count()
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
    run(repo, &[], &["task", "set", &task_one, "--check", "done"]);
    run(repo, &[], &["task", "explore", &task_one]);
    run(repo, &[], &["task", "accept", &task_one]);
    run(repo, &[], &["task", "claim", &task_one]);
    run(
        repo,
        &[],
        &[
            "task",
            "complete",
            &task_one,
            "--summary",
            "done",
            "--claim",
            "GREEN: done",
            "--proof",
            "GREEN: done",
        ],
    );
    clear_runs(repo);

    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "peer-sess",
        &[
            skill_runtime_event("peer-sess", "maestro-card", "droid", &ts_minutes_ago(2)),
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
    let header = line_with(&out, "AGENT");
    assert!(
        header.find("AGENT").unwrap() < header.find("SESSION").unwrap(),
        "AGENT column should render before SESSION\n{out}"
    );

    let peer = line_with(&out, "peer-sess");
    assert!(
        peer.trim_start().starts_with("droid"),
        "latest non-empty runtime survives later missing event\n{out}"
    );
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
        you.trim_start().starts_with('-'),
        "legacy row without agent_runtime renders dash\n{out}"
    );
    assert!(
        you.contains("design"),
        "running mode (maestro-design)\n{out}"
    );
    assert!(you.contains("you"), "running session marked you\n{out}");
}

#[test]
fn active_shows_compact_session_detail_hint_when_activity_is_hidden() {
    let temp = cards_repo("active-session-activity-hint");
    let repo = temp.path();

    let card = create_id(repo, &["-t", "task", "Busy task"]);
    clear_runs(repo);

    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "busy-sess",
        &[
            skill_event("busy-sess", "maestro-card", &recent),
            card_touch_event("busy-sess", &card, &recent),
        ],
    );
    seed_activity(
        repo,
        "busy-sess",
        &[format!(
            r#"{{"schema_version":"{SESSION_ACTIVITY_SCHEMA_VERSION}","source":"run_event","source_event_type":"PostToolUse","kind":"command_finished","session_id":"busy-sess","ts":"{recent}","command":{{"program":"Shell","input_hash":"sha256:abc"}}}}"#
        )],
    );

    let out = run(repo, &[], &["active"]);
    let line = line_with(&out, "busy-sess");

    assert!(
        line.contains("activity: 1 cmd"),
        "active row should show a compact activity count\n{out}"
    );
    assert!(
        line.contains("maestro session show busy-sess"),
        "active row should point at the detail readout\n{out}"
    );
    assert!(
        !out.contains("Timeline:"),
        "active must stay compact and not inline the session readout\n{out}"
    );
}

#[test]
fn active_connect_prints_advisory_commands_without_linking() {
    let temp = cards_repo("active-connect-advisory");
    let repo = temp.path();

    let mine = create_id(repo, &["-t", "task", "Mine"]);
    let peer = create_id(repo, &["-t", "task", "Peer"]);
    clear_runs(repo);

    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "peer-sess",
        &[
            skill_event("peer-sess", "maestro-card", &recent),
            card_touch_event("peer-sess", &peer, &recent),
        ],
    );
    seed_run(
        repo,
        "you-sess",
        &[
            skill_event("you-sess", "maestro-card", &recent),
            card_touch_event("you-sess", &mine, &recent),
        ],
    );

    let out = run(
        repo,
        &[("MAESTRO_SESSION_ID", "you-sess")],
        &["active", "--connect"],
    );

    assert!(out.contains("suggested coordination"), "{out}");
    assert!(
        out.contains(&format!("maestro link add {mine} {peer}")),
        "{out}"
    );
    assert!(
        out.contains(&format!("maestro msg send --from {mine} {peer}")),
        "{out}"
    );
    let links = run(repo, &[], &["card", "graph", &mine]);
    assert!(
        !links.contains(&peer),
        "active --connect must not create related edges:\n{links}"
    );
}

#[test]
fn active_connect_messages_parent_card_for_peer_task_endpoint() {
    let temp = cards_repo("active-connect-task-parent");
    let repo = temp.path();

    let mine = create_id(repo, &["-t", "feature", "Sender topic"]);
    let parent = create_id(repo, &["-t", "feature", "Peer topic"]);
    let peer_task = create_id(repo, &["-t", "task", "Task one", "--parent", &parent]);
    clear_runs(repo);

    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "peer-sess",
        &[
            skill_event("peer-sess", "maestro-card", &recent),
            card_touch_event("peer-sess", &peer_task, &recent),
        ],
    );
    seed_run(
        repo,
        "you-sess",
        &[
            skill_event("you-sess", "maestro-card", &recent),
            card_touch_event("you-sess", &mine, &recent),
        ],
    );

    let out = run(
        repo,
        &[("MAESTRO_SESSION_ID", "you-sess")],
        &["active", "--connect"],
    );

    assert!(
        out.contains(&format!("maestro link add {mine} {parent}")),
        "{out}"
    );
    assert!(
        out.contains(&format!(
            "maestro msg send --from {mine} {parent} \"<text>\""
        )),
        "{out}"
    );
    assert!(
        !out.contains(&format!(
            "maestro msg send --from {mine} {peer_task} \"<text>\""
        )),
        "{out}"
    );
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
fn default_active_hides_non_actionable_rows_and_diagnostics_with_all_escape_hatch() {
    let temp = cards_repo("active-compact-default");
    let repo = temp.path();
    let working = create_id(repo, &["-t", "task", "Working card"]);
    let quiet = create_id(repo, &["-t", "task", "Quiet card"]);
    let idle = create_id(repo, &["-t", "task", "Idle card"]);
    let released = create_id(repo, &["-t", "task", "Released card"]);
    let done = create_id(repo, &["-t", "task", "Done card"]);
    let unconfirmed = create_id(repo, &["-t", "task", "Unconfirmed card"]);
    let stale = create_id(repo, &["-t", "task", "Stale card"]);
    clear_runs(repo);

    seed_run(
        repo,
        "working-sess",
        &[
            skill_event("working-sess", "maestro-card", &ts_minutes_ago(1)),
            card_touch_event("working-sess", &working, &ts_minutes_ago(1)),
        ],
    );
    seed_run(
        repo,
        "quiet-sess",
        &[ownership_acquire_event(
            "quiet-sess",
            &quiet,
            &ts_minutes_ago(10),
        )],
    );
    seed_run(
        repo,
        "waiting-sess",
        &[
            skill_event("waiting-sess", "maestro-card", &ts_minutes_ago(2)),
            stop_event("waiting-sess", &ts_minutes_ago(1)),
        ],
    );
    seed_run(
        repo,
        "idle-sess",
        &[card_touch_event("idle-sess", &idle, &ts_minutes_ago(10))],
    );
    seed_run(
        repo,
        "released-sess",
        &[ownership_release_event(
            "released-sess",
            &released,
            "released",
            &ts_minutes_ago(10),
        )],
    );
    seed_run(
        repo,
        "done-sess",
        &[ownership_release_event(
            "done-sess",
            &done,
            "done",
            &ts_minutes_ago(10),
        )],
    );
    seed_run(
        repo,
        "unconfirmed-sess",
        &[ownership_acquire_event(
            "unconfirmed-sess",
            &unconfirmed,
            &ts_minutes_ago(121),
        )],
    );
    seed_run(
        repo,
        "stale-sess",
        &[card_touch_event("stale-sess", &stale, &ts_minutes_ago(40))],
    );

    let default = run(repo, &[], &["active"]);
    assert!(default.contains("working-sess"), "{default}");
    assert!(default.contains("quiet-sess"), "{default}");
    assert!(default.contains("waiting-sess"), "{default}");
    assert!(!default.contains("idle-sess"), "{default}");
    assert!(!default.contains("released-sess"), "{default}");
    assert!(!default.contains("done-sess"), "{default}");
    assert!(!default.contains("unconfirmed-sess"), "{default}");
    assert!(!default.contains("stale-sess"), "{default}");
    assert!(
        !default.contains("harness:"),
        "default active board should omit harness diagnostics\n{default}"
    );
    assert!(
        default.contains("inactive hidden") && default.contains("--all to show"),
        "default output should name hidden inactive rows compactly\n{default}"
    );

    let all = run(repo, &[], &["active", "--all"]);
    for session in [
        "working-sess",
        "quiet-sess",
        "waiting-sess",
        "idle-sess",
        "released-sess",
        "done-sess",
        "unconfirmed-sess",
        "stale-sess",
    ] {
        assert!(
            all.contains(session),
            "--all should reveal {session}\n{all}"
        );
    }
    assert!(
        all.contains("harness:"),
        "--all keeps the historical diagnostic footer\n{all}"
    );
}

#[test]
fn active_connect_dedupes_related_commands_by_target_card() {
    let temp = cards_repo("active-connect-dedupes");
    let repo = temp.path();

    let mine = create_id(repo, &["-t", "task", "Mine"]);
    let peer = create_id(repo, &["-t", "task", "Peer"]);
    clear_runs(repo);

    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "you-sess",
        &[
            skill_event("you-sess", "maestro-card", &recent),
            card_touch_event("you-sess", &mine, &recent),
        ],
    );
    seed_run(
        repo,
        "peer-one",
        &[
            skill_event("peer-one", "maestro-card", &recent),
            card_touch_event("peer-one", &peer, &recent),
        ],
    );
    seed_run(
        repo,
        "peer-two",
        &[
            skill_event("peer-two", "maestro-card", &recent),
            card_touch_event("peer-two", &peer, &recent),
        ],
    );

    let default = run(repo, &[("MAESTRO_SESSION_ID", "you-sess")], &["active"]);
    assert!(
        default.contains("related:") && default.contains("maestro active --connect"),
        "default output should collapse related guidance behind --connect\n{default}"
    );
    assert!(
        !default.contains(&format!("maestro link add {mine} {peer}")),
        "default output should not print repeated link commands\n{default}"
    );

    let connect = run(
        repo,
        &[("MAESTRO_SESSION_ID", "you-sess")],
        &["active", "--connect"],
    );
    assert_eq!(
        occurrence_count(&connect, &format!("maestro link add {mine} {peer}")),
        1,
        "connect should print one link command per target card\n{connect}"
    );
    assert_eq!(
        occurrence_count(
            &connect,
            &format!("maestro msg send --from {mine} {peer} \"<text>\"")
        ),
        1,
        "connect should print one message command per target card\n{connect}"
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
fn ownership_events_keep_quiet_sessions_visible_and_moves_old_context_to_all() {
    let temp = cards_repo("active-ownership-states");
    let repo = temp.path();
    let owned = create_id(repo, &["-t", "task", "Owned card"]);
    let touched = create_id(repo, &["-t", "task", "Touched only"]);
    let old_owned = create_id(repo, &["-t", "task", "Old owned"]);
    clear_runs(repo);

    seed_run(
        repo,
        "owned-sess",
        &[ownership_acquire_event(
            "owned-sess",
            &owned,
            &ts_minutes_ago(10),
        )],
    );
    seed_run(
        repo,
        "touch-sess",
        &[card_touch_event(
            "touch-sess",
            &touched,
            &ts_minutes_ago(10),
        )],
    );
    seed_run(
        repo,
        "old-owned-sess",
        &[ownership_acquire_event(
            "old-owned-sess",
            &old_owned,
            &ts_minutes_ago(121),
        )],
    );

    let out = run(repo, &[], &["active"]);

    assert!(
        line_with(&out, "owned-sess").contains("[quiet-working"),
        "owned quiet session stays owned\n{out}"
    );
    assert!(
        !out.contains("touch-sess"),
        "card_touch-only idle context is hidden from the compact default\n{out}"
    );
    assert!(
        !out.contains("old-owned-sess"),
        "owned session past two hours moves behind --all\n{out}"
    );

    let all = run(repo, &[], &["active", "--all"]);
    assert!(
        line_with(&all, "touch-sess").contains("[idle"),
        "card_touch-only idle context remains available through --all\n{all}"
    );
    assert!(
        line_with(&all, "old-owned-sess").contains("[unconfirmed"),
        "owned session past two hours remains auditable through --all\n{all}"
    );
}

#[test]
fn active_card_filter_includes_stale_rows_for_that_feature_only() {
    let temp = cards_repo("active-card-filter");
    let repo = temp.path();
    run(repo, &[], &["create", "-t", "feature", "Peer topic"]);
    let task = create_id(repo, &["-t", "task", "Task one", "--parent", "peer-topic"]);
    let other = create_id(repo, &["-t", "task", "Other task"]);
    clear_runs(repo);

    seed_run(
        repo,
        "old-feature-sess",
        &[ownership_acquire_event(
            "old-feature-sess",
            &task,
            &ts_minutes_ago(121),
        )],
    );
    seed_run(
        repo,
        "old-unrelated-sess",
        &[ownership_acquire_event(
            "old-unrelated-sess",
            &other,
            &ts_minutes_ago(121),
        )],
    );

    let out = run(repo, &[], &["active", "--card", "peer-topic"]);

    assert!(
        out.contains("old-feature-sess"),
        "focused filter should include stale work under the target feature\n{out}"
    );
    assert!(
        !out.contains("old-unrelated-sess"),
        "focused filter must not require active --all or show unrelated stale rows\n{out}"
    );
}

#[test]
fn design_mutations_acquire_ownership_so_quiet_design_is_not_idle() {
    let temp = cards_repo("active-design-mutations-own");
    let repo = temp.path();
    clear_runs(repo);

    run(
        repo,
        &[("MAESTRO_SESSION_ID", "design-sess")],
        &[
            "feature",
            "new",
            "Design Owned",
            "--description",
            "map current state",
        ],
    );
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "design-sess")],
        &[
            "feature",
            "set",
            "design-owned",
            "--question",
            "Which ownership boundary?",
        ],
    );
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "design-sess")],
        &[
            "decision",
            "new",
            "Adopt explicit ownership",
            "--feature",
            "design-owned",
        ],
    );
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "design-sess")],
        &[
            "feature",
            "spec",
            "design-owned",
            "--section",
            "Current state",
            "--append",
            "Observed card_touch-only idle drift.",
        ],
    );

    let events = run_log(repo, "design-sess");
    assert!(
        events.contains(r#""event_type":"ownership_acquire""#),
        "design mutations should acquire ownership\n{events}"
    );
    age_session(repo, "design-sess", 10);

    let active = run(repo, &[], &["active"]);
    let line = line_with(&active, "design-sess");
    assert!(
        line.contains("[quiet-working"),
        "quiet design owner should not fall back to idle\n{active}"
    );
    assert!(
        !line.contains("[idle"),
        "ownership-acquired design work must not render idle\n{active}"
    );
}

#[test]
fn generic_card_mutations_acquire_ownership_without_claim() {
    let temp = cards_repo("active-card-mutations-own");
    let repo = temp.path();
    clear_runs(repo);

    let card = run(
        repo,
        &[("MAESTRO_SESSION_ID", "card-sess")],
        &["create", "-t", "bug", "Owned Bug", "--id-only"],
    )
    .trim()
    .to_string();
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "card-sess")],
        &["note", &card, "captured repro"],
    );
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "card-sess")],
        &["card", "update", &card, "--description", "write path"],
    );

    let events = run_log(repo, "card-sess");
    assert!(
        events.contains(r#""event_type":"ownership_acquire""#),
        "generic card mutations should acquire ownership\n{events}"
    );
    age_session(repo, "card-sess", 10);

    let active = run(repo, &[], &["active"]);
    let line = line_with(&active, "card-sess");
    assert!(
        line.contains("[quiet-working"),
        "quiet card mutation owner should not fall back to idle\n{active}"
    );
}

#[test]
fn generic_card_status_updates_release_instead_of_reacquire() {
    let temp = cards_repo("active-card-status-release");
    let repo = temp.path();
    clear_runs(repo);

    let card = run(
        repo,
        &[("MAESTRO_SESSION_ID", "status-sess")],
        &["create", "-t", "bug", "Closable Bug", "--id-only"],
    )
    .trim()
    .to_string();
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "status-sess")],
        &["card", "update", &card, "--status", "closed"],
    );

    let events = run_log(repo, "status-sess");
    let acquire = events
        .find(r#""event_type":"ownership_acquire""#)
        .expect("invariant: create should acquire ownership");
    let release = events
        .rfind(r#""event_type":"ownership_release""#)
        .expect("status close should release ownership");
    assert!(
        release > acquire && events.contains(r#""status":"done""#),
        "terminal status updates should release after the acquire\n{events}"
    );

    let active = run(repo, &[], &["active", "--all"]);
    let line = line_with(&active, "status-sess");
    assert!(
        line.contains("[done") && !line.contains("[quiet-working"),
        "terminal status update must not leave the session working\n{active}"
    );
}

#[test]
fn active_release_records_release_without_changing_card_status() {
    let temp = cards_repo("active-release-command");
    let repo = temp.path();
    let task = create_id(repo, &["-t", "task", "Release me"]);
    clear_runs(repo);

    let out = run(
        repo,
        &[("MAESTRO_SESSION_ID", "owner-sess")],
        &["active", "release", &task, "--reason", "handoff"],
    );
    assert!(
        out.contains("released") && out.contains("idle/released"),
        "release command receipt\n{out}"
    );

    let active = run(repo, &[], &["active", "--all"]);
    assert!(
        line_with(&active, "owner-sess").contains("[idle/released"),
        "release event is visible as idle/released\n{active}"
    );

    let show = run(repo, &[], &["card", "show", &task]);
    let first = show.lines().next().unwrap_or_default();
    assert!(
        first.contains("open"),
        "active release must not change card lifecycle status\n{show}"
    );
}

#[test]
fn task_claim_and_complete_emit_ownership_lifecycle() {
    let temp = cards_repo("active-task-lifecycle");
    let repo = temp.path();
    run(repo, &[], &["create", "-t", "feature", "Lifecycle feature"]);
    let task = create_id(
        repo,
        &[
            "-t",
            "task",
            "Lifecycle task",
            "--parent",
            "lifecycle-feature",
        ],
    );
    run(repo, &[], &["task", "set", &task, "--check", "done"]);
    run(repo, &[], &["task", "explore", &task]);
    run(repo, &[], &["task", "accept", &task]);
    clear_runs(repo);

    run(
        repo,
        &[("MAESTRO_SESSION_ID", "owner-sess")],
        &["task", "claim", &task],
    );
    let claimed = run(repo, &[], &["active"]);
    assert!(
        line_with(&claimed, "owner-sess").contains("[working]"),
        "task claim starts ownership\n{claimed}"
    );

    run(
        repo,
        &[("MAESTRO_SESSION_ID", "owner-sess")],
        &[
            "task",
            "complete",
            &task,
            "--summary",
            "done",
            "--claim",
            "GREEN: done",
            "--proof",
            "GREEN: done",
        ],
    );
    let completed = run(repo, &[], &["active", "--all"]);
    assert!(
        line_with(&completed, "owner-sess").contains("[done"),
        "successful task complete releases ownership as done\n{completed}"
    );
}

#[test]
fn direct_task_verify_after_recovery_releases_ownership() {
    let temp = cards_repo("active-task-verify-release");
    let repo = temp.path();
    run(repo, &[], &["create", "-t", "feature", "Verify feature"]);
    let task = create_id(
        repo,
        &[
            "-t",
            "task",
            "Verify release task",
            "--parent",
            "verify-feature",
        ],
    );
    run(repo, &[], &["task", "set", &task, "--check", "done"]);
    run(repo, &[], &["task", "explore", &task]);
    run(repo, &[], &["task", "accept", &task]);
    clear_runs(repo);

    run(
        repo,
        &[("MAESTRO_SESSION_ID", "verify-sess")],
        &["task", "claim", &task],
    );
    run_failure(
        repo,
        &[("MAESTRO_SESSION_ID", "verify-sess")],
        &[
            "task",
            "complete",
            &task,
            "--summary",
            "done",
            "--claim",
            "UNIQUE-CLAIM-TOKEN",
        ],
    );
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "verify-sess")],
        &[
            "event",
            "create",
            "--task-id",
            &task,
            "--message",
            "UNIQUE-CLAIM-TOKEN",
            "--claim",
            "UNIQUE-CLAIM-TOKEN",
        ],
    );
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "verify-sess")],
        &["task", "verify", &task],
    );

    let active = run(repo, &[], &["active", "--all"]);
    assert!(
        line_with(&active, "verify-sess").contains("[done"),
        "direct task verify should release ownership as done\n{active}"
    );
}

#[test]
fn terminal_task_transitions_release_ownership() {
    let temp = cards_repo("active-task-terminal-release");
    let repo = temp.path();
    let rejected = create_id(repo, &["-t", "task", "Reject owned"]);
    run(
        repo,
        &[],
        &["task", "set", &rejected, "--check", "not used"],
    );
    run(repo, &[], &["task", "explore", &rejected]);
    run(repo, &[], &["task", "accept", &rejected]);
    clear_runs(repo);
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "reject-sess")],
        &["task", "claim", &rejected],
    );
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "reject-sess")],
        &["task", "reject", &rejected, "--reason", "invalid"],
    );

    let old = create_id(repo, &["-t", "task", "Superseded owned"]);
    let new = create_id(repo, &["-t", "task", "Replacement task"]);
    run(repo, &[], &["task", "set", &old, "--check", "not used"]);
    run(repo, &[], &["task", "explore", &old]);
    run(repo, &[], &["task", "accept", &old]);
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "supersede-sess")],
        &["task", "claim", &old],
    );
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "supersede-sess")],
        &[
            "task",
            "supersede",
            &old,
            "--by",
            &new,
            "--reason",
            "replaced",
        ],
    );

    let active = run(repo, &[], &["active", "--all"]);
    assert!(
        line_with(&active, "reject-sess").contains("[done"),
        "reject should release ownership as done\n{active}"
    );
    assert!(
        line_with(&active, "supersede-sess").contains("[done"),
        "supersede should release ownership as done\n{active}"
    );
}

fn create_ready_feature(repo: &Path, title: &str, slug: &str) {
    create_feature_contract(repo, title, slug);
    run(
        repo,
        &[],
        &[
            "feature",
            "accept",
            slug,
            "--qa",
            "none",
            "--reason",
            "integration coverage",
        ],
    );
}

fn create_feature_contract(repo: &Path, title: &str, slug: &str) {
    run(repo, &[], &["feature", "new", title]);
    run(
        repo,
        &[],
        &[
            "feature",
            "set",
            slug,
            "--acceptance",
            "observable behavior works",
            "--area",
            "active ownership",
        ],
    );
    run(repo, &[], &["feature", "reconcile", slug]);
    run(repo, &[], &["feature", "finalize", slug]);
}

#[test]
fn feature_gate_worktree_advisory_is_target_aware() {
    let temp = cards_repo("active-feature-worktree-advisory");
    let repo = temp.path();

    create_feature_contract(repo, "Primary Feature", "primary-feature");
    create_feature_contract(repo, "Other Feature", "other-feature");
    clear_runs(repo);
    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "peer-other",
        &[ownership_acquire_event(
            "peer-other",
            "other-feature",
            &recent,
        )],
    );
    let accept_primary = run_output(
        repo,
        &[("MAESTRO_SESSION_ID", "you-sess")],
        &[
            "feature",
            "accept",
            "primary-feature",
            "--qa",
            "none",
            "--reason",
            "integration coverage",
        ],
    );
    let unrelated_stderr =
        String::from_utf8(accept_primary.stderr).expect("invariant: stderr should be UTF-8");
    assert!(
        !unrelated_stderr.contains("[worktree]"),
        "unrelated fresh peer should not force a worktree nudge:\n{unrelated_stderr}"
    );

    create_feature_contract(repo, "Shared Feature", "shared-feature");
    clear_runs(repo);
    seed_run(
        repo,
        "peer-same",
        &[ownership_acquire_event(
            "peer-same",
            "shared-feature",
            &recent,
        )],
    );
    let accept_shared = run_output(
        repo,
        &[("MAESTRO_SESSION_ID", "you-sess")],
        &[
            "feature",
            "accept",
            "shared-feature",
            "--qa",
            "none",
            "--reason",
            "integration coverage",
        ],
    );
    let shared_stderr =
        String::from_utf8(accept_shared.stderr).expect("invariant: stderr should be UTF-8");
    assert!(
        shared_stderr.contains("[worktree] 1 fresh related session: shared-feature"),
        "same-feature fresh peer should still get a worktree nudge:\n{shared_stderr}"
    );
}

#[test]
fn feature_prepare_start_and_close_emit_ownership_lifecycle() {
    let temp = cards_repo("active-feature-lifecycle");
    let repo = temp.path();

    create_ready_feature(repo, "Prepared Feature", "prepared-feature");
    clear_runs(repo);
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "prepare-sess")],
        &[
            "feature",
            "prepare",
            "prepared-feature",
            "--task",
            "T1: Build child",
            "--check",
            "done",
            "--covers",
            "ac-1",
        ],
    );
    let prepared = run(repo, &[], &["active"]);
    assert!(
        line_with(&prepared, "prepare-sess").contains("[working]"),
        "feature prepare starts ownership\n{prepared}"
    );

    create_ready_feature(repo, "Closable Feature", "closable-feature");
    clear_runs(repo);
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "close-sess")],
        &["feature", "start", "closable-feature"],
    );
    let started = run(repo, &[], &["active"]);
    assert!(
        line_with(&started, "close-sess").contains("[working]"),
        "feature start starts ownership\n{started}"
    );
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "close-sess")],
        &[
            "feature",
            "verify",
            "closable-feature",
            "--prove",
            "ac-1",
            "--evidence",
            "observed in integration test",
            "--no-close",
        ],
    );
    write_valid_witness(&MaestroPaths::new(repo), "closable-feature");
    run(
        repo,
        &[("MAESTRO_SESSION_ID", "close-sess")],
        &["feature", "close", "closable-feature", "--outcome", "done"],
    );
    let closed = run(repo, &[], &["active", "--all"]);
    assert!(
        line_with(&closed, "close-sess").contains("[done"),
        "feature close releases ownership as done\n{closed}"
    );
}

#[test]
fn default_collapses_link_hint_and_creates_no_edge() {
    // bl-005: default output points at the peer's card without auto-linking; the
    // copy-pasteable command expansion lives behind `--connect` so the routine
    // board stays compact.
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
    assert!(
        out.contains("related:") && out.contains("maestro active --connect"),
        "compact related hint present\n{out}"
    );
    assert!(
        out.contains("peer-topic"),
        "hint names the peer card\n{out}"
    );
    assert!(
        !out.contains("maestro link add"),
        "default output should not print full link commands\n{out}"
    );

    let show = run(repo, &[], &["show", "peer-topic"]);
    assert!(
        !show.contains("related"),
        "active must not auto-create a related edge\n{show}"
    );
}

#[test]
fn relation_and_ownership_columns_reflect_current_caller_and_peers() {
    // RELATION names how the row relates to the current session. OWNERSHIP names
    // current ownership only. The footer still suggests `link add` only for
    // unlinked peers and names already-linked ones instead of re-suggesting them.
    let temp = cards_repo("active-relation-ownership-columns");
    let repo = temp.path();

    let a = create_id(repo, &["-t", "chore", "Card A"]);
    let b = create_id(repo, &["-t", "chore", "Card B"]);
    let c = create_id(repo, &["-t", "chore", "Card C"]);
    run(repo, &[], &["link", "add", &a, &b]);
    clear_runs(repo);

    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "you-sess",
        &[card_touch_event("you-sess", &a, &recent)],
    );
    seed_run(
        repo,
        "same-owner",
        &[ownership_acquire_event("same-owner", &a, &recent)],
    );
    seed_run(
        repo,
        "peer-b",
        &[ownership_acquire_event("peer-b", &b, &recent)],
    );
    seed_run(repo, "peer-c", &[card_touch_event("peer-c", &c, &recent)]);

    let out = run(repo, &[("MAESTRO_SESSION_ID", "you-sess")], &["active"]);

    let header = line_with(&out, "AGENT");
    assert!(
        header.contains("RELATION") && header.contains("OWNERSHIP") && !header.contains("LINK"),
        "active table should expose relation/ownership instead of LINK\n{out}"
    );
    assert!(
        !out.contains("(you)"),
        "active output must not render the old ambiguous you marker\n{out}"
    );
    let you = line_with(&out, "you-sess");
    assert!(
        you.contains("self") && you.contains("observer"),
        "own non-owner row reads self observer\n{out}"
    );
    assert!(
        line_with(&out, "peer-b").contains("linked") && line_with(&out, "peer-b").contains("owner"),
        "linked owner peer row reads linked owner\n{out}"
    );
    assert!(
        line_with(&out, "peer-c").contains("related")
            && !line_with(&out, "peer-c").contains("linked"),
        "unlinked peer row reads related, not linked\n{out}"
    );

    // Default footer: summarizes link/message opportunities without expanding
    // full command pairs.
    assert!(
        out.contains("related:") && out.contains("maestro active --connect"),
        "default footer points at the explicit command expansion\n{out}"
    );
    assert!(
        !out.contains(format!("maestro link add {a} {c}").as_str()),
        "default footer must not print full link commands\n{out}"
    );

    let connect = run(
        repo,
        &[("MAESTRO_SESSION_ID", "you-sess")],
        &["active", "--connect"],
    );
    // Expanded footer: link the unlinked peer (not the linked one), and offer a
    // ready `msg send` template addressing each peer by full card id.
    assert!(
        connect.contains(format!("maestro link add {a} {c}").as_str()),
        "expanded footer suggests linking the unlinked peer\n{connect}"
    );
    assert!(
        !connect.contains(format!("maestro link add {a} {b}").as_str()),
        "expanded footer must not re-suggest an already-linked peer\n{connect}"
    );
    assert!(
        connect.contains(format!("maestro msg send --from {a} {b} \"<text>\"").as_str()),
        "expanded footer offers a msg-send template addressing the already-linked peer by id\n{connect}"
    );
    assert!(
        connect.contains(format!("maestro msg send --from {a} {c} \"<text>\"").as_str()),
        "expanded footer offers a msg-send template for the unlinked peer too (link, then message)\n{connect}"
    );

    let all = run(
        repo,
        &[("MAESTRO_SESSION_ID", "you-sess")],
        &["active", "--all"],
    );
    let all_header = line_with(&all, "AGENT");
    assert!(
        all_header.contains("RELATION") && all_header.contains("OWNERSHIP"),
        "--all keeps the default table schema\n{all}"
    );
    assert!(
        line_with(&all, "same-owner").contains("same-card")
            && line_with(&all, "same-owner").contains("owner"),
        "same-card owner stays visible in --all with explicit relation/ownership\n{all}"
    );
}

#[test]
fn active_marks_current_owner_conflicts_without_overwriting_relation() {
    let temp = cards_repo("active-relation-ownership-conflict");
    let repo = temp.path();

    let card = create_id(repo, &["-t", "chore", "Contended card"]);
    clear_runs(repo);

    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "you-sess",
        &[ownership_acquire_event("you-sess", &card, &recent)],
    );
    seed_run(
        repo,
        "peer-owner",
        &[ownership_acquire_event("peer-owner", &card, &recent)],
    );

    let out = run(repo, &[("MAESTRO_SESSION_ID", "you-sess")], &["active"]);

    let you = line_with(&out, "you-sess");
    let peer = line_with(&out, "peer-owner");
    assert!(
        you.contains("self") && you.contains("owner") && you.contains("[CONFLICT]"),
        "self conflict row keeps relation and ownership\n{out}"
    );
    assert!(
        peer.contains("same-card") && peer.contains("owner") && peer.contains("[CONFLICT]"),
        "peer conflict row keeps relation and ownership\n{out}"
    );
}

#[test]
fn link_hint_drops_terminal_peers_but_keeps_already_linked() {
    // dec-terminal-card-link-msg-keep-the-live-5878: active must not suggest a
    // `link add` to a peer whose bound card is terminal (the guard would refuse
    // it); the peer's row + STATUS still show, and an already-linked terminal
    // peer still renders 'linked'.
    let temp = cards_repo("active-terminal-hint");
    let repo = temp.path();

    let a = create_id(repo, &["-t", "chore", "Card A"]); // running card, live
    let term = create_id(repo, &["-t", "chore", "Doomed"]); // unlinked terminal peer
    let ally = create_id(repo, &["-t", "chore", "Ally"]); // already-linked terminal peer
    run(repo, &[], &["link", "add", &a, &ally]);
    run(repo, &[], &["close", &term]);
    run(repo, &[], &["close", &ally]);
    clear_runs(repo);

    let recent = ts_minutes_ago(1);
    seed_run(
        repo,
        "you-sess",
        &[card_touch_event("you-sess", &a, &recent)],
    );
    seed_run(
        repo,
        "peer-term",
        &[card_touch_event("peer-term", &term, &recent)],
    );
    seed_run(
        repo,
        "peer-ally",
        &[card_touch_event("peer-ally", &ally, &recent)],
    );

    let out = run(repo, &[("MAESTRO_SESSION_ID", "you-sess")], &["active"]);

    // The terminal unlinked peer is shown (row + status) but never suggested.
    assert!(
        out.contains("peer-term"),
        "terminal peer row still shown\n{out}"
    );
    assert!(
        line_with(&out, "peer-term").contains("closed"),
        "terminal peer status still rendered\n{out}"
    );
    assert!(
        !out.contains(format!("maestro link add {a} {term}").as_str()),
        "no link-add suggestion for a terminal peer\n{out}"
    );

    let connect = run(
        repo,
        &[("MAESTRO_SESSION_ID", "you-sess")],
        &["active", "--connect"],
    );

    // The already-linked terminal peer still reads 'linked' and stays messageable.
    assert!(
        line_with(&out, "peer-ally").contains("linked"),
        "already-linked terminal peer still reads linked\n{out}"
    );
    assert!(
        connect.contains(format!("maestro msg send --from {a} {ally} \"<text>\"").as_str()),
        "already-linked terminal peer still offers a msg-send template\n{connect}"
    );
    // Every unlinked peer is terminal, so the link suggestion section is absent.
    assert!(
        !connect.contains("maestro link add"),
        "no link suggestion section when every unlinked peer is terminal\n{out}"
    );
}
