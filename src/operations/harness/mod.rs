mod detect;
mod friction;
mod propose;

pub use detect::detect;
pub use friction::looks_like_correction;
pub use propose::{apply, refresh};
