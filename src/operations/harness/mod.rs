mod detect;
mod friction;
mod policy;
mod propose;

pub use detect::detect;
pub use friction::looks_like_correction;
pub use propose::{
    AppliedItem, OverThresholdItem, apply, dismiss, load_backlog, measure, over_threshold_items,
    refresh,
};
