#!/usr/bin/env python3
"""Evaluate provisional Stage 12 architecture and release-preflight blockers."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any, Mapping


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[2]
sys.dont_write_bytecode = True
sys.path.insert(0, str(TOOLS))

from census import (  # type: ignore[import-not-found]  # noqa: E402
    DEFAULT_POLICY,
    DESIGN_SHA256,
    FANOUT_COMMIT,
    FANOUT_MANIFEST_SCHEMA,
    FANOUT_MANIFEST_SHA256,
    FANOUT_MANIFEST_STATE,
    FANOUT_TREE,
    MATERIALIZATION_DECISIONS,
    CensusError,
    build_census,
    load_json,
)


DEFAULT_RELEASE_INPUTS = (
    WORKSPACE / "tests/fixtures/vnext/stage12/release-proof-inputs.v1.json"
)
RELEASE_INPUT_SCHEMA = "maestro.test-only.vnext-stage12-release-proof-inputs.v1"
EXPECTED_EXTERNAL_SLOTS = (
    "stage6_integrated_unsealed_checkpoint",
    "stage7_integrated_unsealed_checkpoint",
    "stage8_integrated_unsealed_checkpoint",
    "stage9_integrated_unsealed_checkpoint",
    "stage10_integrated_unsealed_checkpoint",
    "stage11_integrated_unsealed_checkpoint",
    "stage6_through_stage11_ordered_integration_ancestry",
    "stage11_migration_association_closure",
    "stage11_active_consumer_closure",
    "stage11_retained_reader_manifest",
    "stage11_retention_hold_manifest",
    "stage12_candidate_ancestry_and_ownership",
    "stage12_frozen_interface_readback",
    "post_promotion_namespace_census",
    "source_move_identity_parity",
    "negative_compatibility_execution",
    "migration_rollback_rehearsal",
    "fresh_removal_authority_receipt",
    "dogfood_recovery_rehearsal",
    "final_full_chain_seal",
)
EXPECTED_CLAIM_KEYS = {
    "ancestry_verified",
    "certified",
    "consumer_zero",
    "current",
    "integration_complete",
    "migration_closed",
    "namespace_promoted",
    "pruning_authorized",
    "release_ready",
    "removal_authorized",
    "rollback_rehearsed",
    "sealed_reader_zero",
}
EXPECTED_OBLIGATIONS = (
    "ordered_stage6_through_stage11_integration_ancestry",
    "active_consumer_zero",
    "sealed_reader_zero",
    "retention_hold_zero",
    "migration_association_and_finality_closure",
    "candidate_ancestry_and_owned_path_closure",
    "frozen_interface_byte_identity",
    "temporary_namespace_zero",
    "canonical_owner_facade_parity",
    "content_identity_byte_parity",
    "negative_compatibility_refusal",
    "fresh_removal_authority",
    "retained_audit_rollback_closure",
    "dogfood_and_recovery_rehearsal",
    "final_edge_sweep",
)
EXPECTED_FORBIDDEN_OPERATIONS = {
    "authority_claim",
    "canonical_namespace_promotion",
    "certification",
    "currentness_claim",
    "legacy_removal",
    "migration_activation",
    "physical_pruning",
    "production_deletion",
    "production_mutation",
    "receipt_publication",
    "removal_execution",
    "stage6_through_stage11_product_mutation",
}


class ArchitectureGuardError(RuntimeError):
    """The Stage 12 architecture preflight input is invalid."""


def parse_evidence(values: list[str]) -> dict[str, Path]:
    evidence: dict[str, Path] = {}
    for value in values:
        slot, separator, path = value.partition("=")
        if not separator or not slot or not path or slot in evidence:
            raise ArchitectureGuardError(
                "evidence must be unique slot=path pairs"
            )
        evidence[slot] = Path(path)
    return evidence


def _external_bindings(
    slots: list[str], evidence: Mapping[str, Path]
) -> tuple[list[dict[str, str]], list[dict[str, object]]]:
    bindings: list[dict[str, str]] = []
    blockers: list[dict[str, object]] = []
    unknown = sorted(set(evidence) - set(slots))
    if unknown:
        raise ArchitectureGuardError(
            f"unknown release-preflight evidence slots: {', '.join(unknown)}"
        )
    for slot in slots:
        path = evidence.get(slot)
        if path is None:
            blockers.append({"id": "missing_external_input", "slot": slot})
            continue
        if path.is_symlink() or not path.is_file():
            blockers.append(
                {"id": "unsafe_or_missing_external_input", "slot": slot, "path": str(path)}
            )
            continue
        data = path.read_bytes()
        bindings.append(
            {
                "path": str(path),
                "sha256": hashlib.sha256(data).hexdigest(),
                "slot": slot,
            }
        )
    return bindings, blockers


def evaluate(
    repo: Path,
    policy: Mapping[str, Any],
    release_inputs: Mapping[str, Any],
    evidence: Mapping[str, Path],
    *,
    release_preflight: bool,
) -> tuple[dict[str, object], int]:
    if release_inputs.get("schema_version") != RELEASE_INPUT_SCHEMA:
        raise ArchitectureGuardError("release-proof input schema differs")
    if (
        release_inputs.get("authority_state") != "none"
        or release_inputs.get("input_state") != "provisional_read_only"
    ):
        raise ArchitectureGuardError("release-proof input authority or state differs")
    if (
        release_inputs.get("fanout_commit") != FANOUT_COMMIT
        or release_inputs.get("fanout_tree") != FANOUT_TREE
        or release_inputs.get("design_sha256") != DESIGN_SHA256
        or release_inputs.get("fanout_manifest_schema") != FANOUT_MANIFEST_SCHEMA
        or release_inputs.get("fanout_manifest_sha256") != FANOUT_MANIFEST_SHA256
        or release_inputs.get("fanout_manifest_state") != FANOUT_MANIFEST_STATE
        or release_inputs.get("fanout_manifest_preservation_only") is not True
        or release_inputs.get("materialization_decisions")
        != MATERIALIZATION_DECISIONS
    ):
        raise ArchitectureGuardError("release-proof input binding differs")
    claims = release_inputs.get("claims")
    if (
        not isinstance(claims, dict)
        or set(claims) != EXPECTED_CLAIM_KEYS
        or any(claims.values())
    ):
        raise ArchitectureGuardError("release-proof input makes a positive claim")
    if tuple(release_inputs.get("external_input_slots", [])) != EXPECTED_EXTERNAL_SLOTS:
        raise ArchitectureGuardError("release-proof external input closure differs")
    if tuple(release_inputs.get("proof_obligations", [])) != EXPECTED_OBLIGATIONS:
        raise ArchitectureGuardError("release-proof obligation closure differs")
    if set(release_inputs.get("forbidden_operations", [])) != EXPECTED_FORBIDDEN_OPERATIONS:
        raise ArchitectureGuardError("release-proof forbidden operation closure differs")
    census = build_census(repo, policy)
    rows = census["rows"]
    if not isinstance(rows, list):
        raise ArchitectureGuardError("census rows are malformed")
    observed_release_blockers: list[dict[str, object]] = []
    rule_counts: dict[str, int] = {}
    for row in rows:
        if not isinstance(row, dict):
            raise ArchitectureGuardError("census row is malformed")
        rule_id = str(row["rule_id"])
        rule_counts[rule_id] = rule_counts.get(rule_id, 0) + 1
    for rule_id, count in sorted(rule_counts.items()):
        if count:
            observed_release_blockers.append(
                {"count": count, "id": "consumer_rows_nonzero", "rule_id": rule_id}
            )
    warnings = census.get("scan_warnings")
    if not isinstance(warnings, list):
        raise ArchitectureGuardError("census scan warnings are malformed")
    for warning in warnings:
        observed_release_blockers.append(
            {"id": "census_scan_warning", "warning": warning}
        )

    if not release_preflight:
        return (
            {
                "authority_state": "none",
                "candidate_state": "stage_12_candidate_read_only_wip_unverified",
                "census_row_count": census["row_count"],
                "census_sha256": census["scan_sha256"],
                "compile_lane_needed": True,
                "observed_release_blockers": observed_release_blockers,
                "release_evaluated": False,
                "status": "pass",
            },
            0,
        )

    bindings, external_blockers = _external_bindings(
        list(EXPECTED_EXTERNAL_SLOTS), evidence
    )
    blockers = observed_release_blockers + external_blockers
    status = "blocked" if blockers else "provisional_inputs_bound_unverified"
    return (
        {
            "authority_state": "none",
            "blockers": blockers,
            "candidate_state": "stage_12_candidate_read_only_wip_unverified",
            "certification_claim": False,
            "census_row_count": census["row_count"],
            "census_sha256": census["scan_sha256"],
            "compile_lane_needed": True,
            "external_input_bindings": bindings,
            "release_evaluated": False,
            "status": status,
        },
        2 if blockers else 0,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=WORKSPACE)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--release-inputs", type=Path, default=DEFAULT_RELEASE_INPUTS)
    parser.add_argument("--release-preflight", action="store_true")
    parser.add_argument("--evidence", action="append", default=[])
    args = parser.parse_args()
    try:
        payload, exit_code = evaluate(
            args.repo,
            load_json(args.policy),
            load_json(args.release_inputs),
            parse_evidence(args.evidence),
            release_preflight=args.release_preflight,
        )
    except (ArchitectureGuardError, CensusError) as error:
        print(json.dumps({"status": "error", "error": str(error)}, sort_keys=True))
        return 1
    print(json.dumps(payload, indent=2, sort_keys=True))
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
