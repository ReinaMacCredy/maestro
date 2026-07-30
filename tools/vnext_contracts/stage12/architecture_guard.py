#!/usr/bin/env python3
"""Evaluate Stage 12 canonical-promotion and release-preflight blockers."""

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
V8_CANDIDATE_STATE = (
    "v8_loss_root_guard_coordinator_bound_legacy_pruning_blocked_unverified"
)
V8_CLOSURE = {
    "packet_sha256": "d0953ac33f361ccad2fe0c7844294324b7b33cb974e16a11639ad3aad19e40e2",
    "design_commit": "bb7b1ee0e51fa591b21943e8c7d50844cb4d0b05",
    "design_tree": "cb6b62cc187abdecebef8f621206289029fb590b",
    "implementation_preimage_commit": "1685b39138a045bcd5e87744860d95eb589999d2",
    "implementation_preimage_tree": "2daa5f8458411cf9e6d6288bf51606c98a4e31c9",
    "primary_boundary_identity": "e5b4c0592b8cf373ea68fc5e0e3f84020c14f3f422c5779e8d4a423930aa6054",
    "ownership_identity": "699c6b98c8e4f1c8d92bf3a7377759fcc65e685c4f59272c36f13b65b3dc9cfd",
    "integration_plan_identity": "789cd36b82f4e6a0d534833446b9a2c35d6cfafcd96e1123fb9e3215a5df0f29",
    "foundation_closure": "FoundationLegacyQuarantineClosureV2",
    "foundation_owner_evidence_mint": "FoundationOwnerEvidenceMintV1",
    "loss_manifest": "UnavailablePreexistingLossManifestV4",
    "loss_audit_currentness": "UnavailablePreexistingLossAuditCurrentnessV4",
    "loss_audit_custody": "QuarantineCustodyLeaseV1",
    "loss_audit_gate": "UnavailablePreexistingLossAuditGateErrorV1",
    "rollback": "LegacyRollbackAssessmentV4",
    "epoch": "LegacyQuarantineEpochV4",
    "guard": "LegacyRemovalGuardV3",
    "guard_consumer_binding": "LegacyRemovalConsumerBindingV3",
    "coordinator": "Stage12LegacyCutCoordinatorV3",
}
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
    "stage11_v4_loss_root_universe_contract",
    "stage11_owner_history_currentness",
    "stage11_complete_root_universe",
    "stage11_foundation_v2_closure",
    "stage11_foundation_owner_evidence_mint",
    "stage11_custody_bound_loss_audit",
    "stage12_candidate_ancestry_and_ownership",
    "stage12_frozen_interface_readback",
    "stage12_legacy_removal_guard_v3",
    "stage12_expected_old_guard_binding",
    "stage12_legacy_cut_coordinator_v3",
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
    "authenticated_owner_history_and_v4_loss_closure",
    "complete_owner_root_universe_and_two_pass_finality",
    "foundation_v2_closure_and_v4_rollback_epoch",
    "foundation_one_use_owner_evidence_mint",
    "custody_bound_unavailable_preexisting_loss_audit_before_finality",
    "legacy_removal_guard_v3_exact_v4_dependency",
    "legacy_removal_guard_v3_expected_old_binding",
    "stage12_legacy_cut_coordinator_v3_single_isolated_cas",
    "proof_runner_effect_inert",
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
EVIDENCE_RECEIPT_SCHEMA = "maestro.external.stage12-evidence-receipt.v1"
ZERO_COUNT_FIELDS = {
    "stage11_active_consumer_closure": "active_consumer_count",
    "stage11_retained_reader_manifest": "sealed_reader_count",
    "stage11_retention_hold_manifest": "retention_hold_count",
    "post_promotion_namespace_census": "temporary_namespace_count",
}
INSTALLATION_ROOT_ROLES = [
    "Active",
    "Inactive",
    "Snapshot",
    "Cache",
    "Archive",
    "Host",
    "Legacy",
]


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


