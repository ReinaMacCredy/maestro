#!/usr/bin/env python3
"""Falsifiers for the packet-bound successor approval provenance."""

from __future__ import annotations

import copy
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_successor_input_bindings as successor_builder
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
    except SystemExit:
        return True
    return False


def require_causal_guard_rejection(
    label: str,
    guard: object,
    argument: object,
) -> None:
    assert callable(guard)
    try:
        if isinstance(argument, tuple):
            guard(*argument)
        else:
            guard(argument)
    except SystemExit:
        return
    raise SystemExit(f"successor verifier omitted {label} guard")


def verify_causal_mutants() -> dict[str, tuple[object, object]]:
    causal_mutants = {
        "event order": (
            verifier.require_successor_approval_record_order,
            [2, 1, 3],
        ),
        "packet publication order": (
            verifier.require_successor_packet_record_order,
            [9, 8],
        ),
        "pre-publication approval": (
            verifier.require_successor_packet_before_approval,
            (
                verifier.SUCCESSOR_APPROVAL_STARTED_AT,
                verifier.SUCCESSOR_APPROVAL_STARTED_AT,
            ),
        ),
    }
    for label, (guard, argument) in causal_mutants.items():
        require_causal_guard_rejection(label, guard, argument)
    return causal_mutants


def main(*, causal_only: bool = False) -> None:
    causal_mutants = verify_causal_mutants()
    if causal_only:
        print(
            json.dumps(
                {
                    "schema": "maestro.vnext.stage0-successor-causal-guard-test.v1",
                    "rejected_mutants": sorted(causal_mutants),
                },
                sort_keys=True,
                separators=(",", ":"),
            )
        )
        return

    original = bindings()
    verifier.verify_successor_external_approval_event(original)
    packet_verifier = verifier.verify_successor_packet_artifacts

    def captured_packet(_bindings: dict[str, object]) -> dict[str, bytes]:
        return {
            "replacement-build-approval-packet.v1.json": b'{"snapshot":"captured"}\n'
        }

    verifier.verify_successor_packet_artifacts = captured_packet
    try:
        if successor_builder.verified_packet() != {"snapshot": "captured"}:
            raise SystemExit("successor builder did not consume the verifier-captured bytes")
    finally:
        verifier.verify_successor_packet_artifacts = packet_verifier
    repository_root = Path(__file__).resolve().parents[3]
    serialized = json.loads(
        (repository_root / "contracts/vnext/stage0/input-bindings.json").read_text(
            encoding="utf-8"
        )
    )
    verifier.verify_serialized_external_control_bindings(serialized)
    control_mutants = (
        (
            "nested source digest",
            ("external_control_bindings", "source_git_control", "repository_config_sha256"),
            "0" * 64,
        ),
        (
            "duplicate source path",
            ("external_control_bindings", "source_git_control", "source_repository_realpath"),
            "/private/tmp/substituted-source",
        ),
        (
            "duplicate baseline commit",
            ("external_control_bindings", "source_git_control", "baseline_commit"),
            "0" * 40,
        ),
        (
            "destination ancestor metadata",
            ("external_control_bindings", "destination_ancestors", "ancestors", "0", "mode"),
            "0777",
        ),
        (
            "sandbox authority",
            ("external_control_bindings", "checkout_sandbox_profile", "network"),
            "enabled",
        ),
    )
    for label, path, value in control_mutants:
        mutant = copy.deepcopy(serialized)
        cursor: object = mutant
        for component in path[:-1]:
            if isinstance(cursor, dict):
                cursor = cursor[component]
            else:
                assert isinstance(cursor, list)
                cursor = cursor[int(component)]
        assert isinstance(cursor, dict)
        cursor[path[-1]] = value
        try:
            verifier.verify_serialized_external_control_bindings(mutant)
        except SystemExit:
            pass
        else:
            raise SystemExit(
                f"successor verifier accepted {label} under retained control identities"
            )
    extra_field_mutant = copy.deepcopy(serialized)
    extra_controls = extra_field_mutant["external_control_bindings"]
    assert isinstance(extra_controls, dict)
    extra_source = extra_controls["source_git_control"]
    assert isinstance(extra_source, dict)
    extra_source["unbound_authority"] = "accepted"
    try:
        verifier.verify_serialized_external_control_bindings(extra_field_mutant)
    except SystemExit:
        pass
    else:
        raise SystemExit("successor verifier accepted an unbound control field")

    with tempfile.TemporaryDirectory(prefix="maestro-stage0-replace-ref-") as directory:
        replacement_repo = Path(directory)
        git_environment = {
            **os.environ,
            "GIT_AUTHOR_EMAIL": "proof@example.invalid",
            "GIT_AUTHOR_NAME": "Proof",
            "GIT_COMMITTER_EMAIL": "proof@example.invalid",
            "GIT_COMMITTER_NAME": "Proof",
        }

        def git(*arguments: str) -> str:
            return subprocess.run(
                ["/usr/bin/git", *arguments],
                cwd=replacement_repo,
                check=True,
                capture_output=True,
                text=True,
                env=git_environment,
            ).stdout.strip()

        git("init", "--quiet")
        tracked = replacement_repo / "tracked"
        tracked.write_text("approved\n", encoding="utf-8")
        git("add", "tracked")
        git("commit", "--quiet", "-m", "approved")
        approved_commit = git("rev-parse", "HEAD")
        approved_tree = git("rev-parse", "HEAD^{tree}")
        tracked.write_text("substituted\n", encoding="utf-8")
        git("commit", "--quiet", "-am", "substituted")
        substituted_commit = git("rev-parse", "HEAD")
        git("replace", approved_commit, substituted_commit)
        archived_bindings = {
            "baseline": {"commit": approved_commit, "tree": approved_tree},
            "current_implementation_base": {
                "commit": approved_commit,
                "tree": approved_tree,
            },
        }
        verifier.verify_archived_baseline_objects(
            archived_bindings, replacement_repo
        )
        alternates = replacement_repo / ".git/objects/info/alternates"
        alternates.write_text(
            str(replacement_repo / ".git/objects") + "\n",
            encoding="utf-8",
        )
        try:
            verifier.verify_archived_baseline_objects(
                archived_bindings, replacement_repo
            )
        except SystemExit:
            pass
        else:
            raise SystemExit("archived verifier accepted an alternate object source")
        alternates.unlink()
        approved_tree_object = (
            replacement_repo / ".git/objects" / approved_tree[:2] / approved_tree[2:]
        )
        displaced_tree_object = approved_tree_object.with_suffix(".displaced")
        approved_tree_object.rename(displaced_tree_object)
        try:
            verifier.verify_archived_baseline_objects(
                archived_bindings, replacement_repo
            )
        except (subprocess.CalledProcessError, SystemExit):
            pass
        else:
            raise SystemExit("archived verifier accepted a missing bound tree object")

    alternate_root = subprocess.run(
        [
            sys.executable,
            str(
                repository_root
                / "tools/vnext_contracts/stage0/build_successor_input_bindings.py"
            ),
            "--packet-root",
            "/private/tmp/untrusted-successor-packet",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    if alternate_root.returncode == 0 or "unrecognized arguments" not in alternate_root.stderr:
        raise SystemExit("successor builder still accepts a caller-selected packet root")
    cases = {
        "packet": ("external_approval", "packet_sha256"),
        "candidate": ("external_approval", "candidate_input_commitment"),
        "handoff": ("external_approval", "build_plan_handoff"),
        "packet artifact": ("external_approval_event", "packet_root"),
        "event recipient": ("external_approval_event", "recipient_thread_id"),
        "synthetic approval log": ("external_approval_event", "log_realpath"),
    }
    for name, (parent, key) in cases.items():
        candidate = copy.deepcopy(original)
        section = candidate[parent]
        assert isinstance(section, dict)
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
        append_only = root / "append-only.jsonl"
        first = b'{"record":1}\n'
        second = b'{"record":2}\n'
        append_only.write_bytes(first)
        append_only.chmod(0o600)

        def append_record() -> None:
            with append_only.open("ab") as handle:
                handle.write(second)

        if (
            verifier.descriptor_capture_append_only(
                append_only,
                after_read_for_test=append_record,
            )
            != first
        ):
            raise SystemExit("append-only descriptor capture changed the stable prefix")

        def mutate_prefix() -> None:
            with append_only.open("r+b") as handle:
                handle.write(b'{"record":0}\n')

        try:
            verifier.descriptor_capture_append_only(
                append_only,
                after_read_for_test=mutate_prefix,
            )
        except (OSError, SystemExit):
            pass
        else:
            raise SystemExit("append-only descriptor capture accepted prefix mutation")
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

        in_place = root / "in-place"
        in_place.mkdir()
        in_place_artifact = in_place / "artifact"
        in_place_artifact.write_bytes(b"captured\n")
        in_place_artifact.chmod(0o600)

        def mutate_in_place(name: str) -> None:
            if name == "artifact":
                in_place_artifact.write_bytes(b"mutated!\n")

        try:
            verifier.descriptor_capture_directory(
                in_place, after_file_read_for_test=mutate_in_place
            )
        except (OSError, SystemExit):
            pass
        else:
            raise SystemExit("descriptor capture accepted in-place packet mutation")

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
                "rejected_mutants": sorted([*cases, *causal_mutants]),
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )


if __name__ == "__main__":
    main(causal_only=sys.argv[1:] == ["--causal-only"])
