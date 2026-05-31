use crate::domain::run;

/// Shared lifecycle hook events installed and recorded for both Claude and Codex,
/// sourced from `embedded/hooks/events.yaml` via the Run contract.
pub fn shared_hook_events() -> &'static [String] {
    run::hook_event_contract().shared_events()
}

/// Encode a session id for use as a collision-resistant run directory name.
pub fn run_dir_name(session_id: &str) -> String {
    run::run_dir_name(session_id)
}
