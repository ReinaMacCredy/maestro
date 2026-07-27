#![allow(
    dead_code,
    reason = "Stage 10 rendering is dormant until cross-stage integration"
)]

//! Read-only hook rendering.

use crate::operations::vnext::adapters::{AdapterFrameV1, Stage10AdapterError};

pub fn render(frame: &AdapterFrameV1) -> Result<String, Stage10AdapterError> {
    frame.validate()?;
    Ok(format!(
        "{{\"schema\":\"maestro.vnext.adapter-rendering.v1\",\"outcome\":\"{}\"}}\n",
        frame.outcome
    ))
}
