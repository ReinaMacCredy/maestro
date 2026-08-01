#!/usr/bin/env python3
"""Falsifying tests for the external Stage-0 approval/input boundary."""

from __future__ import annotations

import copy
import json
import os
import pwd
import runpy
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
BINDINGS = ROOT / "contracts/vnext/stage0/input-bindings.json"
VERIFIER = Path(__file__).with_name("verify_input_bindings.py")


def subprocess_environment() -> dict[str, str]:
    home = pwd.getpwuid(os.getuid()).pw_dir
    return {
        "HOME": home,
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONNOUSERSITE": "1",
        "RUBYLIB": "",
        "RUBYOPT": "",
    }


def verify(document: dict[str, object]) -> subprocess.CompletedProcess[str]:
    with tempfile.TemporaryDirectory() as directory:
        candidate = Path(directory) / "input-bindings.json"
        candidate.write_text(
            json.dumps(document, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        return subprocess.run(
            [
                sys.executable,
                str(VERIFIER),
                "--bindings",
                str(candidate),
                "--source",
                str(document["source_repository_realpath"]),
                "--artifact-reconstruction",
            ],
            check=False,
            capture_output=True,
            text=True,
            env=subprocess_environment(),
        )


def mutate(document: dict[str, object], path: tuple[str, ...], value: object) -> None:
    cursor: object = document
    for name in path[:-1]:
        if isinstance(cursor, dict):
            cursor = cursor[name]
        else:
            assert isinstance(cursor, list)
            cursor = cursor[int(name)]
    if isinstance(cursor, dict):
        cursor[path[-1]] = value
    else:
        assert isinstance(cursor, list)
        cursor[int(path[-1])] = value


def main() -> None:
    original = json.loads(BINDINGS.read_text(encoding="utf-8"))
    positive = verify(original)
    if positive.returncode != 0:
        raise SystemExit(f"positive input-binding verification failed:\n{positive.stderr}")

    cases: list[tuple[str, tuple[str, ...], object]] = [
        (
            "attestation source",
            ("external_approval_event", "attestation_source"),
            "self_declared",
        ),
        (
            "record-set digest",
            ("external_approval_event", "record_set_sha256"),
            "0" * 64,
        ),
        (
            "session record",
            ("external_approval_event", "record_sha256", "session_meta"),
            "1" * 64,
        ),
        (
            "packet record",
            ("external_approval_event", "record_sha256", "packet_task_complete"),
            "2" * 64,
        ),
        (
            "approval-start record",
            ("external_approval_event", "record_sha256", "approval_task_started"),
            "3" * 64,
        ),
        (
            "approval-message record",
            ("external_approval_event", "record_sha256", "approval_user_message"),
            "4" * 64,
        ),
        (
            "packet body",
            ("external_approval_event", "packet_body_sha256"),
            "5" * 64,
        ),
        (
            "packet turn",
            ("external_approval_event", "packet_turn_id"),
            "019f0000-0000-7000-8000-000000000001",
        ),
        (
            "approval turn",
            ("external_approval_event", "approval_turn_id"),
            "019f0000-0000-7000-8000-000000000002",
        ),
        (
            "approval message",
            ("external_approval_event", "user_message_id"),
            "msg_fabricated",
        ),
        (
            "packet completion",
            ("external_approval_event", "packet_turn_completed_at"),
            1784000000,
        ),
        (
            "approval start",
            ("external_approval_event", "approval_turn_started_at"),
            1784000500,
        ),
        (
            "exact instruction",
            ("external_approval_event", "exact_instruction"),
            "APPROVE A DIFFERENT PACKET",
        ),
        (
            "current design head",
            ("current_source_inputs", "design_sha256"),
            "6" * 64,
        ),
        (
            "successor Decision inventory",
            ("external_candidate_input_fields", "raw_decision_inventory_sha256"),
            "9" * 64,
        ),
        (
            "successor packet section",
            ("external_packet_sections", "architecture_ownership"),
            "a" * 64,
        ),
        (
            "successor baseline",
            ("baseline", "commit"),
            "b" * 40,
        ),
    ]
    rejected: list[str] = []
    for name, path, value in cases:
        candidate = copy.deepcopy(original)
        mutate(candidate, path, value)
        result = verify(candidate)
        if result.returncode == 0:
            raise SystemExit(f"input-binding mutant was accepted: {name}")
        rejected.append(name)

    combined = copy.deepcopy(original)
    event = combined["external_approval_event"]
    assert isinstance(event, dict)
    event.update(
        {
            "packet_turn_id": "019f0000-0000-7000-8000-000000000010",
            "approval_turn_id": "019f0000-0000-7000-8000-000000000011",
            "user_message_id": "msg_fabricated",
            "recipient_thread_created_at": 1783999000,
            "packet_turn_completed_at": 1784000000,
            "approval_turn_started_at": 1784000500,
        }
    )
    if verify(combined).returncode == 0:
        raise SystemExit("combined fabricated approval event was accepted")
    rejected.append("combined fabricated approval event")

    verifier = runpy.run_path(VERIFIER)
    approval_order = verifier["require_successor_approval_record_order"]
    packet_order = verifier["require_successor_packet_record_order"]
    packet_before_approval = verifier["require_successor_packet_before_approval"]
    approval_order([10, 11, 12])
    packet_order([20, 21])
    packet_before_approval(30, 31)
    causal_mutants = (
        ("successor approval record order", approval_order, [10, 12, 11]),
        ("successor packet record order", packet_order, [21, 20]),
        (
            "successor packet publication before approval",
            packet_before_approval,
            (31, 31),
        ),
    )
    for name, guard, arguments in causal_mutants:
        try:
            if isinstance(arguments, tuple):
                guard(*arguments)
            else:
                guard(arguments)
        except SystemExit:
            rejected.append(name)
        else:
            raise SystemExit(f"causal approval mutant was accepted: {name}")

    verifier_source = VERIFIER.read_text(encoding="utf-8")
    for forbidden in ("os.environ", "PYTHONPATH", "PYTHONHOME"):
        if forbidden in verifier_source:
            raise SystemExit(f"successor verifier inherits unsafe environment input: {forbidden}")
    for required in ('"RUBYLIB": ""', '"RUBYOPT": ""'):
        if verifier_source.count(required) != 2:
            raise SystemExit(f"successor verifier does not sanitize every Ruby subprocess: {required}")

    print(
        json.dumps(
            {
                "schema": "maestro.vnext.stage0-input-binding-test.v1",
                "positive": "verified",
                "rejected_mutants": rejected,
                "rejected_mutant_count": len(rejected),
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )


if __name__ == "__main__":
    main()
