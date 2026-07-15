#!/usr/bin/env python3
"""Independent semantic validation and mutants for Dispatch/cutover literals."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[4]
DEFAULT_CONTRACT = ROOT / "contracts/vnext/stage0/dispatch-cutover"
RUBY_VALIDATOR = Path(__file__).with_name("verify.rb")

STEMS = {
    "dispatch": "dispatch-attempt-state.v1",
    "expected_delta": "expected-delta-manifest.v1",
    "migration_candidate": "migration-cutover-successor.v1",
}
DELTA_NAMES = [
    "7138_public_contract",
    "d116_bounded_recovery",
    "h2_causal_join",
    "h3_cancellation_label",
    "efa0_core_catalogs",
    "c868_behavioral_suite",
    "release_binding",
    "writer_compatibility",
]


class ValidationError(Exception):
    pass


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
    raise ValidationError("unsigned integer exceeds u64")


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
        if len(raw) != 32:
            raise ValidationError("bytes32 required")
        return encode_head(2, len(raw)) + raw
    raise ValidationError(f"outside deterministic CBOR subset: {value!r}")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def load_documents(contract: Path) -> dict[str, dict[str, Any]]:
    return {
        label: json.loads((contract / f"{stem}.json").read_text(encoding="ascii"))
        for label, stem in STEMS.items()
    }


def validate_envelope(contract: Path, label: str, document: dict[str, Any]) -> None:
    encoded = encode([document["identity_domain"], document["canonical_value"]])
    stem = STEMS[label]
    require((contract / f"{stem}.cbor").read_bytes() == encoded, f"{label}: CBOR mismatch")
    digest = hashlib.sha256(encoded).hexdigest()
    require(document["candidate_literal_id"] == digest, f"{label}: identity mismatch")
    require(document["cbor_sha256"] == digest, f"{label}: CBOR digest mismatch")
    require(document["byte_length"] == len(encoded), f"{label}: byte length mismatch")
    require(document["runtime_activated"] is False, f"{label}: runtime activation forbidden")


def validate_dispatch(document: dict[str, Any]) -> None:
    value = document["canonical_value"]
    require(value[:2] == [1, "maestro.vnext.dispatch-attempt-state.v1"], "dispatch header")
    require(value[2] == [[1, "reserved_unsealed", 0, 0], [2, "sealed_in_flight", 1, 0], [3, "terminal", 0, 1]], "dispatch state union")
    require(value[3] == [[1, "pre_seal_locally_rejected", 0, [1]], [2, "sealed_dispatch_terminal", 1, [2, 3, 4]]], "dispatch terminal union")
    require(value[4] == [[1, "locally_rejected", 1], [2, "definitely_not_sent", 2], [3, "response_received", 2], [4, "ambiguous_transport", 2]], "dispatch outcome projection")
    require(value[5] == [[1, [0], 3, [1, 1]], [1, [0], 2, [0]], [2, [0], 3, [1, 2]]], "dispatch legal transitions")
    fields = value[6]
    require([row[0] for row in fields] == list(range(1, 15)), "dispatch binding tags")
    require(len({row[1] for row in fields}) == 14, "dispatch binding fields")
    require(value[7] == [1, "seal_id", "seal_is_exact_binding_snapshot", fields], "dispatch identical seal")
    require([row[0] for row in value[8]] == list(range(1, 15)), "dispatch invariant tags")
    require(value[9] == [1, 1, "successful_live_seal_cas_caller_only", False], "dispatch race descriptor")
    require(value[10] == [1, 0, False, False, False, False, ["bounded_handle", "reconcile"]], "dispatch zero-I/O recovery")
    require(document["outcome_count"] == 4, "dispatch outcome count")
    require(document["legal_transition_count"] == 3, "dispatch transition count")


def validate_delta(document: dict[str, Any]) -> None:
    value = document["canonical_value"]
    require(value[:2] == [1, "maestro.vnext.migration-cutover-expected-delta.v1"], "delta header")
    rows = value[2]
    require(len(rows) == 8, "delta row count")
    require([row[0] for row in rows] == list(range(1, 9)), "delta tags")
    require([row[1] for row in rows] == DELTA_NAMES, "delta names")
    require(all(row[3] == [0] and row[4] is True for row in rows), "delta blocking option-none")
    require(len(rows[5][2]) == 3 and "38_62_61" in rows[5][5], "c868 delta evidence")
    require(document["successor_ids"] == [None] * 8, "delta successor identities")
    require(document["publication_status"] == "blocked_unresolved_dependencies", "delta publication status")


def validate_migration(document: dict[str, Any], delta_id: str) -> None:
    value = document["canonical_value"]
    require(value[:2] == [1, "maestro.vnext.migration-cutover-successor-candidate.v1"], "migration header")
    require(
        value[2]
        == [
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
        ],
        "migration preserved counts",
    )
    predecessors = value[3]
    require(len(predecessors) == 10, "migration predecessor count")
    require([row[0] for row in predecessors] == list(range(1, 11)), "migration predecessor tags")
    require(len({row[2]["bytes"] for row in predecessors}) == 10, "migration predecessor reuse")
    require(len(value[4][2]) == 3, "predecessor finality schemas")
    current = value[5]
    require(current[0] == ["successor_manifest_id", [0]], "successor ManifestId blocker")
    require(current[1] == ["finality_schema_ids", [[0], [0], [0]]], "successor SchemaId blockers")
    require(all(row[1] in ([0], [[0], [0], [0]]) for row in current), "fabricated successor identity")
    require(len(value[6]) == 15 and [row[0] for row in value[6]] == list(range(1, 16)), "association fields")
    require(value[7] == [[1, "repository", [0]], [2, "installation", [1, "exact_release_id"]]], "Release matrix")
    require(value[8][0] == [1, "active_store", ["distribution_receipt", "distribution_commit_record"], ["atomic", "migration_cutover_association", "owning_head"]], "ActiveStore finality")
    require(value[8][1] == [2, "pre_store", ["sealed_ceremony_attempt"], ["atomic", "migration_cutover_association", "candidate_seal", "protected_expected_old_cas"]], "PreStore finality")
    require(len(value[9]) == 11 and [row[0] for row in value[9]] == list(range(1, 12)), "currentness refusal matrix")
    policies = {row[1]: row[2] for row in value[10]}
    require(policies["association_is_typed_atomic_participant"] is True, "association participant")
    require(policies["association_consumed_exactly_once"] is True, "association reuse")
    require(policies["filename_or_sidecar_inference"] is False, "filename inference")
    require(policies["old_reader_admission"] is False, "old reader admission")
    require(policies["h2_causal_join_promotes_evidence"] is False, "H2 evidence promotion")
    require(policies["h3_cancel_label_promotes_evidence"] is False, "H3 evidence promotion")
    require(policies["partial_finality_is_current"] is False, "partial finality")
    require(value[11] == [[1, 46], [2, 46], [3, 46], [4, 46]], "read/write cohorts")
    require(value[12][:3] == [38, 62, 61], "c868 preserved counts")
    require(value[12][5:] == [[0], [0]], "c868 rotated identity blockers")
    require(value[13]["bytes"] == delta_id, "expected-delta identity binding")
    require([row[1] for row in value[14]] == DELTA_NAMES, "migration blocker slots")
    require(document["successor_manifest_id"] is None, "successor ManifestId fabricated")
    require(document["current_finality_schema_ids"] == [None, None, None], "current SchemaIds fabricated")
    require(document["publication_status"] == "blocked_unresolved_dependencies", "migration publication status")


def validate_contract(contract: Path) -> dict[str, Any]:
    documents = load_documents(contract)
    for label, document in documents.items():
        validate_envelope(contract, label, document)
    validate_dispatch(documents["dispatch"])
    validate_delta(documents["expected_delta"])
    validate_migration(documents["migration_candidate"], documents["expected_delta"]["candidate_literal_id"])
    return {
        "status": "pass",
        "encoder": "python-independent",
        "artifact_ids": {label: document["candidate_literal_id"] for label, document in documents.items()},
        "semantic_validation": "pass",
        "blocked_dependencies": 8,
    }


def rewrite_artifacts(contract: Path, documents: dict[str, dict[str, Any]]) -> None:
    for label, document in documents.items():
        encoded = encode([document["identity_domain"], document["canonical_value"]])
        digest = hashlib.sha256(encoded).hexdigest()
        document["candidate_literal_id"] = digest
        document["cbor_sha256"] = digest
        document["byte_length"] = len(encoded)
        stem = STEMS[label]
        (contract / f"{stem}.json").write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n", encoding="ascii")
        (contract / f"{stem}.cbor").write_bytes(encoded)


def policy(value: list[Any], name: str) -> list[Any]:
    return next(row for row in value[10] if row[1] == name)


def refusal(value: list[Any], name: str) -> list[Any]:
    return next(row for row in value[9] if row[1] == name)


def mutate(name: str, documents: dict[str, dict[str, Any]]) -> None:
    dispatch = documents["dispatch"]["canonical_value"]
    delta = documents["expected_delta"]["canonical_value"]
    migration = documents["migration_candidate"]["canonical_value"]
    if name == "duplicate_state_tag":
        dispatch[2][1][0] = 1
    elif name == "unknown_state_tag":
        dispatch[2][2][0] = 99
    elif name == "duplicate_outcome_tag":
        dispatch[4][3][0] = 3
    elif name == "unknown_outcome_tag":
        dispatch[4][3][0] = 99
    elif name == "direct_reserved_to_sealed_terminal":
        dispatch[5].append([1, [0], 3, [1, 2]])
    elif name == "terminal_escape":
        dispatch[5].append([3, [1, 2], 2, [0]])
    elif name == "seal_binding_omission":
        dispatch[7][3].pop()
    elif name == "seal_replacement":
        dispatch[7][3][0][1] = "replacement_attempt_id"
    elif name == "sealed_local_rejection":
        dispatch[3][1][3].insert(0, 1)
    elif name == "unsealed_remote_outcome":
        dispatch[3][0][3].append(2)
    elif name == "nonterminal_outcome":
        dispatch[2][1][3] = 1
    elif name == "release_reconstruction":
        dispatch[10][2] = True
    elif name == "multiple_race_winners":
        dispatch[9][1] = 2
    elif name == "association_omission":
        migration[8][0][3].remove("migration_cutover_association")
    elif name == "association_duplication":
        migration[8][0][3].insert(2, "migration_cutover_association")
    elif name == "association_reuse":
        policy(migration, "association_consumed_exactly_once")[2] = False
    elif name == "wrong_context":
        migration[8][0][1] = "pre_store"
    elif name == "wrong_generation":
        migration[9].remove(refusal(migration, "wrong_generation"))
    elif name == "wrong_epoch":
        migration[9].remove(refusal(migration, "wrong_epoch"))
    elif name == "wrong_receipt":
        migration[8][0][2][0] = "other_receipt"
    elif name == "repository_release":
        migration[7][0][2] = [1, "release_id"]
    elif name == "installation_release_missing":
        migration[7][1][2] = [0]
    elif name == "partial_finality":
        policy(migration, "partial_finality_is_current")[2] = True
    elif name == "mixed_release":
        migration[9].remove(refusal(migration, "mixed_release"))
    elif name == "old_reader":
        policy(migration, "old_reader_admission")[2] = True
    elif name == "h2_promotion":
        policy(migration, "h2_causal_join_promotes_evidence")[2] = True
    elif name == "h3_promotion":
        policy(migration, "h3_cancel_label_promotes_evidence")[2] = True
    elif name == "cohort_row_change":
        migration[11][2][1] = 45
    elif name == "c868_semantic_change":
        migration[12][2] = 60
    elif name == "writer_epoch_nonblocking":
        delta[2][7][4] = False
    elif name == "delta_omission":
        delta[2].pop()
    elif name == "delta_addition":
        delta[2].append([9, "fabricated_delta", [], [0], True, "fabricated"])
    elif name == "predecessor_promotion":
        migration[5][1][1][0] = [1, migration[4][2][0]]
    elif name == "fabricated_successor_id":
        migration[5][0][1] = [1, {"bytes": "11" * 32}]
    else:
        raise AssertionError(name)


def command(args: list[str], contract: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        check=False,
        capture_output=True,
        text=True,
        env={**os.environ, "STAGE0_DISPATCH_CUTOVER_ROOT": str(contract)},
    )


def run_mutants(contract: Path) -> dict[str, Any]:
    documents = load_documents(contract)
    names = [
        "duplicate_state_tag",
        "unknown_state_tag",
        "duplicate_outcome_tag",
        "unknown_outcome_tag",
        "direct_reserved_to_sealed_terminal",
        "terminal_escape",
        "seal_binding_omission",
        "seal_replacement",
        "sealed_local_rejection",
        "unsealed_remote_outcome",
        "nonterminal_outcome",
        "release_reconstruction",
        "multiple_race_winners",
        "association_omission",
        "association_duplication",
        "association_reuse",
        "wrong_context",
        "wrong_generation",
        "wrong_epoch",
        "wrong_receipt",
        "repository_release",
        "installation_release_missing",
        "partial_finality",
        "mixed_release",
        "old_reader",
        "h2_promotion",
        "h3_promotion",
        "cohort_row_change",
        "c868_semantic_change",
        "writer_epoch_nonblocking",
        "delta_omission",
        "delta_addition",
        "predecessor_promotion",
        "fabricated_successor_id",
    ]
    rejected = {"python": 0, "ruby": 0}
    escaped: list[str] = []
    for name in names:
        with tempfile.TemporaryDirectory(prefix="maestro-dispatch-cutover-mutant-") as directory:
            mutant_root = Path(directory)
            mutant_documents = copy.deepcopy(documents)
            mutate(name, mutant_documents)
            rewrite_artifacts(mutant_root, mutant_documents)
            python = command(["python3", str(Path(__file__)), "--root", str(mutant_root)], mutant_root)
            ruby = command(["ruby", str(RUBY_VALIDATOR)], mutant_root)
            for label, result in (("python", python), ("ruby", ruby)):
                if result.returncode == 0:
                    escaped.append(f"{label}:{name}")
                else:
                    rejected[label] += 1
    require(not escaped, f"mutants escaped: {escaped}")
    return {"cases": names, "rejected": rejected, "total": len(names) * 2, "escaped": escaped}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--mutant-suite", action="store_true")
    args = parser.parse_args()
    contract = args.root.resolve()
    try:
        python_receipt = validate_contract(contract)
    except (ValidationError, KeyError, IndexError, TypeError, ValueError) as error:
        print(json.dumps({"status": "fail", "error": str(error)}, sort_keys=True, separators=(",", ":")))
        return 1
    if not args.mutant_suite:
        print(json.dumps(python_receipt, sort_keys=True, separators=(",", ":")))
        return 0

    ruby = command(["ruby", str(RUBY_VALIDATOR)], contract)
    require(ruby.returncode == 0, f"Ruby baseline failed: {ruby.stderr}")
    ruby_receipt = json.loads(ruby.stdout)
    require(ruby_receipt["artifact_ids"] == python_receipt["artifact_ids"], "independent encoders disagree")
    mutants = run_mutants(contract)
    receipt = {
        "schema": "maestro.vnext.dispatch-cutover-validation-receipt.v1",
        "status": "pass",
        "runtime_activated": False,
        "artifact_ids": python_receipt["artifact_ids"],
        "encoder_equality": "pass",
        "semantic_validation": {"python": "pass", "ruby": "pass"},
        "blocked_dependencies": 8,
        "mutants": mutants,
        "validator_sha256": {
            "python": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "ruby": hashlib.sha256(RUBY_VALIDATOR.read_bytes()).hexdigest(),
        },
    }
    output = json.dumps(receipt, sort_keys=True, separators=(",", ":")) + "\n"
    (contract / "validation-receipt.v1.json").write_text(output, encoding="ascii")
    print(output, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
