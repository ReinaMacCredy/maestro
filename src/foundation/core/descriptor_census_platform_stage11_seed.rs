use super::descriptor_census_platform::{
    NamespaceWideSnapshotBackendSealedV1, NamespaceWideSnapshotBackendV1,
    NamespaceWideSnapshotFactsV1,
};
use super::secure_fs::{SecureFsError, SecureFsResult, SecureRoot};

pub(super) struct Stage11NamespaceWideSnapshotSeedV1 {
    _private: (),
}

impl NamespaceWideSnapshotBackendSealedV1 for Stage11NamespaceWideSnapshotSeedV1 {}

impl NamespaceWideSnapshotBackendV1 for Stage11NamespaceWideSnapshotSeedV1 {
    fn current_facts(&mut self) -> SecureFsResult<NamespaceWideSnapshotFactsV1> {
        Err(SecureFsError::CensusRefused)
    }
}

pub(super) fn acquire_namespace_wide_snapshot(
    _root: &SecureRoot,
) -> SecureFsResult<Stage11NamespaceWideSnapshotSeedV1> {
    // Stage 11 replaces only this owner seed with the supported-platform
    // namespace snapshot/journal implementation. The Foundation interface,
    // opaque lease, and final recheck remain unchanged.
    Err(SecureFsError::CensusRefused)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_owner_seed_is_explicitly_fail_closed() {
        let root = tempfile_root();
        let secure = SecureRoot::open_or_create(&root).unwrap();
        assert!(matches!(
            acquire_namespace_wide_snapshot(&secure),
            Err(SecureFsError::CensusRefused)
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    fn tempfile_root() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "maestro-stage11-census-seed-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&path);
        path
    }
}
