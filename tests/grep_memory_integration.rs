mod support;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::process::Command;

use maestro::foundation::core::schema::RUN_EVIDENCE_SCHEMA_VERSION;
use serde_json::Value;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
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

fn stderr_failure(output: std::process::Output, args: &[&str]) -> String {
    assert!(
        !output.status.success(),
        "maestro {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stderr).expect("invariant: stderr should be UTF-8")
}

fn cards_repo(name: &str) -> TestTempDir {
    let temp = TestTempDir::new(name);
    fs::create_dir_all(temp.path().join(".maestro/cards"))
        .expect("invariant: cards dir should be creatable");
    temp
}

#[test]
fn grep_memory_json_returns_feature_hit_from_indexed_sidecar() {
    let temp = cards_repo("grep-memory-feature-json");
    let repo = temp.path();

    let id = stdout(
        maestro(
            &[
                "feature",
                "new",
                "Runtime Identity",
                "--description",
                "Agent runtime design record",
                "--id-only",
            ],
            repo,
        ),
        &["feature", "new"],
    )
    .trim()
    .to_string();
    stdout(
        maestro(
            &[
                "feature",
                "spec",
                &id,
                "--section",
                "Problem",
                "--append",
                "managed proof evidence should be searchable through memory grep",
            ],
            repo,
        ),
        &["feature", "spec"],
    );

    let out = stdout(
        maestro(
            &["grep", "--json", "proof corpus:memory type:feature"],
            repo,
        ),
        &["grep", "--json"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["schema"], "maestro.grep.v1");
    assert_eq!(json["ok"], true);
    assert_eq!(json["partial"], false);
    assert_eq!(json["hits"][0]["corpus"], "memory");
    assert_eq!(json["hits"][0]["kind"], "feature");
    assert_eq!(json["hits"][0]["id"], id);
    assert!(
        json["hits"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("proof")
    );
    assert!(repo.join(".maestro/index/search/memory.shard").exists());
}

#[cfg(unix)]
#[test]
fn grep_memory_ignores_symlinked_sidecars() {
    let temp = cards_repo("grep-memory-sidecar-symlink");
    let repo = temp.path();
    let id = stdout(
        maestro(
            &[
                "feature",
                "new",
                "Symlinked Sidecar",
                "--description",
                "feature with an unsafe sidecar",
                "--id-only",
            ],
            repo,
        ),
        &["feature", "new"],
    )
    .trim()
    .to_string();
    let secret = temp.path().join("outside-secret.txt");
    fs::write(&secret, "sidecar-secret-token").expect("secret fixture should be writable");
    let sidecar = repo.join(".maestro/cards").join(&id).join("spec.md");
    let _ = fs::remove_file(&sidecar);
    unix_fs::symlink(&secret, &sidecar).expect("sidecar symlink should be creatable");

    let out = stdout(
        maestro(
            &["grep", "--json", "sidecar-secret-token corpus:memory"],
            repo,
        ),
        &["grep", "--json", "sidecar-secret-token corpus:memory"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(
        json["hits"]
            .as_array()
            .expect("hits should be an array")
            .len(),
        0
    );
}

#[test]
fn grep_memory_handles_unicode_casefold_offsets() {
    let temp = cards_repo("grep-memory-unicode-offsets");
    let repo = temp.path();
    let id = stdout(
        maestro(
            &[
                "feature",
                "new",
                "Unicode Sidecar",
                "--description",
                "feature with non-ascii text",
                "--id-only",
            ],
            repo,
        ),
        &["feature", "new"],
    )
    .trim()
    .to_string();
    stdout(
        maestro(
            &[
                "feature",
                "spec",
                &id,
                "--section",
                "Problem",
                "--append",
                "İ proof marker remains searchable",
            ],
            repo,
        ),
        &["feature", "spec"],
    );

    let out = stdout(
        maestro(
            &["grep", "--json", "proof corpus:memory type:feature"],
            repo,
        ),
        &["grep", "--json", "proof corpus:memory type:feature"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["id"], id);
    assert!(
        json["hits"][0]["snippet"]
            .as_str()
            .expect("snippet should be text")
            .contains("proof")
    );
}

#[test]
fn grep_memory_indexes_progress_tasks_as_tasks() {
    let temp = cards_repo("grep-memory-progress-task");
    let repo = temp.path();
    let id = stdout(
        maestro(&["task", "add", "Progress grep task", "--id-only"], repo),
        &["task", "add", "--id-only"],
    )
    .trim()
    .to_string();

    let out = stdout(
        maestro(
            &["grep", "--json", "Progress corpus:memory type:task"],
            repo,
        ),
        &["grep", "--json", "Progress corpus:memory type:task"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert!(
        json["hits"]
            .as_array()
            .expect("hits should be an array")
            .iter()
            .any(|hit| hit["id"] == id && hit["kind"] == "task"),
        "progress task should be indexed as a memory task:\n{json:#}"
    );
}

#[test]
fn grep_memory_rejects_archived_filter_as_unsupported_atom() {
    let temp = cards_repo("grep-memory-unsupported-archived");
    let repo = temp.path();

    let err = stderr_failure(
        maestro(&["grep", "runtime archived:true"], repo),
        &["grep", "runtime archived:true"],
    );
    assert!(err.contains("unsupported_filter"), "{err}");
    assert!(err.contains("archived:"), "{err}");
}

#[test]
fn grep_memory_indexes_managed_run_evidence_but_not_raw_events() {
    let temp = cards_repo("grep-memory-run-evidence");
    let repo = temp.path();
    let run_dir = repo.join(".maestro/runs/session-demo");
    fs::create_dir_all(&run_dir).expect("run dir should be creatable");
    fs::write(
        run_dir.join("run_evidence.yaml"),
        format!(
            "schema_version: {RUN_EVIDENCE_SCHEMA_VERSION}\nsession_id: session-demo\nagent: codex-runtime\ntask_id: task-proof\nduration_seconds: 7\nhuman_interventions: 0\n"
        ),
    )
    .expect("run evidence should be writable");
    fs::write(
        run_dir.join("events.jsonl"),
        r#"{"event":"PostToolUse","payload":"raw-secret-hook-payload"}"#,
    )
    .expect("raw event log should be writable");

    let out = stdout(
        maestro(&["grep", "--json", "codex-runtime type:run_evidence"], repo),
        &["grep", "--json", "codex-runtime type:run_evidence"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["kind"], "run_evidence");
    assert_eq!(json["hits"][0]["parent"], "task-proof");

    let out = stdout(
        maestro(
            &["grep", "--json", "raw-secret-hook-payload corpus:memory"],
            repo,
        ),
        &["grep", "--json", "raw-secret-hook-payload corpus:memory"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(
        json["hits"]
            .as_array()
            .expect("hits should be an array")
            .len(),
        0
    );
}
