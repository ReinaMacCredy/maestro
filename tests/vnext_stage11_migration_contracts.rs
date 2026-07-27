use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const INSTANCE_FIXTURE: &str = include_str!("fixtures/vnext/stage11/migration_instances.v1.jsonl");
const E204: &str = include_str!("../contracts/vnext/public/embedded_resources.e204.v1.json");
const C325: &str = include_str!("../contracts/vnext/public/direct_consumers.c325.v1.json");
const SKILL_LEDGER: &str = include_str!("../contracts/vnext/public/v1_skill_ledger.v1.json");
const PHYSICAL_CENSUS: &str =
    include_str!("../contracts/vnext/public/physical_census.commitment.v1.json");
const CLASSIFICATION_SOURCE: &str =
    include_str!("../src/domain/vnext/migration/runtime/classification.rs");
const CONSUMER_SOURCE: &str = include_str!("../src/domain/vnext/migration/runtime/consumer.rs");
const ASSOCIATION_SOURCE: &str =
    include_str!("../src/domain/vnext/migration/runtime/association.rs");
const CENSUS_SOURCE: &str = include_str!("../src/operations/vnext/migration/census.rs");
const IMPORT_SOURCE: &str = include_str!("../src/domain/vnext/migration/runtime/import.rs");
const ROLLBACK_SOURCE: &str = include_str!("../src/domain/vnext/migration/runtime/rollback.rs");
const UNIT_TEST_SOURCE: &str = include_str!("../src/operations/vnext/migration/tests.rs");

fn json(value: &str) -> Value {
    serde_json::from_str(value).expect("valid checked-in JSON")
}

fn instance_records() -> Vec<Value> {
    INSTANCE_FIXTURE
        .lines()
        .map(|line| serde_json::from_str(line).expect("valid checked-in JSONL row"))
        .collect()
}

#[test]
fn retained_sources_have_exact_rows_and_physical_uses_only_frozen_commitment() {
    let mut records = instance_records();
    let header = records.remove(0);
    assert_eq!(
        header["schema"],
        "maestro.vnext.stage11.migration-instances.v1"
    );
    let expected_counts =
        BTreeMap::from([("c325", 325_usize), ("e204", 204), ("skill_ledger", 35)]);
    let mut grouped = BTreeMap::<&str, Vec<&Value>>::new();
    for record in &records {
        grouped
            .entry(record["family"].as_str().expect("family"))
            .or_default()
            .push(record);
    }
    assert_eq!(
        grouped
            .iter()
            .map(|(family, rows)| (*family, rows.len()))
            .collect::<BTreeMap<_, _>>(),
        expected_counts
    );
    for (family, rows) in &grouped {
        let expected = match *family {
            "e204" => json(E204)["rows"].as_array().expect("E204 rows").clone(),
            "c325" => json(C325)["rows"].as_array().expect("C325 rows").clone(),
            "skill_ledger" => json(SKILL_LEDGER)["rows"]
                .as_array()
                .expect("Skill rows")
                .clone(),
            _ => panic!("unexpected instance family"),
        };
        assert_eq!(
            rows.iter()
                .enumerate()
                .map(|(index, row)| {
                    assert_eq!(row["ordinal"].as_u64(), Some((index + 1) as u64));
                    row["row"].clone()
                })
                .collect::<Vec<_>>(),
            expected
        );
    }
    let physical = json(PHYSICAL_CENSUS);
    assert!(!grouped.contains_key("physical"));
    assert_eq!(
        header["source_files"]["physical"]["fixture_posture"],
        "aggregate_commitment_only_no_fabricated_rows"
    );
    assert_eq!(
        physical["historical_attested_receipt"]["node_count"],
        28_102
    );
    assert_eq!(physical["directory_containers_included"], false);
    assert_eq!(physical["literal_historical_rows_retained"], false);
    assert_eq!(
        physical["identity_row_grammar"],
        serde_json::json!([
            "type",
            "payload_length",
            "payload_sha256",
            "lexically_normalized_absolute_locator"
        ])
    );
}

