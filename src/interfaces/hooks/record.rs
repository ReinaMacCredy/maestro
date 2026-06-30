use std::io::{self, IsTerminal, Read};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::domain::run;
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::session::agent_runtime_from_env;

pub(crate) fn optional_stdin_payload() -> Result<Option<Value>> {
    let mut stdin = io::stdin();
    if stdin.is_terminal() {
        return Ok(None);
    }

    let mut raw = String::new();
    stdin
        .read_to_string(&mut raw)
        .context("failed to read hook payload from stdin")?;
    if raw.trim().is_empty() {
        return Ok(None);
    }

    let payload = serde_json::from_str(&raw).context("failed to parse hook payload JSON")?;
    Ok(Some(payload))
}

pub(crate) fn payload_session_id(payload: &Value) -> Option<String> {
    payload
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn record_value(paths: &MaestroPaths, payload: &Value) -> Result<run::RecordOutcome> {
    let outcome = run::record_hook_event(paths, payload, agent_runtime_from_env())?;
    if let run::RecordOutcome::Ignored { event_type } = &outcome {
        match event_type {
            Some(event_type) => {
                eprintln!("maestro hook record: ignored unrecognized event type `{event_type}`");
            }
            None => {
                eprintln!("maestro hook record: ignored payload with no recognizable event type");
            }
        }
    }
    Ok(outcome)
}
