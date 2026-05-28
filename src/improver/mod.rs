//! Compatibility shim for the legacy `crate::improver` root.

pub mod detect {
    pub use crate::operations::improver::detect;
}

pub mod propose {
    pub use crate::operations::improver::{apply, refresh};
}
