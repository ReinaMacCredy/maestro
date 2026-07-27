pub mod mission_control;
pub mod task_list_watch;

use crate::operations::adapters::{AdapterFrameV1, Stage10AdapterError};

#[allow(
    dead_code,
    reason = "the canonical rendering entrypoint is exercised by adapter contract tests"
)]
pub fn render(frame: &AdapterFrameV1) -> Result<String, Stage10AdapterError> {
    frame.validate()?;
    Ok(format!("Maestro: {}\n{}", frame.outcome, frame.summary))
}
