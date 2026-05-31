//! Feature aggregate facade.

pub mod query;
pub(crate) mod registry;
pub mod schema;

pub use registry::{
    accept, amend, cancel, create, diagnose, list, set, ship, show, start, status_label, titles,
    AmendReport, CancelReport, ContractAdditions, ContractEdits, FeatureDiagnostic, FeatureView,
    TransitionReport,
};
pub use schema::FeatureStatus;
