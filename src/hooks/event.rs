use serde_json::Value;

/// Shared hook events supported by Claude and Codex V1 hook config.
pub const SHARED_HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
];

/// Run directory used when a hook payload has no session id.
pub const UNATTRIBUTED_SESSION: &str = "unattributed";

/// Return whether an event type is accepted by `maestro hook record`.
pub fn is_accepted_event(event_type: &str) -> bool {
    SHARED_HOOK_EVENTS.contains(&event_type)
        || matches!(event_type, "SkillActivation" | "skill_activation")
}

/// Normalize event aliases into the persisted event type.
pub fn normalized_event_type(event_type: &str) -> &str {
    match event_type {
        "SkillActivation" => "skill_activation",
        other => other,
    }
}

/// Extract a string field from a JSON object.
pub fn string_field(source: &Value, field: &str) -> Option<String> {
    source
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Sanitize a session id for use as a run directory name.
pub fn run_dir_name(session_id: &str) -> String {
    let sanitized = session_id
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => character,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        UNATTRIBUTED_SESSION.to_string()
    } else {
        sanitized
    }
}
