use std::collections::BTreeMap;

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
const INSTALLATION_CONSUMER_SNAPSHOT_SOURCE: &str =
    include_str!("../src/domain/vnext/installation/consumer_snapshot.rs");
const INSTALLATION_FACADE_SOURCE: &str = include_str!("../src/domain/vnext/installation/mod.rs");
const FOUNDATION_FACADE_SOURCE: &str = include_str!("../src/foundation/core/mod.rs");

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

    assert!(CONSUMER_SOURCE.contains("evaluate_installation_snapshot"));
    assert!(CONSUMER_SOURCE.contains("InstallationMigrationConsumerSnapshotV1"));
    assert!(CONSUMER_SOURCE.contains("snapshot.into_parts()"));
    assert!(CONSUMER_SOURCE.contains("AuthoritativeConsumerCensusV1"));
    assert!(CONSUMER_SOURCE.contains("expected_member_count == 0"));
    assert!(!CONSUMER_SOURCE.contains("mut consumers: Vec<ConsumerRecordV1>"));
    assert!(IMPORT_SOURCE.contains("consumers.census().entries().is_empty()"));
    assert!(ASSOCIATION_SOURCE.contains("consumers.census().entries().is_empty()"));
    assert!(CLASSIFICATION_SOURCE.contains("Stage4PublicationReused"));
    assert!(CLASSIFICATION_SOURCE.contains("CancellationJoinRowMismatch"));
}

#[test]
fn production_consumer_snapshot_finality_and_census_routes_are_bound() {
    assert!(CONSUMER_SOURCE.contains("evaluate_installation_snapshot"));
    assert!(CONSUMER_SOURCE.contains("snapshot.into_parts()"));
    for required in [
        "ConsumerClosureDurableLinearizationV1",
        "durable_effect.commit",
        "bind_migration_census",
    ] {
        assert!(
            INSTALLATION_CONSUMER_SNAPSHOT_SOURCE.contains(required),
            "missing production consumer snapshot route: {required}"
        );
    }
    for required in [
        "pub(in crate::domain::vnext) mod stage11_finality_v2",
        "ProtectedLocatorLeaseV2",
        "execute_pre_store",
    ] {
        assert!(
            INSTALLATION_FACADE_SOURCE.contains(required),
            "missing production PreStore finality route: {required}"
        );
    }
    for required in [
        "pub(crate) mod stage11_aggregate_census",
        "census_from_stage11_owner_v2",
        "consume_for_stage11",
    ] {
        assert!(
            FOUNDATION_FACADE_SOURCE.contains(required),
            "missing production aggregate census route: {required}"
        );
    }
    assert!(CENSUS_SOURCE.contains("census_admitted_owner_roots_v2"));
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
