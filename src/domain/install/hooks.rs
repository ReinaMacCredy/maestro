use serde_json::{json, Map, Value};

use crate::domain::run;

use super::InstallAgent;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedHookConfig {
    pub(crate) relative_path: &'static str,
    pub(crate) contents: Value,
    pub(crate) managed_keys: Vec<String>,
}

impl ManagedHookConfig {
    pub(crate) fn for_agent(agent: InstallAgent) -> Self {
        match agent {
            InstallAgent::Claude => Self {
                relative_path: ".claude/settings.local.json",
                contents: hook_json(HookConfigFlavor::Claude),
                managed_keys: vec!["hooks".to_string()],
            },
            InstallAgent::Codex => Self {
                relative_path: ".codex/hooks.json",
                contents: hook_json(HookConfigFlavor::Codex),
                managed_keys: vec!["hooks".to_string()],
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HookConfigFlavor {
    Claude,
    Codex,
}

fn hook_json(flavor: HookConfigFlavor) -> Value {
    let contract = run::hook_event_contract();
    let script = contract.script();

    let mut hooks = Map::new();
    for event in contract.shared_events() {
        // Each agent shell-evals the command string and resolves the repo root
        // its own way (Claude exports $CLAUDE_PROJECT_DIR; Codex may start in a
        // subdir, so it self-discovers via `git rev-parse`).
        let command = match flavor {
            HookConfigFlavor::Claude => json!({
                "type": "command",
                "command": format!("sh \"$CLAUDE_PROJECT_DIR/.maestro/hooks/{script}\""),
            }),
            HookConfigFlavor::Codex => json!({
                "type": "command",
                "command": format!("sh \"$(git rev-parse --show-toplevel)/.maestro/hooks/{script}\""),
                "timeout": contract.codex_timeout(),
            }),
        };
        hooks.insert(
            event.to_string(),
            json!([{"matcher": "*", "hooks": [command]}]),
        );
    }

    json!({ "hooks": hooks })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every event the installer writes must also be accepted by the recorder;
    /// otherwise an installed hook would fire `maestro hook record` on an event
    /// the recorder silently drops. This locks the single-source installer ⊆
    /// recorder invariant by driving the installer's real output path
    /// (`hook_json`) for both flavors, so it keeps holding if per-agent events
    /// are ever added to `resources/hooks/events.yaml`.
    #[test]
    fn installed_events_are_accepted_by_the_recorder() {
        for flavor in [HookConfigFlavor::Claude, HookConfigFlavor::Codex] {
            let json = hook_json(flavor);
            let hooks = json
                .get("hooks")
                .and_then(Value::as_object)
                .expect("hook_json always produces a hooks object");
            for event in hooks.keys() {
                assert!(
                    run::is_accepted_event(event),
                    "{flavor:?} installs {event} but the recorder rejects it"
                );
            }
        }
    }
}
