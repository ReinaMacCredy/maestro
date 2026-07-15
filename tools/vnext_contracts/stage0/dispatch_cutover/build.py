#!/usr/bin/env python3
"""Builds inert Stage-0 DispatchAttempt and Migration cutover literals."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[4]
OUTPUT = ROOT / "contracts/vnext/stage0/dispatch-cutover"

DESIGN_SHA256 = "85787cfb4fb32eefe078adbf9ede66114b12c6304af10857bd676a1cd9875d18"
DECISIONS_SHA256 = "1f97e67b156d5a17d13b94ff955ad17efeb3bb71a4b74b1aec14e20dac1100dd"
CARD_SHA256 = "2cdf1f74843a6eca926ff3bc48e060654350e6a03b65342f8d7be48d111379b4"

PREDECESSOR_ARTIFACT_SHA256 = "f9a2ecbff7b8b1912b78ed7c6b028eb0d9c3bdba92e0d9ac8f0377214e8150d9"
PREDECESSOR_MANIFEST_ID = "60e9a3a77104b74f044527232802841e230e192279881286c0a2a9d3618be2c6"
PREDECESSOR_ASSOCIATION_SCHEMA_ID = "fddd9d43b7f8662187b834a64ef5fb0ba96b2182b6218c1a2c1b5aaca0e26808"
PREDECESSOR_ACTIVE_HEAD_SCHEMA_ID = "55106c12ddae6246d8db91ec5f81b37b527b00214af163b60b30c43b401d44db"
PREDECESSOR_PRESTORE_SEAL_SCHEMA_ID = "dc376892ebcc68640b1c1795fe1f736d5c61a41bbd74d8fa5005aff049df23b5"
PREDECESSOR_FINALITY_EDGE_MANIFEST_ID = "026b61dd18923e40917167af14737124ec11b1cabdb69fdb2422bb50d4a80466"
PREDECESSOR_RW_SET_ID = "99333b038139e952f55ae22bd82383679a978ce8c2559ac44eeaebc15b3addec"
PREDECESSOR_WRITER_EPOCH_ID = "f3e6d7c105193f278bcfdd744d7b715358a59ffc8b7b02c3f17fe1592d1c6e6b"
PREDECESSOR_MIGRATION_EPOCH_ID = "95d517009025279d79108c8cf81418cf101ff77fedd333326fde03ac223e0a69"
C868_SUITE_ID = "5057a0a8e088a0dd394106b928f9b1c4c7a356040b23363455de1177e33e013f"
C868_EDGE_ID = "917376f49f5ed01ab53a7a71f1527fc0b3fc03d2632b47b68333cf2ba7899fe2"
C868_ARTIFACT_SHA256 = "d55e34610d888fca3ec6995820e50fe744332748fe28b766be4c64bbd2672622"


def b(value: str) -> dict[str, str]:
    if len(value) != 64 or any(char not in "0123456789abcdef" for char in value):
        raise ValueError(f"invalid bytes32 literal: {value}")
    return {"bytes": value}


def optional(value: Any | None) -> list[Any]:
    return [0] if value is None else [1, value]


def encode_head(major: int, value: int) -> bytes:
    prefix = major << 5
    if value < 24:
        return bytes([prefix | value])
    if value <= 0xFF:
        return bytes([prefix | 24, value])
    if value <= 0xFFFF:
        return bytes([prefix | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([prefix | 26]) + value.to_bytes(4, "big")
    if value <= 0xFFFFFFFFFFFFFFFF:
        return bytes([prefix | 27]) + value.to_bytes(8, "big")
    raise ValueError("unsigned integer exceeds u64")


def encode(value: Any) -> bytes:
    if isinstance(value, bool):
        return b"\xf5" if value else b"\xf4"
    if isinstance(value, int) and value >= 0:
        return encode_head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return encode_head(3, len(raw)) + raw
    if isinstance(value, list):
        return encode_head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and set(value) == {"bytes"}:
        raw = bytes.fromhex(value["bytes"])
        return encode_head(2, len(raw)) + raw
    raise TypeError(f"outside deterministic CBOR subset: {value!r}")


def dispatch_value() -> list[Any]:
    binding_fields = [
        [1, "attempt_id", "bytes32"],
        [2, "attempt_revision", "nonzero_unsigned"],
        [3, "effect_intent_home_id", "bytes32"],
        [4, "effect_intent_use_fence_id", "bytes32"],
        [5, "application_envelope_id", "bytes32"],
        [6, "provider_operation_contract_id", "bytes32"],
        [7, "provider_scope_id", "bytes32"],
        [8, "provider_key_id", "bytes32"],
        [9, "credential_id", "bytes32"],
        [10, "authority_basis_id", "bytes32"],
        [11, "dispatch_fence_id", "bytes32"],
        [12, "material_stamp_id", "bytes32"],
        [13, "run_set_revision_id", "bytes32"],
        [14, "accounting_basis_id", "bytes32"],
    ]
    outcomes = [
        [1, "locally_rejected", 1],
        [2, "definitely_not_sent", 2],
        [3, "response_received", 2],
        [4, "ambiguous_transport", 2],
    ]
    invariants = [
        [1, "reserved_has_no_seal_and_no_outcome"],
        [2, "in_flight_has_exactly_one_immutable_seal_and_no_terminal_outcome"],
        [3, "preseal_terminal_has_no_seal_and_only_locally_rejected"],
        [4, "sealed_terminal_carries_identical_seal_and_exactly_one_remote_outcome"],
        [5, "direct_reserved_to_sealed_terminal_forbidden"],
        [6, "seal_replacement_forbidden"],
        [7, "sealed_local_rejection_forbidden"],
        [8, "unsealed_remote_outcome_forbidden"],
        [9, "nonterminal_outcome_forbidden"],
        [10, "terminal_escape_forbidden"],
        [11, "duplicate_and_unknown_tags_forbidden"],
        [12, "persisted_state_never_reconstructs_live_release_capability"],
        [13, "only_successful_live_seal_cas_caller_receives_ephemeral_release"],
        [14, "recovery_never_synthesizes_truth_refund_or_retry"],
    ]
    return [
        1,
        "maestro.vnext.dispatch-attempt-state.v1",
        [
            [1, "reserved_unsealed", 0, 0],
            [2, "sealed_in_flight", 1, 0],
            [3, "terminal", 0, 1],
        ],
        [
            [1, "pre_seal_locally_rejected", 0, [1]],
            [2, "sealed_dispatch_terminal", 1, [2, 3, 4]],
        ],
        outcomes,
        [
            [1, optional(None), 3, optional(1)],
            [1, optional(None), 2, optional(None)],
            [2, optional(None), 3, optional(2)],
        ],
        binding_fields,
        [1, "seal_id", "seal_is_exact_binding_snapshot", binding_fields],
        invariants,
        [1, 1, "successful_live_seal_cas_caller_only", False],
        [1, 0, False, False, False, False, ["bounded_handle", "reconcile"]],
    ]


def expected_delta_value() -> list[Any]:
    rows = [
        [1, "7138_public_contract", [], optional(None), True, "rotate_public_contract_dependency_ids"],
        [2, "d116_bounded_recovery", [], optional(None), True, "bind_bounded_recovery_contract"],
        [3, "h2_causal_join", [], optional(None), True, "bind_causal_join_without_evidence_promotion"],
        [4, "h3_cancellation_label", [], optional(None), True, "bind_cancel_label_without_evidence_promotion"],
        [5, "efa0_core_catalogs", [], optional(None), True, "rotate_effect_action_and_grammar_catalog_ids"],
        [
            6,
            "c868_behavioral_suite",
            [b(C868_SUITE_ID), b(C868_EDGE_ID), b(C868_ARTIFACT_SHA256)],
            optional(None),
            True,
            "rotate_dependency_ids_preserve_38_62_61",
        ],
        [7, "release_binding", [], optional(None), True, "freeze_repository_absent_installation_exact_release"],
        [8, "writer_compatibility", [b(PREDECESSOR_WRITER_EPOCH_ID)], optional(None), True, "rotate_writer_compatibility_and_epoch_ids"],
    ]
    return [1, "maestro.vnext.migration-cutover-expected-delta.v1", rows]


def migration_value(delta_id: str) -> list[Any]:
    predecessors = [
        [1, "c868_resource_contract_suite_manifest", b(C868_SUITE_ID)],
        [2, "c868_distribution_runtime_edge_manifest", b(C868_EDGE_ID)],
        [3, "c868_artifact_bytes", b(C868_ARTIFACT_SHA256)],
        [4, "coherent_installation_cutover_decision_body", b("8973259b81aa30ca3c33f48a4dc3dca778b527b6e8051ab90d49677b4e6ee36a")],
        [5, "installation_publication_decision_body", b("6297558046bd2eb4c5e57011a09ae772a943e9f73128596e8d9c75b2629b4d5a")],
        [6, "writer_compatibility_decision_body", b("fc7d4526acb6680ed3c8405e80942583a789704069d75a091e6d71e9176e32b5")],
        [7, "binary_update_decision_body", b("8e17a764e78ab2a5a299e0efd311ed1c05b60f45720da7ad18dcf255c0ff45b9")],
        [8, "v1_migration_decision_body", b("14af689aadf9d155e017ad06a5c4a2c9a3fdba60b01f3148992c3175ac3d72be")],
        [9, "two_snapshot_decision_body", b("9d1982f29c87b72ec4b4e98172a0d91c3279a82a38f66d750c6c15ef1abcfc01")],
        [10, "managed_custody_decision_body", b("c35ea3e23e6f52c0224493a3f650bb49e2a1c45d2c03c0d6676a464ab41a65c2")],
    ]
    counts = [
        [1, "schemas", 12],
        [2, "invariants", 23],
        [3, "predecessors", 10],
        [4, "components", 50],
        [5, "finality_schema_ids", 3],
        [6, "finality_edge_rows", 11],
        [7, "read_write_cohorts", 4],
        [8, "read_write_rows_per_cohort", 46],
        [9, "c868_schemas", 38],
        [10, "c868_suite_components", 62],
        [11, "c868_runtime_edges", 61],
    ]
    association_fields = [
        [1, "association_id"],
        [2, "distribution_domain_ref"],
        [3, "generation"],
        [4, "epoch"],
        [5, "inventory_id"],
        [6, "target_set_id"],
        [7, "quarantine_set_id"],
        [8, "consumer_set_id"],
        [9, "distribution_receipt_id"],
        [10, "candidate_store_root_id"],
        [11, "schema_read_write_set_id"],
        [12, "writer_protocol_epoch_id"],
        [13, "migration_epoch_id"],
        [14, "release_binding"],
        [15, "context_binding"],
    ]
    refusal_reasons = [
        [1, "missing_association"],
        [2, "duplicate_association"],
        [3, "reused_association"],
        [4, "wrong_domain"],
        [5, "wrong_generation"],
        [6, "wrong_epoch"],
        [7, "wrong_receipt"],
        [8, "wrong_release"],
        [9, "incomplete_finality"],
        [10, "mixed_release"],
        [11, "old_reader_or_writer_epoch"],
    ]
    blockers = [[tag, name] for tag, name in enumerate(
        [
            "7138_public_contract",
            "d116_bounded_recovery",
            "h2_causal_join",
            "h3_cancellation_label",
            "efa0_core_catalogs",
            "c868_behavioral_suite",
            "release_binding",
            "writer_compatibility",
        ],
        start=1,
    )]
    return [
        1,
        "maestro.vnext.migration-cutover-successor-candidate.v1",
        counts,
        predecessors,
        [
            b(PREDECESSOR_ARTIFACT_SHA256),
            b(PREDECESSOR_MANIFEST_ID),
            [
                b(PREDECESSOR_ASSOCIATION_SCHEMA_ID),
                b(PREDECESSOR_ACTIVE_HEAD_SCHEMA_ID),
                b(PREDECESSOR_PRESTORE_SEAL_SCHEMA_ID),
            ],
            b(PREDECESSOR_FINALITY_EDGE_MANIFEST_ID),
            b(PREDECESSOR_RW_SET_ID),
            b(PREDECESSOR_WRITER_EPOCH_ID),
            b(PREDECESSOR_MIGRATION_EPOCH_ID),
        ],
        [
            ["successor_manifest_id", optional(None)],
            ["finality_schema_ids", [optional(None), optional(None), optional(None)]],
            ["finality_edge_manifest_id", optional(None)],
            ["schema_read_write_set_id", optional(None)],
            ["writer_protocol_epoch_id", optional(None)],
            ["migration_epoch_id", optional(None)],
            ["c868_suite_manifest_id", optional(None)],
            ["c868_edge_manifest_id", optional(None)],
        ],
        association_fields,
        [
            [1, "repository", optional(None)],
            [2, "installation", optional("exact_release_id")],
        ],
        [
            [
                1,
                "active_store",
                ["distribution_receipt", "distribution_commit_record"],
                ["atomic", "migration_cutover_association", "owning_head"],
            ],
            [
                2,
                "pre_store",
                ["sealed_ceremony_attempt"],
                [
                    "atomic",
                    "migration_cutover_association",
                    "candidate_seal",
                    "protected_expected_old_cas",
                ],
            ],
        ],
        refusal_reasons,
        [
            [1, "association_is_typed_atomic_participant", True],
            [2, "association_consumed_exactly_once", True],
            [3, "filename_or_sidecar_inference", False],
            [4, "old_reader_admission", False],
            [5, "h2_causal_join_promotes_evidence", False],
            [6, "h3_cancel_label_promotes_evidence", False],
            [7, "partial_finality_is_current", False],
            [8, "writer_and_migration_epoch_match_required", True],
            [9, "receipt_mutation_is_migrate", True],
        ],
        [[1, 46], [2, 46], [3, 46], [4, 46]],
        [38, 62, 61, b(C868_SUITE_ID), b(C868_EDGE_ID), optional(None), optional(None)],
        b(delta_id),
        blockers,
    ]


def artifact(domain: str, schema: str, value: list[Any], extra: dict[str, Any]) -> tuple[dict[str, Any], bytes]:
    envelope = [domain, value]
    encoded = encode(envelope)
    digest = hashlib.sha256(encoded).hexdigest()
    document = {
        "schema": schema,
        "identity_domain": domain,
        "candidate_literal_id": digest,
        "cbor_sha256": digest,
        "byte_length": len(encoded),
        "canonical_value": value,
        "runtime_activated": False,
        **extra,
    }
    return document, encoded


def outputs() -> dict[Path, bytes]:
    delta, delta_cbor = artifact(
        "maestro.vnext.migration-cutover-expected-delta.candidate-literal.v1",
        "maestro.vnext.migration-cutover-expected-delta-artifact.v1",
        expected_delta_value(),
        {"publication_status": "blocked_unresolved_dependencies", "successor_ids": [None] * 8},
    )
    dispatch, dispatch_cbor = artifact(
        "maestro.vnext.dispatch-attempt-state.candidate-literal.v1",
        "maestro.vnext.dispatch-attempt-state-artifact.v1",
        dispatch_value(),
        {"publication_status": "candidate_only", "outcome_count": 4, "legal_transition_count": 3},
    )
    migration, migration_cbor = artifact(
        "maestro.vnext.migration-cutover-successor.candidate-literal.v1",
        "maestro.vnext.migration-cutover-successor-artifact.v1",
        migration_value(delta["candidate_literal_id"]),
        {
            "publication_status": "blocked_unresolved_dependencies",
            "successor_manifest_id": None,
            "current_finality_schema_ids": [None, None, None],
            "expected_delta_literal_id": delta["candidate_literal_id"],
        },
    )
    documents = {
        OUTPUT / "dispatch-attempt-state.v1.json": json_bytes(dispatch),
        OUTPUT / "dispatch-attempt-state.v1.cbor": dispatch_cbor,
        OUTPUT / "expected-delta-manifest.v1.json": json_bytes(delta),
        OUTPUT / "expected-delta-manifest.v1.cbor": delta_cbor,
        OUTPUT / "migration-cutover-successor.v1.json": json_bytes(migration),
        OUTPUT / "migration-cutover-successor.v1.cbor": migration_cbor,
    }
    receipt = {
        "schema": "maestro.vnext.dispatch-cutover-build-receipt.v1",
        "status": "pass",
        "runtime_activated": False,
        "source_sha256": {
            "design.md": DESIGN_SHA256,
            "decisions.yaml": DECISIONS_SHA256,
            "card.yaml": CARD_SHA256,
        },
        "artifact_ids": {
            "dispatch": dispatch["candidate_literal_id"],
            "expected_delta": delta["candidate_literal_id"],
            "migration_candidate": migration["candidate_literal_id"],
        },
        "blocked_dependency_count": 8,
        "build_script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    }
    documents[OUTPUT / "build-receipt.v1.json"] = json_bytes(receipt)
    return documents


def json_bytes(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = outputs()
    mismatches: list[str] = []
    for path, content in expected.items():
        if args.check:
            if not path.exists() or path.read_bytes() != content:
                mismatches.append(str(path.relative_to(ROOT)))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
    receipt = {
        "status": "pass" if not mismatches else "fail",
        "mode": "check" if args.check else "write",
        "outputs": len(expected),
        "mismatches": mismatches,
    }
    print(json.dumps(receipt, sort_keys=True, separators=(",", ":")))
    return 0 if not mismatches else 1


if __name__ == "__main__":
    raise SystemExit(main())
