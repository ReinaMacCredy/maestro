mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use support::TestTempDir;

fn maestro(args: &[&str], cwd: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(args)
        .current_dir(cwd)
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

fn ranking_repo(name: &str) -> (TestTempDir, String) {
    let temp = TestTempDir::new(name);
    let repo = temp.path();
    git(&["init", "-q"], repo);
    fs::create_dir_all(repo.join(".maestro/cards")).expect("cards dir should be creatable");
    fs::create_dir_all(repo.join("src")).expect("src dir should be creatable");
    fs::write(
        repo.join("src/agent_runtime.rs"),
        "pub fn agent_runtime() {\n    // agent runtime source path\n}\n",
    )
    .expect("source fixture should be writable");

    let feature_id = stdout(
        maestro(
            &[
                "feature",
                "new",
                "Agent Runtime Decision",
                "--description",
                "Decision record: we added agent runtime for durable proof history and workflow notes.",
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
                &feature_id,
                "--section",
                "Decision",
                "--append",
                "The agent runtime was added because proof history and workflow notes need durable Maestro memory.",
            ],
            repo,
        ),
        &["feature", "spec"],
    );
    stdout(
        maestro(&["index", "rebuild", "--memory"], repo),
        &["index", "rebuild", "--memory"],
    );
    stdout(
        maestro(&["index", "rebuild", "--source"], repo),
        &["index", "rebuild", "--source"],
    );
    (temp, feature_id)
}

#[test]
fn grep_ranking_uses_symbolic_intent_and_exactness() {
    let (temp, feature_id) = ranking_repo("grep-ranking-intent");
    let repo = temp.path();

    let why = stdout(
        maestro(&["grep", "--json", "why did we add agent runtime"], repo),
        &["grep", "--json", "why did we add agent runtime"],
    );
    let json: Value = serde_json::from_str(&why).expect("grep output should be JSON");
    assert_eq!(json["intent"], "memory");
    assert!(reason_contains(&json, "natural-language"), "{json:#}");
    assert_eq!(json["hits"][0]["corpus"], "memory");
    assert!(hit_reason_contains(&json["hits"][0], "artifact_type"));
    assert!(hit_reason_contains(&json["hits"][0], "intent_boost"));

    let code = stdout(
        maestro(&["grep", "--json", "agent_runtime"], repo),
        &["grep", "--json", "agent_runtime"],
    );
    let json: Value = serde_json::from_str(&code).expect("grep output should be JSON");
    assert_eq!(json["intent"], "source");
    assert!(reason_contains(&json, "code-shaped"), "{json:#}");
    assert_eq!(json["hits"][0]["corpus"], "source");

    let mixed = stdout(
        maestro(&["grep", "--json", "agent runtime"], repo),
        &["grep", "--json", "agent runtime"],
    );
    let json: Value = serde_json::from_str(&mixed).expect("grep output should be JSON");
    assert_eq!(json["intent"], "ambiguous");
    assert_eq!(json["hits"][0]["corpus"], "memory");
    assert!(
        json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|hit| hit["corpus"] == "source")
    );

    let source_only = stdout(
        maestro(&["grep", "--json", "agent runtime corpus:source"], repo),
        &["grep", "--json", "agent runtime corpus:source"],
    );
    let json: Value = serde_json::from_str(&source_only).expect("grep output should be JSON");
    assert_eq!(json["intent"], "source");
    assert_eq!(json["explicit_filter_overrides"][0], "corpus");
    assert!(
        json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .all(|hit| hit["corpus"] == "source")
    );

    let exact_id = stdout(
        maestro(&["grep", "--json", &feature_id], repo),
        &["grep", "--json", "feature id"],
    );
    let json: Value = serde_json::from_str(&exact_id).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["id"], feature_id);
    assert!(hit_reason_contains(&json["hits"][0], "exact_id"));

    let exact_path = stdout(
        maestro(&["grep", "--json", "src/agent_runtime.rs"], repo),
        &["grep", "--json", "path"],
    );
    let json: Value = serde_json::from_str(&exact_path).expect("grep output should be JSON");
    assert_eq!(json["hits"][0]["corpus"], "source");
    assert_eq!(json["hits"][0]["path"], "src/agent_runtime.rs");
    assert!(hit_reason_contains(&json["hits"][0], "exact_path"));
}

fn reason_contains(json: &Value, needle: &str) -> bool {
    json["intent_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason.as_str().unwrap().contains(needle))
}

fn hit_reason_contains(hit: &Value, needle: &str) -> bool {
    hit["score_reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason["factor"].as_str().unwrap().contains(needle))
}
