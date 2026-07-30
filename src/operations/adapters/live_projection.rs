//! Read-only production projection over the current repository-local Maestro state.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::domain::capability::generated_catalog::{
    GeneratedCapabilityCatalogV1, OperationCatalogKindV1, PUBLIC_CATALOG_REF_V1,
};
use crate::domain::integration::public_literals::{
    BootstrapContextV1, BootstrapRouteFactViewV1, McpCliSearchBoundsV1, McpCliSearchCompletenessV1,
    McpCliSearchEnvelopeV1, McpCliSearchHitV1, McpCliSearchQueryV1, McpCliSearchRequestV1,
    McpPacketReadModeV1, McpPacketReadRequestV1, OperationSpecRefV1,
};
use crate::domain::projection::{ProjectionErrorV1, ProjectionReadPortV1, ProjectionReadStateV1};
use crate::foundation::core::paths::MaestroPaths;
use crate::foundation::core::secure_fs::{SecureFsError, SecureRoot};

use super::Stage10AdapterError;

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
            release_ref: digest_ref(&digest),
            digest,
            version: env!("MAESTRO_VERSION").to_owned(),
            executable_slot: executable.display().to_string(),
        })
    }
}

#[derive(Debug)]
pub(crate) struct LiveProjectionReadProviderV1 {
    repository_root: SecureRoot,
    repository_locator: String,
    running_binary: RunningBinaryIdentityV1,
}

impl LiveProjectionReadProviderV1 {
    pub(crate) fn open_explicit_repository(
        repository_locator: &str,
    ) -> Result<Self, ProjectionErrorV1> {
        let root_path = explicit_component_normal_locator(repository_locator)?;
        let repository_root =
            SecureRoot::open(&root_path).map_err(|_| ProjectionErrorV1::InvalidProjection)?;
        Ok(Self {
            repository_root,
            repository_locator: repository_locator.to_owned(),
            running_binary: RunningBinaryIdentityV1::load()?,
        })
    }

    #[allow(
        dead_code,
        reason = "MainIntegration still compiles against the pre-correction constructor until it adopts open_explicit_repository"
    )]
    pub(crate) fn load(paths: MaestroPaths) -> Result<Self, ProjectionErrorV1> {
        let repository_locator = paths
            .repo_root()
            .to_str()
            .ok_or(ProjectionErrorV1::InvalidProjection)?;
        Self::open_explicit_repository(repository_locator)
    }

    #[cfg(test)]
    fn with_identity(
        repository_locator: &str,
        running_binary: RunningBinaryIdentityV1,
    ) -> Result<Self, ProjectionErrorV1> {
        let root_path = explicit_component_normal_locator(repository_locator)?;
        let repository_root =
            SecureRoot::open(&root_path).map_err(|_| ProjectionErrorV1::InvalidProjection)?;
        Ok(Self {
            repository_root,
            repository_locator: repository_locator.to_owned(),
            running_binary,
        })
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
        if request.repository_locator != self.repository_locator {
            return Ok(ProjectionReadStateV1::Stale {
                reason_ref: "candidate:projection:repository-locator-mismatch:v1".to_owned(),
            });
        }
        self.repository_root
            .verify_path_binding()
            .map_err(|_| ProjectionErrorV1::InvalidProjection)?;

        if !repository_state_is_present(&self.repository_root)? {
            let bootstrap_route_fact_view =
                matches!(request.read_mode, McpPacketReadModeV1::BootstrapNoRecipeV1).then(|| {
                    bootstrap_fact_view(&self.repository_locator, &self.running_binary.release_ref)
                });
            return Ok(ProjectionReadStateV1::NoActiveStore {
                bootstrap_route_fact_view,
            });
        }

        Ok(ProjectionReadStateV1::Unavailable {
            reason_ref: "candidate:projection:canonical-store-locator-unavailable:v1".to_owned(),
        })
    }
}

const PACKET_READ_COMMAND_REF_V1: &str = "candidate:cli-command:maestro-packet-read:v1";

