/// Schema version for `.maestro/harness/harness.yml`.
pub const HARNESS_SCHEMA_VERSION: &str = "maestro.harness.v1";
/// Schema version for `.maestro/features/<id>/feature.yaml`.
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
/// Schema version for the canonical verification report restore journal.
pub const VERIFICATION_RESTORE_SCHEMA_VERSION: &str = "maestro.verification.restore.v1";

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
    VERIFICATION_RESTORE_SCHEMA_VERSION,
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
    /// The on-disk version is an older generation of a schema family that the
    /// bundled migration (`operations/migrate/v0_106_to_v0_8`) can convert.
    /// Route the user to `maestro migrate`.
    NeedsMigration,
    /// The on-disk version is unknown or unparseable. Default-deny: stop on
    /// every gate and write path; only display/diagnostic paths may degrade.
    Incompatible,
}

/// Classify an on-disk schema version (`found`) against the version this binary
/// expects for the same artifact (`expected`).
///
/// `NeedsMigration` is determined by a small match against the one bundled
/// migration module rather than a migration chain or registry: only the schema
/// families that `operations/migrate/v0_106_to_v0_8` converts are eligible, and
/// only when `found` is an older generation of that same family (a pre-v1
/// generation tag or the absent/`<missing>` marker that v0.106.1 artifacts
/// carry). Any other gap is `Incompatible`.
///
/// # Examples
///
/// ```
/// use maestro::foundation::core::schema::{
///     classify, Compat, FEATURE_SCHEMA_VERSION, INSTALL_LOCK_SCHEMA_VERSION,
/// };
///
/// assert_eq!(classify(FEATURE_SCHEMA_VERSION, FEATURE_SCHEMA_VERSION), Compat::Exact);
/// assert_eq!(classify("maestro.feature.v0", FEATURE_SCHEMA_VERSION), Compat::NeedsMigration);
/// assert_eq!(classify("maestro.galaxy.v9", FEATURE_SCHEMA_VERSION), Compat::Incompatible);
/// // Non-migratable family: an older generation still stops hard.
/// assert_eq!(
///     classify("maestro.install_lock.v0", INSTALL_LOCK_SCHEMA_VERSION),
///     Compat::Incompatible,
/// );
/// ```
pub fn classify(found: &str, expected: &str) -> Compat {
    if found == expected {
        return Compat::Exact;
    }
    if expected_is_migratable(expected) && is_older_generation_of(found, expected) {
        return Compat::NeedsMigration;
    }
    Compat::Incompatible
}

/// Schema families that the bundled `v0_106_to_v0_8` migration produces v1
/// artifacts for, identified by their expected v1 version. Determined by a
/// direct match against that one migration module; deliberately not a registry.
fn expected_is_migratable(expected: &str) -> bool {
    matches!(
        expected,
        FEATURE_SCHEMA_VERSION
            | TASK_SCHEMA_VERSION
            | ACCEPTANCE_SCHEMA_VERSION
            | HARNESS_SCHEMA_VERSION
    )
}

/// True when `found` is a recognizable older generation of the same schema
/// family as `expected`: the absent/`<missing>` marker that v0.106.1 artifacts
/// carry, or a `maestro.<family>.v<N>` tag with a lower generation than expected.
fn is_older_generation_of(found: &str, expected: &str) -> bool {
    if found.is_empty() || found == "<missing>" {
        return true;
    }
    let (Some((found_family, found_gen)), Some((expected_family, expected_gen))) = (
        split_family_generation(found),
        split_family_generation(expected),
    ) else {
        return false;
    };
    found_family == expected_family && found_gen < expected_gen
}

/// Split a `maestro.<family>.v<N>` version into its family path and numeric
/// generation, returning `None` for any string that does not match that shape.
fn split_family_generation(version: &str) -> Option<(&str, u32)> {
    let (family, version_tag) = version.rsplit_once('.')?;
    let generation = version_tag.strip_prefix('v')?;
    let generation: u32 = generation.parse().ok()?;
    Some((family, generation))
}
