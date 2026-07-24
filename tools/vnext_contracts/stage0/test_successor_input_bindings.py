#!/usr/bin/env python3
"""Falsifiers for the packet-bound successor approval provenance."""

from __future__ import annotations

import copy
import json
import os
import sys
import tempfile
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
        "packet_publication_log_realpath": str(verifier.SUCCESSOR_PACKET_PUBLICATION_LOG),
        "packet_publication_turn_id": verifier.SUCCESSOR_PACKET_PUBLICATION_TURN,
        "packet_publication_message_id": verifier.SUCCESSOR_PACKET_PUBLICATION_MESSAGE,
        "packet_publication_record_sha256": dict(
            verifier.SUCCESSOR_PACKET_PUBLICATION_RECORDS
        ),
        "packet_turn_completed_at": verifier.SUCCESSOR_PACKET_COMPLETED_AT,
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
        "packet publication": (
            "external_approval_event",
            "packet_publication_record_sha256",
        ),
        "synthetic approval log": ("external_approval_event", "log_realpath"),
        "pre-publication approval": (
            "external_approval_event",
            "packet_turn_completed_at",
        ),
    }
    for name, (parent, key) in cases.items():
        candidate = copy.deepcopy(original)
        section = candidate[parent]
        assert isinstance(section, dict)
        if name in {"event order", "packet publication"}:
            records = section[key]
            assert isinstance(records, dict)
            names = list(records)
            records[names[0]], records[names[1]] = records[names[1]], records[names[0]]
        elif name == "pre-publication approval":
            section[key] = verifier.SUCCESSOR_APPROVAL_STARTED_AT + 1
        else:
            section[key] = "0" * 64
        if not rejected(candidate):
            raise SystemExit(f"successor input-binding mutant was accepted: {name}")

    with tempfile.TemporaryDirectory(prefix="maestro-stage0-descriptor-") as directory:
        root = Path(directory)
        regular = root / "input.json"
        regular.write_bytes(b'{"exact":true}\n')
        regular.chmod(0o600)
        if verifier.descriptor_capture(regular) != b'{"exact":true}\n':
            raise SystemExit("descriptor capture changed exact bytes")
        symlink = root / "substituted.json"
        os.symlink(regular.name, symlink)
        try:
            verifier.descriptor_capture(symlink)
        except (OSError, SystemExit):
            pass
        else:
            raise SystemExit("descriptor capture accepted a symlink substitution")
        ancestor = root / "ancestor"
        packet = ancestor / "packet"
        packet.mkdir(parents=True)
        (packet / "artifact").write_bytes(b"captured\n")
        (packet / "artifact").chmod(0o600)

        def replace_ancestor() -> None:
            ancestor.rename(root / "displaced-ancestor")
            replacement = root / "ancestor" / "packet"
            replacement.mkdir(parents=True)
            (replacement / "artifact").write_bytes(b"substituted\n")
            (replacement / "artifact").chmod(0o600)

        try:
            verifier.descriptor_capture_directory(
                packet, after_open_for_test=replace_ancestor
            )
        except (OSError, SystemExit):
            pass
        else:
            raise SystemExit("descriptor capture accepted ancestor replacement")

        packet_root = root / "packet-root"
        packet_root.mkdir()
        (packet_root / "artifact").write_bytes(b"captured\n")
        (packet_root / "artifact").chmod(0o600)

        def replace_packet_directory() -> None:
            packet_root.rename(root / "displaced-packet")
            packet_root.mkdir()
            (packet_root / "artifact").write_bytes(b"substituted\n")
            (packet_root / "artifact").chmod(0o600)

        try:
            verifier.descriptor_capture_directory(
                packet_root, after_open_for_test=replace_packet_directory
            )
        except (OSError, SystemExit):
            pass
        else:
            raise SystemExit("descriptor capture accepted packet-directory replacement")

        fake_ruby = root / "ruby"
        fake_ruby.write_bytes(b"#!/bin/sh\nexit 0\n")
        fake_ruby.chmod(0o500)
        original_ruby = verifier.SUCCESSOR_RUBY
        verifier.SUCCESSOR_RUBY = fake_ruby
        try:
            if not rejected(original):
                raise SystemExit("successor verifier accepted substituted Ruby executable")
        finally:
            verifier.SUCCESSOR_RUBY = original_ruby
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
