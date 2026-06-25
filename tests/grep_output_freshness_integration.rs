mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
}

fn maestro_with_env(args: &[&str], cwd: &Path, envs: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_maestro"));
    command.args(args).current_dir(cwd);
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .output()
        .expect("invariant: compiled maestro binary should be runnable in integration tests")
}

fn git(args: &[&str], cwd: &Path) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("invariant: git should be runnable in integration tests");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
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

fn search_repo(name: &str) -> (TestTempDir, String) {
    let temp = TestTempDir::new(name);
    let repo = temp.path();
    git(&["init", "-q"], repo);
    fs::create_dir_all(repo.join(".maestro/cards")).expect("cards dir should be creatable");
    fs::create_dir_all(repo.join("src")).expect("src dir should be creatable");
    fs::write(
        repo.join("src/agent_runtime.rs"),
        "pub struct TaskRegistry;\n\npub fn agent_runtime() {\n    println!(\"Agent runtime ready\");\n}\n",
    )
    .expect("source fixture should be writable");

    let id = stdout(
        maestro(
            &[
                "feature",
                "new",
                "Agent runtime decision",
                "--description",
                "We added agent runtime tracking so workflow proof stays durable.",
                "--id-only",
            ],
            repo,
        ),
        &["feature", "new"],
    )
    .trim()
    .to_string();

    (temp, id)
}

fn freshness_for<'a>(json: &'a Value, corpus: &str) -> &'a Value {
    json["freshness"]
        .as_array()
        .expect("freshness should be an array")
        .iter()
        .find(|item| item["corpus"] == corpus)
        .unwrap_or_else(|| panic!("freshness should include {corpus}: {json}"))
}

#[test]
fn grep_human_output_is_compact_and_grep_native() {
    let (temp, feature_id) = search_repo("grep-output-human");
    let repo = temp.path();

    let out = stdout(
        maestro(&["grep", "agent runtime"], repo),
        &["grep", "agent runtime"],
    );

    assert!(out.contains("1. memory:feature"), "{out}");
    assert!(out.contains(&feature_id), "{out}");
    assert!(out.contains("score="), "{out}");
    assert!(out.contains("Agent runtime decision"), "{out}");
    assert!(out.contains("open: maestro card show"), "{out}");
    assert!(out.contains("source:file"), "{out}");
    assert!(out.contains("src/agent_runtime.rs:"), "{out}");
}

#[test]
fn grep_json_exposes_freshness_error_and_span_contracts() {
    let (temp, _) = search_repo("grep-output-json");
    let repo = temp.path();

    let sym = stdout(
        maestro_with_env(
            &["grep", "--json", "sym:TaskRegistry", "corpus:source"],
            repo,
            &[("PATH", "")],
        ),
        &["grep", "--json", "sym:TaskRegistry corpus:source"],
    );
    let json: Value = serde_json::from_str(&sym).expect("grep output should be JSON");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "ctags_unavailable");
    assert_eq!(json["diagnostics"][0]["code"], "ctags_unavailable");
    assert_eq!(json["explicit_filter_overrides"], json!(["sym", "corpus"]));

    let out = stdout(
        maestro(&["grep", "--json", "agent_runtime corpus:source"], repo),
        &["grep", "--json", "agent_runtime corpus:source"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["schema"], "maestro.grep.v1");
    assert_eq!(json["ok"], true);
    assert_eq!(json["partial"], false);
    assert_eq!(json["hits"][0]["match_spans"][0]["line"], 3);
    assert_eq!(json["hits"][0]["match_spans"][0]["byte_start"], 7);
    assert!(json["hits"][0]["score_reasons"][0]["factor"].is_string());
    let source_freshness = freshness_for(&json, "source");
    assert_eq!(source_freshness["fresh"], true);
    assert_eq!(
        source_freshness["outline_extractor_version"],
        "maestro.outline.v1"
    );
    assert!(
        source_freshness["manifest_entries"].as_u64().unwrap() >= 1,
        "{source_freshness}"
    );
    assert!(source_freshness["skipped_files"].as_u64().is_some());

    let no_hit = stdout(
        maestro(&["grep", "--json", "not-present corpus:memory"], repo),
        &["grep", "--json", "not-present corpus:memory"],
    );
    let json: Value = serde_json::from_str(&no_hit).expect("grep output should be JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["partial"], false);
    assert_eq!(json["hits"].as_array().unwrap().len(), 0);
    assert!(json.get("error").is_none(), "{json}");
    assert_eq!(freshness_for(&json, "memory")["fresh"], true);

    let parse = stdout(
        maestro(&["grep", "--json", "/[/", "corpus:source"], repo),
        &["grep", "--json", "/[/ corpus:source"],
    );
    let json: Value = serde_json::from_str(&parse).expect("grep output should be JSON");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "parse_error");
    assert_eq!(json["diagnostics"][0]["code"], "parse_error");
}

