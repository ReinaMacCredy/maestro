mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use maestro::domain::search::transcript::{
    TranscriptConsentRecord, TranscriptConsentScope, TranscriptProvider, TranscriptSegmentInput,
    TranscriptStore,
};
use serde_json::Value;
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

struct TranscriptFixture {
    temp: TestTempDir,
    _transcript_temp: TestTempDir,
    transcript_home: String,
}

impl TranscriptFixture {
    fn repo(&self) -> &Path {
        self.temp.path()
    }

    fn envs(&self) -> [(&str, &str); 1] {
        [("MAESTRO_TRANSCRIPT_HOME", self.transcript_home.as_str())]
    }
}

fn transcript_repo(name: &str) -> TranscriptFixture {
    let temp = TestTempDir::new(name);
    let transcript_temp = TestTempDir::new(&format!("{name}-global-transcripts"));
    let repo = temp.path();
    git(&["init", "-q"], repo);
    stdout(maestro(&["init", "--yes"], repo), &["init", "--yes"]);
    fs::create_dir_all(repo.join(".maestro/cards")).expect("cards dir should be creatable");
    fs::create_dir_all(repo.join("src")).expect("src dir should be creatable");
    fs::write(
        repo.join("src/runtime.rs"),
        "pub fn source_probe() { println!(\"ordinary source result\"); }\n",
    )
    .expect("source fixture should be writable");
    stdout(
        maestro(
            &[
                "feature",
                "new",
                "Ordinary source result",
                "--description",
                "ordinary memory result",
            ],
            repo,
        ),
        &["feature", "new", "Ordinary source result"],
    );

    let transcript_home = transcript_temp.path().join("transcripts");
    let store = TranscriptStore::new(&transcript_home);
    let workspace = repo.display().to_string();
    store
        .grant_consent(TranscriptConsentRecord {
            provider: TranscriptProvider::Codex,
            workspace: workspace.clone(),
            scope: TranscriptConsentScope::Project,
            granted: true,
            reason: Some("integration test consent".to_string()),
        })
        .expect("consent should persist");
    store
        .append_redacted_segment(TranscriptSegmentInput {
            provider: TranscriptProvider::Codex,
            session_id: "019f-transcript".to_string(),
            segment_id: "turn-1".to_string(),
            source_kind: "codex_assistant_message".to_string(),
            workspace,
            text: "transcript rocket launch password=hunter2".to_string(),
            raw_tool_arguments: Some("{\"api_key\":\"tool-secret\"}".to_string()),
            raw_tool_output: Some("tool output secret".to_string()),
            raw_reasoning: Some("private reasoning".to_string()),
            raw_environment: Some("OPENAI_API_KEY=env-secret".to_string()),
        })
        .expect("segment should persist");

    TranscriptFixture {
        temp,
        _transcript_temp: transcript_temp,
        transcript_home: transcript_home.display().to_string(),
    }
}

