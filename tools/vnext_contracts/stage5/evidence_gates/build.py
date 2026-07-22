#!/usr/bin/env python3
"""Build one inactive Stage 5 Evidence and Gates proof artifact."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(WORKSPACE / "tools/vnext_contracts/catalogs"))
sys.path.insert(0, str(TOOLS))
import cbor_py  # type: ignore[import-not-found]  # noqa: E402
from behavior import (  # type: ignore[import-not-found]  # noqa: E402
    EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
    EXPECTED_RUNS,
    EXPECTED_TESTS,
    compiled_behavior,
)


DOMAIN = "maestro.vnext.stage5.evidence-gates.v1"
PUBLICATION_STATE = "inactive_candidate"
DIAGNOSTIC_PROOF_CLAIM = "test_adapter_only"
ARTIFACT_KEYS = {
    "artifact_id",
    "behavior",
    "behavior_manifest_identity",
    "byte_length",
    "cbor_hex",
    "domain",
    "diagnostic_proof_claim",
    "invalidation_reasons",
    "invariants",
    "observation_catalog_manifest_id",
    "observation_contract_table_identity",
    "observation_kinds",
    "predecessors",
    "protocol",
    "publication_state",
    "schema_version",
    "source_closure",
    "stage",
}
BUILDER_RECEIPT_KEYS = {
    "artifact_id",
    "artifact_sha256",
    "behavior_manifest_identity",
    "behavior_passed",
    "behavior_runs",
    "builder_sha256",
    "diagnostic_proof_claim",
    "publication_state",
    "receipt_identity",
    "schema_version",
    "source_closure_sha256",
}
SOURCE_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "contracts/vnext/catalogs/generated/catalog-01-observation.json",
    "contracts/vnext/catalogs/generated/catalog-09-action-spec.json",
    "src/lib.rs",
    "src/domain/mod.rs",
    "src/domain/vnext/mod.rs",
    "src/domain/vnext/authority/action_basis.rs",
    "src/domain/vnext/authority/downstream_action_basis.rs",
    "src/domain/vnext/authority/facade.rs",
    "src/domain/vnext/authority/facade_tests.rs",
    "src/domain/vnext/authority/facade/repository_admission.rs",
    "src/domain/vnext/authority/facade/repository_leaf_authority.rs",
    "src/domain/vnext/authority/mod.rs",
    "src/domain/vnext/authority/protected_diagnostic_envelope.rs",
    "src/domain/vnext/authority/protected_diagnostic_envelope_stage8_seed.rs",
    "src/domain/vnext/authority/result.rs",
    "src/domain/vnext/contract/runtime.rs",
    "src/domain/vnext/evidence/assessment.rs",
    "src/domain/vnext/evidence/claim.rs",
    "src/domain/vnext/evidence/erasure.rs",
    "src/domain/vnext/evidence/identity.rs",
    "src/domain/vnext/evidence/mod.rs",
    "src/domain/vnext/evidence/observation.rs",
    "src/domain/vnext/evidence/submission_claim.rs",
    "src/domain/vnext/evidence/store.rs",
    "src/domain/vnext/execution/store.rs",
    "src/domain/vnext/execution/runtime.rs",
    "src/domain/vnext/gate/mod.rs",
    "src/domain/vnext/integration/mod.rs",
    "src/domain/vnext/integration/trusted_host_diagnostic.rs",
    "src/domain/vnext/integration/trusted_host_diagnostic_stage10_seed.rs",
    "src/domain/vnext/persistence/mod.rs",
    "src/domain/vnext/persistence/idempotency.rs",
    "src/domain/vnext/persistence/metadata.rs",
    "src/domain/vnext/persistence/store.rs",
    "src/domain/vnext/persistence/protected_diagnostic.rs",
    "src/domain/vnext/persistence/protected_diagnostic_stage9_seed.rs",
    "src/domain/vnext/persistence/tests/atomic_publication.rs",
    "src/domain/vnext/repository/mod.rs",
    "src/domain/vnext/repository/tests.rs",
    "src/domain/vnext/work/lifecycle.rs",
    "src/domain/vnext/work/mod.rs",
    "src/domain/vnext/work/submission.rs",
    "src/foundation/core/secure_fs.rs",
    "tests/vnext_evidence_claims.rs",
    "tests/vnext_submission_claim_set.rs",
    "tests/vnext_stage5_contracts.rs",
    "tests/vnext_stage5_evidence_gates.rs",
    "tests/architecture_imports.rs",
    "tests/vnext_work_lifecycle.rs",
    "tools/vnext_contracts/catalogs/cbor_py.py",
    "tools/vnext_contracts/proof_engine/__init__.py",
    "tools/vnext_contracts/proof_engine/README.md",
    "tools/vnext_contracts/proof_engine/engine.py",
    "tools/vnext_contracts/proof_engine/test_engine.py",
    "tools/vnext_contracts/stage5/evidence_gates/behavior.py",
    "tools/vnext_contracts/stage5/evidence_gates/build.py",
    "tools/vnext_contracts/stage5/evidence_gates/consensus.py",
    "tools/vnext_contracts/stage5/evidence_gates/harness.py",
    "tools/vnext_contracts/stage5/evidence_gates/predecessor.py",
    "tools/vnext_contracts/stage5/evidence_gates/validate.py",
    "tools/vnext_contracts/stage5/evidence_gates/verify.rb",
    "tools/vnext_contracts/stage5/evidence_gates/seal.py",
    "tools/vnext_contracts/stage5/evidence_gates/test_consensus.py",
    "tools/vnext_contracts/stage5/evidence_gates/test_consensus_harness_contract.py",
    "tools/vnext_contracts/stage5/evidence_gates/test_seal.py",
    "tools/vnext_contracts/stage5/evidence_gates/test_toolchain.py",
    "tools/vnext_contracts/stage5/evidence_gates/toolchain.py",
)
PREDECESSOR_PATHS = (
    "contracts/vnext/stage4/execution/execution-effects.v1.json",
    "contracts/vnext/stage4/execution/execution-effects.v1.cbor",
    "contracts/vnext/stage4/execution/behavioral-proof-receipt.v1.json",
    "contracts/vnext/stage4/execution/python-encoder-receipt.v1.json",
    "contracts/vnext/stage4/execution/semantic-validation-receipt.v1.json",
    "contracts/vnext/stage4/execution/ruby-verification-receipt.v1.json",
)
OBSERVATION_CATALOG = "contracts/vnext/catalogs/generated/catalog-01-observation.json"
OBSERVATION_CONTRACT_TABLE_IDENTITY = (
    "sha256:a5f0e9137c091972802cb7084d86070a930091f0570cefcc7df445074478a676"
)
RESULTS = ((1, "Pass"), (2, "Fail"), (3, "Indeterminate"), (4, "Error"))
INPUT_CLASSES = ((1, "Evidence"), (2, "Authority"), (3, "Mixed"), (4, "Composite"))
OPERATORS = (
    (1, "Leaf"),
    (2, "All"),
    (3, "Any"),
    (4, "Quorum"),
    (5, "Veto"),
    (6, "DenyOverrides"),
)
ACQUISITION_MODES = (
    (1, "EffectFree", "zero_run"),
    (2, "RunMediated", "exact_execution_attempt_owner"),
    (3, "DeclaredDerivation", "source_observation_closure"),
)
INVALIDATION_REASONS = (
    (1, "WorkGenerationAdvanced"),
    (2, "StepRevisionAdvanced"),
    (3, "GateSnapshotChanged"),
    (4, "EvaluatorChanged"),
    (5, "InputTombstoned"),
    (6, "InputCorrected"),
    (7, "FreshnessExpired"),
    (8, "IntegrityFailure"),
    (9, "AuthorizationReceiptRevoked"),
)
INVARIANTS = (
    "observation_kind_exact_43_dense_closed",
    "observation_catalog_binds_producer_action_routes_and_cma",
    "observation_payload_schemas_are_exact_typed_and_kind_specific",
    "observation_scope_binds_exact_work_step_submission_and_generation",
    "observation_secret_scan_redaction_and_retention_are_typed_and_authenticated",
    "secret_scan_is_deterministically_recomputed_from_exact_payload_bytes",
    "observation_is_immutable_non_bearer",
    "observation_publication_requires_typed_action_authority_and_atomic_store_index",
    "stored_evidence_records_require_canonical_identity_consistent_decoding",
    "payload_identity_distinct_from_observation_identity",
    "effect_free_acquisition_has_zero_run",
    "effecting_acquisition_binds_exact_run_and_attempt_owner",
    "acquisition_identity_is_unique_per_store",
    "declared_derivation_equals_lineage",
    "claim_binds_exactly_one_submission",
    "claim_publication_resolves_exact_observation_records",
    "submission_claim_set_has_exact_three_field_carrier",
    "assessment_evaluates_exactly_one_gate_node",
    "assessment_scope_store_generation_and_evidence_cut_are_exact",
    "assessment_support_binds_pairwise_independent_contributors_and_sources",
    "assessment_uses_trusted_time_freshness_and_pinned_trust_root",
    "empirical_authority_and_composite_inputs_are_nominally_distinct",
    "gate_snapshot_is_complete_content_addressed_and_acyclic",
    "gate_snapshot_has_no_detached_nodes",
    "gate_leaf_cannot_accept_a_proposed_result",
    "gate_composite_evaluation_is_pure_and_pinned",
    "closed_semantic_leaf_evaluators_produce_pass_or_fail_from_exact_inputs",
    "only_pass_derives_satisfaction",
    "fail_indeterminate_and_error_block",
    "equally_applicable_conflict_is_indeterminate",
    "applicability_has_no_newest_selector",
    "invalidation_requires_typed_authority_and_exact_evidence_cut",
    "assessment_and_invalidation_publication_require_complete_store_derived_cut",
    "security_erasure_derives_complete_narrow_invalidation_closure",
    "security_erasure_transitively_invalidates_composite_dependents",
    "security_erasure_publishes_in_doubt_intent_before_physical_absence",
    "security_erasure_receipt_requires_verified_physical_absence_and_exact_resume",
    "security_erasure_revokes_every_secret_bearing_sealed_export_under_one_durable_barrier",
    "security_erasure_restores_exact_insert_only_schema_before_publication_commit",
    "security_erasure_finalization_survives_authority_head_advance",
    "physical_erasure_never_resolves_while_hard_link_or_crash_debt_remains",
    "atomic_publication_builders_reduce_supersets_to_the_exact_generation_closure",
    "raw_atomic_publication_rejects_every_object_outside_the_exact_generation_closure",
    "idempotency_results_remain_durable_replay_horizons",
    "work_completion_atomically_commits_current_claim_gate_and_submission_evidence",
    "work_completion_requires_repository_derived_current_satisfied_submission_closure",
    "persisted_invalidation_rejoins_exact_authorized_action_and_effect_intent",
    "stage3_claim_and_work_submission_v1_bytes_remain_exact",
    "scheduling_and_admission_assessments_are_outside_evidence",
)
def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_row(relative: str) -> list[object]:
    path = WORKSPACE / relative
    data = path.read_bytes()
    return [relative, len(data), sha256(data)]


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")


def pretty_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode("ascii")


def observation_rows(catalog: dict[str, Any]) -> list[list[object]]:
    if (
        catalog.get("schema_version") != "maestro.vnext.catalog.literal.v1"
        or catalog.get("publication_state") != "inactive_candidate"
        or catalog.get("catalog_tag") != 1
        or catalog.get("catalog_slug") != "observation"
        or catalog.get("catalog_type") != "ObservationKindV1"
    ):
        raise RuntimeError("Observation catalog header identity differs")
    schemas = catalog.get("schemas", {})
    if set(schemas) != {"descriptor", "header", "manifest"}:
        raise RuntimeError("Observation catalog schema closure differs")
    for schema in schemas.values():
        encoded = cbor_py.encode(schema["identity_envelope"])
        if (
            encoded.hex() != schema["cbor_hex"]
            or len(encoded) != schema["byte_length"]
            or sha256(encoded) != schema["schema_id"]
        ):
            raise RuntimeError("Observation catalog schema identity differs")
    descriptors = catalog.get("descriptors", [])
    expected_cma = {
        29: ([45], [1], [[1, 1]]), 30: ([45], [2], [[1, 2]]),
        31: ([45], [7], [[4, 7]]), 32: ([45], [8], [[5, 8]]),
        33: ([45], [4, 6], [[2, 4], [3, 6]]), 34: ([45], [3], [[2, 3]]),
        35: ([45], [5], [[3, 5]]), 36: ([45], [9], [[5, 9]]),
        37: ([45], [10], [[5, 10]]),
    }
    for index, descriptor in enumerate(descriptors, start=1):
        value = descriptor["value"]
        encoded = cbor_py.encode(descriptor["identity_envelope"])
        expected_relations = expected_cma.get(
            index,
            ([43], [], []) if index == 17 else ([44], [], []) if index == 18 else ([39], [], []),
        )
        if (
            value[0] != index
            or descriptor["identity_envelope"][2] != value
            or encoded.hex() != descriptor["cbor_hex"]
            or len(encoded) != descriptor["byte_length"]
            or sha256(encoded) != descriptor["descriptor_id"]
            or (value[3], value[4], value[5]) != expected_relations
        ):
            raise RuntimeError("Observation descriptor identity or producer relation differs")
    if len(descriptors) != 43:
        raise RuntimeError("ObservationKindV1 catalog is not the exact dense 43-row closure")
    owner = catalog["primary_owner_relation"]
    owner_encoded = cbor_py.encode(owner["identity_envelope"])
    expected_owner_rows = [[row["value"][0], *row["value"][2]] for row in descriptors]
    if (
        owner["rows"] != expected_owner_rows
        or owner["identity_envelope"][1] != expected_owner_rows
        or owner_encoded.hex() != owner["cbor_hex"]
        or len(owner_encoded) != owner["byte_length"]
        or sha256(owner_encoded) != owner["relation_id"]
    ):
        raise RuntimeError("Observation primary-owner relation differs")
    header = catalog["manifest_header"]
    if (
        header[:3] != [1, 1, 1]
        or header[3] != {"bytes": catalog["grammar_id"]}
        or header[4] != []
        or header[5] != {"bytes": owner["relation_id"]}
        or header[6:8] != [43, 43]
        or header[10] != 1
    ):
        raise RuntimeError("Observation manifest header grammar or ownership binding differs")
    expected_rows = [
        [row["value"][0], {"bytes": row["descriptor_id"]}, row["value"]]
        for row in descriptors
    ]
    manifest_encoded = cbor_py.encode(catalog["manifest_identity_envelope"])
    if (
        catalog["manifest_rows"] != expected_rows
        or catalog["manifest_identity_envelope"][3] != header
        or catalog["manifest_identity_envelope"][4] != expected_rows
        or manifest_encoded.hex() != catalog["cbor_hex"]
        or len(manifest_encoded) != catalog["byte_length"]
        or sha256(manifest_encoded) != catalog["manifest_id"]
    ):
        raise RuntimeError("Observation manifest canonical bytes or identity differs")
    return expected_rows


def run_behavior(cargo: Path, rustc: Path) -> dict[str, object]:
    runs = compiled_behavior(cargo, rustc, WORKSPACE)
    return {
        "passed": sum(int(run["passed"]) for run in runs),
        "runs": runs,
    }


def exact_behavior_runs(runs: object) -> bool:
    if not isinstance(runs, list) or len(runs) != len(EXPECTED_RUNS) + 1:
        return False
    binary_by_target: dict[str, str] = {}
    for run, (label, target, tests) in zip(runs[:-1], EXPECTED_RUNS, strict=True):
        if not isinstance(run, dict) or set(run) != {
            "binary_sha256",
            "label",
            "passed",
            "tests",
        }:
            return False
        binary = run.get("binary_sha256")
        actual_tests = run.get("tests")
        if (
            run.get("label") != label
            or type(run.get("passed")) is not int
            or run.get("passed") != len(tests)
            or not isinstance(binary, str)
            or re.fullmatch(r"[0-9a-f]{64}", binary) is None
            or not isinstance(actual_tests, list)
            or len(actual_tests) != len(tests)
        ):
            return False
        if binary_by_target.setdefault(target, binary) != binary:
            return False
        for actual, test in zip(actual_tests, tests, strict=True):
            if actual != {
                "command": [target, test, "--exact", "--nocapture"],
                "name": test,
                "result": "pass",
            }:
                return False
    first_target = EXPECTED_RUNS[0][1]
    first_test = EXPECTED_RUNS[0][2][0]
    mutant = runs[-1]
    return isinstance(mutant, dict) and mutant == {
        "binary_sha256": binary_by_target[first_target],
        "command": [
            first_target,
            f"{first_test}_same_count_substitution_mutant",
            "--exact",
            "--nocapture",
        ],
        "label": "same-count-substitution-mutant",
        "passed": 0,
        "rejected": True,
        "result": "rejected",
        "substituted_for": first_test,
    }


def exact_behavior(behavior: object) -> bool:
    if behavior == {"mode": "preflight", "passed": 0}:
        return True
    return (
        isinstance(behavior, dict)
        and set(behavior) == {"passed", "runs"}
        and behavior.get("passed") == EXPECTED_TESTS
        and exact_behavior_runs(behavior.get("runs"))
    )


def build(output_root: Path, cargo: Path, rustc: Path, execute_behavior: bool) -> None:
    catalog_path = WORKSPACE / OBSERVATION_CATALOG
    catalog = json.loads(catalog_path.read_text(encoding="ascii"))
    sources = [file_row(path) for path in sorted(SOURCE_PATHS)]
    predecessors = [file_row(path) for path in PREDECESSOR_PATHS]
    observation = observation_rows(catalog)
    behavior = run_behavior(cargo, rustc) if execute_behavior else {"passed": 0, "mode": "preflight"}
    semantic_value: list[object] = [
        DOMAIN,
        PUBLICATION_STATE,
        DIAGNOSTIC_PROOF_CLAIM,
        5,
        catalog["manifest_id"],
        OBSERVATION_CONTRACT_TABLE_IDENTITY,
        observation,
        [list(row) for row in RESULTS],
        [list(row) for row in INPUT_CLASSES],
        [list(row) for row in OPERATORS],
        [list(row) for row in ACQUISITION_MODES],
        [list(row) for row in INVALIDATION_REASONS],
        list(INVARIANTS),
        sources,
        predecessors,
        EXPECTED_TESTS,
        EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
    ]
    encoded = cbor_py.encode(semantic_value)
    artifact_id = sha256(encoded)
    artifact = {
        "artifact_id": artifact_id,
        "behavior": behavior,
        "behavior_manifest_identity": EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
        "byte_length": len(encoded),
        "cbor_hex": encoded.hex(),
        "domain": DOMAIN,
        "diagnostic_proof_claim": DIAGNOSTIC_PROOF_CLAIM,
        "invalidation_reasons": [list(row) for row in INVALIDATION_REASONS],
        "invariants": list(INVARIANTS),
        "observation_catalog_manifest_id": catalog["manifest_id"],
        "observation_contract_table_identity": OBSERVATION_CONTRACT_TABLE_IDENTITY,
        "observation_kinds": observation,
        "predecessors": predecessors,
        "protocol": {
            "acquisition_modes": [list(row) for row in ACQUISITION_MODES],
            "gate_input_classes": [list(row) for row in INPUT_CLASSES],
            "gate_operators": [list(row) for row in OPERATORS],
            "gate_results": [list(row) for row in RESULTS],
        },
        "publication_state": PUBLICATION_STATE,
        "schema_version": DOMAIN,
        "source_closure": sources,
        "stage": 5,
    }
    if (
        set(artifact) != ARTIFACT_KEYS
        or artifact["diagnostic_proof_claim"] != DIAGNOSTIC_PROOF_CLAIM
        or not exact_behavior(artifact["behavior"])
        or artifact
        != {
            "artifact_id": artifact_id,
            "behavior": behavior,
            "behavior_manifest_identity": EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
            "byte_length": len(encoded),
            "cbor_hex": encoded.hex(),
            "domain": DOMAIN,
            "diagnostic_proof_claim": DIAGNOSTIC_PROOF_CLAIM,
            "invalidation_reasons": [list(row) for row in INVALIDATION_REASONS],
            "invariants": list(INVARIANTS),
            "observation_catalog_manifest_id": catalog["manifest_id"],
            "observation_contract_table_identity": OBSERVATION_CONTRACT_TABLE_IDENTITY,
            "observation_kinds": observation,
            "predecessors": predecessors,
            "protocol": {
                "acquisition_modes": [list(row) for row in ACQUISITION_MODES],
                "gate_input_classes": [list(row) for row in INPUT_CLASSES],
                "gate_operators": [list(row) for row in OPERATORS],
                "gate_results": [list(row) for row in RESULTS],
            },
            "publication_state": PUBLICATION_STATE,
            "schema_version": DOMAIN,
            "source_closure": sources,
            "stage": 5,
        }
    ):
        raise RuntimeError("Stage 5 artifact proof claim schema differs")
    artifact_bytes = pretty_json(artifact)
    receipt_value = {
        "artifact_id": artifact_id,
        "artifact_sha256": sha256(artifact_bytes),
        "behavior_manifest_identity": EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
        "behavior_passed": behavior["passed"],
        "behavior_runs": behavior.get("runs", []),
        "builder_sha256": file_row("tools/vnext_contracts/stage5/evidence_gates/build.py")[2],
        "diagnostic_proof_claim": DIAGNOSTIC_PROOF_CLAIM,
        "publication_state": PUBLICATION_STATE,
        "schema_version": "maestro.vnext.stage5.python-builder-receipt.v1",
        "source_closure_sha256": sha256(canonical_json(sources)),
    }
    receipt = {
        **receipt_value,
        "receipt_identity": f"sha256:{sha256(canonical_json(receipt_value))}",
    }
    if (
        set(receipt) != BUILDER_RECEIPT_KEYS
        or receipt["diagnostic_proof_claim"] != DIAGNOSTIC_PROOF_CLAIM
        or receipt["behavior_runs"] != behavior.get("runs", [])
        or (receipt["behavior_runs"] != [] and not exact_behavior_runs(receipt["behavior_runs"]))
    ):
        raise RuntimeError("Stage 5 builder receipt proof claim schema differs")
    output_root.mkdir(parents=True, exist_ok=True)
    (output_root / "evidence-gates.v1.json").write_bytes(artifact_bytes)
    (output_root / "evidence-gates.v1.cbor").write_bytes(encoded)
    (output_root / "python-builder-receipt.v1.json").write_bytes(pretty_json(receipt))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--cargo", type=Path, required=True)
    parser.add_argument("--rustc", type=Path, required=True)
    parser.add_argument("--run-behavior", action="store_true")
    args = parser.parse_args()
    build(args.output_root, args.cargo.resolve(strict=True), args.rustc.resolve(strict=True), args.run_behavior)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
