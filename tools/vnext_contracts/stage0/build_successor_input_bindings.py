#!/usr/bin/env python3
"""Materialize the approved successor packet into the Stage-0 input record."""

from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path

import verify_input_bindings as verifier


ROOT = Path(__file__).resolve().parents[3]
DEFAULT_INPUT = ROOT / "contracts/vnext/stage0/input-bindings.json"
DEFAULT_OUTPUT = DEFAULT_INPUT


def approval_event() -> dict[str, object]:
    causal_order = [
        "approval_task_started",
        "approval_user_message",
        "session_meta",
    ]
    record_set = verifier.digest_records(
        b"maestro.external-successor-approval-capture.v1\0",
        [
            ("log_realpath", str(verifier.SUCCESSOR_APPROVAL_LOG)),
            ("recipient_task_id", verifier.SUCCESSOR_RECIPIENT_TASK),
            *[
                (f"{name}_sha256", verifier.SUCCESSOR_APPROVAL_RECORDS[name])
                for name in causal_order
            ],
        ],
    )
    return {
        "attestation_source": "pinned_local_codex_session_records_v1",
        "provenance_assurance": "unsigned_local_platform_log_bound_by_sha256",
        "log_realpath": str(verifier.SUCCESSOR_APPROVAL_LOG),
        "record_set_sha256": record_set,
        "record_sha256": dict(verifier.SUCCESSOR_APPROVAL_RECORDS),
        "packet_publication_kind": "independently_verified_external_packet_v1",
        "packet_root": str(verifier.SUCCESSOR_PACKET_ROOT),
        "independent_verifier": str(verifier.SUCCESSOR_PACKET_VERIFIER),
        "independent_verifier_sha256": verifier.SUCCESSOR_PACKET_VERIFIER_SHA256,
        "recipient_thread_id": verifier.SUCCESSOR_RECIPIENT_TASK,
        "approval_turn_id": verifier.SUCCESSOR_APPROVAL_TURN,
        "approval_turn_started_at": verifier.SUCCESSOR_APPROVAL_STARTED_AT,
        "user_message_id": verifier.SUCCESSOR_APPROVAL_MESSAGE,
        "actor_role": "user",
        "exact_instruction": verifier.SUCCESSOR_APPROVAL_INSTRUCTION,
        "packet_publication_log_realpath": str(
            verifier.SUCCESSOR_PACKET_PUBLICATION_LOG
        ),
        "packet_publication_turn_id": verifier.SUCCESSOR_PACKET_PUBLICATION_TURN,
        "packet_publication_message_id": (
            verifier.SUCCESSOR_PACKET_PUBLICATION_MESSAGE
        ),
        "packet_publication_record_sha256": dict(
            verifier.SUCCESSOR_PACKET_PUBLICATION_RECORDS
        ),
        "packet_turn_completed_at": verifier.SUCCESSOR_PACKET_COMPLETED_AT,
    }


def control_bindings(packet: dict[str, object]) -> dict[str, object]:
    control = packet["control_bindings"]
    assert isinstance(control, dict)
    source_records = control["source_records"]
    sandbox_fields = control["sandbox_fields"]
    ancestors = control["ancestors"]
    assert isinstance(source_records, dict)
    assert isinstance(sandbox_fields, dict)
    assert isinstance(ancestors, list)
    return {
        "source_git_control": {
            "identity_sha256": control["source_identity"],
            **source_records,
            "git_control_path_manifest": str(control["control_manifest"]).splitlines(),
        },
        "destination_ancestors": {
            "identity_sha256": control["ancestor_identity"],
            "ancestors": ancestors,
        },
        "checkout_sandbox_profile": {
            "identity_sha256": control["sandbox_identity"],
            **sandbox_fields,
        },
    }


