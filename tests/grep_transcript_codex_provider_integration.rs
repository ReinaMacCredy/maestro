use maestro::domain::search::transcript::{TranscriptProvider, parse_codex_transcript_jsonl};

#[test]
fn codex_transcript_fixture_parses_messages_and_tool_calls() {
    let fixture = r##"{"type":"session_meta","payload":{"id":"codex-session","cwd":"/tmp/repo","workspace_roots":["/tmp/repo"]}}
{"type":"response_item","timestamp":"2026-07-02T01:00:00Z","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Search the transcript corpus"}]}}
{"type":"response_item","timestamp":"2026-07-02T01:00:01Z","payload":{"type":"function_call","name":"shell","arguments":"{\"cmd\":\"echo tool-input-secret\"}"}}
{"type":"response_item","timestamp":"2026-07-02T01:00:02Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Transcript corpus found"}]}}"##;

    let parsed =
        parse_codex_transcript_jsonl(fixture, "fallback-session").expect("Codex fixture parses");

    assert_eq!(parsed.session_id, "codex-session");
    assert_eq!(parsed.cwd.as_deref(), Some("/tmp/repo"));
    assert_eq!(parsed.workspace_roots, vec!["/tmp/repo"]);
    assert_eq!(parsed.segments.len(), 3);
    assert!(
        parsed
            .segments
            .iter()
            .all(|segment| segment.provider == TranscriptProvider::Codex)
    );
    assert_eq!(parsed.segments[0].source_kind, "codex_user_message");
    assert_eq!(parsed.segments[0].text, "Search the transcript corpus");
    assert_eq!(parsed.segments[1].source_kind, "codex_tool_call");
    assert_eq!(parsed.segments[1].text, "tool call: shell");
    assert!(parsed.segments[1].raw_tool_arguments.is_some());
    assert!(!parsed.segments[1].text.contains("tool-input-secret"));
    assert_eq!(parsed.segments[2].source_kind, "codex_assistant_message");
}

#[test]
fn codex_project_matching_excludes_ambiguous_sessions_by_default() {
    let matched = parse_codex_transcript_jsonl(
        r#"{"type":"session_meta","payload":{"id":"matched","cwd":"/tmp/repo/sub","workspace_roots":["/tmp/repo"]}}"#,
        "matched",
    )
    .expect("matched fixture parses");
    let reasons = matched.project_match_reasons("/tmp/repo", None, false);
    assert!(
        reasons.contains(&"workspace_root".to_string()),
        "{reasons:?}"
    );
    assert!(matched.visible_in_project_by_default("/tmp/repo"));

    let ambiguous =
        parse_codex_transcript_jsonl(r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"no project metadata"}]}}"#, "ambiguous")
            .expect("ambiguous fixture parses");
    assert!(
        ambiguous
            .project_match_reasons("/tmp/repo", None, false)
            .is_empty()
    );
    assert!(!ambiguous.visible_in_project_by_default("/tmp/repo"));

    let explicit = ambiguous.project_match_reasons("/tmp/repo", Some("ambiguous"), false);
    assert_eq!(explicit, vec!["explicit_session"]);
    let global = ambiguous.project_match_reasons("/tmp/repo", None, true);
    assert_eq!(global, vec!["scope_global"]);
}
