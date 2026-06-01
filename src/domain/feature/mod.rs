//! Feature aggregate facade.

pub(crate) mod archive;
mod qa;
pub mod query;
pub(crate) mod registry;
pub mod schema;

pub use archive::{archive_feature, unarchive_feature};
pub use registry::{
    AmendReport, CancelReport, ContractAdditions, ContractEdits, FeatureDiagnostic, FeatureView,
    TransitionReport, accept, amend, cancel, create, diagnose, list, list_archived, set, ship,
    show, show_archived, start, status_label, titles,
};
pub use schema::FeatureStatus;
