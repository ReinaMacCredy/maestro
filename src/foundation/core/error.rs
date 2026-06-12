use std::path::PathBuf;

use thiserror::Error;

use crate::foundation::core::schema::{FEATURE_SCHEMA_VERSION, TASK_SCHEMA_VERSION};

/// Shared recoverable errors for Maestro foundation modules.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MaestroError {
    /// No supported repository marker was found while walking ancestors.
    #[error("failed to discover repository root from {start}: no .maestro or .git directory found")]
    RepoRootNotFound {
        /// Directory where discovery started.
        start: PathBuf,
    },

    /// An artifact schema version did not match the expected V1 schema.
    #[error("schema mismatch for {artifact}: expected {expected}, found {found}")]
    SchemaMismatch {
        /// Human-readable artifact name or path.
        artifact: String,
        /// Expected schema version.
        expected: &'static str,
        /// Actual schema version found on disk.
        found: String,
    },

    /// An artifact schema version is outside this binary's declared read set
    /// (its family's schema pack). A named legacy version carries the pack's
    /// explicit migrate route; an undeclared version carries none.
    #[error("schema mismatch for {artifact}: found {found}, this binary reads {read}")]
    UnsupportedSchemaVersion {
        /// Human-readable artifact name or path.
        artifact: String,
        /// Actual schema version found on disk.
        found: String,
        /// The family's declared read set, rendered for display.
        read: String,
        /// Bare migrate command from the pack's legacy route, if the version
        /// is a named legacy one.
        route: Option<String>,
    },

    /// An operation would write outside the discovered repository root.
    #[error("operation would write outside repository root: {path}")]
    OutsideRepository {
        /// Path that failed repository containment validation.
        path: PathBuf,
    },

    /// A backup destination would follow a symlink outside the repo-local backup tree.
    #[error("backup path must not contain symlink components: {path}")]
    BackupPathContainsSymlink {
        /// Symlink path that would affect backup writes.
        path: PathBuf,
    },

    /// A managed mirror path contains a symlink component.
    #[error("managed mirror path must not contain symlink components: {path}")]
    ManagedPathContainsSymlink {
        /// Symlink path that would affect mirror reads or writes.
        path: PathBuf,
    },

    /// A backup or install operation name is not safe as a path segment.
    #[error(
        "operation name must be a non-empty slug using only ASCII letters, digits, '-' or '_': {operation}"
    )]
    InvalidOperationName {
        /// Invalid operation name.
        operation: String,
    },

    /// Existing mirror content is not owned by Maestro's managed markers or keys.
    #[error("managed content is not owned by maestro: {path}")]
    UnownedManagedContent {
        /// Mirror path containing unowned content.
        path: PathBuf,
    },

    /// A JSON mirror file did not contain a top-level JSON object.
    #[error("managed JSON mirror must be a top-level object")]
    InvalidJsonMirror,

    /// An id lookup failed. `nearest` carries an existing card id close enough
    /// to be a plausible typo, surfaced as a hint only -- never auto-resolved.
    #[error("{kind} not found: {id}")]
    IdNotFound {
        /// The lookup's noun, e.g. "decision" or "task".
        kind: &'static str,
        /// The id that failed to resolve.
        id: String,
        /// A near-match existing id, if one is plausible.
        nearest: Option<String>,
    },
}

impl MaestroError {
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::RepoRootNotFound { .. } => Some("run maestro init --yes".to_string()),
            Self::SchemaMismatch {
                expected, found, ..
            } if (*expected == FEATURE_SCHEMA_VERSION && found == "maestro.feature.v1")
                || (*expected == TASK_SCHEMA_VERSION && found == "maestro.task.v1") =>
            {
                Some("run maestro migrate-v2".to_string())
            }
            Self::SchemaMismatch { .. } => Some("run maestro doctor".to_string()),
            Self::UnsupportedSchemaVersion { route, .. } => Some(match route {
                Some(route) => format!("run {route}"),
                None => "run maestro doctor".to_string(),
            }),
            Self::IdNotFound { nearest, .. } => nearest
                .as_ref()
                .map(|nearest| format!("did you mean {nearest}?")),
            Self::OutsideRepository { .. }
            | Self::BackupPathContainsSymlink { .. }
            | Self::ManagedPathContainsSymlink { .. }
            | Self::InvalidOperationName { .. }
            | Self::UnownedManagedContent { .. }
            | Self::InvalidJsonMirror => None,
        }
    }
}
