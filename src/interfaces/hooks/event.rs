use serde_json::Value;

use crate::domain::run;

/// Shared lifecycle hook events installed and recorded for both Claude and Codex,
/// sourced from `resources/hooks/events.yaml` via the Run contract.
pub fn shared_hook_events() -> &'static [String] {
    run::hook_event_contract().shared_events()
}

/// Run directory used when a hook payload has no session id.
pub const UNATTRIBUTED_SESSION: &str = run::UNATTRIBUTED_SESSION;

/// Return whether an event type is accepted by `maestro hook record`.
pub fn is_accepted_event(event_type: &str) -> bool {
    run::is_accepted_event(event_type)
}

/// Normalize event aliases into the persisted event type.
pub fn normalized_event_type(event_type: &str) -> &str {
    run::normalized_event_type(event_type)
}

/// Extract a string field from a JSON object.
pub fn string_field(source: &Value, field: &str) -> Option<String> {
    run::string_field(source, field)
}

/// Encode a session id for use as a collision-resistant run directory name.
pub fn run_dir_name(session_id: &str) -> String {
    run::run_dir_name(session_id)
}