#[derive(Clone, Debug, Eq, PartialEq)]
struct SearchRowV1 {
    searchable: String,
    hit: McpCliSearchHitV1,
}

pub(crate) fn cli_search(
    request: &McpCliSearchRequestV1,
    running_binary: &RunningBinaryIdentityV1,
) -> Result<McpCliSearchEnvelopeV1, Stage10AdapterError> {
    request
        .validate()
        .map_err(|_| Stage10AdapterError::InvalidFrame)?;
    if request.expected_release_ref != running_binary.release_ref
        || request.expected_public_catalog_ref != PUBLIC_CATALOG_REF_V1
    {
        return Err(Stage10AdapterError::SearchCurrentnessRejected);
    }
    let catalog = GeneratedCapabilityCatalogV1::load_frozen()
        .map_err(|_| Stage10AdapterError::CapabilityCatalogUnavailable)?;
    let mut rows = vec![SearchRowV1 {
        searchable: format!("{PACKET_READ_COMMAND_REF_V1} maestro packet read packet projection"),
        hit: McpCliSearchHitV1::PureRead {
            exact_command_ref: PACKET_READ_COMMAND_REF_V1.to_owned(),
        },
    }];
    rows.extend(
        catalog
            .actions()
            .iter()
            .chain(catalog.ceremonies())
            .map(|entry| {
                let hit = match entry.operation_spec_ref() {
                    OperationSpecRefV1::Action(spec) => McpCliSearchHitV1::Action {
                        exact_action_spec_ref: spec.exact_action_spec_ref,
                    },
                    OperationSpecRefV1::Ceremony(spec) => McpCliSearchHitV1::Ceremony {
                        exact_ceremony_spec_ref: spec.exact_ceremony_spec_ref,
                    },
                };
                let exact_spec_ref = match &hit {
                    McpCliSearchHitV1::PureRead { exact_command_ref } => exact_command_ref,
                    McpCliSearchHitV1::Action {
                        exact_action_spec_ref,
                    } => exact_action_spec_ref,
                    McpCliSearchHitV1::Ceremony {
                        exact_ceremony_spec_ref,
                    } => exact_ceremony_spec_ref,
                };
                SearchRowV1 {
                    searchable: format!(
                        "{} {} {} {}",
                        entry.name(),
                        entry.descriptor_ref(),
                        exact_spec_ref,
                        match entry.kind() {
                            OperationCatalogKindV1::Action => "action",
                            OperationCatalogKindV1::Ceremony => "ceremony",
                        }
                    ),
                    hit,
                }
            }),
    );

    let mut matching = rows
        .into_iter()
        .filter(|row| search_matches(&request.query, row))
        .map(|row| row.hit)
        .collect::<Vec<_>>();
    let total_matching_count =
        u64::try_from(matching.len()).map_err(|_| Stage10AdapterError::SearchEnvelopeRejected)?;
    let bound = usize::try_from(request.finite_bound).unwrap_or(usize::MAX);
    let truncated = matching.len() > bound;
    matching.truncate(bound);
    let catalog_snapshot_ref = catalog_snapshot_ref(
        catalog.grammar_ref(),
        &running_binary.release_ref,
        &matching,
        total_matching_count,
    );
    let envelope = McpCliSearchEnvelopeV1 {
        schema_version: 1,
        request_id: request.request_id.clone(),
        running_binary_release: running_binary.release_ref.clone(),
        binary_digest: running_binary.digest,
        binary_version: running_binary.version.clone(),
        executable_slot: running_binary.executable_slot.clone(),
        core_catalog_ref: catalog.grammar_ref().to_owned(),
        public_catalog_ref: PUBLIC_CATALOG_REF_V1.to_owned(),
        catalog_snapshot_ref: catalog_snapshot_ref.clone(),
        completeness: if truncated {
            McpCliSearchCompletenessV1::BoundedTruncated
        } else {
            McpCliSearchCompletenessV1::Complete
        },
        bounds: McpCliSearchBoundsV1 {
            requested_bound: request.finite_bound,
            returned_count: matching.len() as u64,
            total_matching_count,
        },
        cursor: truncated.then(|| format!("{catalog_snapshot_ref}:{}", matching.len())),
        hits: matching,
    };
    envelope
        .validate_for(request)
        .map_err(|_| Stage10AdapterError::SearchEnvelopeRejected)?;
    Ok(envelope)
}

