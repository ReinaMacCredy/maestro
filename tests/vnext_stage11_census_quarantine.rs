const CENSUS_SOURCE: &str = include_str!("../src/operations/migration/census.rs");
const QUARANTINE_OPERATION_SOURCE: &str = include_str!("../src/operations/migration/quarantine.rs");
const QUARANTINE_DOMAIN_SOURCE: &str =
    include_str!("../src/domain/migration/runtime/quarantine.rs");
const UNIT_TEST_SOURCE: &str = include_str!("../src/operations/migration/tests.rs");
const OPERATIONS_FACADE_SOURCE: &str = include_str!("../src/operations/migration/mod.rs");

#[test]
fn census_consumes_only_the_foundation_owned_v2_continuation() {
    for required in [
        "MigrationClassificationContinuationV2",
        "continuation.consume_for_stage11()",
        "Stage11CensusContinuationV2",
        "ProtectedLocatorLeaseV2",
        "stage11_finality_v2",
    ] {
        assert!(CENSUS_SOURCE.contains(required), "missing {required}");
    }
    for forbidden in [
        "DeclaredRootScanV1",
        "recensus_declared_roots",
        "PathBuf",
        "for_admitted_root_set",
        "descriptor_census_platform::census",
    ] {
        assert!(
            !CENSUS_SOURCE.contains(forbidden),
            "Migration regained Foundation-owned physical scope: {forbidden}"
        );
    }
    assert!(OPERATIONS_FACADE_SOURCE.contains(
        "pub(crate) use census::{Stage11CensusContinuationV2, consume_foundation_census_v2};"
    ));
    assert!(OPERATIONS_FACADE_SOURCE.contains("#[cfg(test)]\nmod legacy_census_v1;"));
    assert!(OPERATIONS_FACADE_SOURCE.contains(
        "#[cfg(test)]\n#[allow(\n    unused_imports,\n    reason = \"V1 physical census evidence remains test-only while V2 Foundation continuation owns production scope\"\n)]\npub(crate) use legacy_census_v1"
    ));
    assert!(
        UNIT_TEST_SOURCE.contains("production_census_consumes_only_the_foundation_v2_continuation")
    );
}

#[test]
fn quarantine_is_sealed_no_replace_and_outside_live_discovery() {
    for required in [
        "QuarantineInsideDiscovery",
        "InvalidDiscoveryRootSet",
        "UnexpectedEntry",
        "create_file_if_absent",
        "read_exact",
        "read_immutable",
        "chunk_digests",
    ] {
        assert!(
            QUARANTINE_DOMAIN_SOURCE.contains(required)
                || QUARANTINE_OPERATION_SOURCE.contains(required),
            "missing {required}"
        );
    }
    for forbidden in ["remove_file", "remove_dir_all", "rename("] {
        assert!(
            !QUARANTINE_OPERATION_SOURCE.contains(forbidden),
            "forbidden quarantine mutation {forbidden}"
        );
    }
    assert!(
        UNIT_TEST_SOURCE
            .contains("sealed_quarantine_replays_exact_bytes_and_rejects_discovery_overlap")
    );
}
