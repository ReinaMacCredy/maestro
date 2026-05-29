mod friction;
mod summary;

pub use friction::looks_like_correction;
pub(crate) use summary::summarize_task_entries;
pub use summary::{
    load_run_evidence, render_summary, summarize, task_verification_durations, AgentSummary,
    MetricsSummary, RunEvidenceLoad, RunEvidenceRecord,
};
