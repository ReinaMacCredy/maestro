//! Feature aggregate facade.

pub mod query;
pub(crate) mod registry;
pub mod schema;

pub use registry::{
    create, diagnose, list, set_status, show, status_label, titles, FeatureDiagnostic, FeatureView,
};
pub use schema::FeatureStatus;
