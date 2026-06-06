//! Feature aggregate facade.

pub(crate) mod archive;
mod qa;
pub mod query;
pub(crate) mod registry;
pub mod schema;
mod verification;

pub use archive::{archive_feature, unarchive_feature};
pub use registry::{
    AmendReport, CancelReport, ContractAdditions, ContractEdits, FeatureDiagnostic, FeatureView,
    NoteReport, TransitionReport, accept, accept_with_qa_none, amend, cancel, create, diagnose,
    list, list_archived, note, set, ship, show, show_archived, start, status_label, titles,
};
pub use schema::FeatureStatus;
pub use verification::{
    AcceptanceCoverage, AcceptanceProof, AcceptanceSweepItem, AcceptanceSweepReport,
    FeatureProofUpdate, FeatureVerifyReport, acceptance_coverage, acceptance_coverage_archived,
    acceptance_id, normalize_acceptance_id, uncovered_acceptance, verify_feature,
};
