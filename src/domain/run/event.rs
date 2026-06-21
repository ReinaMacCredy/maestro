use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;
use serde_json::Value;

/// Single source for the lifecycle hook events Maestro installs and records,
/// shared by the installer (`install/hooks.rs`) and the recorder
/// (`is_accepted_event`). Editing the embedded file changes both.
static HOOK_CONFIG_YAML: &str = include_str!("../../../embedded/hooks/events.yaml");

/// Deserialized form of `embedded/hooks/events.yaml`.
#[derive(Debug, Deserialize)]
struct HookConfig {
    /// Events installed for every supported agent.
    events: Vec<String>,
    /// Hook script under `.maestro/hooks/` each installed hook entry runs.
    script: String,
    /// Codex-only knobs.
    codex: CodexConfig,
}

/// Codex-only hook knobs.
#[derive(Debug, Deserialize)]
struct CodexConfig {
    /// Per-hook timeout in seconds Codex applies to the recorder command.
    timeout: u64,
}

/// Parse the embedded hook config once. The file ships in the binary and is not
/// runtime user input, so a malformed file is a build-time bug, not a recoverable
/// error.
fn hook_config() -> &'static HookConfig {
    static CONFIG: OnceLock<HookConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
        serde_yaml::from_str(HOOK_CONFIG_YAML)
            .expect("invariant: embedded/hooks/events.yaml is valid")
    })
}

/// Run directory used when a hook payload has no session id.
pub(crate) const UNATTRIBUTED_SESSION: &str = "unattributed";

/// Accepted hook event contract used by hook installers and recorders.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HookEventContract;

impl HookEventContract {
    /// Hook event names installed and recorded for every supported agent.
    pub fn shared_events(self) -> &'static [String] {
        &hook_config().events
    }

    /// Hook script under `.maestro/hooks/` each installed hook entry runs. The
    /// installer wraps it in the per-agent invocation that resolves the repo root.
    pub fn script(self) -> &'static str {
        &hook_config().script
    }

    /// Per-hook timeout in seconds Codex applies to the recorder command.
    pub fn codex_timeout(self) -> u64 {
        hook_config().codex.timeout
    }
}

/// Return the accepted Run hook event contract.
pub fn hook_event_contract() -> HookEventContract {
    HookEventContract
}

/// Return whether an event type is accepted by `maestro hook record`.
pub(crate) fn is_accepted_event(event_type: &str) -> bool {
    hook_config()
        .events
        .iter()
        .any(|event| event.as_str() == event_type)
        // SkillActivation and card_touch are CLI-synthesized, not hook events, so
        // they are accepted here without being part of the installed contract.
        || matches!(
            event_type,
            "SkillActivation" | "skill_activation" | "card_touch" | "scope_declaration"
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_config_matches_the_shared_contract() {
        let contract = hook_event_contract();

        let events: Vec<&str> = contract
            .shared_events()
            .iter()
            .map(String::as_str)
            .collect();
        assert_eq!(
            events,
            [
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PermissionRequest",
                "PostToolUse",
                "Stop",
            ]
        );
        assert_eq!(contract.script(), "record.sh");
        assert_eq!(contract.codex_timeout(), 5);
    }

    #[test]
    fn is_accepted_event_accepts_contract_events_and_skill_aliases() {
        for event in hook_event_contract().shared_events() {
            assert!(is_accepted_event(event), "{event} should be accepted");
        }
        assert!(is_accepted_event("SkillActivation"));
        assert!(is_accepted_event("skill_activation"));
        assert!(is_accepted_event("card_touch"));
    }

    #[test]
    fn is_accepted_event_rejects_unknown_events() {
        // `Notification` is a real Claude event but Maestro neither installs nor
        // records it, so the recorder must reject it.
        assert!(!is_accepted_event("Notification"));
        assert!(!is_accepted_event("PreToolUseX"));
        assert!(!is_accepted_event(""));
    }
}