pub(crate) fn decode_cli_search_request(
    input: &str,
) -> Result<McpCliSearchRequestV1, Stage10AdapterError> {
    let value = decode_json_with_unique_keys(input)?;
    let object = exact_object(
        &value,
        &[
            "schema_version",
            "request_id",
            "query",
            "finite_bound",
            "expected_release_ref",
            "expected_public_catalog_ref",
        ],
    )?;
    let query = exact_object(field(object, "query")?, &["variant", "value"])?;
    let query_value = text_field(query, "value")?;
    let query = match text_ref(query, "variant")? {
        "ExactCommandId" => McpCliSearchQueryV1::ExactCommandId(query_value),
        "BoundedFuzzyIntent" => McpCliSearchQueryV1::BoundedFuzzyIntent(query_value),
        _ => return Err(Stage10AdapterError::InvalidFrame),
    };
    let request = McpCliSearchRequestV1 {
        schema_version: u64_field(object, "schema_version")?,
        request_id: text_field(object, "request_id")?,
        query,
        finite_bound: u64_field(object, "finite_bound")?,
        expected_release_ref: text_field(object, "expected_release_ref")?,
        expected_public_catalog_ref: text_field(object, "expected_public_catalog_ref")?,
    };
    request
        .validate()
        .map_err(|_| Stage10AdapterError::InvalidFrame)?;
    Ok(request)
}

pub(crate) fn encode_cli_search_envelope(
    envelope: &McpCliSearchEnvelopeV1,
    request: &McpCliSearchRequestV1,
) -> Result<String, Stage10AdapterError> {
    envelope
        .validate_for(request)
        .map_err(|_| Stage10AdapterError::SearchEnvelopeRejected)?;
    let completeness = match envelope.completeness {
        McpCliSearchCompletenessV1::Complete => "Complete",
        McpCliSearchCompletenessV1::BoundedTruncated => "BoundedTruncated",
    };
    let hits = envelope
        .hits
        .iter()
        .map(|hit| match hit {
            McpCliSearchHitV1::PureRead { exact_command_ref } => json!({
                "variant": "PureRead",
                "value": {"exact_command_ref": exact_command_ref},
            }),
            McpCliSearchHitV1::Action {
                exact_action_spec_ref,
            } => json!({
                "variant": "Action",
                "value": {"exact_action_spec_ref": exact_action_spec_ref},
            }),
            McpCliSearchHitV1::Ceremony {
                exact_ceremony_spec_ref,
            } => json!({
                "variant": "Ceremony",
                "value": {"exact_ceremony_spec_ref": exact_ceremony_spec_ref},
            }),
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&json!({
        "schema_version": envelope.schema_version,
        "request_id": envelope.request_id,
        "running_binary_release": envelope.running_binary_release,
        "binary_digest": digest_ref(&envelope.binary_digest),
        "binary_version": envelope.binary_version,
        "executable_slot": envelope.executable_slot,
        "core_catalog_ref": envelope.core_catalog_ref,
        "public_catalog_ref": envelope.public_catalog_ref,
        "catalog_snapshot_ref": envelope.catalog_snapshot_ref,
        "completeness": completeness,
        "bounds": {
            "requested_bound": envelope.bounds.requested_bound,
            "returned_count": envelope.bounds.returned_count,
            "total_matching_count": envelope.bounds.total_matching_count,
        },
        "cursor": envelope.cursor,
        "hits": hits,
    }))
    .map(|encoded| format!("{encoded}\n"))
    .map_err(|_| Stage10AdapterError::SearchEnvelopeRejected)
}

fn search_matches(query: &McpCliSearchQueryV1, row: &SearchRowV1) -> bool {
    match query {
        McpCliSearchQueryV1::ExactCommandId(value) => match &row.hit {
            McpCliSearchHitV1::PureRead { exact_command_ref } => {
                value == exact_command_ref || value == "maestro packet read"
            }
            McpCliSearchHitV1::Action {
                exact_action_spec_ref,
            } => value == exact_action_spec_ref,
            McpCliSearchHitV1::Ceremony {
                exact_ceremony_spec_ref,
            } => value == exact_ceremony_spec_ref,
        },
        McpCliSearchQueryV1::BoundedFuzzyIntent(value) => {
            let searchable = row.searchable.to_ascii_lowercase();
            value
                .split_ascii_whitespace()
                .all(|term| searchable.contains(&term.to_ascii_lowercase()))
        }
    }
}

fn catalog_snapshot_ref(
    core_catalog_ref: &str,
    release_ref: &str,
    hits: &[McpCliSearchHitV1],
    total_matching_count: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"maestro.vnext.cli-search-catalog-snapshot.v1");
    hasher.update([0]);
    hasher.update(core_catalog_ref.as_bytes());
    hasher.update([0]);
    hasher.update(release_ref.as_bytes());
    hasher.update(total_matching_count.to_be_bytes());
    for hit in hits {
        hasher.update([0]);
        match hit {
            McpCliSearchHitV1::PureRead { exact_command_ref } => {
                hasher.update(b"read");
                hasher.update(exact_command_ref.as_bytes());
            }
            McpCliSearchHitV1::Action {
                exact_action_spec_ref,
            } => {
                hasher.update(b"action");
                hasher.update(exact_action_spec_ref.as_bytes());
            }
            McpCliSearchHitV1::Ceremony {
                exact_ceremony_spec_ref,
            } => {
                hasher.update(b"ceremony");
                hasher.update(exact_ceremony_spec_ref.as_bytes());
            }
        }
    }
    let digest: [u8; 32] = hasher.finalize().into();
    digest_ref(&digest)
}

