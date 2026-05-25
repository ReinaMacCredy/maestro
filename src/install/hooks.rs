use serde_json::{json, Map, Value};

use crate::install::InstallAgent;

const HOOK_COMMAND: &str = "maestro hook record";
const HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
];

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
                contents: hook_json(false),
                managed_keys: vec!["hooks".to_string()],
            },
            InstallAgent::Codex => Self {
                relative_path: ".codex/hooks.json",
                contents: hook_json(true),
                managed_keys: vec!["hooks".to_string()],
            },
        }
    }
}

fn hook_json(with_timeout: bool) -> Value {
    let mut hooks = Map::new();
    for event in HOOK_EVENTS {
        let command = if with_timeout {
            json!({"type": "command", "command": HOOK_COMMAND, "timeout": 5})
        } else {
            json!({"type": "command", "command": HOOK_COMMAND})
        };
        hooks.insert(
            event.to_string(),
            json!([{"matcher": "*", "hooks": [command]}]),
        );
    }

    json!({ "hooks": hooks })
}
