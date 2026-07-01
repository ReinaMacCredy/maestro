//! Integration coverage for Codex-thread-primary delivery on `maestro msg send`.
//! The test fakes `codex app-server proxy` on PATH so the contract is stable
//! without requiring a live Desktop app-server socket.

pub mod card_support;
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use card_support::{cards_repo, id_by_title};
use maestro::foundation::core::time::format_utc_seconds_rfc3339_millis;
use serde_json::Value;

fn maestro(repo: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command
        .args(args)
        .current_dir(repo)
        .env("MAESTRO_AGENT", "codex")
        .env("MAESTRO_AUTO_UPDATE", "0")
        .env_remove("MAESTRO_SESSION_ID")
        .env_remove("MAESTRO_RUN_ID")
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CODEX_CLI")
        .env_remove("CODEX_SANDBOX")
        .env_remove("CLAUDE_SESSION_ID")
        .env_remove("CLAUDECODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID");
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

fn clear_runs(repo: &Path) {
    let runs = repo.join(".maestro/runs");
    if runs.exists() {
        fs::remove_dir_all(runs).expect("invariant: runs dir should be removable");
    }
}

fn ts_minutes_ago(minutes: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("invariant: clock is after the Unix epoch")
        .as_secs();
    format_utc_seconds_rfc3339_millis(now - minutes * 60)
}

fn seed_codex_touch(repo: &Path, session: &str, card: &str) {
    let run_dir = repo.join(".maestro/runs").join(session);
    fs::create_dir_all(&run_dir).expect("invariant: run dir should be creatable");
    fs::write(
        run_dir.join("events.jsonl"),
        format!(
            "{{\"event_type\":\"card_touch\",\"session_id\":\"{session}\",\"agent_runtime\":\"codex\",\"card_id\":\"{card}\",\"ts\":\"{}\"}}\n",
            ts_minutes_ago(1)
        ),
    )
    .expect("invariant: event log fixture should be writable");
}

fn fake_codex(repo: &Path, exit_code: i32) -> (PathBuf, PathBuf) {
    let bin_dir = repo.join("fake-bin");
    fs::create_dir_all(&bin_dir).expect("invariant: fake bin dir should be creatable");
    let log = repo.join("codex-proxy-request.json");
    let script = bin_dir.join("codex");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nif [ \"$1\" = app-server ] && [ \"$2\" = proxy ]; then\n  payload=$(cat)\n  printf '%s\\n' \"$payload\" > \"$MAESTRO_FAKE_CODEX_PROXY_LOG\"\n  if [ {exit_code} -eq 0 ]; then\n    printf '{{\"id\":\"maestro-msg-send\",\"result\":{{}}}}\\n'\n  else\n    printf 'fake proxy failure\\n' >&2\n  fi\n  exit {exit_code}\nfi\nexit 127\n"
        ),
    )
    .expect("invariant: fake codex should be writable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script)
            .expect("invariant: fake codex metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions)
            .expect("invariant: fake codex should be executable");
    }
    (bin_dir, log)
}

fn prepend_path(bin_dir: &Path) -> String {
    let path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{path}", bin_dir.display())
}

fn setup_linked_cards(repo: &Path) -> (String, String) {
    run(repo, &[], &["create", "-t", "chore", "Alpha"]);
    let alpha = id_by_title(repo, "Alpha");
    run(repo, &[], &["create", "-t", "chore", "Bravo"]);
    let bravo = id_by_title(repo, "Bravo");
    run(repo, &[], &["link", "add", &alpha, &bravo]);
    clear_runs(repo);
    run(
        repo,
        &[("CODEX_THREAD_ID", "source-thread")],
        &["note", &alpha, "bind"],
    );
    seed_codex_touch(repo, "target-thread", &bravo);
    (alpha, bravo)
}

#[test]
fn codex_to_codex_msg_send_uses_target_thread_without_unread_local_duplicate() {
    let temp = cards_repo("msg-codex-direct");
    let repo = temp.path();
    let (_alpha, bravo) = setup_linked_cards(repo);
    let (bin_dir, log) = fake_codex(repo, 0);
    let path = prepend_path(&bin_dir);

    let sent = run(
        repo,
        &[
            ("CODEX_THREAD_ID", "source-thread"),
            ("PATH", path.as_str()),
            ("MAESTRO_FAKE_CODEX_PROXY_LOG", log.to_str().unwrap()),
        ],
        &["msg", "send", &bravo, "please review the final scope"],
    );
    assert!(
        sent.contains("via codex thread target-thread") && sent.contains("receipt "),
        "send should prefer target Codex thread and name the receipt:\n{sent}"
    );

    let request: Value = serde_json::from_str(
        &fs::read_to_string(&log).expect("invariant: fake codex should record request"),
    )
    .expect("invariant: proxy request should be JSON");
    assert_eq!(request["method"], "turn/start");
    assert_eq!(request["params"]["threadId"], "target-thread");
    assert!(
        request["params"]["input"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("please review the final scope"),
        "target turn should include the original message:\n{request}"
    );

    let target_read = run(
        repo,
        &[("CODEX_THREAD_ID", "target-thread")],
        &["msg", "read"],
    );
    assert!(
        !target_read.contains("please review the final scope"),
        "direct delivery receipt must not become a duplicate local unread message:\n{target_read}"
    );

    let ledger = run(
        repo,
        &[("CODEX_THREAD_ID", "source-thread")],
        &["msg", "list", &bravo],
    );
    assert!(
        ledger.contains("delivery receipts:")
            && ledger.contains("codex_thread")
            && ledger.contains("delivered")
            && ledger.contains("please review the final scope"),
        "msg list should expose the durable delivery receipt:\n{ledger}"
    );
}

#[test]
fn codex_thread_failure_falls_back_to_local_channel_and_records_receipt() {
    let temp = cards_repo("msg-codex-fallback");
    let repo = temp.path();
    let (_alpha, bravo) = setup_linked_cards(repo);
    let (bin_dir, log) = fake_codex(repo, 1);
    let path = prepend_path(&bin_dir);

    let sent = run(
        repo,
        &[
            ("CODEX_THREAD_ID", "source-thread"),
            ("PATH", path.as_str()),
            ("MAESTRO_FAKE_CODEX_PROXY_LOG", log.to_str().unwrap()),
        ],
        &["msg", "send", &bravo, "fallback this message"],
    );
    assert!(
        sent.contains("saved local fallback") && sent.contains("fake proxy failure"),
        "send should keep delivery by falling back to the local channel:\n{sent}"
    );

    let target_read = run(
        repo,
        &[("CODEX_THREAD_ID", "target-thread")],
        &["msg", "read"],
    );
    assert!(
        target_read.contains("fallback this message"),
        "fallback local message should be unread for the target:\n{target_read}"
    );

    let ledger = run(
        repo,
        &[("CODEX_THREAD_ID", "source-thread")],
        &["msg", "list", &bravo],
    );
    assert!(
        ledger.contains("fallback_local") && ledger.contains("fallback this message"),
        "fallback should be recorded in the receipt ledger:\n{ledger}"
    );
}