fn decode_json_with_unique_keys(input: &str) -> Result<Value, Stage10AdapterError> {
    struct UniqueKeyValueSeed;

    impl<'de> serde::de::DeserializeSeed<'de> for UniqueKeyValueSeed {
        type Value = Value;

        fn deserialize<D>(self, deserializer: D) -> Result<Value, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            deserializer.deserialize_any(UniqueKeyValueVisitor)
        }
    }

    struct UniqueKeyValueVisitor;

    impl<'de> serde::de::Visitor<'de> for UniqueKeyValueVisitor {
        type Value = Value;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a JSON value whose objects have unique keys")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
            Ok(Value::Bool(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
            Ok(Value::from(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
            Ok(Value::from(value))
        }

        fn visit_f64<E>(self, value: f64) -> Result<Value, E>
        where
            E: serde::de::Error,
        {
            serde_json::Number::from_f64(value)
                .map(Value::Number)
                .ok_or_else(|| E::custom("non-finite JSON number"))
        }

        fn visit_str<E>(self, value: &str) -> Result<Value, E> {
            Ok(Value::String(value.to_owned()))
        }

        fn visit_string<E>(self, value: String) -> Result<Value, E> {
            Ok(Value::String(value))
        }

        fn visit_unit<E>(self) -> Result<Value, E> {
            Ok(Value::Null)
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some(value) = sequence.next_element_seed(UniqueKeyValueSeed)? {
                values.push(value);
            }
            Ok(Value::Array(values))
        }

        fn visit_map<A>(self, mut entries: A) -> Result<Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut object = Map::new();
            while let Some(key) = entries.next_key::<String>()? {
                let value = entries.next_value_seed(UniqueKeyValueSeed)?;
                if object.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate object key"));
                }
            }
            Ok(Value::Object(object))
        }
    }

    let mut deserializer = serde_json::Deserializer::from_str(input);
    let value = serde::de::DeserializeSeed::deserialize(UniqueKeyValueSeed, &mut deserializer)
        .map_err(|_| Stage10AdapterError::InvalidFrame)?;
    deserializer
        .end()
        .map_err(|_| Stage10AdapterError::InvalidFrame)?;
    Ok(value)
}

