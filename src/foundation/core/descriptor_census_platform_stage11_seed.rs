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
    let _ = NamespaceWideSnapshotFactsV1::from_stage11_owner;
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
        static NEXT_TEMP_ROOT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let temp_directory =
            std::fs::canonicalize(std::env::temp_dir()).expect("resolve existing temp directory");
        let path = temp_directory.join(format!(
            "maestro-stage11-census-seed-{}-{}",
            std::process::id(),
            NEXT_TEMP_ROOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        path
    }
}
