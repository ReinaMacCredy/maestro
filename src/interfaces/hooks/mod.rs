pub mod event;
pub mod record;

/// Read-only canonical hook rendering.
use crate::operations::adapters::{AdapterFrameV1, Stage10AdapterError};

#[allow(
    dead_code,
    reason = "the canonical rendering entrypoint is exercised by adapter contract tests"
)]
pub fn render(frame: &AdapterFrameV1) -> Result<String, Stage10AdapterError> {
    frame.validate()?;
    Ok(format!(
        "{{\"schema\":\"maestro.vnext.adapter-rendering.v1\",\"outcome\":\"{}\"}}\n",
        frame.outcome
    ))
}