fn exact_object<'value>(
    value: &'value Value,
    expected: &[&str],
) -> Result<&'value Map<String, Value>, Stage10AdapterError> {
    let object = value.as_object().ok_or(Stage10AdapterError::InvalidFrame)?;
    if object.len() != expected.len() || !expected.iter().all(|key| object.contains_key(*key)) {
        return Err(Stage10AdapterError::InvalidFrame);
    }
    Ok(object)
}

fn field<'value>(
    object: &'value Map<String, Value>,
    name: &str,
) -> Result<&'value Value, Stage10AdapterError> {
    object.get(name).ok_or(Stage10AdapterError::InvalidFrame)
}

fn text_ref<'value>(
    object: &'value Map<String, Value>,
    name: &str,
) -> Result<&'value str, Stage10AdapterError> {
    field(object, name)?
        .as_str()
        .ok_or(Stage10AdapterError::InvalidFrame)
}

fn text_field(object: &Map<String, Value>, name: &str) -> Result<String, Stage10AdapterError> {
    text_ref(object, name).map(str::to_owned)
}

fn u64_field(object: &Map<String, Value>, name: &str) -> Result<u64, Stage10AdapterError> {
    field(object, name)?
        .as_u64()
        .ok_or(Stage10AdapterError::InvalidFrame)
}

fn explicit_component_normal_locator(
    repository_locator: &str,
) -> Result<PathBuf, ProjectionErrorV1> {
    let supplied = Path::new(repository_locator);
    if !supplied.is_absolute()
        || supplied
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(ProjectionErrorV1::InvalidProjection);
    }
    let normalized = supplied.components().collect::<PathBuf>();
    if normalized.to_str() != Some(repository_locator) {
        return Err(ProjectionErrorV1::InvalidProjection);
    }
    Ok(normalized)
}

fn repository_state_is_present(root: &SecureRoot) -> Result<bool, ProjectionErrorV1> {
    if secure_regular_file_exists(root, ".maestro/store.db")? {
        return Ok(true);
    }
    for relative in [".maestro/cards", ".maestro/tasks", ".maestro/features"] {
        match root.open_dir(relative) {
            Ok(_) => return Ok(true),
            Err(error) if secure_not_found(&error) => {}
            Err(_) => return Err(ProjectionErrorV1::InvalidProjection),
        }
    }
    Ok(false)
}

fn secure_regular_file_exists(
    root: &SecureRoot,
    relative: &str,
) -> Result<bool, ProjectionErrorV1> {
    match root.validate_regular_file(relative) {
        Ok(()) => Ok(true),
        Err(error) if secure_not_found(&error) => Ok(false),
        Err(_) => Err(ProjectionErrorV1::InvalidProjection),
    }
}

fn secure_not_found(error: &SecureFsError) -> bool {
    matches!(
        error,
        SecureFsError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound
    )
}

fn bootstrap_fact_view(repository_locator: &str, release_ref: &str) -> BootstrapRouteFactViewV1 {
    let mut view = BootstrapRouteFactViewV1 {
        schema_version: 1,
        bootstrap_context: BootstrapContextV1::RepositoryBootstrap,
        resolution_basis_ref: PUBLIC_CATALOG_REF_V1.to_owned(),
        ordered_source_fact_commitments: vec![
            domain_ref(
                "maestro.vnext.bootstrap.repository.v1",
                repository_locator.as_bytes(),
            ),
            domain_ref("maestro.vnext.bootstrap.release.v1", release_ref.as_bytes()),
        ],
        fact_view_hash: [0; 32],
    };
    view.ordered_source_fact_commitments.sort();
    view.fact_view_hash = view.semantic_hash_without_hash();
    view
}

