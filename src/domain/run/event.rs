use std::path::Path;

use serde_json::Value;

/// The lifecycle events Maestro installs and records, identical for Claude and
/// Codex today.
///
/// Split into per-agent sets only if Maestro ever installs or consumes an event
/// valid for one agent but not the other.
pub(crate) const SHARED_HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
];

/// Run directory used when a hook payload has no session id.
pub(crate) const UNATTRIBUTED_SESSION: &str = "unattributed";

/// Accepted hook event contract used by hook installers and recorders.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HookEventContract;

impl HookEventContract {
    /// Hook event names shared across supported agents.
    pub fn shared_events(self) -> [&'static str; 6] {
        SHARED_HOOK_EVENTS
    }

    /// Return whether an event type is accepted by `maestro hook record`.
    pub fn accepts(self, event_type: &str) -> bool {
        is_accepted_event(event_type)
    }
}

/// Return the accepted Run hook event contract.
pub fn hook_event_contract() -> HookEventContract {
    HookEventContract
}

/// Return whether an event type is accepted by `maestro hook record`.
pub(crate) fn is_accepted_event(event_type: &str) -> bool {
    SHARED_HOOK_EVENTS.contains(&event_type)
        || matches!(event_type, "SkillActivation" | "skill_activation")
}

/// Normalize event aliases into the persisted event type.
pub(crate) fn normalized_event_type(event_type: &str) -> &str {
    match event_type {
        "SkillActivation" => "skill_activation",
        other => other,
    }
}

/// Extract a string field from a JSON object.
pub(crate) fn string_field(source: &Value, field: &str) -> Option<String> {
    source
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Encode a session id for use as a collision-resistant run directory name.
pub(crate) fn run_dir_name(session_id: &str) -> String {
    if session_id.is_empty() {
        return UNATTRIBUTED_SESSION.to_string();
    }

    let mut encoded = String::with_capacity(session_id.len());
    for byte in session_id.bytes() {
        if is_literal_run_dir_byte(byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    if encoded == UNATTRIBUTED_SESSION {
        "%75nattributed".to_string()
    } else {
        encoded
    }
}

fn is_literal_run_dir_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.'
    )
}

pub(crate) fn logical_session_id_from_run_path(path: &Path) -> String {
    let Some(name) = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
    else {
        return UNATTRIBUTED_SESSION.to_string();
    };
    decode_run_dir_name(name).unwrap_or_else(|| name.to_string())
}

fn decode_run_dir_name(name: &str) -> Option<String> {
    let mut decoded = Vec::with_capacity(name.len());
    let bytes = name.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let high = bytes.get(index + 1).copied().and_then(hex_value)?;
        let low = bytes.get(index + 2).copied().and_then(hex_value)?;
        decoded.push((high << 4) | low);
        index += 3;
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