#[test]
fn h3_join_construction_is_owner_bound_and_migration_consumes_the_frozen_carrier() {
    let join_impl = CLASSIFICATION_SOURCE
        .split("impl NativeCancellationCausalJoinV1")
        .nth(1)
        .expect("H3 implementation");
    let constructor = join_impl
        .split("pub const fn id")
        .next()
        .expect("H3 constructor");
    assert!(constructor.contains("#[cfg(test)]"));
    assert!(constructor.contains("test_only_from_stage4_publication"));
    assert!(constructor.contains("ActiveStoreEffectSnapshotV1"));
    assert!(constructor.contains("ActiveStoreEffectWithdrawalOutcomeV1"));
    assert!(constructor.contains("source_id: MigrationDigestV1"));
    assert!(constructor.contains("target_id: MigrationDigestV1"));
    assert!(!constructor.contains("pub fn new"));
    assert!(!CLASSIFICATION_SOURCE.contains("NativeCancellationCausalJoinV1::new"));
    assert!(ASSOCIATION_SOURCE.contains("VerifiedH3WithdrawalPublicationUseV1"));
    assert!(ASSOCIATION_SOURCE.contains("H3NativeCancelledMigrationMemberV1"));
    assert!(ASSOCIATION_SOURCE.contains("consume_native_cancelled_member_for_migration"));
    assert!(ASSOCIATION_SOURCE.contains("H3NativeCancelledSourceMemberV1::new"));
    assert!(ASSOCIATION_SOURCE.contains("H3NativeCancelledTargetMemberV1::new"));
    assert!(ASSOCIATION_SOURCE.contains("H3NativeCancelledClassificationV1::new"));
    assert!(!ASSOCIATION_SOURCE.contains(".consume_for_migration("));
    assert!(!ASSOCIATION_SOURCE.contains("ConsumedH3WithdrawalPublicationV1"));
    assert!(ASSOCIATION_SOURCE.contains("H3MigrationFinalityV1::ActiveStore"));
    assert!(ASSOCIATION_SOURCE.contains("H3MigrationFinalityV1::PreStore"));
    assert!(ASSOCIATION_SOURCE.contains("H3CarrierCountMismatch"));
    assert!(ASSOCIATION_SOURCE.contains("H3MemberCoverageMismatch"));
    assert!(ASSOCIATION_SOURCE.contains("H3MemberDuplicate"));
    assert!(ASSOCIATION_SOURCE.contains("H3MemberContradiction"));
    assert!(ASSOCIATION_SOURCE.contains("maestro.execution.h3-native-cancelled-member.v1\\0"));
    assert!(ASSOCIATION_SOURCE.contains("H3VerifiedMigrationAssociationUseV1"));
    assert!(ASSOCIATION_SOURCE.contains("_consumed_members"));

    assert!(
        CONSUMER_SOURCE.contains("#[cfg(test)]\npub trait Stage9Stage10ConsumerCensusAdapterV1")
    );
    assert!(CONSUMER_SOURCE.contains("AuthoritativeConsumerCensusV1"));
    assert!(CONSUMER_SOURCE.contains("expected_member_count == 0"));
    assert!(!CONSUMER_SOURCE.contains("mut consumers: Vec<ConsumerRecordV1>"));
    assert!(IMPORT_SOURCE.contains("consumers.census().entries().is_empty()"));
    assert!(ASSOCIATION_SOURCE.contains("consumers.census().entries().is_empty()"));
    assert!(CLASSIFICATION_SOURCE.contains("Stage4PublicationReused"));
    assert!(CLASSIFICATION_SOURCE.contains("CancellationJoinRowMismatch"));
}

#[test]
fn stage9_and_stage10_candidate_adapters_are_explicit_and_test_only() {
    assert!(
        ASSOCIATION_SOURCE.contains("#[cfg(test)]\npub trait Stage9CutoverAssociationAdapterV1")
    );
    assert!(ASSOCIATION_SOURCE.contains("from_stage9_adapter"));
    assert!(ASSOCIATION_SOURCE.contains("cutover_finality"));
    assert!(ASSOCIATION_SOURCE.contains("ActiveStoreFinalityV1"));
    assert!(ASSOCIATION_SOURCE.contains("PreStoreFinalityV1"));
    assert!(
        ASSOCIATION_SOURCE
            .contains("material.association_id.as_bytes() != meaning.id().as_bytes()")
    );
    assert!(ASSOCIATION_SOURCE.contains("MigrationCutoverAssociationV1"));
    assert!(!ASSOCIATION_SOURCE.contains("pub struct AssociationExternalBindingsV1"));
    assert!(ROLLBACK_SOURCE.contains("#[cfg(test)]\npub trait Stage9Stage10CutoverHostAdapterV1"));
    assert!(ROLLBACK_SOURCE.contains("from_cutover_host_adapter"));
    assert!(!ROLLBACK_SOURCE.contains("pub fn assess("));
    assert!(UNIT_TEST_SOURCE.contains("TestOnlyStage9AssociationAdapterV1"));
    assert!(UNIT_TEST_SOURCE.contains("TestOnlyStage9Stage10CutoverHostAdapterV1"));
    assert!(UNIT_TEST_SOURCE.contains("TestOnlyConsumerCensusAdapterV1"));
    assert!(UNIT_TEST_SOURCE.contains("test_only_consumer_adapter_rejects_empty_membership"));
    assert!(ASSOCIATION_SOURCE.contains("destination_domain_id"));
    assert!(UNIT_TEST_SOURCE.contains("zero_migration_identities_are_unconstructible"));
}