fn domain_ref(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    digest_ref(&hasher.finalize().into())
}

fn digest_ref(digest: &[u8; 32]) -> String {
    format!("sha256:{}", lower_hex(digest))
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
            let path = secure_test_temp_root().join(format!(
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

    #[cfg(target_os = "macos")]
    fn secure_test_temp_root() -> PathBuf {
        Path::new("/private").join(
            std::env::temp_dir()
                .strip_prefix("/")
                .expect("absolute macOS temporary directory"),
        )
    }

    #[cfg(not(target_os = "macos"))]
    fn secure_test_temp_root() -> PathBuf {
        std::env::temp_dir()
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

    fn locator(path: &Path) -> &str {
        path.to_str().expect("UTF-8 test path")
    }

    #[test]
    fn cli_search_is_literal_and_does_not_compute_a_next_action() {
        let running_binary = RunningBinaryIdentityV1 {
            release_ref: "candidate:release:test".to_owned(),
            digest: [9; 32],
            version: "test".to_owned(),
            executable_slot: "test-slot".to_owned(),
        };
        let request = |query| McpCliSearchRequestV1 {
            schema_version: 1,
            request_id: "request-1".to_owned(),
            query,
            finite_bound: 10,
            expected_release_ref: running_binary.release_ref.clone(),
            expected_public_catalog_ref: PUBLIC_CATALOG_REF_V1.to_owned(),
        };
        let exact = cli_search(
            &request(McpCliSearchQueryV1::ExactCommandId(
                "maestro packet read".to_owned(),
            )),
            &running_binary,
        )
        .unwrap();
        assert_eq!(
            exact.hits,
            vec![McpCliSearchHitV1::PureRead {
                exact_command_ref: PACKET_READ_COMMAND_REF_V1.to_owned(),
            }]
        );
        assert!(
            cli_search(
                &request(McpCliSearchQueryV1::ExactCommandId("next".to_owned())),
                &running_binary
            )
            .unwrap()
            .hits
            .is_empty()
        );
        assert!(
            cli_search(
                &request(McpCliSearchQueryV1::ExactCommandId("action".to_owned())),
                &running_binary
            )
            .unwrap()
            .hits
            .is_empty()
        );
    }

    #[test]
    fn cli_search_transport_is_exact_and_canonical() {
        let request = decode_cli_search_request(
            r#"{
                "schema_version":1,
                "request_id":"request-1",
                "query":{"variant":"BoundedFuzzyIntent","value":"packet read"},
                "finite_bound":1,
                "expected_release_ref":"candidate:release:test",
                "expected_public_catalog_ref":"candidate:catalog:test"
            }"#,
        )
        .unwrap();
        assert_eq!(
            request.query,
            McpCliSearchQueryV1::BoundedFuzzyIntent("packet read".to_owned())
        );
        let envelope = McpCliSearchEnvelopeV1 {
            schema_version: 1,
            request_id: request.request_id.clone(),
            running_binary_release: request.expected_release_ref.clone(),
            binary_digest: [9; 32],
            binary_version: "test".to_owned(),
            executable_slot: "test-slot".to_owned(),
            core_catalog_ref: "candidate:catalog:core".to_owned(),
            public_catalog_ref: request.expected_public_catalog_ref.clone(),
            catalog_snapshot_ref: "candidate:catalog:snapshot".to_owned(),
            completeness: McpCliSearchCompletenessV1::Complete,
            bounds: McpCliSearchBoundsV1 {
                requested_bound: 1,
                returned_count: 1,
                total_matching_count: 1,
            },
            cursor: None,
            hits: vec![McpCliSearchHitV1::PureRead {
                exact_command_ref: PACKET_READ_COMMAND_REF_V1.to_owned(),
            }],
        };
        let encoded = encode_cli_search_envelope(&envelope, &request).unwrap();
        assert!(encoded.ends_with('\n'));
        assert!(encoded.contains(r#""completeness":"Complete""#));
        assert!(encoded.contains(r#""binary_digest":"sha256:0909"#));
        assert!(encoded.contains(r#""variant":"PureRead""#));
    }

    #[test]
    fn cli_search_transport_rejects_duplicate_and_unknown_fields() {
        let duplicate = r#"{
            "schema_version":1,
            "schema_version":1,
            "request_id":"request-1",
            "query":{"variant":"ExactCommandId","value":"maestro packet read"},
            "finite_bound":1,
            "expected_release_ref":"candidate:release:test",
            "expected_public_catalog_ref":"candidate:catalog:test"
        }"#;
        assert_eq!(
            decode_cli_search_request(duplicate),
            Err(Stage10AdapterError::InvalidFrame)
        );
        let unknown = r#"{
            "schema_version":1,
            "request_id":"request-1",
            "query":{"variant":"ExactCommandId","value":"maestro packet read"},
            "finite_bound":1,
            "expected_release_ref":"candidate:release:test",
            "expected_public_catalog_ref":"candidate:catalog:test",
            "extra":true
        }"#;
        assert_eq!(
            decode_cli_search_request(unknown),
            Err(Stage10AdapterError::InvalidFrame)
        );
    }

    #[test]
    fn absent_store_returns_bootstrap_fact_view_without_writing() {
        let temp = TestDir::new("absent-store");
        let provider =
            LiveProjectionReadProviderV1::with_identity(locator(temp.path()), identity())
                .expect("provider");
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
            LiveProjectionReadProviderV1::with_identity(locator(temp.path()), identity())
                .expect("provider");
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
            LiveProjectionReadProviderV1::with_identity(locator(temp.path()), identity())
                .expect("provider");
        assert!(matches!(
            read_packet(&provider, &request).expect("read"),
            McpPacketReadEnvelopeV1::Stale { .. }
        ));
    }

    #[test]
    fn repository_locator_rejects_dot_dotdot_and_textual_aliases() {
        let temp = TestDir::new("alias-closed");
        let root = locator(temp.path());
        assert!(LiveProjectionReadProviderV1::with_identity(root, identity()).is_ok());
        assert!(
            LiveProjectionReadProviderV1::with_identity(&format!("{root}/."), identity()).is_err()
        );
        assert!(
            LiveProjectionReadProviderV1::with_identity(&format!("{root}/child/.."), identity())
                .is_err()
        );
        assert!(
            LiveProjectionReadProviderV1::with_identity(&format!("{root}/"), identity()).is_err()
        );
        assert!(LiveProjectionReadProviderV1::with_identity(".", identity()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn repository_locator_rejects_root_and_intermediate_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = TestDir::new("symlink-root");
        let actual = temp.path().join("actual");
        fs::create_dir(&actual).expect("actual root");
        let root_alias = temp.path().join("root-alias");
        symlink(&actual, &root_alias).expect("root symlink");
        assert!(
            LiveProjectionReadProviderV1::with_identity(locator(&root_alias), identity()).is_err()
        );

        let intermediate = temp.path().join("intermediate");
        symlink(temp.path(), &intermediate).expect("intermediate symlink");
        let through_intermediate = intermediate.join("actual");
        assert!(
            LiveProjectionReadProviderV1::with_identity(locator(&through_intermediate), identity())
                .is_err()
        );
    }

    #[test]
    fn retained_repository_descriptor_refuses_path_replacement() {
        let temp = TestDir::new("root-replacement");
        let root = temp.path().join("repository");
        let displaced = temp.path().join("displaced");
        fs::create_dir(&root).expect("repository root");
        let provider = LiveProjectionReadProviderV1::with_identity(locator(&root), identity())
            .expect("provider");
        fs::rename(&root, &displaced).expect("displace admitted root");
        fs::create_dir(&root).expect("replacement root");
        fs::create_dir_all(root.join(".maestro/cards")).expect("replacement state");

        assert!(
            read_packet(
                &provider,
                &request(&root, McpPacketReadModeV1::DiscoverSelectionContextV1)
            )
            .is_err()
        );
    }
}
