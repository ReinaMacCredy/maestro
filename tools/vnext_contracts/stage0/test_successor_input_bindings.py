#!/usr/bin/env python3
"""Falsifiers for the packet-bound successor approval provenance."""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import verify_input_bindings as verifier


def event() -> dict[str, object]:
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
    }


def bindings() -> dict[str, object]:
    return {
        "source_repository_realpath": "/Users/reinamaccredy/Code/maestro",
        "external_approval": {
            "packet_sha256": verifier.SUCCESSOR_PACKET_SHA256,
            "candidate_input_commitment": verifier.SUCCESSOR_CANDIDATE_INPUT,
            "build_plan_handoff": verifier.SUCCESSOR_BUILD_HANDOFF,
        },
        "external_approval_event": event(),
    }


def rejected(document: dict[str, object]) -> bool:
    try:
        verifier.verify_successor_external_approval_event(document)
    except (AssertionError, KeyError, SystemExit):
        return True
    return False


def main() -> None:
    original = bindings()
    verifier.verify_successor_external_approval_event(original)
    cases = {
        "packet": ("external_approval", "packet_sha256"),
        "candidate": ("external_approval", "candidate_input_commitment"),
        "handoff": ("external_approval", "build_plan_handoff"),
        "packet artifact": ("external_approval_event", "packet_root"),
        "event recipient": ("external_approval_event", "recipient_thread_id"),
        "event order": ("external_approval_event", "record_sha256"),
    }
    for name, (parent, key) in cases.items():
        candidate = copy.deepcopy(original)
        section = candidate[parent]
        assert isinstance(section, dict)
        if name == "event order":
            records = section[key]
            assert isinstance(records, dict)
            records["approval_task_started"], records["approval_user_message"] = (
                records["approval_user_message"],
                records["approval_task_started"],
            )
        else:
            section[key] = "0" * 64
        if not rejected(candidate):
            raise SystemExit(f"successor input-binding mutant was accepted: {name}")
    print(
        json.dumps(
            {
                "schema": "maestro.vnext.stage0-successor-input-binding-test.v1",
                "positive": "verified",
                "rejected_mutants": sorted(cases),
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )


if __name__ == "__main__":
    main()
