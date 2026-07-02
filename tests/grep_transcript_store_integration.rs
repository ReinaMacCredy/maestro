mod support;

use std::fs;
use std::path::Path;

use maestro::domain::search::transcript::{
    TranscriptConsentRecord, TranscriptConsentScope, TranscriptProvider, TranscriptSegmentInput,
    TranscriptStore, resolve_transcript_home,
};
use support::TestTempDir;

#[test]
fn transcript_home_uses_env_override_or_global_maestro_default() {
    assert_eq!(
        resolve_transcript_home(
            Some(Path::new("/tmp/custom-transcripts")),
            Some(Path::new("/Users/example"))
        )
        .as_deref(),
        Some(Path::new("/tmp/custom-transcripts"))
    );
    assert_eq!(
        resolve_transcript_home(None, Some(Path::new("/Users/example"))).as_deref(),
        Some(Path::new("/Users/example/.maestro/transcripts"))
    );
    assert!(resolve_transcript_home(None, None).is_none());
}

#[test]
fn transcript_store_requires_consent_and_persists_only_redacted_segments() {
    let temp = TestTempDir::new("grep-transcript-store");
    let store = TranscriptStore::new(temp.path().join("transcripts"));
    let workspace = temp.path().join("repo").display().to_string();

    let input = TranscriptSegmentInput {
        provider: TranscriptProvider::Codex,
        session_id: "019f-store".to_string(),
        segment_id: "turn-7".to_string(),
        source_kind: "assistant_message".to_string(),
        workspace: workspace.clone(),
        text: "deploy with sk-proj-secret123 and password=hunter2".to_string(),
        raw_tool_arguments: Some("{\"secret\":\"tool-input-secret\"}".to_string()),
        raw_tool_output: Some("raw tool output with api_key=output-secret".to_string()),
        raw_reasoning: Some("private chain of thought".to_string()),
        raw_environment: Some("OPENAI_API_KEY=env-secret".to_string()),
    };

    let without_consent = store.append_redacted_segment(input.clone());
    assert!(
        without_consent.is_err(),
        "segment writes must require consent"
    );

    store
        .grant_consent(TranscriptConsentRecord {
            provider: TranscriptProvider::Codex,
            workspace: workspace.clone(),
            scope: TranscriptConsentScope::Project,
            granted: true,
            reason: Some("fixture consent".to_string()),
        })
        .expect("consent should persist");
    assert!(store.has_consent(TranscriptProvider::Codex, &workspace));

    let stored = store
        .append_redacted_segment(input)
        .expect("consented segment should persist");
    assert_eq!(stored.provider, TranscriptProvider::Codex);
    assert_eq!(stored.session_id, "019f-store");
    assert_eq!(stored.authority, "transcript_context");
    assert!(!stored.proof_eligible);
    assert!(stored.redacted_text.contains("[REDACTED]"));
    assert!(!stored.redacted_text.contains("sk-proj-secret123"));
    assert!(!stored.redacted_text.contains("hunter2"));
    assert!(
        stored
            .excluded_fields
            .contains(&"raw_tool_arguments".to_string())
    );
    assert!(
        stored
            .excluded_fields
            .contains(&"raw_tool_output".to_string())
    );
    assert!(
        stored
            .excluded_fields
            .contains(&"raw_reasoning".to_string())
    );
    assert!(
        stored
            .excluded_fields
            .contains(&"raw_environment".to_string())
    );

    let persisted = fs::read_to_string(store.segment_file(TranscriptProvider::Codex, "019f-store"))
        .expect("segment jsonl should be readable");
    assert!(persisted.contains("\"redacted_text\""));
    assert!(persisted.contains("\"authority\":\"transcript_context\""));
    assert!(persisted.contains("\"proof_eligible\":false"));
    assert!(!persisted.contains("tool-input-secret"));
    assert!(!persisted.contains("output-secret"));
    assert!(!persisted.contains("private chain of thought"));
    assert!(!persisted.contains("env-secret"));
    assert!(!persisted.contains("sk-proj-secret123"));
    assert!(!persisted.contains("hunter2"));
}
