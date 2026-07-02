mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use maestro::domain::search::types::{
    MatchSpan, ScoreReason, SearchCorpus, SearchHit, TranscriptRedactionMetadata,
};
use serde_json::{Value, json};
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
                "Agent runtime transcript decision",
                "--description",
                "Agent runtime transcript search should stay explicit.",
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

#[test]
fn transcript_corpus_filters_parse_to_unavailable_corpus_diagnostic() {
    let (temp, _) = search_repo("grep-transcript-corpus");
    let repo = temp.path();

    let out = stdout(
        maestro(
            &[
                "grep",
                "--json",
                "handoff corpus:transcript provider:codex session:019f scope:global workspace:/tmp/maestro",
            ],
            repo,
        ),
        &["grep", "--json", "handoff corpus:transcript provider:codex"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");

    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "transcript_corpus_unavailable");
    assert_eq!(json["error"]["corpus"], "transcript");
    assert_eq!(
        json["diagnostics"][0]["code"],
        "transcript_corpus_unavailable"
    );
    assert_eq!(
        json["explicit_filter_overrides"],
        json!(["corpus", "provider", "session", "scope", "workspace"])
    );
}

#[test]
fn include_transcript_keeps_default_memory_source_and_reports_partial() {
    let (temp, _) = search_repo("grep-transcript-include");
    let repo = temp.path();

    let out = stdout(
        maestro(
            &["grep", "--json", "agent runtime include:transcript"],
            repo,
        ),
        &["grep", "--json", "agent runtime include:transcript"],
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
        json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|hit| hit["corpus"] == "source"),
        "{json}"
    );
    assert!(
        json["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "transcript_corpus_unavailable"
                    && diagnostic["corpus"] == "transcript"
            }),
        "{json}"
    );
    assert_eq!(json["explicit_filter_overrides"], json!(["include"]));
}

#[test]
fn transcript_only_filters_require_explicit_transcript_request() {
    let (temp, _) = search_repo("grep-transcript-filter-guard");
    let repo = temp.path();

    let out = stdout(
        maestro(&["grep", "--json", "agent provider:codex"], repo),
        &["grep", "--json", "agent provider:codex"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");

    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], "invalid_filter");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("require corpus:transcript or include:transcript"),
        "{json}"
    );
}

#[test]
fn transcript_hit_json_shape_carries_authority_metadata() {
    let hit = SearchHit {
        rank: 1,
        corpus: SearchCorpus::Transcript,
        kind: "message".to_string(),
        id: "codex:019f:assistant:7".to_string(),
        path: None,
        line: None,
        title: "assistant turn".to_string(),
        snippet: "redacted transcript snippet".to_string(),
        score: 0.91,
        score_reasons: vec![ScoreReason {
            factor: "transcript_literal".to_string(),
            value: 1.0,
            detail: "confirmed against redacted transcript segment".to_string(),
        }],
        opener: Some("maestro session show 019f --transcript".to_string()),
        archived: false,
        feature: None,
        parent: None,
        symbol_kind: None,
        match_spans: vec![MatchSpan::Memory {
            segment_id: "turn-7".to_string(),
            byte_start: 0,
            byte_end: 8,
        }],
        provider: Some("codex".to_string()),
        session_id: Some("019f".to_string()),
        authority: Some("transcript_context".to_string()),
        proof_eligible: Some(false),
        source_kind: Some("assistant_message".to_string()),
        project_match_reasons: vec!["workspace_root".to_string()],
        redaction: Some(TranscriptRedactionMetadata {
            state: "redacted".to_string(),
            excluded: vec!["secrets".to_string()],
        }),
    };

    let json = serde_json::to_value(hit).expect("hit should serialize");
    assert_eq!(json["corpus"], "transcript");
    assert_eq!(json["provider"], "codex");
    assert_eq!(json["session_id"], "019f");
    assert_eq!(json["authority"], "transcript_context");
    assert_eq!(json["proof_eligible"], false);
    assert_eq!(json["source_kind"], "assistant_message");
    assert_eq!(json["project_match_reasons"], json!(["workspace_root"]));
    assert_eq!(json["redaction"]["state"], "redacted");
    assert_eq!(json["redaction"]["excluded"], json!(["secrets"]));
}
