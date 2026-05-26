use serde_json::Value;

/// Return true when a user prompt looks like a correction or interruption.
pub fn looks_like_correction(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let trimmed = lower.trim();
    let short_prompt = trimmed.split_whitespace().count() <= 4 && trimmed.len() <= 48;
    let correction_keyword = trimmed.contains("actually")
        || trimmed.contains("wait")
        || trimmed.contains(" no ")
        || trimmed.starts_with("no ")
        || trimmed == "no";

    short_prompt || (correction_keyword && trimmed.len() <= 160)
}

/// Extract a hook event kind from known event fields.
pub fn event_kind(event: &Value) -> String {
    string_field(event, "event_type")
        .or_else(|| string_field(event, "kind"))
        .or_else(|| string_field(event, "event"))
        .or_else(|| string_field(event, "type"))
        .unwrap_or_else(|| "<unknown>".to_string())
}

/// Extract user-visible prompt text from known event fields.
pub fn event_text(event: &Value) -> Option<String> {
    string_field(event, "message")
        .or_else(|| string_field(event, "prompt"))
        .or_else(|| string_field(event, "text"))
}

/// Extract a string field from a JSON event.
pub fn string_field(event: &Value, field: &str) -> Option<String> {
    event.get(field).and_then(Value::as_str).map(str::to_string)
}
