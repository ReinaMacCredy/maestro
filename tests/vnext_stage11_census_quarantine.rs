const CENSUS_SOURCE: &str = include_str!("../src/operations/migration/census.rs");
const QUARANTINE_OPERATION_SOURCE: &str = include_str!("../src/operations/migration/quarantine.rs");
const QUARANTINE_DOMAIN_SOURCE: &str =
    include_str!("../src/domain/migration/runtime/quarantine.rs");
const UNIT_TEST_SOURCE: &str = include_str!("../src/operations/migration/tests.rs");
const OPERATIONS_FACADE_SOURCE: &str = include_str!("../src/operations/migration/mod.rs");
const FOUNDATION_QUARANTINE_V2_SOURCE: &str =
    include_str!("../src/foundation/core/legacy_quarantine.rs");
const FOUNDATION_ROOT_UNIVERSE_SOURCE: &str =
    include_str!("../src/foundation/core/root_universe.rs");
const FOUNDATION_LOSS_EVIDENCE_SOURCE: &str =
    include_str!("../src/foundation/core/legacy_loss_evidence.rs");

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

#[test]
fn v4_complete_root_universe_is_closed_and_owner_current_through_finality() {
    for required in [
        "FoundationDeclaredRootRoleV1",
        "RepositoryStore",
        "Active",
        "Inactive",
        "Snapshot",
        "Cache",
        "Archive",
        "Host",
        "Legacy",
        "FoundationDeclaredRootDispositionV1",
        "Present",
        "DeclaredAbsent",
        "Unsupported",
        "DeclaredRootUniverseLeaseV1",
        "OwnerUniverseFinalRecheckPortV1",
        "FoundationOwnerUniverseCurrentnessV1",
        "DuplicateDeclaration",
        "OwnerCurrentnessDrift",
    ] {
        assert!(
            FOUNDATION_ROOT_UNIVERSE_SOURCE.contains(required),
            "missing complete-universe contract {required}"
        );
    }
    for forbidden in [
        "#[derive(Clone)]\npub(crate) struct FoundationDeclaredRootUniverseFactsV1",
        "impl Clone for FoundationDeclaredRootUniverseFactsV1",
    ] {
        assert!(
            !FOUNDATION_ROOT_UNIVERSE_SOURCE.contains(forbidden),
            "root-universe capability became replayable: {forbidden}"
        );
    }

    let finish = FOUNDATION_QUARANTINE_V2_SOURCE
        .split("pub(crate) fn finish(\n        self,\n        candidate_manifest: [u8; 32],")
        .nth(2)
        .and_then(|tail| {
            tail.split("pub(crate) enum FoundationLegacyQuarantineFinalityV2")
                .next()
        })
        .expect("V2 Foundation finality");
    let physical = finish
        .find("consume_final_recheck")
        .expect("physical recheck");
    let expected_old = finish.find("seal_expected_old").expect("expected-old seal");
    let owner = finish
        .find(".final_recheck(&repository_hold.expected)")
        .expect("owner final recheck");
    assert!(
        physical < expected_old && expected_old < owner,
        "V2 must retain physical and owner fences through the expected-old custody cut"
    );
    for required in [
        "FoundationLegacyQuarantineFinalityV2::InDoubt",
        "FoundationLegacyQuarantineFinalityV2::RecoveryRequired",
        "FoundationLegacyQuarantineClosureV2",
        "repository_universe",
        "installation_universe",
        "final_currentness",
    ] {
        assert!(
            FOUNDATION_QUARANTINE_V2_SOURCE.contains(required),
            "missing V2 finality state {required}"
        );
    }
}

#[test]
fn v4_loss_evidence_is_owner_issued_move_only_and_consumed_once() {
    for required in [
        "LegacySourceHistoryKindV1",
        "RepositoryStore",
        "InstallationStore",
        "ProtectedPrimaryJournal",
        "LegacySourceHistoricalBindingV1",
        "LegacySourceCurrentBindingV1",
        "OwnerUnavailablePreexistingLossWitnessV1",
        "OwnerIssuedUnavailablePreexistingLossEvidenceSetV1",
        "FoundationOwnerEvidenceIssuanceBindingV1",
        "OwnerUnavailablePreexistingLossEvidenceIssuerPortV1",
        "issue_for_foundation",
        "FoundationValidatedUnavailablePreexistingLossReceiptV1",
        "into_foundation_witnesses",
        "PhantomData<Rc<()>>",
    ] {
        assert!(
            FOUNDATION_LOSS_EVIDENCE_SOURCE.contains(required),
            "missing owner loss-evidence contract {required}"
        );
    }
    for forbidden in [
        "impl Clone for OwnerIssuedUnavailablePreexistingLossEvidenceSetV1",
        "derive(Clone, Debug",
        "serde",
        "PathBuf",
        "pub fn identity",
    ] {
        assert!(
            !FOUNDATION_LOSS_EVIDENCE_SOURCE.contains(forbidden),
            "loss evidence gained replay/raw authority: {forbidden}"
        );
    }
    let expected_v4 = FOUNDATION_QUARANTINE_V2_SOURCE
        .split("pub(crate) struct LegacyQuarantineExpectedSourceV4")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) struct LegacyQuarantineExpectedSourceSetV4")
                .next()
        })
        .expect("V4 expected-source contract");
    for forbidden in ["loss_evidence_id", "root_binding", "PathBuf"] {
        assert!(
            !expected_v4.contains(forbidden),
            "packet expected-source regained forbidden authority: {forbidden}"
        );
    }
    for required in [
        "pass_a_absence_id",
        "pass_b_absence_id",
        "UnexpectedLossEvidence",
        "SourceChanged",
    ] {
        assert!(
            FOUNDATION_QUARANTINE_V2_SOURCE.contains(required),
            "missing absence/disappearance distinction {required}"
        );
    }
}
