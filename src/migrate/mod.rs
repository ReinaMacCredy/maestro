//! Legacy Migration compatibility root.
//!
//! New production callers should use `crate::operations::migrate`.

pub use crate::operations::migrate::v0_106_to_v0_8;
