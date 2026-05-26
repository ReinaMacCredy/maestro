//! Transitional interface module root for external entrypoints.
//!
//! These are compatibility aliases over the current flat modules. Later phases
//! move implementations under this folder while preserving the legacy paths.

pub use crate::commands as cli;
pub use crate::hooks;
pub use crate::mcp;
pub use crate::shell;
pub use crate::tui;