#[test]
fn grep_repairs_stale_shards_and_lock_contention_is_retryable() {
    let (temp, _) = search_repo("grep-output-repair-lock");
    let repo = temp.path();
    let memory_shard = repo.join(".maestro/index/search/memory.shard");
    let lock_file = repo.join(".maestro/index/search/write.lock");

    stdout(
        maestro(&["grep", "--json", "agent corpus:memory"], repo),
        &["grep", "--json", "agent corpus:memory"],
    );
    fs::write(&memory_shard, "not a shard").expect("memory shard should be corruptible");

    let repaired = stdout(
        maestro(&["grep", "--json", "agent corpus:memory"], repo),
        &["grep", "--json", "agent corpus:memory"],
    );
    let json: Value = serde_json::from_str(&repaired).expect("grep output should be JSON");
    assert_eq!(json["ok"], true);
    assert!(
        json["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "memory_shard_repaired"),
        "{json}"
    );
    assert_eq!(freshness_for(&json, "memory")["repaired"], true);

    fs::write(&memory_shard, "not a shard again").expect("memory shard should be corruptible");
    fs::write(&lock_file, "held by test").expect("lock file should be writable");
    let locked = stdout(
        maestro(&["grep", "--json", "agent corpus:memory"], repo),
        &["grep", "--json", "agent corpus:memory"],
    );
    let json: Value = serde_json::from_str(&locked).expect("grep output should be JSON");
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "search_index_locked");
    assert_eq!(json["error"]["retryable"], true);
    assert_eq!(json["hits"].as_array().unwrap().len(), 0);
    fs::remove_file(lock_file).expect("test lock should be removable");
}

#[test]
fn grep_json_marks_partial_only_for_unavailable_non_contention_corpus() {
    let (temp, _) = search_repo("grep-output-partial");
    let repo = temp.path();
    let source_shard = repo.join(".maestro/index/search/source.shard");

    stdout(
        maestro(&["grep", "--json", "agent runtime"], repo),
        &["grep", "--json", "agent runtime"],
    );
    fs::remove_file(&source_shard).expect("source shard should exist");
    fs::create_dir(&source_shard).expect("directory should block source shard replacement");

    let out = stdout(
        maestro(&["grep", "--json", "agent runtime"], repo),
        &["grep", "--json", "agent runtime"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["partial"], true);
    assert!(
        json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|hit| hit["corpus"] == "memory"),
        "{json}"
    );
    assert!(
        json["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "source_shard_unavailable"),
        "{json}"
    );
}

#[test]
fn index_rebuild_scope_flags_follow_memory_source_cards_contract() {
    let (temp, _) = search_repo("grep-output-index-scope");
    let repo = temp.path();

    let conflict = stderr_failure(
        maestro(&["index", "rebuild", "--memory", "--source"], repo),
        &["index", "rebuild", "--memory", "--source"],
    );
    assert!(
        conflict.contains("cannot be used with") || conflict.contains("conflict"),
        "{conflict}"
    );

    let cards = stdout(
        maestro(&["index", "rebuild", "--cards"], repo),
        &["index", "rebuild", "--cards"],
    );
    assert!(
        cards.contains("cards compatibility index rebuilt"),
        "{cards}"
    );
    assert!(cards.contains("text index rebuilt"), "{cards}");
    assert!(cards.contains("memory shard rebuilt"), "{cards}");
    assert!(!cards.contains("source shard rebuilt"), "{cards}");
    assert!(repo.join(".maestro/index/text.json").exists());
    assert!(repo.join(".maestro/index/search/memory.shard").exists());

    let lock_file = repo.join(".maestro/index/search/write.lock");
    fs::write(&lock_file, "held by test").expect("lock file should be writable");
    let locked = stderr_failure(
        maestro(&["index", "rebuild", "--memory"], repo),
        &["index", "rebuild", "--memory"],
    );
    assert!(locked.contains("search_index_locked"), "{locked}");
    fs::remove_file(&lock_file).expect("test lock should be removable");

    let all = stdout(maestro(&["index", "rebuild"], repo), &["index", "rebuild"]);
    assert!(all.contains("cards compatibility index rebuilt"), "{all}");
    assert!(all.contains("memory shard rebuilt"), "{all}");
    assert!(all.contains("source shard rebuilt"), "{all}");
}