#[test]
fn stage11_cannot_reconstruct_physical_census_or_generic_finality_success() {
    for required in [
        "MigrationClassificationContinuationV2",
        "continuation.consume_for_stage11()",
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
    ] {
        assert!(
            !CENSUS_SOURCE.contains(forbidden),
            "Migration has regained physical census authority: {forbidden}"
        );
    }
    assert!(ASSOCIATION_SOURCE.contains("#[cfg(test)]\n    pub(in crate::domain::vnext) fn from_verified_h3_native_cancelled_members"));
}

const MIGRATION_OPERATIONS_MOD: &str = include_str!("../src/operations/vnext/migration/mod.rs");

fn rust_sources_under(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("readable source directory") {
        let path = entry.expect("readable directory entry").path();
        if path.is_dir() {
            rust_sources_under(&path, into);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            into.push(path);
        }
    }
}

fn production_scope(source: &str) -> &str {
    match source.find("\n#[cfg(test)]\nmod tests {") {
        Some(index) => &source[..index],
        None => source,
    }
}

/// The Stage-9/Stage-10 cutover-host adapter stays `#[cfg(test)]` because this
/// base has no surface that observes a live host's recorded attempt identity,
/// acceptance, or effect crossing. That deferral is only honest while it stays
/// true, so pin it: the moment a real host-acceptance/effect adapter binds
/// these facts in production, this proof fails and the adapter must be
/// revisited rather than silently left as a test double.
#[test]
fn no_production_surface_supplies_the_stage9_stage10_cutover_host_facts() {
    assert!(
        MIGRATION_OPERATIONS_MOD.contains("#[cfg(test)]\nmod tests;"),
        "the migration operations unit tests must stay entirely test-gated"
    );

    let mut sources = Vec::new();
    rust_sources_under(Path::new("src"), &mut sources);
    assert!(
        sources.len() > 100,
        "the source walk must actually reach the tree"
    );

    let rollback_path = Path::new("src/domain/vnext/migration/runtime/rollback.rs");
    let unit_test_path = Path::new("src/operations/vnext/migration/tests.rs");
    let mut scanned = 0_usize;
    for path in &sources {
        if path == rollback_path || path == unit_test_path {
            continue;
        }
        let source = fs::read_to_string(path).expect("readable Rust source");
        let production = production_scope(&source);
        for forbidden in [
            "CutoverAcceptanceV1::",
            "EffectCrossingV1::",
            "RollbackAssessmentV1::",
            "ActiveStoreFinalityV1::new",
            "PreStoreFinalityV1::new",
        ] {
            assert!(
                !production.contains(forbidden),
                "{forbidden} gained a production producer in {}; the cutover-host \
                 adapter deferral is no longer accurate and must be re-decided",
                path.display()
            );
        }
        scanned += 1;
    }
    assert_eq!(scanned, sources.len() - 2);

    // Inside the runtime itself the host facts are only ever named by the
    // `#[cfg(test)]` constructor, and that constructor is the sole way to build
    // a RollbackAssessmentV1 at all.
    let adapter_start = ROLLBACK_SOURCE
        .find("    #[cfg(test)]\n    pub fn from_cutover_host_adapter")
        .expect("cfg(test) cutover-host constructor");
    let adapter_end = adapter_start
        + ROLLBACK_SOURCE[adapter_start..]
            .find("\n    pub const fn cutover_attempt_id")
            .expect("constructor precedes the production accessors");
    for token in ["CutoverAcceptanceV1::", "EffectCrossingV1::", "Ok(Self {"] {
        let sites: Vec<usize> = ROLLBACK_SOURCE
            .match_indices(token)
            .map(|(at, _)| at)
            .collect();
        assert!(!sites.is_empty(), "{token} must still be present");
        for at in sites {
            assert!(
                at > adapter_start && at < adapter_end,
                "{token} escaped the cfg(test) cutover-host constructor"
            );
        }
    }
    assert_eq!(ROLLBACK_SOURCE.matches("Ok(Self {").count(), 1);
}

#[test]
fn rollback_and_import_sources_have_no_production_delete_or_activation_path() {
    let combined = format!("{IMPORT_SOURCE}\n{ROLLBACK_SOURCE}");
    for forbidden in [
        "remove_file",
        "remove_dir",
        "activate_repository",
        "activate_installation",
    ] {
        assert!(
            !combined.contains(forbidden),
            "{forbidden} must stay absent"
        );
    }
    assert!(ROLLBACK_SOURCE.contains("ProtectedExactV1RollbackEligible"));
    assert!(ROLLBACK_SOURCE.contains("VNextFreshGenerationRecoveryOnly"));
    assert!(ROLLBACK_SOURCE.contains("RefusedStaleHost"));
}
