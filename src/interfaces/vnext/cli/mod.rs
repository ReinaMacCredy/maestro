//! Stage-6 CLI and JSON transport facade.

mod adapter;

#[allow(
    unused_imports,
    reason = "Stage 6 preserves its frozen candidate facade before root integration"
)]
pub(crate) use adapter::{Stage6CliOutputV1, run};
