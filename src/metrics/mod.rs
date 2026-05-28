//! Compatibility shim for the legacy `crate::metrics` root.

pub mod friction {
    pub use crate::operations::metrics::{
        event_kind, event_text, looks_like_correction, string_field,
    };
}

pub mod summary {
    pub use crate::operations::metrics::{
        load_run_evidence, render_summary, summarize, task_verification_durations, AgentSummary,
        MetricsSummary, RunEvidenceLoad, RunEvidenceRecord,
    };
}