def _is_sha256(value: object) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def _load_evidence_receipt(slot: str, path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_bytes())
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ArchitectureGuardError(
            f"invalid evidence receipt for {slot}: {error}"
        ) from error
    if (
        not isinstance(value, dict)
        or value.get("schema_version") != EVIDENCE_RECEIPT_SCHEMA
        or value.get("slot") != slot
        or value.get("status") != "satisfied"
        or not _is_sha256(value.get("subject_id"))
        or not _is_sha256(value.get("currentness_id"))
        or not _is_sha256(value.get("proof_id"))
        or not isinstance(value.get("payload"), dict)
    ):
        raise ArchitectureGuardError(f"evidence receipt contract differs for {slot}")
    payload = value["payload"]
    zero_field = ZERO_COUNT_FIELDS.get(slot)
    if zero_field is not None and payload.get(zero_field) != 0:
        raise ArchitectureGuardError(
            f"evidence receipt {slot} does not prove {zero_field}=0"
        )
    if slot == "source_move_identity_parity":
        if (
            payload.get("entry_count") != 210
            or payload.get("collision_count") != 10
            or payload.get("namespace_counts")
            != {
                "src/domain/vnext": 186,
                "src/interfaces/vnext": 8,
                "src/operations/vnext": 16,
            }
            or payload.get("mismatched_paths") != []
            or not _is_sha256(payload.get("manifest_sha256"))
            or not _is_sha256(payload.get("destination_set_sha256"))
        ):
            raise ArchitectureGuardError(
                "source move identity parity evidence is incomplete or mismatched"
            )
    if slot == "stage12_frozen_interface_readback" and (
        payload.get("facade_mismatch_count") != 0
        or not _is_sha256(payload.get("interface_manifest_sha256"))
    ):
        raise ArchitectureGuardError("canonical facade parity evidence differs")
    if slot == "fresh_removal_authority_receipt" and (
        payload.get("authority_fresh") is not True
        or not _is_sha256(payload.get("consumer_closure_id"))
        or not _is_sha256(payload.get("rollback_closure_id"))
    ):
        raise ArchitectureGuardError("removal authority is not fresh and closure-bound")
    if slot == "stage11_v4_loss_root_universe_contract" and (
        payload.get("foundation_closure")
        != "FoundationLegacyQuarantineClosureV2"
        or payload.get("loss_manifest") != "UnavailablePreexistingLossManifestV4"
        or payload.get("rollback") != "LegacyRollbackAssessmentV4"
        or payload.get("epoch") != "LegacyQuarantineEpochV4"
        or payload.get("census_schema_delta") is not False
        or payload.get("public_surface_delta") is not False
    ):
        raise ArchitectureGuardError("Stage 11 V4 contract evidence differs")
    if slot == "stage11_owner_history_currentness" and (
        payload.get("all_owner_history_current") is not True
        or payload.get("orphan_history_count") != 0
        or payload.get("replayed_history_count") != 0
        or payload.get("post_admission_loss_count") != 0
    ):
        raise ArchitectureGuardError("owner-history currentness evidence differs")
    if slot == "stage11_complete_root_universe" and (
        payload.get("repository_roles") != ["RepositoryStore"]
        or payload.get("installation_roles") != INSTALLATION_ROOT_ROLES
        or payload.get("omission_count") != 0
        or payload.get("duplicate_count") != 0
        or payload.get("unsupported_count") != 0
        or payload.get("alias_count") != 0
        or payload.get("header_references_define_scope") is not False
    ):
        raise ArchitectureGuardError("complete root-universe evidence differs")
    if slot == "stage11_foundation_v2_closure" and (
        payload.get("closure") != "FoundationLegacyQuarantineClosureV2"
        or payload.get("two_pass_exact") is not True
        or payload.get("final_owner_rechecks_current") is not True
        or payload.get("expected_old_custody_sealed") is not True
        or payload.get("migration_locator_count") != 0
    ):
        raise ArchitectureGuardError("Foundation V2 closure evidence differs")
    if slot == "stage12_legacy_removal_guard_v3" and (
        payload.get("guard") != "LegacyRemovalGuardV3"
        or payload.get("loss_manifest") != "UnavailablePreexistingLossManifestV4"
        or payload.get("rollback") != "LegacyRollbackAssessmentV4"
        or payload.get("epoch") != "LegacyQuarantineEpochV4"
        or payload.get("historical_adapter_count") != 0
    ):
        raise ArchitectureGuardError("LegacyRemovalGuardV3 evidence differs")
    if slot == "stage12_legacy_cut_coordinator_v3" and (
        payload.get("schema")
        != "maestro.external.stage12-legacy-cut-coordinator.v3"
        or payload.get("coordinator") != "Stage12LegacyCutCoordinatorV3"
        or payload.get("candidate_ref")
        != "refs/heads/codex/maestro-vnext-legacy-cutover-successor-candidate-v8"
        or payload.get("cas_state") != "exact_declared_postimage"
        or payload.get("cas_write_count") != 1
        or payload.get("primary_target") is not False
        or payload.get("live_installation_target") is not False
        or payload.get("proof_runner_effect_inert") is not True
    ):
        raise ArchitectureGuardError("Stage12LegacyCutCoordinatorV3 evidence differs")
    return value


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
        try:
            receipt = _load_evidence_receipt(slot, path)
        except ArchitectureGuardError as error:
            blockers.append(
                {
                    "id": "invalid_external_evidence_receipt",
                    "slot": slot,
                    "path": str(path),
                    "reason": str(error),
                }
            )
            continue
        bindings.append(
            {
                "currentness_id": str(receipt["currentness_id"]),
                "path": str(path),
                "proof_id": str(receipt["proof_id"]),
                "sha256": hashlib.sha256(data).hexdigest(),
                "slot": slot,
                "subject_id": str(receipt["subject_id"]),
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
    if release_inputs.get("v8_closure") != V8_CLOSURE:
        raise ArchitectureGuardError("release-proof V8 closure differs")
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
    if tuple(release_inputs.get("v8_external_input_slots", [])) != EXPECTED_EXTERNAL_SLOTS:
        raise ArchitectureGuardError("release-proof external input closure differs")
    if tuple(release_inputs.get("v8_proof_obligations", [])) != EXPECTED_OBLIGATIONS:
        raise ArchitectureGuardError("release-proof obligation closure differs")
    if release_inputs.get("v8_local_inputs") != [
        "tests/fixtures/vnext/stage11/live_set_v4_contract.v1.json",
        "tests/fixtures/vnext/stage11/root-universe.v1.json",
        "tools/vnext_contracts/stage11/validate_v4.py",
        "tests/fixtures/vnext/stage12/stage12-legacy-cut-coordinator.v3.json",
        "tools/vnext_contracts/stage12/coordinator_v3.py",
    ]:
        raise ArchitectureGuardError("release-proof V8 local input closure differs")
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
                "candidate_state": V8_CANDIDATE_STATE,
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
            "candidate_state": V8_CANDIDATE_STATE,
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
