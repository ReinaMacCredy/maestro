//! Read-only production projection over the current repository-local Maestro state.

use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::domain::capability::generated_catalog::PUBLIC_CATALOG_REF_V1;
use crate::domain::integration::public_literals::{
    BootstrapContextV1, BootstrapRouteFactViewV1, McpPacketReadModeV1, McpPacketReadRequestV1,
};
use crate::domain::projection::{ProjectionErrorV1, ProjectionReadPortV1, ProjectionReadStateV1};
use crate::foundation::core::paths::MaestroPaths;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunningBinaryIdentityV1 {
    pub release_ref: String,
    pub digest: [u8; 32],
    pub version: String,
    pub executable_slot: String,
}

impl RunningBinaryIdentityV1 {
    pub(crate) fn load() -> Result<Self, ProjectionErrorV1> {
        let executable =
            std::env::current_exe().map_err(|_| ProjectionErrorV1::InvalidProjection)?;
        let bytes = fs::read(&executable).map_err(|_| ProjectionErrorV1::InvalidProjection)?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        Ok(Self {
            release_ref: digest_ref(digest),
            digest,
            version: env!("MAESTRO_VERSION").to_owned(),
            executable_slot: executable.display().to_string(),
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LiveProjectionReadProviderV1 {
    paths: MaestroPaths,
    running_binary: RunningBinaryIdentityV1,
}

impl LiveProjectionReadProviderV1 {
    pub(crate) fn load(paths: MaestroPaths) -> Result<Self, ProjectionErrorV1> {
        Ok(Self {
            paths,
            running_binary: RunningBinaryIdentityV1::load()?,
        })
    }

    #[cfg(test)]
    fn with_identity(paths: MaestroPaths, running_binary: RunningBinaryIdentityV1) -> Self {
        Self {
            paths,
            running_binary,
        }
    }

    pub(crate) fn running_binary(&self) -> &RunningBinaryIdentityV1 {
        &self.running_binary
    }
}

impl ProjectionReadPortV1 for LiveProjectionReadProviderV1 {
    fn read_once(
        &self,
        request: &McpPacketReadRequestV1,
    ) -> Result<ProjectionReadStateV1, ProjectionErrorV1> {
        if request.expected_public_catalog_ref != PUBLIC_CATALOG_REF_V1 {
            return Ok(ProjectionReadStateV1::Incompatible {
                reason_ref: "candidate:projection:public-catalog-mismatch:v1".to_owned(),
            });
        }
        if request.expected_release_ref != self.running_binary.release_ref {
            return Ok(ProjectionReadStateV1::Stale {
                reason_ref: "candidate:projection:running-release-mismatch:v1".to_owned(),
            });
        }
        if !same_repository(&request.repository_locator, self.paths.repo_root()) {
            return Ok(ProjectionReadStateV1::Stale {
                reason_ref: "candidate:projection:repository-locator-mismatch:v1".to_owned(),
            });
        }

        if !repository_state_is_present(&self.paths) {
            let bootstrap_route_fact_view =
                matches!(request.read_mode, McpPacketReadModeV1::BootstrapNoRecipeV1)
                    .then(|| bootstrap_fact_view(&self.paths, &self.running_binary.release_ref));
            return Ok(ProjectionReadStateV1::NoActiveStore {
                bootstrap_route_fact_view,
            });
        }

        Ok(ProjectionReadStateV1::Unavailable {
            reason_ref: "candidate:projection:canonical-store-locator-unavailable:v1".to_owned(),
        })
    }
}

fn repository_state_is_present(paths: &MaestroPaths) -> bool {
    [
        paths.store_db_file(),
        paths.cards_dir(),
        paths.tasks_dir(),
        paths.features_dir(),
    ]
    .into_iter()
    .any(|path| path.exists())
}

fn bootstrap_fact_view(paths: &MaestroPaths, release_ref: &str) -> BootstrapRouteFactViewV1 {
    let mut view = BootstrapRouteFactViewV1 {
        schema_version: 1,
        bootstrap_context: BootstrapContextV1::RepositoryBootstrap,
        resolution_basis_ref: PUBLIC_CATALOG_REF_V1.to_owned(),
        ordered_source_fact_commitments: vec![
            domain_ref(
                "maestro.vnext.bootstrap.repository.v1",
                paths.repo_root().display().to_string().as_bytes(),
            ),
            domain_ref("maestro.vnext.bootstrap.release.v1", release_ref.as_bytes()),
        ],
        fact_view_hash: [0; 32],
    };
    view.ordered_source_fact_commitments.sort();
    view.fact_view_hash = view.semantic_hash_without_hash();
    view
}

fn same_repository(locator: &str, repo_root: &Path) -> bool {
    let requested = Path::new(locator);
    match (requested.canonicalize(), repo_root.canonicalize()) {
        (Ok(requested), Ok(actual)) => requested == actual,
        _ => false,
    }
}

fn domain_ref(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    digest_ref(hasher.finalize().into())
}

fn digest_ref(digest: [u8; 32]) -> String {
    format!("sha256:{}", lower_hex(&digest))
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("invariant: string formatting cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::domain::integration::public_literals::{McpPacketReadEnvelopeV1, ProjectionScopeV1};
    use crate::domain::projection::read_packet;

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(1);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "maestro-v7-stage12-{label}-{}-{}",
                std::process::id(),
                NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).expect("remove isolated test directory");
        }
    }

    fn identity() -> RunningBinaryIdentityV1 {
        RunningBinaryIdentityV1 {
            release_ref: "candidate:release:test".to_owned(),
            digest: [7; 32],
            version: "test".to_owned(),
            executable_slot: "test-slot".to_owned(),
        }
    }

    fn request(root: &Path, mode: McpPacketReadModeV1) -> McpPacketReadRequestV1 {
        McpPacketReadRequestV1 {
            schema_version: 1,
            request_id: "request-1".to_owned(),
            repository_locator: root.display().to_string(),
            authenticated_host_connection_context_ref: "candidate:host:test:v1".to_owned(),
            projection_scope: ProjectionScopeV1::Repository,
            expected_release_ref: identity().release_ref,
            expected_public_catalog_ref: PUBLIC_CATALOG_REF_V1.to_owned(),
            bounded_response_redaction_profile: "repository-local".to_owned(),
            read_mode: mode,
        }
    }

    #[test]
    fn absent_store_returns_bootstrap_fact_view_without_writing() {
        let temp = TestDir::new("absent-store");
        let provider =
            LiveProjectionReadProviderV1::with_identity(MaestroPaths::new(temp.path()), identity());
        let envelope = read_packet(
            &provider,
            &request(temp.path(), McpPacketReadModeV1::BootstrapNoRecipeV1),
        )
        .expect("read");
        assert!(matches!(
            envelope,
            McpPacketReadEnvelopeV1::NoActiveStore {
                bootstrap_route_fact_view: Some(_)
            }
        ));
        assert!(!temp.path().join(".maestro").exists());
    }

    #[test]
    fn legacy_state_without_a_canonical_store_locator_is_unavailable() {
        let temp = TestDir::new("legacy-state");
        fs::create_dir_all(temp.path().join(".maestro/cards")).expect("state");
        let provider =
            LiveProjectionReadProviderV1::with_identity(MaestroPaths::new(temp.path()), identity());
        let envelope = read_packet(
            &provider,
            &request(temp.path(), McpPacketReadModeV1::DiscoverSelectionContextV1),
        )
        .expect("read");
        assert!(matches!(
            envelope,
            McpPacketReadEnvelopeV1::Unavailable { reason_ref }
                if reason_ref == "candidate:projection:canonical-store-locator-unavailable:v1"
        ));
    }

    #[test]
    fn mismatched_release_is_stale_before_repository_state_is_read() {
        let temp = TestDir::new("release-mismatch");
        let mut request = request(temp.path(), McpPacketReadModeV1::DiscoverSelectionContextV1);
        request.expected_release_ref = "candidate:release:other".to_owned();
        let provider =
            LiveProjectionReadProviderV1::with_identity(MaestroPaths::new(temp.path()), identity());
        assert!(matches!(
            read_packet(&provider, &request).expect("read"),
            McpPacketReadEnvelopeV1::Stale { .. }
        ));
    }
}
