use anyhow::Result;

use crate::domain::run;
use crate::foundation::core::paths::MaestroPaths;

/// Aggregate `.maestro/runs/<session_id>/events.jsonl` into `run_evidence.yaml`.
pub fn write_for_session(paths: &MaestroPaths, session_id: &str) -> Result<()> {
    run::write_evidence_for_session(paths, session_id)
}
