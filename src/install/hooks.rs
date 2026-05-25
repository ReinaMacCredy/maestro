use serde_json::{json, Map, Value};

use crate::hooks::event::SHARED_HOOK_EVENTS;
use crate::install::InstallAgent;

const HOOK_COMMAND: &str = "maestro hook record";

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
    let mut hooks = Map::new();
    for event in SHARED_HOOK_EVENTS {
        let command = match flavor {
            HookConfigFlavor::Claude => json!({"type": "command", "command": HOOK_COMMAND}),
            HookConfigFlavor::Codex => {
                json!({"type": "command", "command": HOOK_COMMAND, "timeout": 5})
            }
        };
        hooks.insert(
            event.to_string(),
            json!([{"matcher": "*", "hooks": [command]}]),
        );
    }

    json!({ "hooks": hooks })
}
