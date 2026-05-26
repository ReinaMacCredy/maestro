/// Schema version for `.maestro/harness/harness.yml`.
pub const HARNESS_SCHEMA_VERSION: &str = "maestro.harness.v1";
/// Schema version for `.maestro/features/features.yaml`.
pub const FEATURE_SCHEMA_VERSION: &str = "maestro.feature.v1";
/// Schema version for task metadata.
pub const TASK_SCHEMA_VERSION: &str = "maestro.task.v1";
/// Schema version for run metadata.
pub const RUN_SCHEMA_VERSION: &str = "maestro.run.v1";
/// Schema version for hook events.
pub const EVENT_SCHEMA_VERSION: &str = "maestro.event.v1";
/// Schema version for run evidence summaries.
pub const RUN_EVIDENCE_SCHEMA_VERSION: &str = "maestro.run_evidence.v1";
/// Schema version for task verification proof.
pub const VERIFICATION_SCHEMA_VERSION: &str = "maestro.verification.v1";
/// Schema version for task acceptance criteria.
pub const ACCEPTANCE_SCHEMA_VERSION: &str = "maestro.acceptance.v1";
/// Schema version for install ownership lockfiles.
pub const INSTALL_LOCK_SCHEMA_VERSION: &str = "maestro.install_lock.v1";
/// Schema version for harness backlog proposals.
pub const BACKLOG_SCHEMA_VERSION: &str = "maestro.backlog.v1";
/// Schema version for decision markdown frontmatter.
pub const DECISION_SCHEMA_VERSION: &str = "maestro.decision.v1";

/// Every V1 schema version supported by this binary.
pub const ALL_SCHEMA_VERSIONS: &[&str] = &[
    HARNESS_SCHEMA_VERSION,
    FEATURE_SCHEMA_VERSION,
    TASK_SCHEMA_VERSION,
    RUN_SCHEMA_VERSION,
    EVENT_SCHEMA_VERSION,
    RUN_EVIDENCE_SCHEMA_VERSION,
    VERIFICATION_SCHEMA_VERSION,
    ACCEPTANCE_SCHEMA_VERSION,
    INSTALL_LOCK_SCHEMA_VERSION,
    BACKLOG_SCHEMA_VERSION,
    DECISION_SCHEMA_VERSION,
];
