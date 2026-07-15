//! Pure Design and Decision authoring kernels.
//!
//! These records are immutable governance and provenance inputs. They do not
//! own Work lifecycle, runtime behavior, Contract publication, or authority.

mod batch;
mod closure;
mod common;
mod decision;
mod materialization;
mod revision;

pub use crate::domain::vnext::work::WorkIdV1;
pub use batch::*;
pub use closure::*;
pub use common::*;
pub use decision::*;
pub use materialization::*;
pub use revision::*;
