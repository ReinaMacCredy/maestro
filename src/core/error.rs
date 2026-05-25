use std::path::PathBuf;

use thiserror::Error;

/// Shared recoverable errors for Maestro foundation modules.
#[derive(Debug, Error)]
pub enum MaestroError {
    /// No supported repository marker was found while walking ancestors.
    #[error(
        "failed to discover repository root from {start}: no .maestro or .git directory found"
    )]
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

    /// A backup or install operation name is not safe as a path segment.
    #[error("operation name must be a non-empty slug using only ASCII letters, digits, '-' or '_': {operation}")]
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
}
