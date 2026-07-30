#!/usr/bin/env python3
"""Validate the checked-in Stage-11 migration fixtures and owned runtime surface."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "tests/fixtures/vnext/stage11"
PUBLIC = ROOT / "contracts/vnext/public"
RUNTIME = ROOT / "src/domain/migration/runtime"
OPERATIONS = ROOT / "src/operations/migration"
INSTALLATION = ROOT / "src/domain/installation"
FOUNDATION = ROOT / "src/foundation/core"
EXPECTED_DISPOSITIONS = [
    "MappedNormative",
    "MappedHistoricalNonBearer",
    "OpaquePreserved",
    "Quarantined",
    "UnavailablePreexistingLoss",
]
EXPECTED_DIGESTS = {
    "e204": "c8fc4c6cd53d81272d19c3b402e99a0ca3f69ebd18cf9464539db1d1ecf85388",
    "c325": "9aee8ea371f770e8694131079d4bfb4845f849d59d0b545005a2f0371a42976a",
}
EXPECTED_INSTANCE_COUNTS = {
    "e204": 204,
    "c325": 325,
    "skill_ledger": 35,
}
EXPECTED_PHYSICAL_CATEGORIES = {
    "legacy": 4816,
    "c115": 3230,
    "repo": 4665,
    "cache": 15277,
    "binary": 64,
    "perroot": 34,
    "user": 16,
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def load(name: str) -> dict[str, object]:
    path = FIXTURES / name
    raw = path.read_bytes()
    require(raw.endswith(b"\n"), f"{name} must end in newline")
    require(b"\r" not in raw and all(byte < 128 for byte in raw), f"{name} must be ASCII LF")
    return json.loads(raw)

def load_public(name: str) -> dict[str, object]:
    return json.loads((PUBLIC / name).read_bytes())


def load_instances() -> tuple[dict[str, object], list[dict[str, object]], bytes]:
    raw = (FIXTURES / "migration_instances.v1.jsonl").read_bytes()
    require(raw.endswith(b"\n"), "migration instance fixture must end in newline")
    require(b"\r" not in raw and all(byte < 128 for byte in raw), "instance fixture must be ASCII LF")
    records = [json.loads(line) for line in raw.splitlines()]
    require(records, "migration instance fixture is empty")
    return records[0], records[1:], raw

def main() -> None:
    cases = load("migration_cases.v1.json")
    gates = load("consumer_gates.v1.json")
    instance_header, instance_rows, instance_bytes = load_instances()
    physical_commitment = load_public("physical_census.commitment.v1.json")
    historical = cases["historical_non_promoting_inputs"]
    require(cases["closed_dispositions"] == EXPECTED_DISPOSITIONS, "disposition set drifted")
    require(cases["known_upstream_interface_gaps"] == [], "closed owner route was reopened")
    require(
        cases["production_owner_routes"]
        == [
            "installation_consumer_snapshot_to_migration_closure",
            "durable_consumer_finality_receipt",
            "foundation_v2_aggregate_census_continuation",
            "installation_v2_pre_store_finality",
        ],
        "production owner-route closure drifted",
    )
    require(
        {
            "arbitrary_h3_digest_vector_is_unconstructible",
            "consumer_zero_without_authoritative_nonempty_census_is_refused",
            "raw_cutover_host_facts_are_unconstructible",
        }.issubset(cases["required_adversarial_cases"]),
        "owner-bound adversarial cases drifted",
    )
    for key, digest in EXPECTED_DIGESTS.items():
        require(historical[key]["manifest_digest"] == digest, f"{key} digest drifted")
    require(historical["e204"]["row_count"] == 204, "E204 count drifted")
    require(historical["c325"]["row_count"] == 325, "C325 count drifted")
    require(historical["physical_census"] == {
        "historical_node_count": 28102,
        "requires_fresh_recensus": True,
    }, "physical census posture drifted")
    historical_receipt = physical_commitment["historical_attested_receipt"]
    require(
        historical_receipt == {
            "node_count": 28102,
            "regular_file_count": 27883,
            "symlink_count": 219,
            "payload_bytes": 2723337235,
            "locator_digest": "0490f6c1960b840181e119d9a5d493a6906686bc3240dfe55f049e5c09d791be",
            "identity_digest_pass1": "29bfc337d3b4187c04f9e61c3a9f0bc012bdaef9fb93cc5af9a6ff58b8505d8c",
            "identity_digest_pass2": "29bfc337d3b4187c04f9e61c3a9f0bc012bdaef9fb93cc5af9a6ff58b8505d8c",
            "stable": True,
            "changed_rows": 0,
        },
        "physical historical commitment drifted",
    )
    require(
        {
            row["category"]: row["count"]
            for row in physical_commitment["historical_category_counts"]
        }
        == EXPECTED_PHYSICAL_CATEGORIES,
        "physical category commitment drifted",
    )
    require(
        physical_commitment["identity_row_grammar"]
        == [
            "type",
            "payload_length",
            "payload_sha256",
            "lexically_normalized_absolute_locator",
        ]
        and physical_commitment["symlink_payload"] == "undereferenced link target"
        and physical_commitment["directory_containers_included"] is False
        and physical_commitment["literal_historical_rows_retained"] is False
        and physical_commitment["stage11_live_migration_admission"]
        == "blocked_pending_recensus",
        "physical census grammar or fail-closed posture drifted",
    )
    require(historical["skill_ledger"] == {
        "file_count": 35,
        "line_count": 3853,
        "byte_count": 193039,
        "rewrite": 19,
        "replace": 9,
        "migration_only": 7,
        "semantic_destination_count": 21,
    }, "skill ledger totals drifted")
    require(
        instance_header["schema"] == "maestro.vnext.stage11.migration-instances.v1",
        "instance fixture schema drifted",
    )
    require(instance_header["row_counts"] == EXPECTED_INSTANCE_COUNTS, "instance header counts drifted")
    require(cases["instance_fixture"]["row_counts"] == EXPECTED_INSTANCE_COUNTS, "case instance counts drifted")
    require(
        cases["instance_fixture"]["sha256"] == hashlib.sha256(instance_bytes).hexdigest(),
        "instance fixture digest drifted",
    )
    grouped = {
        family: [record for record in instance_rows if record["family"] == family]
        for family in EXPECTED_INSTANCE_COUNTS
    }
    require(
        sum(len(rows) for rows in grouped.values()) == len(instance_rows),
        "unknown instance fixture family",
    )
    for family, count in EXPECTED_INSTANCE_COUNTS.items():
        require(len(grouped[family]) == count, f"{family} instance count drifted")
        require(
            [record["ordinal"] for record in grouped[family]] == list(range(1, count + 1)),
            f"{family} instance ordinals drifted",
        )
    exact_sources = {
        "e204": load_public("embedded_resources.e204.v1.json")["rows"],
        "c325": load_public("direct_consumers.c325.v1.json")["rows"],
        "skill_ledger": load_public("v1_skill_ledger.v1.json")["rows"],
    }
    for family, source_rows in exact_sources.items():
        require(
            [record["row"] for record in grouped[family]] == source_rows,
            f"{family} instance rows do not equal the frozen source",
        )
    require(
        cases["instance_fixture"]["physical_posture"]
        == "aggregate_commitment_only_no_fabricated_rows"
        and cases["instance_fixture"]["requires_fresh_live_recensus"] is True,
        "physical fixture posture drifted",
    )
    physical_source = instance_header["source_files"]["physical"]
    require(
        physical_source["literal_historical_rows_retained"] is False
        and physical_source["historical_node_count"] == 28102
        and physical_source["sha256"]
        == hashlib.sha256((PUBLIC / "physical_census.commitment.v1.json").read_bytes()).hexdigest()
        and physical_source["fixture_posture"]
        == "aggregate_commitment_only_no_fabricated_rows"
        and not any(record["family"] == "physical" for record in instance_rows),
        "fabricated physical identity rows remain",
    )
    require(
        gates["authoritative_census"] == {
            "source_manifest_required": True,
            "owner_snapshot_required": True,
            "closure_attestation_required": True,
            "declared_members_must_be_nonzero": True,
            "every_member_resolution": ["Observed", "Removed"],
            "empty_observation_vector_is_not_consumer_zero": True,
        },
        "authoritative census gate drifted",
    )
    require(
        gates["production_owner_routes"]
        == {
            "consumer_snapshot": "ConsumerClosureV1::evaluate_installation_snapshot",
            "durable_consumer_finality": "ConsumerClosureDurableLinearizationV1",
            "aggregate_census": "census_admitted_owner_roots_v2",
            "pre_store_finality": "stage11_finality_v2::execute_pre_store",
        },
        "production owner-route contract drifted",
    )
    require([gate["stage"] for gate in gates["gates"]] == [
        "BeforeSemanticCurrentness",
        "ProtectedRetention",
        "PhysicalPruning",
    ], "consumer gate ordering drifted")

    runtime = "\n".join(path.read_text(encoding="utf-8") for path in sorted(RUNTIME.glob("*.rs")))
    operations = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted(OPERATIONS.glob("*.rs"))
        if path.name != "tests.rs"
    )
    inventory_source = (RUNTIME / "inventory.rs").read_text(encoding="utf-8")
    census_source = (OPERATIONS / "census.rs").read_text(encoding="utf-8")
    consumer_source = (RUNTIME / "consumer.rs").read_text(encoding="utf-8")
    consumer_snapshot_source = (INSTALLATION / "consumer_snapshot.rs").read_text(encoding="utf-8")
    installation_facade = (INSTALLATION / "mod.rs").read_text(encoding="utf-8")
    foundation_facade = (FOUNDATION / "mod.rs").read_text(encoding="utf-8")
    require(
        all(
            token in inventory_source
            for token in (
                "CborValue::Unsigned(kind.tag())",
                "CborValue::Unsigned(payload.byte_length())",
                "payload.sha256().canonical_value()",
                "display_locator.canonical_value()",
            )
        ),
        "runtime source identity grammar drifted",
    )
    require(
        all(
            token in census_source
            for token in (
                "MigrationClassificationContinuationV2",
                "continuation.consume_for_stage11()",
                "Stage11CensusContinuationV2",
                "ProtectedLocatorLeaseV2",
                "stage11_finality_v2",
            )
        ),
        "Stage 11 no longer consumes the V2 Foundation continuation or records its V2 finality dependency",
    )
    for forbidden in (
        "DeclaredRootScanV1",
        "recensus_declared_roots",
        "PathBuf",
        "for_admitted_root_set",
        "descriptor_census_platform::census",
    ):
        require(
            forbidden not in census_source,
            f"Migration reconstructed physical census scope through retired V1 input: {forbidden}",
        )
    operations_facade = (OPERATIONS / "mod.rs").read_text(encoding="utf-8")
    require(
        "mod census;" in operations_facade
        and "pub(crate) use census::{Stage11CensusContinuationV2, consume_foundation_census_v2};"
        in operations_facade
        and "#[cfg(test)]\nmod legacy_census_v1;" in operations_facade
        and "#[cfg(test)]\n#[allow(\n    unused_imports,\n    reason = \"V1 physical census evidence remains test-only while V2 Foundation continuation owns production scope\"\n)]\npub(crate) use legacy_census_v1"
        in operations_facade,
        "Foundation V2 census is not production-wired or V1 evidence escaped test-only scope",
    )
    for token in EXPECTED_DISPOSITIONS:
        require(token in runtime, f"missing disposition {token}")
    require(
        "evaluate_installation_snapshot" in consumer_source
        and "InstallationMigrationConsumerSnapshotV1" in consumer_source
        and "snapshot.into_parts()" in consumer_source,
        "Migration no longer consumes the Installation-owned consumer snapshot",
    )
    require(
        "ConsumerClosureDurableLinearizationV1" in consumer_snapshot_source
        and "bind_migration_census" in consumer_snapshot_source
        and "durable_effect.commit" in consumer_snapshot_source,
        "Installation durable consumer finality route drifted",
    )
    require(
        "pub(in crate::domain) mod stage11_finality_v2" in installation_facade
        and "execute_pre_store" in installation_facade
        and "ProtectedLocatorLeaseV2" in installation_facade,
        "Installation V2 PreStore finality route drifted",
    )
    require(
        "pub(crate) mod stage11_aggregate_census" in foundation_facade
        and "census_from_stage11_owner_v2" in foundation_facade
        and "consume_for_stage11" in foundation_facade
        and "census_admitted_owner_roots_v2" in census_source,
        "Foundation V2 aggregate-census owner route drifted",
    )
    for token in (
        "AuthoritativeConsumerCensusV1",
        "NativeCancellationCausalJoinV1",
        "test_only_from_stage4_publication",
        "SealedQuarantineManifestV1",
        "MigrationCutoverAssociationV1",
        "RefusedBeforeCurrentness",
        "RefusedStaleHost",
        "ActiveStoreFinalityV1",
        "PreStoreFinalityV1",
    ):
        require(token in runtime, f"missing runtime proof {token}")
    require(
        "NativeCancellationCausalJoinV1::new" not in runtime,
        "arbitrary-digest H3 constructor remains",
    )
    require(
        "#[cfg(test)]\n    pub fn test_only_from_stage4_publication" in runtime,
        "incomplete Stage-4 H3 adapter escaped test-only scope",
    )
    require(
        "CancellationJoinRowMismatch" in runtime
        and "Stage4PublicationReused" in runtime,
        "H3 publication is not row-bound and single-consumption",
    )
    require(
        "VerifiedH3WithdrawalPublicationUseV1" in runtime
        and "H3NativeCancelledMigrationMemberV1" in runtime
        and "consume_native_cancelled_member_for_migration" in runtime
        and "H3NativeCancelledSourceMemberV1::new" in runtime
        and "H3NativeCancelledTargetMemberV1::new" in runtime
        and "H3NativeCancelledClassificationV1::new" in runtime
        and "H3MigrationFinalityV1::ActiveStore" in runtime
        and "H3MigrationFinalityV1::PreStore" in runtime
        and "H3CarrierCountMismatch" in runtime
        and "H3MemberCoverageMismatch" in runtime
        and "H3MemberDuplicate" in runtime
        and "H3MemberContradiction" in runtime
        and "H3VerifiedMigrationAssociationUseV1" in runtime
        and "_consumed_members" in runtime
        and "maestro.execution.h3-native-cancelled-member.v1\\0" in runtime
        and ".consume_for_migration(" not in runtime
        and "ConsumedH3WithdrawalPublicationV1" not in runtime,
        "frozen H3 native-cancelled member is not consumed exactly by migration",
    )
    require(
        "ZeroDigest" in runtime and "ZeroIdentity" in runtime,
        "zero migration identities remain admissible",
    )
    require("store.import_inactive(sealed_backup)" in operations, "real inactive Store seam missing")
    for forbidden in ("activate_repository", "activate_installation", "remove_dir_all", "remove_file"):
        require(forbidden not in operations, f"forbidden production mutation present: {forbidden}")

    combined = b"".join(
        (FIXTURES / name).read_bytes()
        for name in (
            "consumer_gates.v1.json",
            "migration_cases.v1.json",
            "migration_instances.v1.jsonl",
        )
    )
    print(json.dumps({
        "schema": "maestro.vnext.stage11.fixture-validation.v1",
        "fixture_sha256": hashlib.sha256(combined).hexdigest(),
        "status": "ok",
    }, sort_keys=True))


if __name__ == "__main__":
    main()
