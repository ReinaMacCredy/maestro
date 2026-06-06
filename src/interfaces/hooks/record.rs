use std::io::{self, Read};

use anyhow::{Context, Result};
use serde_json::Value;

use crate::domain::run;
use crate::foundation::core::paths::MaestroPaths;

pub(crate) fn record_stdin(paths: &MaestroPaths) -> Result<run::RecordOutcome> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .context("failed to read hook payload from stdin")?;
    record_payload(paths, &raw)
}

pub(crate) fn record_value(paths: &MaestroPaths, payload: &Value) -> Result<run::RecordOutcome> {
    let outcome = run::record_hook_event(paths, payload)?;
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

fn record_payload(paths: &MaestroPaths, raw: &str) -> Result<run::RecordOutcome> {
    let payload: Value = serde_json::from_str(raw).context("failed to parse hook payload JSON")?;
    record_value(paths, &payload)
}
