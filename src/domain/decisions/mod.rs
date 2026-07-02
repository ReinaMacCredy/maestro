//! Decision aggregate facade.

pub(crate) mod cards;
pub(crate) mod create;
pub mod decision_set;
pub mod query;
pub mod schema;
pub mod template;

pub use create::{
    DecisionLockReport, DecisionWriteReport, LockInputs, SupersedeInputs, create_locked,
    create_open, empty_store_yaml, lock, supersede,
};
pub use decision_set::{
    DecisionSetArchiveScope, DecisionSetPlan, DecisionSetWarning, DecisionSetWriteReport,
    archive_set, compressed_summary_candidate, compressed_summary_candidates,
    detect_compressed_summary, draft_from_text, plan_from_yaml, repair_compressed_summary,
    write_plan_records,
};
pub use query::{
    DecisionContent, DecisionListEntry, DecisionSource, dangling_reference_warnings,
    decision_bodies, decision_display_id, decision_entries, decision_exists, decision_id,
    decision_title, decisions_for_feature, diagnose, known_decision_ids, list, list_tolerant,
    normalize_decision_id, parse_decision_number, render_record, resolve_decision_path, show,
};
pub use schema::{
    DecisionRecord, DecisionRecordKind, DecisionSetChildSummary, DecisionSetText, DecisionStatus,
    SummaryDecisionOverride,
};
pub use template::decision_file_name;
