//! Feature aggregate facade.

pub(crate) mod archive;
mod qa;
pub mod query;
pub(crate) mod registry;
pub mod schema;
mod verification;

pub use archive::{FeatureArchiveReport, archive_feature, unarchive_feature};
pub use registry::{
    AcceptanceTextEdit, AmendReport, CancelReport, ContractAdditions, ContractChangeCounts,
    ContractEdits, FeatureDiagnostic, FeatureRosterEntry, FeatureView, NoteReport, SetReport,
    TransitionReport, accept, accept_with_qa_none, amend, cancel, create, diagnose, ensure_exists,
    feature_sidecar_dir, list, list_archived, list_tolerant, note, set, set_with_report, ship,
    ship_gaps, show, show_archived, start, status, status_label, titles,
};
pub use schema::{FeatureStatus, normalize_acceptance_id};
pub use verification::{
    AcceptanceCoverage, AcceptanceProof, AcceptanceSweepItem, AcceptanceSweepReport,
    FeatureProofUpdate, FeatureVerifyReport, acceptance_coverage, acceptance_coverage_archived,
    acceptance_id, uncovered_acceptance, verify_feature,
};