#[test]
fn explicit_transcript_corpus_returns_redacted_project_hits() {
    let fixture = transcript_repo("grep-transcript-wired");
    let out = stdout(
        maestro_with_env(
            &["grep", "--json", "rocket corpus:transcript provider:codex"],
            fixture.repo(),
            &fixture.envs(),
        ),
        &["grep", "--json", "rocket corpus:transcript provider:codex"],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");

    assert_eq!(json["ok"], true);
    assert_eq!(json["partial"], false);
    assert_eq!(json["intent"], "transcript");
    let hits = json["hits"].as_array().expect("hits should be an array");
    assert_eq!(hits.len(), 1, "{json}");
    let hit = &hits[0];
    assert_eq!(hit["corpus"], "transcript");
    assert_eq!(hit["provider"], "codex");
    assert_eq!(hit["session_id"], "019f-transcript");
    assert_eq!(hit["authority"], "transcript_context");
    assert_eq!(hit["proof_eligible"], false);
    assert_eq!(hit["source_kind"], "codex_assistant_message");
    assert!(
        hit["project_match_reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason == "workspace"),
        "{hit}"
    );
    assert!(hit["snippet"].as_str().unwrap().contains("rocket"));
    assert!(!hit["snippet"].as_str().unwrap().contains("hunter2"));
    assert_eq!(hit["redaction"]["state"], "redacted");

    let out = stdout(
        maestro_with_env(
            &[
                "grep",
                "--json",
                "rocket corpus:transcript include:transcript provider:codex",
            ],
            fixture.repo(),
            &fixture.envs(),
        ),
        &[
            "grep",
            "--json",
            "rocket corpus:transcript include:transcript provider:codex",
        ],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["hits"].as_array().unwrap().len(), 1, "{json}");
}

#[test]
fn include_transcript_is_opt_in_and_does_not_change_default_grep() {
    let fixture = transcript_repo("grep-transcript-include-wired");

    let default_out = stdout(
        maestro_with_env(
            &["grep", "--json", "rocket"],
            fixture.repo(),
            &fixture.envs(),
        ),
        &["grep", "--json", "rocket"],
    );
    let default_json: Value =
        serde_json::from_str(&default_out).expect("grep output should be JSON");
    assert!(
        default_json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .all(|hit| hit["corpus"] != "transcript"),
        "{default_json}"
    );

    let include_out = stdout(
        maestro_with_env(
            &["grep", "--json", "rocket include:transcript"],
            fixture.repo(),
            &fixture.envs(),
        ),
        &["grep", "--json", "rocket include:transcript"],
    );
    let include_json: Value =
        serde_json::from_str(&include_out).expect("grep output should be JSON");
    assert_eq!(include_json["ok"], true);
    assert!(
        include_json["hits"]
            .as_array()
            .unwrap()
            .iter()
            .any(|hit| hit["corpus"] == "transcript"),
        "{include_json}"
    );
}

#[test]
fn transcript_rebuild_writes_redacted_project_manifest() {
    let fixture = transcript_repo("grep-transcript-rebuild");
    let out = stdout(
        maestro_with_env(
            &["index", "rebuild", "--transcript"],
            fixture.repo(),
            &fixture.envs(),
        ),
        &["index", "rebuild", "--transcript"],
    );

    assert!(out.contains("transcript project view rebuilt"), "{out}");
    assert!(out.contains("sessions: 1, segments: 1"), "{out}");
    let manifest_path = fixture
        .repo()
        .join(".maestro/index/transcripts/manifest.json");
    let manifest = fs::read_to_string(manifest_path).expect("manifest should be written");
    assert!(manifest.contains("\"schema_version\""));
    assert!(manifest.contains("\"session_id\""));
    assert!(!manifest.contains("hunter2"));
    assert!(!manifest.contains("tool-secret"));
}

#[test]
fn doctor_reports_transcript_health_without_payload() {
    let fixture = transcript_repo("grep-transcript-doctor");
    stdout(
        maestro_with_env(
            &["index", "rebuild", "--transcript"],
            fixture.repo(),
            &fixture.envs(),
        ),
        &["index", "rebuild", "--transcript"],
    );

    let out = stdout(
        maestro_with_env(&["doctor"], fixture.repo(), &fixture.envs()),
        &["doctor"],
    );

    assert!(out.contains("check search-transcripts: ok"), "{out}");
    assert!(out.contains("1 session(s), 1 segment(s)"), "{out}");
    assert!(!out.contains("hunter2"));
    assert!(!out.contains("tool-secret"));
}

#[test]
fn session_grep_matches_explicit_transcript_session_filter() {
    let fixture = transcript_repo("grep-transcript-session-grep");
    let session_out = stdout(
        maestro_with_env(
            &["session", "grep", "--json", "019f-transcript", "rocket"],
            fixture.repo(),
            &fixture.envs(),
        ),
        &["session", "grep", "--json", "019f-transcript", "rocket"],
    );
    let explicit_out = stdout(
        maestro_with_env(
            &[
                "grep",
                "--json",
                "rocket corpus:transcript session:019f-transcript",
            ],
            fixture.repo(),
            &fixture.envs(),
        ),
        &[
            "grep",
            "--json",
            "rocket corpus:transcript session:019f-transcript",
        ],
    );
    let session_json: Value =
        serde_json::from_str(&session_out).expect("session grep should emit JSON");
    let explicit_json: Value = serde_json::from_str(&explicit_out).expect("grep should emit JSON");

    assert_eq!(session_json["ok"], true);
    assert_eq!(session_json["hits"], explicit_json["hits"]);
    assert_eq!(session_json["hits"][0]["provider"], "codex");
    assert_eq!(session_json["hits"][0]["session_id"], "019f-transcript");
    assert_eq!(session_json["hits"][0]["authority"], "transcript_context");
    assert_eq!(session_json["hits"][0]["proof_eligible"], false);
    assert!(!session_out.contains("hunter2"));
}

#[test]
fn explicit_transcript_session_filter_does_not_cross_project_boundary() {
    let fixture = transcript_repo("grep-transcript-project-boundary");
    let other = TestTempDir::new("grep-transcript-project-boundary-other");
    git(&["init", "-q"], other.path());
    stdout(
        maestro(&["init", "--yes"], other.path()),
        &["init", "--yes"],
    );

    let out = stdout(
        maestro_with_env(
            &[
                "grep",
                "--json",
                "rocket corpus:transcript session:019f-transcript",
            ],
            other.path(),
            &fixture.envs(),
        ),
        &[
            "grep",
            "--json",
            "rocket corpus:transcript session:019f-transcript",
        ],
    );
    let json: Value = serde_json::from_str(&out).expect("grep output should be JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["hits"].as_array().unwrap().len(), 0, "{json}");
}
