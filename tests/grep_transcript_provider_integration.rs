use maestro::domain::search::transcript::{
    TranscriptProvider, parse_claude_transcript_jsonl, parse_factory_discovery_index,
    parse_factory_mission_state,
};

#[test]
fn claude_transcript_fixture_parses_messages_and_excludes_tool_payloads() {
    let fixture = r#"{"type":"user","timestamp":"2026-07-02T01:00:00Z","content":"Please inspect project state"}
{"type":"tool_use","timestamp":"2026-07-02T01:00:01Z","tool_name":"Bash","tool_input":{"cmd":"echo tool-input-secret"}}
{"type":"tool_result","timestamp":"2026-07-02T01:00:02Z","tool_name":"Bash","tool_input":{"cmd":"echo tool-input-secret"},"tool_output":"sk-proj-secret-output"}
{"type":"assistant","timestamp":"2026-07-02T01:00:03Z","content":"Project state inspected"}"#;

    let segments = parse_claude_transcript_jsonl(fixture, "ses_fixture", "/tmp/repo")
        .expect("Claude fixture should parse");

    assert_eq!(segments.len(), 4);
    assert!(
        segments
            .iter()
            .all(|segment| segment.provider == TranscriptProvider::Claude)
    );
    assert_eq!(segments[0].source_kind, "claude_user_message");
    assert_eq!(segments[0].text, "Please inspect project state");
    assert_eq!(segments[1].source_kind, "claude_tool_use");
    assert_eq!(segments[1].text, "tool use: Bash");
    assert!(segments[1].raw_tool_arguments.is_some());
    assert!(!segments[1].text.contains("tool-input-secret"));
    assert_eq!(segments[2].source_kind, "claude_tool_result");
    assert!(segments[2].raw_tool_output.is_some());
    assert!(!segments[2].text.contains("sk-proj-secret-output"));
    assert_eq!(segments[3].source_kind, "claude_assistant_message");
}

#[test]
fn factory_discovery_and_mission_fixtures_parse_without_scanning() {
    let discovery = r#"{
  "version": 1,
  "sessionsDir": "/Users/example/.factory/sessions",
  "entries": {
    "worker-1": {
      "id": "worker-1",
      "sessionPath": "/Users/example/.factory/sessions/project/worker-1.settings.json",
      "directoryPath": "/Users/example/.factory/sessions/project",
      "title": "Worker task",
      "sessionTitle": "Worker task",
      "cwd": "/tmp/repo",
      "decompMissionId": "mission-1",
      "decompSessionType": "worker",
      "messageCount": 12
    }
  }
}"#;
    let sessions =
        parse_factory_discovery_index(discovery).expect("discovery fixture should parse");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].provider, TranscriptProvider::Factory);
    assert_eq!(sessions[0].session_id, "worker-1");
    assert_eq!(sessions[0].workspace.as_deref(), Some("/tmp/repo"));
    assert_eq!(sessions[0].mission_id.as_deref(), Some("mission-1"));
    assert_eq!(
        sessions[0].session_path.as_deref(),
        Some("/Users/example/.factory/sessions/project/worker-1.settings.json")
    );
    assert_eq!(sessions[0].message_count, Some(12));

    let mission = r#"{
  "missionId": "mission-1",
  "baseSessionId": "base-1",
  "state": "active",
  "workingDirectory": "/tmp/repo",
  "workerSessionIds": ["worker-1", "worker-2"],
  "createdAt": "2026-07-02T01:00:00Z",
  "updatedAt": "2026-07-02T01:10:00Z"
}"#;
    let mission = parse_factory_mission_state(mission).expect("mission fixture should parse");

    assert_eq!(mission.provider, TranscriptProvider::Factory);
    assert_eq!(mission.mission_id, "mission-1");
    assert_eq!(mission.base_session_id.as_deref(), Some("base-1"));
    assert_eq!(mission.workspace.as_deref(), Some("/tmp/repo"));
    assert_eq!(mission.worker_session_ids, vec!["worker-1", "worker-2"]);
    assert_eq!(mission.state.as_deref(), Some("active"));
}
