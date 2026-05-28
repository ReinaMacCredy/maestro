mod friction;
mod summary;

pub use friction::{event_kind, event_text, looks_like_correction, string_field};
pub(crate) use summary::summarize_task_entries;
pub use summary::{
    load_run_evidence, render_summary, summarize, task_verification_durations, AgentSummary,
    MetricsSummary, RunEvidenceLoad, RunEvidenceRecord,
};