def build(
    predecessor: dict[str, object],
    packet: dict[str, object],
) -> dict[str, object]:
    if packet.get("packet_sha256") != verifier.SUCCESSOR_PACKET_SHA256:
        raise SystemExit("successor packet identity is not approved")
    candidate = packet["candidate_input"]
    handoff = packet["build_plan_handoff"]
    sections = packet["sections"]
    counts = packet["decision_counts"]
    assert isinstance(candidate, dict)
    assert isinstance(handoff, dict)
    assert isinstance(sections, dict)
    assert isinstance(counts, dict)
    candidate_records = candidate["records"]
    handoff_records = handoff["records"]
    assert isinstance(candidate_records, dict)
    assert isinstance(handoff_records, dict)

    result = copy.deepcopy(predecessor)
    baseline = {
        "commit": candidate_records["baseline_commit"],
        "tree": candidate_records["baseline_tree"],
    }
    source_inputs = {
        "card_sha256": candidate_records["card_sha256"],
        "design_sha256": candidate_records["design_sha256"],
        "decisions_sha256": candidate_records["decisions_sha256"],
    }
    controls = control_bindings(packet)
    result.update(
        {
            "implementation_workspace_path": candidate_records[
                "implementation_workspace_path"
            ],
            "implementation_workspace_policy": candidate_records[
                "implementation_workspace_policy"
            ],
            "baseline": baseline,
            "current_implementation_base": baseline,
            "canonical_source_inputs": source_inputs,
            "current_source_inputs": source_inputs,
            "external_control_bindings": controls,
            "current_external_control_bindings": copy.deepcopy(controls),
            "external_candidate_input_fields": {
                key: candidate_records[key]
                for key in (
                    "raw_decision_inventory_sha256",
                    "external_design_authority_closure_sha256",
                    "capability_census_sha256",
                    "resource_consumer_census_sha256",
                    "migration_rollback_removal_sha256",
                )
            },
            "external_build_plan": {
                key: handoff_records[key]
                for key in (
                    "recipient_task_id",
                    "stage_plan_sha256",
                    "proof_gate_sha256",
                    "risk_recovery_sha256",
                    "adapter_removal_sha256",
                )
            },
            "current_external_build_plan": {
                key: handoff_records[key]
                for key in (
                    "recipient_task_id",
                    "stage_plan_sha256",
                    "proof_gate_sha256",
                    "risk_recovery_sha256",
                    "adapter_removal_sha256",
                )
            },
            "external_packet_sections": {
                name: section["sha256"]
                for name, section in sections.items()
                if isinstance(section, dict)
            },
            "external_approval": {
                "packet_sha256": packet["packet_sha256"],
                "candidate_input_commitment": candidate["identity"],
                "build_plan_handoff": handoff["identity"],
                "must_not_be_used_as": [
                    "candidate_contract_root",
                    "canonical_build_handoff",
                    "manifest_identity",
                    "mandate",
                    "action_request",
                    "receipt",
                ],
            },
            "external_approval_event": approval_event(),
        }
    )
    inventory = result["decision_inventory"]
    assert isinstance(inventory, dict)
    inventory.update(
        {
            "total": counts["total"],
            "locked": counts["locked"],
            "superseded": counts["superseded"],
            "open": counts["open"],
        }
    )
    return result


def rendered(document: dict[str, object]) -> bytes:
    return (
        json.dumps(document, indent=2, ensure_ascii=False, sort_keys=False) + "\n"
    ).encode("utf-8")


def verified_packet() -> dict[str, object]:
    packet_files = verifier.verify_successor_packet_artifacts(
        {
            "external_approval": {
                "packet_sha256": verifier.SUCCESSOR_PACKET_SHA256,
                "candidate_input_commitment": verifier.SUCCESSOR_CANDIDATE_INPUT,
                "build_plan_handoff": verifier.SUCCESSOR_BUILD_HANDOFF,
            },
            "external_approval_event": approval_event(),
        }
    )
    packet_bytes = packet_files["replacement-build-approval-packet.v1.json"]
    packet = json.loads(packet_bytes)
    if not isinstance(packet, dict):
        raise SystemExit("verified successor packet is not an object")
    return packet


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    packet = verified_packet()
    predecessor = json.loads(args.input.read_text(encoding="utf-8"))
    document = build(predecessor, packet)
    expected = rendered(document)
    if args.check:
        if args.output.read_bytes() != expected:
            raise SystemExit("successor Stage-0 input bindings are stale")
        return
    args.output.write_bytes(expected)


if __name__ == "__main__":
    main()
