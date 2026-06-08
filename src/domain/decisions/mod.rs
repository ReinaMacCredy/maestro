//! Decision aggregate facade.

pub(crate) mod create;
pub mod query;
pub mod schema;
pub mod template;

pub use create::{DecisionLockReport, DecisionWriteReport, create_open, empty_store_yaml, lock};
pub use query::{
    DecisionContent, DecisionListEntry, DecisionSource, dangling_reference_warnings,
    decision_bodies, decision_display_id, decision_entries, decision_exists, decision_id,
    decision_title, decisions_for_feature, diagnose, list, list_tolerant, normalize_decision_id,
    parse_decision_number, resolve_decision_path, show,
};
pub use template::decision_file_name;
