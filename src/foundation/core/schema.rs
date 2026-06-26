/// Schema version for `.maestro/harness/harness.yml`.
pub const HARNESS_SCHEMA_VERSION: &str = "maestro.harness.v1";
/// Schema version for `.maestro/features/<id>/feature.yaml`.
pub const FEATURE_SCHEMA_VERSION: &str = "maestro.feature.v2";
/// Schema version for task metadata.
pub const TASK_SCHEMA_VERSION: &str = "maestro.task.v2";
/// Schema version for `.maestro/cards/<id>/card.yaml`.
pub const CARD_SCHEMA_VERSION: &str = "maestro.card.v1";
/// Schema version for `.maestro/cards/<progress-id>/progress.yml`.
pub const PROGRESS_SCHEMA_VERSION: &str = "maestro.progress.v1";
/// Schema version for run metadata.
pub const RUN_SCHEMA_VERSION: &str = "maestro.run.v1";
/// Schema version for hook events.
pub const EVENT_SCHEMA_VERSION: &str = "maestro.event.v1";
/// Schema version for structured decision stores.
pub const DECISIONS_SCHEMA_VERSION: &str = "maestro.decisions.v1";
/// Schema version for run evidence summaries.
pub const RUN_EVIDENCE_SCHEMA_VERSION: &str = "maestro.run_evidence.v1";
/// Legacy schema version for task verification reports used by migration/tests.
pub const VERIFICATION_SCHEMA_VERSION: &str = "maestro.verification.v1";
/// Legacy schema version for task acceptance sidecars used by migration/tests.
pub const ACCEPTANCE_SCHEMA_VERSION: &str = "maestro.acceptance.v1";
/// Schema version for install ownership lockfiles.
pub const INSTALL_LOCK_SCHEMA_VERSION: &str = "maestro.install_lock.v1";
/// Schema version for the user-level Maestro global skills lock.
pub const GLOBAL_SKILLS_LOCK_SCHEMA_VERSION: &str = "maestro.global_skills_lock.v1";
/// Schema version for harness backlog proposals.
pub const BACKLOG_SCHEMA_VERSION: &str = "maestro.backlog.v1";
/// Legacy schema version for the canonical verification report restore journal.
pub const VERIFICATION_RESTORE_SCHEMA_VERSION: &str = "maestro.verification.restore.v1";

/// Every active schema version supported by this binary.
pub const ALL_SCHEMA_VERSIONS: &[&str] = &[
    HARNESS_SCHEMA_VERSION,
    FEATURE_SCHEMA_VERSION,
    TASK_SCHEMA_VERSION,
    CARD_SCHEMA_VERSION,
    PROGRESS_SCHEMA_VERSION,
    RUN_SCHEMA_VERSION,
    EVENT_SCHEMA_VERSION,
    DECISIONS_SCHEMA_VERSION,
    RUN_EVIDENCE_SCHEMA_VERSION,
    INSTALL_LOCK_SCHEMA_VERSION,
    GLOBAL_SKILLS_LOCK_SCHEMA_VERSION,
    BACKLOG_SCHEMA_VERSION,
];

/// Compatibility classification of an on-disk schema version against the
/// version this binary expects for the same artifact.
///
/// This is the single decision point that the scattered `found != expected`
/// checks route through, so every reader shares one notion of "compatible"
/// while keeping its own reaction (diagnostic, hard error, or watch-loop fallback).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Compat {
    /// The on-disk version is exactly the expected version. Proceed.
    Exact,
    /// The on-disk version is anything other than the expected version (an
    /// older generation, a newer one, or an unparseable tag). Default-deny:
    /// stop on every gate and write path; only display/diagnostic paths may
    /// degrade.
    Incompatible,
}

/// Classify an on-disk schema version (`found`) against the version this binary
/// expects for the same artifact (`expected`).
///
/// This is a clean-rewrite binary with no migration path, so the only
/// compatible state is an exact match; any other version is `Incompatible`.
///
/// # Examples
///
/// ```
/// use maestro::foundation::core::schema::{classify, Compat, FEATURE_SCHEMA_VERSION};
///
/// assert_eq!(classify(FEATURE_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION), Compat::Exact);
/// assert_eq!(classify("maestro.feature.v0", FEATURE_SCHEMA_VERSION), Compat::Incompatible);
/// assert_eq!(classify("maestro.galaxy.v9", FEATURE_SCHEMA_VERSION), Compat::Incompatible);
/// ```
pub fn classify(found: &str, expected: &str) -> Compat {
    if found == expected {
        Compat::Exact
    } else {
        Compat::Incompatible
    }
}
