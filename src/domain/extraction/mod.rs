//! Shared engine for extracting bundled resources into `.maestro/`.
//!
//! Skills, the hook recorder script, and the harness protocol each ship embedded
//! in the binary and extract on init/update through one version-gated core. This
//! facade exposes that core's shared surface; each resource family's planner
//! lives with its own module ([`crate::domain::skills::extract`] for skills).

pub(crate) mod extract;

pub use extract::{rollback_writes, ExtractMode, ExtractReport, ResourceBackup, ResourceWrite};
