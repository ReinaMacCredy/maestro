#!/usr/bin/env python3
"""Verify the immutable external inputs admitted to vNext Stage 0.

The verified values are provenance only. This tool never returns one of them as
a canonical vNext identity and never writes the source repository.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import re
import stat
import subprocess
import tempfile
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
DEFAULT_BINDINGS = ROOT / "contracts/vnext/stage0/input-bindings.json"
SUCCESSOR_PACKET_SHA256 = "fb33b048b59c66df9858558a2c80e59a478d101465761f902366c9a00751cbc5"
SUCCESSOR_CANDIDATE_INPUT = "c180b31a6b2649ed416eeeaa614e98960ec3eef030b69ff90a0e0f63ed1ff93e"
SUCCESSOR_BUILD_HANDOFF = "bd1e6e55ff473250557be13bb8332df97a2c60eac62e964702e5eca67991b352"
SUCCESSOR_PACKET_ROOT = Path("/private/tmp/maestro-vnext-materialization-successor-packet")
SUCCESSOR_PACKET_ARTIFACT_SHA256 = "7f13c85b45799e39daedd30846b4a024d1f264134b46c3e3b3cdf720f8e5fb02"
SUCCESSOR_PACKET_VERIFIER = Path("/private/tmp/verify_maestro_successor_packet.rb")
SUCCESSOR_PACKET_VERIFIER_SHA256 = "66f1164216bd698b15bb73a25bbfd7cf5cb5b5c038938242d4ffc4d14dbb624d"
SUCCESSOR_RUBY = Path("/usr/bin/ruby")
SUCCESSOR_RUBY_SHA256 = "5340838cbee187f366d75d1ec540e6acb962757f1c170111024970c170280c04"
SUCCESSOR_RUBY_PROBE = (
    "ruby 2.6.10p210 (2022-04-12 revision 67958) [universal.arm64e-darwin26]"
)
SUCCESSOR_APPROVAL_LOG = Path(
    "/Users/reinamaccredy/.codex/sessions/2026/07/11/"
    "rollout-2026-07-11T02-55-42-019f4d99-a48a-7390-9560-7c7a9dc63a8d.jsonl"
)
SUCCESSOR_APPROVAL_RECORDS = {
    "approval_task_started": "68d30466c4ba9e09447a860e434020743420f7439a943246770468cb9aafd69e",
    "approval_user_message": "e1493db060141a0ce02b40a6d5159e90f85f694c1943116a68c74f3fe54ee3c8",
    "session_meta": "605f7eb976f7b835ecd6cba019febbb185114d3722b6e7d8f906e60168bccf20",
}
SUCCESSOR_RECIPIENT_TASK = "019f4d99-a48a-7390-9560-7c7a9dc63a8d"
SUCCESSOR_APPROVAL_TURN = "019f9154-6de5-7612-9d3f-136c4c1324fd"
SUCCESSOR_APPROVAL_MESSAGE = "msg_019f9154-6e72-7d00-8d41-f3ef5e7917dc"
SUCCESSOR_APPROVAL_STARTED_AT = 1784849657
SUCCESSOR_PACKET_PUBLICATION_LOG = Path(
    "/Users/reinamaccredy/.codex/sessions/2026/07/21/"
    "rollout-2026-07-21T19-34-16-019f84ab-7237-7ad2-93b7-332abe8329be.jsonl"
)
SUCCESSOR_PACKET_PUBLICATION_TURN = "019f8fff-85d5-7781-89de-5f24fc66223b"
SUCCESSOR_PACKET_PUBLICATION_MESSAGE = "msg_0a802c5cd719d2ce016a625df68b1c819a8de9542cabd67403"
SUCCESSOR_PACKET_PUBLICATION_RECORDS = {
    "packet_assistant_message": "b9911b90224d46d0925dd10cff9330373e4c5fc60cdca763139cd469f3b65c59",
    "packet_task_complete": "a4ed4ca62e8dfa4b05c0247d92d459a35254543fa6f3baf505f8075c15ecb848",
}
SUCCESSOR_PACKET_COMPLETED_AT = 1784831493
SUCCESSOR_APPROVAL_INSTRUCTION = (
    "APPROVE BUILD PACKET sha256:"
    f"{SUCCESSOR_PACKET_SHA256}. Execute the staged build plan."
)


def _directory_flags() -> int:
    if not hasattr(os, "O_NOFOLLOW"):
        raise SystemExit("descriptor capture requires O_NOFOLLOW")
    flags = os.O_RDONLY | os.O_DIRECTORY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    flags |= os.O_NOFOLLOW
    return flags


def _validate_directory_descriptor(descriptor: int, display: Path) -> None:
    metadata = os.fstat(descriptor)
    mode = stat.S_IMODE(metadata.st_mode)
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid not in {0, os.geteuid()}
        or (mode & 0o022 and not mode & stat.S_ISVTX)
    ):
        raise SystemExit(f"unsafe descriptor-captured ancestor: {display}")


def _descriptor_absolute_path(path: Path) -> Path:
    absolute = path.absolute()
    if absolute.parts[:2] == ("/", "var"):
        absolute = Path("/private").joinpath(*absolute.parts[1:])
    elif absolute.parts[:2] == ("/", "tmp"):
        absolute = Path("/private/tmp").joinpath(*absolute.parts[2:])
    return absolute


def _open_directory_chain(path: Path) -> list[int]:
    absolute = _descriptor_absolute_path(path)
    descriptors = [os.open("/", _directory_flags())]
    try:
        _validate_directory_descriptor(descriptors[-1], Path("/"))
        current = Path("/")
        for component in absolute.parts[1:]:
            current /= component
            descriptor = os.open(component, _directory_flags(), dir_fd=descriptors[-1])
            descriptors.append(descriptor)
            _validate_directory_descriptor(descriptor, current)
        return descriptors
    except BaseException:
        for descriptor in reversed(descriptors):
            os.close(descriptor)
        raise


def _validate_directory_chain(descriptors: list[int], path: Path) -> None:
    absolute = _descriptor_absolute_path(path)
    if len(descriptors) != len(absolute.parts):
        raise SystemExit(f"descriptor-captured ancestor closure changed: {path}")
    for index, component in enumerate(absolute.parts[1:], start=1):
        named = os.stat(component, dir_fd=descriptors[index - 1], follow_symlinks=False)
        opened = os.fstat(descriptors[index])
        identity = lambda value: (
            value.st_dev,
            value.st_ino,
            value.st_mode,
            value.st_uid,
            value.st_gid,
        )
        if identity(named) != identity(opened):
            raise SystemExit(f"descriptor-captured ancestor was substituted: {path}")


def _descriptor_capture_at(
    parent: int,
    name: str,
    display: Path,
    *,
    allow_root_owner: bool = False,
    after_read_for_test: object | None = None,
) -> bytes:
    if not hasattr(os, "O_NOFOLLOW"):
        raise SystemExit("descriptor capture requires O_NOFOLLOW")
    named_before = os.stat(name, dir_fd=parent, follow_symlinks=False)
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, dir_fd=parent)
    try:
        before = os.fstat(descriptor)
        if (
            not stat.S_ISREG(before.st_mode)
            or before.st_nlink != 1
            or before.st_uid
            not in ({0, os.geteuid()} if allow_root_owner else {os.geteuid()})
            or stat.S_IMODE(before.st_mode) & 0o022
        ):
            raise SystemExit(f"unsafe descriptor-captured input: {display}")
        named_identity = lambda value: (
            value.st_dev,
            value.st_ino,
            value.st_mode,
            value.st_uid,
            value.st_gid,
            value.st_nlink,
        )
        if named_identity(named_before) != named_identity(before):
            raise SystemExit(f"descriptor-captured name was substituted: {display}")
        chunks: list[bytes] = []
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.append(chunk)
        if after_read_for_test is not None:
            if not callable(after_read_for_test):
                raise SystemExit("descriptor-capture read hook is not callable")
            after_read_for_test()
        after = os.fstat(descriptor)
        named_after = os.stat(name, dir_fd=parent, follow_symlinks=False)
        identity = lambda value: (
            value.st_dev,
            value.st_ino,
            value.st_size,
            value.st_mtime_ns,
            value.st_ctime_ns,
            value.st_mode,
            value.st_uid,
            value.st_gid,
            value.st_nlink,
        )
        if identity(before) != identity(after):
            raise SystemExit(f"descriptor-captured input changed during read: {display}")
        if named_identity(named_before) != named_identity(named_after):
            raise SystemExit(f"descriptor-captured name changed during read: {display}")
        data = b"".join(chunks)
        if len(data) != before.st_size:
            raise SystemExit(f"descriptor-captured input length changed: {display}")
        return data
    finally:
        os.close(descriptor)


def descriptor_capture(path: Path, *, allow_root_owner: bool = False) -> bytes:
    descriptors = _open_directory_chain(path.parent)
    try:
        captured = _descriptor_capture_at(
            descriptors[-1],
            path.name,
            path,
            allow_root_owner=allow_root_owner,
        )
        _validate_directory_chain(descriptors, path.parent)
        return captured
    finally:
        for descriptor in reversed(descriptors):
            os.close(descriptor)


def descriptor_capture_directory(
    path: Path,
    *,
    after_open_for_test: object | None = None,
    after_file_read_for_test: object | None = None,
) -> dict[str, bytes]:
    descriptors = _open_directory_chain(path)
    directory = descriptors[-1]
    try:
        if after_open_for_test is not None:
            if not callable(after_open_for_test):
                raise SystemExit("descriptor-capture test hook is not callable")
            after_open_for_test()
        before = os.fstat(directory)
        captured: dict[str, bytes] = {}
        for entry in sorted(os.scandir(directory), key=lambda candidate: candidate.name):
            if entry.name in {".", ".."}:
                continue
            metadata = os.stat(entry.name, dir_fd=directory, follow_symlinks=False)
            if not stat.S_ISREG(metadata.st_mode):
                raise SystemExit(f"unsafe successor packet entry: {path / entry.name}")
            captured[entry.name] = _descriptor_capture_at(
                directory,
                entry.name,
                path / entry.name,
                after_read_for_test=(
                    (lambda name=entry.name: after_file_read_for_test(name))
                    if callable(after_file_read_for_test)
                    else after_file_read_for_test
                ),
            )
        after = os.fstat(directory)
        identity = lambda value: (
            value.st_dev,
            value.st_ino,
            value.st_mtime_ns,
            value.st_ctime_ns,
            value.st_mode,
            value.st_uid,
            value.st_gid,
        )
        if identity(before) != identity(after):
            raise SystemExit(f"descriptor-captured directory changed during read: {path}")
        _validate_directory_chain(descriptors, path)
        return captured
    finally:
        for descriptor in reversed(descriptors):
            os.close(descriptor)


def sha256(path: Path) -> str:
    return hashlib.sha256(descriptor_capture(path)).hexdigest()


def require_record_order(label: str, ordinals: list[int]) -> None:
    if ordinals != sorted(ordinals) or len(set(ordinals)) != len(ordinals):
        raise SystemExit(f"{label} records are reordered")


def require_strictly_before(label: str, earlier: int, later: int) -> None:
    if earlier >= later:
        raise SystemExit(f"{label} is not strictly ordered")


def capture_log_records(
    path: Path, expected: dict[str, str]
) -> tuple[dict[str, dict[str, object]], dict[str, int]]:
    expected_by_hash = {digest: name for name, digest in expected.items()}
    captured: dict[str, dict[str, object]] = {}
    ordinals: dict[str, int] = {}
    for ordinal, raw_line in enumerate(descriptor_capture(path).splitlines(keepends=True), start=1):
        name = expected_by_hash.get(hashlib.sha256(raw_line).hexdigest())
        if name is None:
            continue
        if name in captured:
            raise SystemExit(f"duplicate captured record: {name}")
        record = json.loads(raw_line)
        if not isinstance(record, dict):
            raise SystemExit(f"captured record is not an object: {name}")
        captured[name] = record
        ordinals[name] = ordinal
    require_equal("captured record closure", set(captured), set(expected))
    return captured, ordinals


def require_equal(label: str, actual: object, expected: object) -> None:
    if actual != expected:
        raise SystemExit(f"{label}: expected {expected!r}, got {actual!r}")


def lp(name: str, value: str) -> bytes:
    encoded = value.encode("utf-8")
    return name.encode("utf-8") + b"=" + str(len(encoded)).encode("ascii") + b":" + encoded + b"\n"


def digest_records(domain: bytes, records: list[tuple[str, str]]) -> str:
    return hashlib.sha256(domain + b"".join(lp(name, value) for name, value in records)).hexdigest()


def successor_approval(bindings: dict[str, object]) -> bool:
    approval = bindings["external_approval"]
    assert isinstance(approval, dict)
    return approval.get("packet_sha256") == SUCCESSOR_PACKET_SHA256


def verify_successor_packet_artifacts(
    bindings: dict[str, object],
) -> dict[str, bytes]:
    approval = bindings["external_approval"]
    event = bindings["external_approval_event"]
    assert isinstance(approval, dict)
    assert isinstance(event, dict)
    require_equal("successor approved packet", approval["packet_sha256"], SUCCESSOR_PACKET_SHA256)
    require_equal("successor candidate input", approval["candidate_input_commitment"], SUCCESSOR_CANDIDATE_INPUT)
    require_equal("successor build handoff", approval["build_plan_handoff"], SUCCESSOR_BUILD_HANDOFF)
    require_equal("successor packet root", event["packet_root"], str(SUCCESSOR_PACKET_ROOT))
    require_equal("successor packet verifier", event["independent_verifier"], str(SUCCESSOR_PACKET_VERIFIER))
    require_equal("successor packet verifier digest", event["independent_verifier_sha256"], SUCCESSOR_PACKET_VERIFIER_SHA256)
    for label, path in (
        ("successor packet root", SUCCESSOR_PACKET_ROOT),
        ("successor packet verifier", SUCCESSOR_PACKET_VERIFIER),
    ):
        metadata = os.lstat(path)
        if stat.S_ISLNK(metadata.st_mode):
            raise SystemExit(f"{label} is a symlink")
    verifier_bytes = descriptor_capture(SUCCESSOR_PACKET_VERIFIER)
    require_equal(
        "successor packet verifier bytes",
        hashlib.sha256(verifier_bytes).hexdigest(),
        SUCCESSOR_PACKET_VERIFIER_SHA256,
    )
    packet_files = descriptor_capture_directory(SUCCESSOR_PACKET_ROOT)
    ruby_bytes = descriptor_capture(SUCCESSOR_RUBY, allow_root_owner=True)
    require_equal(
        "successor Ruby executable bytes",
        hashlib.sha256(ruby_bytes).hexdigest(),
        SUCCESSOR_RUBY_SHA256,
    )
    ruby_probe = subprocess.run(
        [str(SUCCESSOR_RUBY), "--version"],
        check=False,
        capture_output=True,
        text=True,
        env={
            "LANG": "C",
            "LC_ALL": "C",
            "PATH": "/usr/bin:/bin",
            "RUBYLIB": "",
            "RUBYOPT": "",
        },
    )
    require_equal("successor Ruby probe exit", ruby_probe.returncode, 0)
    require_equal(
        "successor Ruby probe",
        (ruby_probe.stdout or ruby_probe.stderr).strip(),
        SUCCESSOR_RUBY_PROBE,
    )
    packet_name = "replacement-build-approval-packet.v1.json"
    if packet_name not in packet_files:
        raise SystemExit("successor packet artifact is absent")
    packet_bytes = packet_files[packet_name]
    require_equal(
        "successor packet artifact bytes",
        hashlib.sha256(packet_bytes).hexdigest(),
        SUCCESSOR_PACKET_ARTIFACT_SHA256,
    )
    packet = json.loads(packet_bytes)
    require_equal("successor packet identity", packet["packet_sha256"], SUCCESSOR_PACKET_SHA256)
    require_equal("successor packet candidate", packet["candidate_input"]["identity"], SUCCESSOR_CANDIDATE_INPUT)
    require_equal("successor packet handoff", packet["build_plan_handoff"]["identity"], SUCCESSOR_BUILD_HANDOFF)
    require_equal(
        "successor packet Decision counts",
        packet["decision_counts"],
        {"locked": 117, "open": 0, "superseded": 96, "total": 213},
    )
    identity_state = packet["sections"]["identity_state"]["text"]
    if (
        "candidate_contract_root=absent-before-stage-0" not in identity_state
        or "canonical_build_handoff=absent-before-stage-0" not in identity_state
    ):
        raise SystemExit("successor packet prematurely promotes an external identity")
    with tempfile.TemporaryDirectory(prefix="maestro-successor-packet-") as directory:
        snapshot = Path(directory)
        snapshot.chmod(0o700)
        packet_snapshot = snapshot / "packet"
        packet_snapshot.mkdir(mode=0o700)
        for name, captured in packet_files.items():
            destination = packet_snapshot / name
            destination.write_bytes(captured)
            destination.chmod(0o400)
        verifier_snapshot = snapshot / "verify.rb"
        verifier_snapshot.write_bytes(verifier_bytes)
        verifier_snapshot.chmod(0o500)
        completed = subprocess.run(
            [str(SUCCESSOR_RUBY), str(verifier_snapshot), str(packet_snapshot)],
            check=False,
            capture_output=True,
            text=True,
            cwd=snapshot,
            env={
                "HOME": str(snapshot),
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
                "RUBYLIB": "",
                "RUBYOPT": "",
            },
        )
    if completed.returncode != 0:
        raise SystemExit(f"independent successor packet verifier failed: {completed.stderr}")
    verdict = json.loads(completed.stdout)
    require_equal("successor packet verifier schema", verdict["schema"], "ExternalBuildApprovalPacketIndependentVerificationV1")
    require_equal("successor packet verifier packet", verdict["packet_sha256"], SUCCESSOR_PACKET_SHA256)
    require_equal("successor packet verifier candidate", verdict["candidate_input"], SUCCESSOR_CANDIDATE_INPUT)
    require_equal("successor packet verifier handoff", verdict["build_plan_handoff"], SUCCESSOR_BUILD_HANDOFF)
    require_equal("successor packet verifier status", verdict["status"], "verified")
    return packet_files


def verify_successor_external_approval_event(
    bindings: dict[str, object],
) -> dict[str, bytes]:
    event = bindings["external_approval_event"]
    assert isinstance(event, dict)
    required = {
        "attestation_source",
        "provenance_assurance",
        "log_realpath",
        "record_set_sha256",
        "record_sha256",
        "packet_publication_kind",
        "packet_root",
        "independent_verifier",
        "independent_verifier_sha256",
        "recipient_thread_id",
        "approval_turn_id",
        "approval_turn_started_at",
        "user_message_id",
        "actor_role",
        "exact_instruction",
        "packet_publication_log_realpath",
        "packet_publication_turn_id",
        "packet_publication_message_id",
        "packet_publication_record_sha256",
        "packet_turn_completed_at",
    }
    optional = {"historical_superseded_events"}
    require_equal("successor approval event keys", set(event), required | (set(event) & optional))
    require_equal("successor approval attestation source", event["attestation_source"], "pinned_local_codex_session_records_v1")
    require_equal("successor approval assurance", event["provenance_assurance"], "unsigned_local_platform_log_bound_by_sha256")
    require_equal("successor packet publication kind", event["packet_publication_kind"], "independently_verified_external_packet_v1")
    require_equal("successor approval log", event["log_realpath"], str(SUCCESSOR_APPROVAL_LOG))
    require_equal("successor approval recipient", event["recipient_thread_id"], SUCCESSOR_RECIPIENT_TASK)
    require_equal("successor approval turn", event["approval_turn_id"], SUCCESSOR_APPROVAL_TURN)
    require_equal("successor approval started", event["approval_turn_started_at"], SUCCESSOR_APPROVAL_STARTED_AT)
    require_equal("successor approval message", event["user_message_id"], SUCCESSOR_APPROVAL_MESSAGE)
    require_equal("successor approval actor", event["actor_role"], "user")
    require_equal("successor approval instruction", event["exact_instruction"], SUCCESSOR_APPROVAL_INSTRUCTION)
    require_equal(
        "successor packet publication log",
        event["packet_publication_log_realpath"],
        str(SUCCESSOR_PACKET_PUBLICATION_LOG),
    )
    require_equal(
        "successor packet publication turn",
        event["packet_publication_turn_id"],
        SUCCESSOR_PACKET_PUBLICATION_TURN,
    )
    require_equal(
        "successor packet publication message",
        event["packet_publication_message_id"],
        SUCCESSOR_PACKET_PUBLICATION_MESSAGE,
    )
    require_equal(
        "successor packet publication completion",
        event["packet_turn_completed_at"],
        SUCCESSOR_PACKET_COMPLETED_AT,
    )
    packet_record_hashes = event["packet_publication_record_sha256"]
    assert isinstance(packet_record_hashes, dict)
    require_equal(
        "successor packet publication records",
        packet_record_hashes,
        SUCCESSOR_PACKET_PUBLICATION_RECORDS,
    )
    record_hashes = event["record_sha256"]
    assert isinstance(record_hashes, dict)
    require_equal("successor approval record set", record_hashes, SUCCESSOR_APPROVAL_RECORDS)
    captured, ordinals = capture_log_records(
        SUCCESSOR_APPROVAL_LOG, SUCCESSOR_APPROVAL_RECORDS
    )
    causal_order = ["approval_task_started", "approval_user_message", "session_meta"]
    require_successor_approval_record_order(
        [ordinals[name] for name in causal_order]
    )
    capture_identity = digest_records(
        b"maestro.external-successor-approval-capture.v1\0",
        [
            ("log_realpath", str(SUCCESSOR_APPROVAL_LOG)),
            ("recipient_task_id", SUCCESSOR_RECIPIENT_TASK),
            *[(f"{name}_sha256", SUCCESSOR_APPROVAL_RECORDS[name]) for name in causal_order],
        ],
    )
    require_equal("successor approval record-set identity", event["record_set_sha256"], capture_identity)
    started = captured["approval_task_started"]
    require_equal("successor start record type", started["type"], "event_msg")
    started_payload = started["payload"]
    assert isinstance(started_payload, dict)
    require_equal("successor start event type", started_payload["type"], "task_started")
    require_equal("successor start turn", started_payload["turn_id"], SUCCESSOR_APPROVAL_TURN)
    require_equal("successor start time", started_payload["started_at"], SUCCESSOR_APPROVAL_STARTED_AT)
    message = captured["approval_user_message"]
    require_equal("successor approval record type", message["type"], "response_item")
    message_payload = message["payload"]
    assert isinstance(message_payload, dict)
    require_equal("successor approval payload type", message_payload["type"], "message")
    require_equal("successor approval payload role", message_payload["role"], "user")
    require_equal("successor approval payload id", message_payload["id"], SUCCESSOR_APPROVAL_MESSAGE)
    metadata = message_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(metadata, dict)
    require_equal("successor approval payload turn", metadata["turn_id"], SUCCESSOR_APPROVAL_TURN)
    require_equal(
        "successor approval payload content",
        message_payload["content"],
        [{"type": "input_text", "text": SUCCESSOR_APPROVAL_INSTRUCTION + "\n"}],
    )
    session = captured["session_meta"]
    require_equal("successor session record type", session["type"], "session_meta")
    session_payload = session["payload"]
    assert isinstance(session_payload, dict)
    require_equal("successor session id", session_payload["session_id"], SUCCESSOR_RECIPIENT_TASK)
    require_equal("successor session payload id", session_payload["id"], SUCCESSOR_RECIPIENT_TASK)
    require_equal("successor session cwd", session_payload["cwd"], bindings["source_repository_realpath"])
    packet_captured, packet_ordinals = capture_log_records(
        SUCCESSOR_PACKET_PUBLICATION_LOG, SUCCESSOR_PACKET_PUBLICATION_RECORDS
    )
    require_successor_packet_record_order(
        [
            packet_ordinals["packet_assistant_message"],
            packet_ordinals["packet_task_complete"],
        ]
    )
    packet_message = packet_captured["packet_assistant_message"]
    require_equal("successor packet message type", packet_message["type"], "response_item")
    packet_message_payload = packet_message["payload"]
    assert isinstance(packet_message_payload, dict)
    require_equal("successor packet message role", packet_message_payload["role"], "assistant")
    require_equal(
        "successor packet message id",
        packet_message_payload["id"],
        SUCCESSOR_PACKET_PUBLICATION_MESSAGE,
    )
    packet_metadata = packet_message_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(packet_metadata, dict)
    require_equal(
        "successor packet message turn",
        packet_metadata["turn_id"],
        SUCCESSOR_PACKET_PUBLICATION_TURN,
    )
    packet_content = packet_message_payload["content"]
    if not isinstance(packet_content, list) or len(packet_content) != 1:
        raise SystemExit("successor packet publication is not one assistant message")
    packet_text = str(packet_content[0].get("text", ""))
    for literal in (
        SUCCESSOR_PACKET_SHA256,
        SUCCESSOR_CANDIDATE_INPUT,
        SUCCESSOR_BUILD_HANDOFF,
        SUCCESSOR_APPROVAL_INSTRUCTION,
    ):
        if literal not in packet_text:
            raise SystemExit("successor packet publication omits an exact bound identity")
    packet_complete = packet_captured["packet_task_complete"]
    require_equal("successor packet completion type", packet_complete["type"], "event_msg")
    packet_complete_payload = packet_complete["payload"]
    assert isinstance(packet_complete_payload, dict)
    require_equal(
        "successor packet completion event",
        packet_complete_payload["type"],
        "task_complete",
    )
    require_equal(
        "successor packet completion turn",
        packet_complete_payload["turn_id"],
        SUCCESSOR_PACKET_PUBLICATION_TURN,
    )
    require_equal(
        "successor packet completion time",
        packet_complete_payload["completed_at"],
        SUCCESSOR_PACKET_COMPLETED_AT,
    )
    require_successor_packet_before_approval(
        int(packet_complete_payload["completed_at"]),
        SUCCESSOR_APPROVAL_STARTED_AT,
    )
    return verify_successor_packet_artifacts(bindings)


def require_successor_approval_record_order(ordinals: list[int]) -> None:
    require_record_order("successor approval", ordinals)


def require_successor_packet_record_order(ordinals: list[int]) -> None:
    require_record_order("successor packet publication", ordinals)


def require_successor_packet_before_approval(
    packet_completed_at: int,
    approval_started_at: int,
) -> None:
    require_strictly_before(
        "successor packet publication before approval",
        packet_completed_at,
        approval_started_at,
    )


def path_metadata(path: Path) -> dict[str, str]:
    metadata = os.stat(path, follow_symlinks=False)
    if not stat.S_ISDIR(metadata.st_mode) or path.is_symlink():
        raise SystemExit(f"bound path is not a no-follow directory: {path}")
    return {
        "device": str(metadata.st_dev),
        "inode": str(metadata.st_ino),
        "uid": str(metadata.st_uid),
        "gid": str(metadata.st_gid),
        "mode": format(stat.S_IMODE(metadata.st_mode), "04o"),
        "type": "directory",
    }


def verify_external_control_bindings(bindings: dict[str, object], source: Path) -> None:
    controls = bindings.get(
        "current_external_control_bindings", bindings["external_control_bindings"]
    )
    assert isinstance(controls, dict)
    source_control = controls["source_git_control"]
    destination = controls["destination_ancestors"]
    sandbox = controls["checkout_sandbox_profile"]
    assert isinstance(source_control, dict)
    assert isinstance(destination, dict)
    assert isinstance(sandbox, dict)

    git_dir = source / ".git"
    object_dir = git_dir / "objects"
    require_equal("git common dir realpath", str(git_dir.resolve(strict=True)), source_control["git_common_dir_realpath"])
    require_equal("object directory realpath", str(object_dir.resolve(strict=True)), source_control["object_directory_realpath"])
    for prefix, path in (("git_common_dir", git_dir), ("object_directory", object_dir)):
        metadata = path_metadata(path)
        for key in ("device", "inode", "uid", "gid", "mode"):
            require_equal(f"{prefix} {key}", metadata[key], source_control[f"{prefix}_{key}"])

    fixed_paths = {
        "config.worktree",
        "objects/info/alternates",
        "info/attributes",
        "info/grafts",
        "shallow",
    }
    replace_root = git_dir / "refs/replace"
    if replace_root.exists():
        fixed_paths.update(
            str(path.relative_to(git_dir))
            for path in replace_root.rglob("*")
            if path.is_file() or path.is_symlink()
        )
    fixed_paths.update(
        str(path.relative_to(git_dir)) for path in object_dir.glob("pack/*.promisor")
    )
    rows: list[str] = []
    for relative in sorted(fixed_paths):
        path = git_dir / relative
        if not path.exists() and not path.is_symlink():
            rows.append(f"{relative}\tabsent\t-")
        elif path.is_file() and not path.is_symlink():
            rows.append(f"{relative}\tregular\t{sha256(path)}")
        else:
            raise SystemExit(f"unsafe Git control path: {path}")
    require_equal("Git control rows", rows, source_control["git_control_path_manifest"])
    manifest = ("\n".join(rows) + "\n").encode("utf-8")
    require_equal(
        "Git control manifest digest",
        hashlib.sha256(manifest).hexdigest(),
        source_control["git_control_path_manifest_sha256"],
    )
    require_equal("repository config digest", sha256(git_dir / "config"), source_control["repository_config_sha256"])

    baseline = bindings["baseline"]
    assert isinstance(baseline, dict)
    source_records = [
        ("source_repository_realpath", str(source)),
        ("git_common_dir_realpath", str(git_dir.resolve(strict=True))),
        ("git_common_dir_device", str(path_metadata(git_dir)["device"])),
        ("git_common_dir_inode", str(path_metadata(git_dir)["inode"])),
        ("git_common_dir_uid", str(path_metadata(git_dir)["uid"])),
        ("git_common_dir_gid", str(path_metadata(git_dir)["gid"])),
        ("git_common_dir_mode", str(path_metadata(git_dir)["mode"])),
        ("object_directory_realpath", str(object_dir.resolve(strict=True))),
        ("object_directory_device", str(path_metadata(object_dir)["device"])),
        ("object_directory_inode", str(path_metadata(object_dir)["inode"])),
        ("object_directory_uid", str(path_metadata(object_dir)["uid"])),
        ("object_directory_gid", str(path_metadata(object_dir)["gid"])),
        ("object_directory_mode", str(path_metadata(object_dir)["mode"])),
        ("object_format", str(source_control["object_format"])),
        ("repository_config_sha256", str(source_control["repository_config_sha256"])),
        ("git_control_path_manifest_sha256", str(source_control["git_control_path_manifest_sha256"])),
        ("baseline_commit", str(baseline["commit"])),
        ("baseline_tree", str(baseline["tree"])),
    ]
    require_equal(
        "source Git control identity",
        digest_records(b"maestro.external-git-source-control.v1\0", source_records),
        source_control["identity_sha256"],
    )

    ancestors = destination["ancestors"]
    assert isinstance(ancestors, list)
    destination_records = [("ancestor_count", str(len(ancestors)))]
    for index, entry in enumerate(ancestors):
        assert isinstance(entry, dict)
        path = Path(str(entry["path"]))
        require_equal(f"ancestor {index} realpath", str(path.resolve(strict=True)), entry["realpath"])
        metadata = path_metadata(path)
        for key in ("device", "inode", "uid", "gid", "mode", "type"):
            require_equal(f"ancestor {index} {key}", metadata[key], entry[key])
        for key in ("path", "realpath", "device", "inode", "uid", "gid", "mode", "type"):
            destination_records.append((f"ancestor_{index}_{key}", str(entry[key])))
    require_equal(
        "destination ancestor identity",
        digest_records(b"maestro.external-destination-ancestors.v1\0", destination_records),
        destination["identity_sha256"],
    )

    sandbox_names = [
        "profile_id", "config_sources", "hooks", "filters", "attributes_transform",
        "external_commands", "network", "submodule_checkout", "object_source",
        "registration", "materializer", "verifier", "symlink_policy",
    ]
    sandbox_records = [(name, str(sandbox[name])) for name in sandbox_names]
    require_equal(
        "checkout sandbox profile identity",
        digest_records(b"maestro.external-checkout-sandbox.v1\0", sandbox_records),
        sandbox["identity_sha256"],
    )


def verify_current_control_rebind(bindings: dict[str, object]) -> None:
    approved = bindings["external_control_bindings"]
    current = bindings["current_external_control_bindings"]
    assert isinstance(approved, dict)
    assert isinstance(current, dict)

    approved_source = approved["source_git_control"]
    current_source = current["source_git_control"]
    assert isinstance(approved_source, dict)
    assert isinstance(current_source, dict)
    require_equal("current source-control keys", set(current_source), set(approved_source))
    source_device_keys = {
        "identity_sha256",
        "git_common_dir_device",
        "object_directory_device",
    }
    for key in sorted(set(approved_source) - source_device_keys):
        require_equal(
            f"current source-control stable field {key}",
            current_source[key],
            approved_source[key],
        )
    approved_device = str(approved_source["git_common_dir_device"])
    current_device = str(current_source["git_common_dir_device"])
    if approved_device == current_device:
        raise SystemExit("current source-control rebind did not change the device")
    require_equal(
        "current object-directory device",
        current_source["object_directory_device"],
        current_device,
    )
    require_equal(
        "approved object-directory device",
        approved_source["object_directory_device"],
        approved_device,
    )

    approved_destination = approved["destination_ancestors"]
    current_destination = current["destination_ancestors"]
    assert isinstance(approved_destination, dict)
    assert isinstance(current_destination, dict)
    approved_ancestors = approved_destination["ancestors"]
    current_ancestors = current_destination["ancestors"]
    assert isinstance(approved_ancestors, list)
    assert isinstance(current_ancestors, list)
    require_equal(
        "current destination ancestor count",
        len(current_ancestors),
        len(approved_ancestors),
    )
    for index, (old_entry, new_entry) in enumerate(
        zip(approved_ancestors, current_ancestors, strict=True)
    ):
        assert isinstance(old_entry, dict)
        assert isinstance(new_entry, dict)
        require_equal(
            f"current destination ancestor {index} keys",
            set(new_entry),
            set(old_entry),
        )
        for key in sorted(set(old_entry) - {"device"}):
            require_equal(
                f"current destination ancestor {index} stable field {key}",
                new_entry[key],
                old_entry[key],
            )
        require_equal(
            f"current destination ancestor {index} device",
            new_entry["device"],
            current_device,
        )
        require_equal(
            f"approved destination ancestor {index} device",
            old_entry["device"],
            approved_device,
        )
    require_equal(
        "current checkout sandbox profile",
        current["checkout_sandbox_profile"],
        approved["checkout_sandbox_profile"],
    )


def verify_post_approval_execution_plan_revisions(
    bindings: dict[str, object],
) -> None:
    revision_set = bindings["post_approval_execution_plan_revisions"]
    canonical_inputs = bindings["canonical_source_inputs"]
    current_inputs = bindings["current_source_inputs"]
    assert isinstance(revision_set, dict)
    assert isinstance(canonical_inputs, dict)
    assert isinstance(current_inputs, dict)
    require_equal(
        "execution-plan revision schema",
        revision_set["schema"],
        "maestro.external-execution-plan-revisions.v1",
    )
    require_equal(
        "execution-plan revision attestation source",
        revision_set["attestation_source"],
        "pinned_local_codex_session_records_v1",
    )
    require_equal(
        "execution-plan revision provenance assurance",
        revision_set["provenance_assurance"],
        "unsigned_local_platform_log_bound_by_sha256",
    )
    require_equal(
        "current source input keys",
        set(current_inputs),
        {"card_sha256", "design_sha256", "decisions_sha256"},
    )
    for key in ("card_sha256", "decisions_sha256"):
        require_equal(
            f"post-approval stable {key}", current_inputs[key], canonical_inputs[key]
        )

    design_revisions = revision_set["design_revisions"]
    handoff = revision_set["stage5_handoff"]
    amendment = revision_set["stage5_execution_amendment"]
    assert isinstance(design_revisions, list)
    assert isinstance(handoff, dict)
    assert isinstance(amendment, dict)
    require_equal("post-approval design revision count", len(design_revisions), 2)
    records = [*design_revisions, handoff, amendment]
    wanted: dict[str, dict[str, object]] = {}
    for entry in records:
        assert isinstance(entry, dict)
        digest = str(entry["record_sha256"])
        if digest in wanted:
            raise SystemExit("post-approval revision record digest is duplicated")
        wanted[digest] = entry

    log_path = Path(str(revision_set["log_realpath"]))
    if not log_path.is_file() or log_path.is_symlink():
        raise SystemExit(f"execution-plan revision log is not a regular file: {log_path}")
    require_equal(
        "execution-plan revision log realpath",
        str(log_path.resolve(strict=True)),
        str(log_path),
    )
    captured: dict[str, dict[str, object]] = {}
    with log_path.open(encoding="utf-8") as handle:
        for raw_line in handle:
            digest = hashlib.sha256(raw_line.encode()).hexdigest()
            if digest not in wanted:
                continue
            if digest in captured:
                raise SystemExit(
                    f"post-approval revision record occurs more than once: {digest}"
                )
            record = json.loads(raw_line)
            if not isinstance(record, dict):
                raise SystemExit("post-approval revision record is not an object")
            captured[digest] = record
    require_equal(
        "post-approval revision record closure", set(captured), set(wanted)
    )

    expected_previous = str(canonical_inputs["design_sha256"])
    last_timestamp: str | None = None
    for index, entry in enumerate(design_revisions):
        assert isinstance(entry, dict)
        require_equal(
            f"design revision {index} previous hash",
            entry["previous_design_sha256"],
            expected_previous,
        )
        record = captured[str(entry["record_sha256"])]
        timestamp = str(record["timestamp"])
        if last_timestamp is not None and timestamp <= last_timestamp:
            raise SystemExit("post-approval design revisions are not in causal order")
        last_timestamp = timestamp
        payload = record["payload"]
        assert isinstance(payload, dict)
        require_equal(f"design revision {index} record type", record["type"], "response_item")
        require_equal(f"design revision {index} payload type", payload["type"], "message")
        require_equal(f"design revision {index} actor", payload["role"], "user")
        require_equal(f"design revision {index} message id", payload["id"], entry["message_id"])
        metadata = payload["internal_chat_message_metadata_passthrough"]
        assert isinstance(metadata, dict)
        require_equal(f"design revision {index} turn", metadata["turn_id"], entry["turn_id"])
        content = payload["content"]
        if not isinstance(content, list) or len(content) != 1 or not isinstance(content[0], dict):
            raise SystemExit(f"design revision {index} is not one exact text item")
        require_equal(f"design revision {index} content type", content[0]["type"], "input_text")
        text = str(content[0]["text"])
        required = (
            f"<source_thread_id>{entry['source_thread_id']}</source_thread_id>",
            str(entry["current_design_sha256"]),
        )
        if not all(fragment in text for fragment in required):
            raise SystemExit(f"design revision {index} does not bind its source and hash")
        classification = entry["classification"]
        if classification == "stage12_acceptance_clarification_only":
            fragments = (
                "post-approval Phase-12 acceptance clarification",
                "should not alter the active Stage 4 runtime slice",
                "preservation of all *V1/content-addressed contract, proof, Receipt, and migration identities",
            )
        elif classification == "implementation_order_only":
            fragments = (
                "The product contracts, Stage deliverables, and canonical integration/certification order remain unchanged.",
                "Existing proof artifacts with design provenance must be rebound/regenerated at the next safe boundary.",
                "Do not create or launch the Orchestrator yet.",
            )
        else:
            raise SystemExit(f"unknown post-approval design classification: {classification}")
        if not all(fragment in text for fragment in fragments):
            raise SystemExit(f"design revision {index} exceeds its declared classification")
        expected_previous = str(entry["current_design_sha256"])
    handoff_record = captured[str(handoff["record_sha256"])]
    handoff_timestamp = str(handoff_record["timestamp"])
    if last_timestamp is not None and handoff_timestamp <= last_timestamp:
        raise SystemExit("Stage-5 handoff is not causally after the design revisions")
    handoff_payload = handoff_record["payload"]
    assert isinstance(handoff_payload, dict)
    require_equal("Stage-5 handoff record type", handoff_record["type"], "response_item")
    require_equal("Stage-5 handoff payload type", handoff_payload["type"], "message")
    require_equal("Stage-5 handoff actor", handoff_payload["role"], "user")
    require_equal("Stage-5 handoff message id", handoff_payload["id"], handoff["message_id"])
    handoff_metadata = handoff_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(handoff_metadata, dict)
    require_equal("Stage-5 handoff turn", handoff_metadata["turn_id"], handoff["turn_id"])
    handoff_content = handoff_payload["content"]
    if not isinstance(handoff_content, list) or len(handoff_content) != 1 or not isinstance(handoff_content[0], dict):
        raise SystemExit("Stage-5 handoff is not one exact text item")
    require_equal("Stage-5 handoff content type", handoff_content[0]["type"], "input_text")
    handoff_text = str(handoff_content[0]["text"])
    require_equal("Stage-5 handoff design", handoff["design_sha256"], expected_previous)
    require_equal(
        "Stage-5 handoff boundary",
        handoff["boundary"],
        "finish_and_commit_stage5_do_not_begin_stage6",
    )
    handoff_fragments = (
        f"<source_thread_id>{handoff['source_thread_id']}</source_thread_id>",
        str(handoff["design_sha256"]),
        "Commit the complete Stage-5-owned source, tests and proof artifacts atomically",
        "Do not begin Stage 6.",
        "Do not create worker threads or an Orchestrator thread yet.",
    )
    if not all(fragment in handoff_text for fragment in handoff_fragments):
        raise SystemExit("Stage-5 handoff does not bind its exact continuation boundary")

    amendment_record = captured[str(amendment["record_sha256"])]
    amendment_timestamp = str(amendment_record["timestamp"])
    if amendment_timestamp <= handoff_timestamp:
        raise SystemExit("Stage-5 execution amendment is not causally after the handoff")
    amendment_payload = amendment_record["payload"]
    assert isinstance(amendment_payload, dict)
    require_equal(
        "Stage-5 execution amendment record type", amendment_record["type"], "response_item"
    )
    require_equal(
        "Stage-5 execution amendment payload type", amendment_payload["type"], "message"
    )
    require_equal("Stage-5 execution amendment actor", amendment_payload["role"], "user")
    require_equal(
        "Stage-5 execution amendment message id",
        amendment_payload["id"],
        amendment["message_id"],
    )
    amendment_metadata = amendment_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(amendment_metadata, dict)
    require_equal(
        "Stage-5 execution amendment turn",
        amendment_metadata["turn_id"],
        amendment["turn_id"],
    )
    amendment_content = amendment_payload["content"]
    if (
        not isinstance(amendment_content, list)
        or len(amendment_content) != 1
        or not isinstance(amendment_content[0], dict)
    ):
        raise SystemExit("Stage-5 execution amendment is not one exact text item")
    require_equal(
        "Stage-5 execution amendment content type",
        amendment_content[0]["type"],
        "input_text",
    )
    require_equal(
        "Stage-5 execution amendment previous design",
        amendment["previous_design_sha256"],
        expected_previous,
    )
    require_equal(
        "Stage-5 execution amendment current design",
        amendment["current_design_sha256"],
        current_inputs["design_sha256"],
    )
    require_equal(
        "Stage-5 execution amendment classification",
        amendment["classification"],
        "full_seal_scheduling_only",
    )
    require_equal(
        "Stage-5 execution amendment boundary",
        amendment["boundary"],
        "checkpoint_then_one_current_stage5_full_seal_then_stop_before_stage6",
    )
    amendment_text = str(amendment_content[0]["text"])
    amendment_fragments = (
        f"<source_thread_id>{amendment['source_thread_id']}</source_thread_id>",
        str(amendment["previous_design_sha256"]),
        str(amendment["current_design_sha256"]),
        "Preserve prior committed Stage receipts as immutable history.",
        "Main runs exactly one full independent multi-engine seal for the current Stage 5.",
        "checkpoint commit / primary ff-only preservation sequence remains in force",
        "Do not open an Orchestrator or workers.",
    )
    if not all(fragment in amendment_text for fragment in amendment_fragments):
        raise SystemExit("Stage-5 execution amendment exceeds its scheduling-only boundary")


def external_candidate_commitment(
    bindings: dict[str, object],
    *,
    source_control_sha256: str | None = None,
    destination_sha256: str | None = None,
) -> str:
    source_inputs = bindings["canonical_source_inputs"]
    controls = bindings["external_control_bindings"]
    external = bindings["external_candidate_input_fields"]
    approval = bindings["external_approval"]
    baseline = bindings["baseline"]
    assert isinstance(source_inputs, dict)
    assert isinstance(controls, dict)
    assert isinstance(external, dict)
    assert isinstance(approval, dict)
    assert isinstance(baseline, dict)
    records = [
        ("source_repository_realpath", str(bindings["source_repository_realpath"])),
        ("implementation_workspace_path", str(bindings["implementation_workspace_path"])),
        ("implementation_workspace_policy", str(bindings["implementation_workspace_policy"])),
        ("baseline_commit", str(baseline["commit"])),
        ("baseline_tree", str(baseline["tree"])),
        ("source_git_control_binding_sha256", source_control_sha256 or str(controls["source_git_control"]["identity_sha256"])),
        ("destination_ancestor_binding_sha256", destination_sha256 or str(controls["destination_ancestors"]["identity_sha256"])),
        ("checkout_sandbox_profile_sha256", str(controls["checkout_sandbox_profile"]["identity_sha256"])),
        ("feature_id", str(bindings["feature_id"])),
        ("feature_state", str(bindings["feature_state"])),
        ("card_sha256", str(source_inputs["card_sha256"])),
        ("design_sha256", str(source_inputs["design_sha256"])),
        ("decisions_sha256", str(source_inputs["decisions_sha256"])),
        ("raw_decision_inventory_sha256", str(external["raw_decision_inventory_sha256"])),
        ("external_design_authority_closure_sha256", str(external["external_design_authority_closure_sha256"])),
        ("capability_census_sha256", str(external["capability_census_sha256"])),
        ("resource_consumer_census_sha256", str(external["resource_consumer_census_sha256"])),
        ("migration_rollback_removal_sha256", str(external["migration_rollback_removal_sha256"])),
    ]
    return digest_records(b"maestro.external-candidate-input.v1\0", records)


def verify_external_candidate_commitment(bindings: dict[str, object]) -> None:
    approval = bindings["external_approval"]
    assert isinstance(approval, dict)
    require_equal(
        "external candidate input commitment",
        external_candidate_commitment(bindings),
        approval["candidate_input_commitment"],
    )


def verify_baseline_objects(bindings: dict[str, object], source: Path) -> None:
    workspace = Path(str(bindings["implementation_workspace_path"]))
    git_file = workspace / ".git"
    if not git_file.is_file() or git_file.is_symlink():
        raise SystemExit(f"implementation workspace lacks a regular Git pointer: {git_file}")
    pointer = git_file.read_text(encoding="utf-8").strip()
    if not pointer.startswith("gitdir: "):
        raise SystemExit("implementation workspace Git pointer is malformed")
    administrative_dir = Path(pointer.removeprefix("gitdir: ")).resolve(strict=True)
    common_dir = (source / ".git").resolve(strict=True)
    try:
        administrative_dir.relative_to(common_dir / "worktrees")
    except ValueError as error:
        raise SystemExit("implementation workspace is not registered under the bound source") from error

    baseline = bindings["baseline"]
    controls = bindings["external_control_bindings"]
    assert isinstance(baseline, dict)
    assert isinstance(controls, dict)
    source_control = controls["source_git_control"]
    assert isinstance(source_control, dict)
    commit_id = str(baseline["commit"])
    tree_id = str(baseline["tree"])
    head_value = (administrative_dir / "HEAD").read_text().strip()
    if head_value.startswith("ref: "):
        relative_ref = head_value.removeprefix("ref: ")
        if not relative_ref.startswith("refs/heads/") or ".." in Path(relative_ref).parts:
            raise SystemExit("implementation workspace HEAD has an unsafe symbolic ref")
        ref_path = common_dir / relative_ref
        if not ref_path.is_file() or ref_path.is_symlink():
            raise SystemExit("implementation workspace branch ref is not a regular file")
        head_value = ref_path.read_text().strip()
    current_base = bindings.get("current_implementation_base", baseline)
    assert isinstance(current_base, dict)
    current_commit_id = str(current_base["commit"])
    current_tree_id = str(current_base["tree"])
    require_equal("linked worktree common-dir pointer", (administrative_dir / "commondir").read_text().strip(), "../..")

    object_dir = Path(str(source_control["object_directory_realpath"]))

    def loose_object(object_id: str, expected_kind: str) -> bytes:
        path = object_dir / object_id[:2] / object_id[2:]
        if not path.is_file() or path.is_symlink():
            raise SystemExit(f"bound baseline {expected_kind} is not an available loose object: {object_id}")
        raw = zlib.decompress(path.read_bytes())
        require_equal(f"{expected_kind} object identity", hashlib.sha1(raw).hexdigest(), object_id)
        header, body = raw.split(b"\0", 1)
        kind, length = header.decode("ascii").split(" ", 1)
        require_equal(f"{expected_kind} object kind", kind, expected_kind)
        require_equal(f"{expected_kind} object length", len(body), int(length))
        return body

    commit = loose_object(commit_id, "commit")
    first_line = commit.splitlines()[0].decode("ascii")
    require_equal("baseline commit tree", first_line, f"tree {tree_id}")
    loose_object(tree_id, "tree")
    current_commit = loose_object(current_commit_id, "commit")
    current_first_line = current_commit.splitlines()[0].decode("ascii")
    require_equal(
        "current implementation commit tree",
        current_first_line,
        f"tree {current_tree_id}",
    )
    loose_object(current_tree_id, "tree")

    head_commit = loose_object(head_value, "commit")
    head_first_line = head_commit.splitlines()[0].decode("ascii")
    if not head_first_line.startswith("tree "):
        raise SystemExit("implementation workspace HEAD commit lacks a tree")
    loose_object(head_first_line.removeprefix("tree "), "tree")

    pending = [head_value]
    visited: set[str] = set()
    while pending:
        candidate = pending.pop()
        if candidate in visited:
            continue
        visited.add(candidate)
        if candidate == current_commit_id:
            break
        body = loose_object(candidate, "commit")
        pending.extend(
            line.removeprefix(b"parent ").decode("ascii")
            for line in body.splitlines()
            if line.startswith(b"parent ")
        )
    else:
        raise SystemExit("implementation workspace HEAD does not descend from the bound base")

    pending = [current_commit_id]
    visited = set()
    while pending:
        candidate = pending.pop()
        if candidate in visited:
            continue
        visited.add(candidate)
        if candidate == commit_id:
            break
        body = loose_object(candidate, "commit")
        parents = [
            line.removeprefix(b"parent ").decode("ascii")
            for line in body.splitlines()
            if line.startswith(b"parent ")
        ]
        pending.extend(parents)
    else:
        raise SystemExit("current implementation HEAD does not descend from the approved baseline")


def external_build_plan_handoff(
    bindings: dict[str, object], candidate_commitment: str | None = None
) -> str:
    plan = bindings["external_build_plan"]
    approval = bindings["external_approval"]
    assert isinstance(plan, dict)
    assert isinstance(approval, dict)
    records = [
        ("external_candidate_input_commitment", candidate_commitment or str(approval["candidate_input_commitment"])),
        ("recipient_task_id", str(plan["recipient_task_id"])),
        ("stage_plan_sha256", str(plan["stage_plan_sha256"])),
        ("proof_gate_sha256", str(plan["proof_gate_sha256"])),
        ("risk_recovery_sha256", str(plan["risk_recovery_sha256"])),
        ("adapter_removal_sha256", str(plan["adapter_removal_sha256"])),
    ]
    return digest_records(b"maestro.external-build-plan-handoff.v1\0", records)


def verify_external_build_plan(bindings: dict[str, object], source: Path) -> None:
    plan = bindings["external_build_plan"]
    current_plan = bindings.get("current_external_build_plan", plan)
    approval = bindings["external_approval"]
    assert isinstance(plan, dict)
    assert isinstance(current_plan, dict)
    assert isinstance(approval, dict)

    design = source / ".maestro/cards/maestro-whole-flow-architecture-refoundation/design.md"
    lines = design.read_text(encoding="utf-8").splitlines()
    header = "| Stage | Prerequisites | Exact deliverables | Smallest falsifying proof and recovery boundary | Next-stage condition |"
    try:
        start = lines.index(header)
    except ValueError as error:
        raise SystemExit("Stage-0 source design lacks the bound stage table") from error
    rows: list[list[str]] = []
    for line in lines[start + 2 :]:
        if not line:
            break
        if not line.startswith("|"):
            continue
        columns = [column.strip() for column in line[1:-1].split("|")]
        require_equal("stage-table column count", len(columns), 5)
        rows.append(columns)
    require_equal("stage-table row count", len(rows), 13)

    stage_lines = ["schema\tExternalStagePlanSummaryV1"] + [
        "\t".join((stage, prerequisites, deliverables, next_condition))
        for stage, prerequisites, deliverables, _proof, next_condition in rows
    ]
    proof_lines = ["schema\tExternalProofGateSummaryV1"] + [
        "\t".join((stage, proof)) for stage, _prerequisites, _deliverables, proof, _next_condition in rows
    ]
    require_equal(
        "external stage-plan digest",
        hashlib.sha256(("\n".join(stage_lines) + "\n").encode()).hexdigest(),
        current_plan["stage_plan_sha256"],
    )
    require_equal(
        "external proof-gate digest",
        hashlib.sha256(("\n".join(proof_lines) + "\n").encode()).hexdigest(),
        current_plan["proof_gate_sha256"],
    )
    require_equal(
        "current external build-plan classification",
        current_plan.get("classification"),
        "post_approval_stage12_clarification_execution_order_and_full_seal_scheduling_only",
    )
    for label, key in (
        ("risk-recovery", "risk_recovery_canonical_lines"),
        ("adapter-removal", "adapter_removal_canonical_lines"),
    ):
        canonical_lines = plan[key]
        assert isinstance(canonical_lines, list)
        require_equal(
            f"external {label} digest",
            hashlib.sha256(("\n".join(str(line) for line in canonical_lines) + "\n").encode()).hexdigest(),
            plan[f"{label.replace('-', '_')}_sha256"],
        )

    require_equal(
        "external build-plan handoff",
        external_build_plan_handoff(bindings),
        approval["build_plan_handoff"],
    )


def external_packet(
    bindings: dict[str, object],
    *,
    source_control_sha256: str | None = None,
    destination_sha256: str | None = None,
    candidate_commitment: str | None = None,
    build_plan_handoff: str | None = None,
) -> str:
    source_inputs = bindings["canonical_source_inputs"]
    controls = bindings["external_control_bindings"]
    external = bindings["external_candidate_input_fields"]
    approval = bindings["external_approval"]
    sections = bindings["external_packet_sections"]
    baseline = bindings["baseline"]
    assert isinstance(source_inputs, dict)
    assert isinstance(controls, dict)
    assert isinstance(external, dict)
    assert isinstance(approval, dict)
    assert isinstance(sections, dict)
    assert isinstance(baseline, dict)
    records = [
        ("feature_id", str(bindings["feature_id"])),
        ("feature_state", str(bindings["feature_state"])),
        ("source_repository_realpath", str(bindings["source_repository_realpath"])),
        ("implementation_workspace_path", str(bindings["implementation_workspace_path"])),
        ("implementation_workspace_policy", str(bindings["implementation_workspace_policy"])),
        ("baseline_commit", str(baseline["commit"])),
        ("baseline_tree", str(baseline["tree"])),
        ("source_git_control_binding_sha256", source_control_sha256 or str(controls["source_git_control"]["identity_sha256"])),
        ("destination_ancestor_binding_sha256", destination_sha256 or str(controls["destination_ancestors"]["identity_sha256"])),
        ("checkout_sandbox_profile_sha256", str(controls["checkout_sandbox_profile"]["identity_sha256"])),
        ("card_sha256", str(source_inputs["card_sha256"])),
        ("design_sha256", str(source_inputs["design_sha256"])),
        ("decisions_sha256", str(source_inputs["decisions_sha256"])),
        ("raw_decision_inventory_sha256", str(external["raw_decision_inventory_sha256"])),
        ("external_design_authority_closure_sha256", str(external["external_design_authority_closure_sha256"])),
        ("external_candidate_input_commitment", candidate_commitment or str(approval["candidate_input_commitment"])),
        ("external_build_plan_handoff", build_plan_handoff or str(approval["build_plan_handoff"])),
        ("candidate_contract_root", "absent-before-stage-0"),
        ("canonical_build_handoff", "absent-before-stage-0"),
    ]
    section_order = [
        "identity_state", "product_constitution", "architecture_ownership",
        "capability_resource_census", "lifecycle_authority", "migration_rollback_removal",
        "implementation_stages_proofs", "risk_recovery", "advisor_dispositions_edge_sweep",
        "deviations_recommendation",
    ]
    records.extend((f"{name}_sha256", str(sections[name])) for name in section_order)
    return digest_records(b"maestro.external-build-approval-packet.v1\0", records)


def verify_external_packet(bindings: dict[str, object]) -> None:
    approval = bindings["external_approval"]
    assert isinstance(approval, dict)
    require_equal(
        "external approval packet",
        external_packet(bindings),
        approval["packet_sha256"],
    )


def verify_external_approval_event(
    bindings: dict[str, object],
) -> dict[str, bytes] | None:
    if successor_approval(bindings):
        return verify_successor_external_approval_event(bindings)

    event = bindings["external_approval_event"]
    plan = bindings["external_build_plan"]
    approval = bindings["external_approval"]
    assert isinstance(event, dict)
    assert isinstance(plan, dict)
    assert isinstance(approval, dict)
    superseded = event["superseded_packet"]
    assert isinstance(superseded, dict)
    require_equal(
        "approval attestation source",
        event["attestation_source"],
        "pinned_local_codex_session_records_v1",
    )
    require_equal(
        "approval provenance assurance",
        event["provenance_assurance"],
        "unsigned_local_platform_log_bound_by_sha256",
    )
    require_equal("approval actor role", event["actor_role"], "user")
    require_equal("approval recipient", event["recipient_thread_id"], plan["recipient_task_id"])

    log_path = Path(str(event["log_realpath"]))
    if not log_path.is_file() or log_path.is_symlink():
        raise SystemExit(f"approval capture is not a regular no-follow log: {log_path}")
    require_equal("approval capture realpath", str(log_path.resolve(strict=True)), str(log_path))
    expected_records = event["record_sha256"]
    assert isinstance(expected_records, dict)
    required_record_names = {
        "session_meta",
        "superseded_packet_task_complete",
        "superseded_approval_task_started",
        "superseded_approval_user_message",
        "packet_task_complete",
        "approval_task_started",
        "approval_user_message",
    }
    require_equal("approval capture record names", set(expected_records), required_record_names)
    expected_by_digest = {str(digest): name for name, digest in expected_records.items()}
    require_equal(
        "approval capture record digest uniqueness",
        len(expected_by_digest),
        len(expected_records),
    )
    captured: dict[str, dict[str, object]] = {}
    captured_ordinals: dict[str, int] = {}
    captured_timestamps: dict[str, datetime.datetime] = {}
    with log_path.open("rb") as source:
        for ordinal, raw_line in enumerate(source, start=1):
            digest = hashlib.sha256(raw_line).hexdigest()
            name = expected_by_digest.get(digest)
            if name is None:
                continue
            if name in captured:
                raise SystemExit(f"approval capture record occurs more than once: {name}")
            record = json.loads(raw_line)
            if not isinstance(record, dict):
                raise SystemExit(f"approval capture record is not an object: {name}")
            captured[name] = record
            captured_ordinals[name] = ordinal
            captured_timestamps[name] = datetime.datetime.fromisoformat(
                str(record["timestamp"]).replace("Z", "+00:00")
            )
    require_equal("approval capture record closure", set(captured), required_record_names)
    ordered_names = [
        "session_meta",
        "superseded_packet_task_complete",
        "superseded_approval_task_started",
        "superseded_approval_user_message",
        "packet_task_complete",
        "approval_task_started",
        "approval_user_message",
    ]
    if [captured_ordinals[name] for name in ordered_names] != sorted(
        captured_ordinals[name] for name in ordered_names
    ):
        raise SystemExit("approval capture records are not in causal log order")
    if [captured_timestamps[name] for name in ordered_names] != sorted(
        captured_timestamps[name] for name in ordered_names
    ):
        raise SystemExit("approval capture record timestamps are not causal")

    capture_records = [
        ("log_realpath", str(log_path)),
        ("session_id", str(event["recipient_thread_id"])),
        ("session_meta_sha256", str(expected_records["session_meta"])),
        (
            "superseded_packet_task_complete_sha256",
            str(expected_records["superseded_packet_task_complete"]),
        ),
        (
            "superseded_approval_task_started_sha256",
            str(expected_records["superseded_approval_task_started"]),
        ),
        (
            "superseded_approval_user_message_sha256",
            str(expected_records["superseded_approval_user_message"]),
        ),
        (
            "packet_task_complete_sha256",
            str(expected_records["packet_task_complete"]),
        ),
        (
            "approval_task_started_sha256",
            str(expected_records["approval_task_started"]),
        ),
        (
            "approval_user_message_sha256",
            str(expected_records["approval_user_message"]),
        ),
        (
            "superseded_packet_body_sha256",
            str(event["superseded_packet"]["packet_body_sha256"]),
        ),
        ("packet_body_sha256", str(event["packet_body_sha256"])),
    ]
    require_equal(
        "approval capture record-set identity",
        digest_records(b"maestro.external-approval-capture.v1\0", capture_records),
        event["record_set_sha256"],
    )

    session_meta = captured["session_meta"]
    require_equal("approval session record type", session_meta["type"], "session_meta")
    session_payload = session_meta["payload"]
    assert isinstance(session_payload, dict)
    require_equal("approval session id", session_payload["session_id"], event["recipient_thread_id"])
    require_equal("approval session payload id", session_payload["id"], event["recipient_thread_id"])
    require_equal(
        "approval session source repository",
        session_payload["cwd"],
        bindings["source_repository_realpath"],
    )
    created_at = int(
        datetime.datetime.fromisoformat(
            str(session_payload["timestamp"]).replace("Z", "+00:00")
        ).timestamp()
    )
    require_equal("approval recipient creation time", created_at, event["recipient_thread_created_at"])

    controls = bindings["external_control_bindings"]
    baseline = bindings["baseline"]
    assert isinstance(controls, dict)
    assert isinstance(baseline, dict)
    source_control = controls["source_git_control"]
    destination = controls["destination_ancestors"]
    sandbox = controls["checkout_sandbox_profile"]
    assert isinstance(source_control, dict)
    assert isinstance(destination, dict)
    assert isinstance(sandbox, dict)
    require_equal("rebind sandbox identity", superseded["checkout_sandbox_profile_sha256"], sandbox["identity_sha256"])
    old_device = str(event["superseded_device"])
    current_device = str(event["current_device"])
    if old_device == current_device:
        raise SystemExit("reboot rebind did not change the filesystem device")
    old_source_records = [
        ("source_repository_realpath", str(bindings["source_repository_realpath"])),
        ("git_common_dir_realpath", str(source_control["git_common_dir_realpath"])),
        ("git_common_dir_device", old_device),
        ("git_common_dir_inode", str(source_control["git_common_dir_inode"])),
        ("git_common_dir_uid", str(source_control["git_common_dir_uid"])),
        ("git_common_dir_gid", str(source_control["git_common_dir_gid"])),
        ("git_common_dir_mode", str(source_control["git_common_dir_mode"])),
        ("object_directory_realpath", str(source_control["object_directory_realpath"])),
        ("object_directory_device", old_device),
        ("object_directory_inode", str(source_control["object_directory_inode"])),
        ("object_directory_uid", str(source_control["object_directory_uid"])),
        ("object_directory_gid", str(source_control["object_directory_gid"])),
        ("object_directory_mode", str(source_control["object_directory_mode"])),
        ("object_format", str(source_control["object_format"])),
        ("repository_config_sha256", str(source_control["repository_config_sha256"])),
        ("git_control_path_manifest_sha256", str(source_control["git_control_path_manifest_sha256"])),
        ("baseline_commit", str(baseline["commit"])),
        ("baseline_tree", str(baseline["tree"])),
    ]
    old_source_identity = digest_records(
        b"maestro.external-git-source-control.v1\0", old_source_records
    )
    require_equal(
        "superseded source-control identity",
        old_source_identity,
        superseded["source_git_control_binding_sha256"],
    )
    ancestors = destination["ancestors"]
    assert isinstance(ancestors, list)
    old_destination_records = [("ancestor_count", str(len(ancestors)))]
    for index, entry in enumerate(ancestors):
        assert isinstance(entry, dict)
        for key in ("path", "realpath", "device", "inode", "uid", "gid", "mode", "type"):
            value = old_device if key == "device" else str(entry[key])
            old_destination_records.append((f"ancestor_{index}_{key}", value))
    old_destination_identity = digest_records(
        b"maestro.external-destination-ancestors.v1\0", old_destination_records
    )
    require_equal(
        "superseded destination identity",
        old_destination_identity,
        superseded["destination_ancestor_binding_sha256"],
    )
    old_candidate = external_candidate_commitment(
        bindings,
        source_control_sha256=old_source_identity,
        destination_sha256=old_destination_identity,
    )
    require_equal(
        "superseded candidate input commitment",
        old_candidate,
        superseded["candidate_input_commitment"],
    )
    old_handoff = external_build_plan_handoff(bindings, old_candidate)
    require_equal(
        "superseded build-plan handoff",
        old_handoff,
        superseded["build_plan_handoff"],
    )
    require_equal(
        "superseded approval packet",
        external_packet(
            bindings,
            source_control_sha256=old_source_identity,
            destination_sha256=old_destination_identity,
            candidate_commitment=old_candidate,
            build_plan_handoff=old_handoff,
        ),
        superseded["packet_sha256"],
    )

    old_packet_record = captured["superseded_packet_task_complete"]
    require_equal("superseded packet record type", old_packet_record["type"], "event_msg")
    old_packet_payload = old_packet_record["payload"]
    assert isinstance(old_packet_payload, dict)
    require_equal("superseded packet event type", old_packet_payload["type"], "task_complete")
    require_equal("superseded packet turn", old_packet_payload["turn_id"], superseded["packet_turn_id"])
    require_equal("superseded packet completion", old_packet_payload["completed_at"], superseded["packet_turn_completed_at"])
    old_packet_body = str(old_packet_payload["last_agent_message"])
    require_equal(
        "superseded packet body identity",
        hashlib.sha256(old_packet_body.encode()).hexdigest(),
        superseded["packet_body_sha256"],
    )
    if f"External packet SHA-256:\n\n`{superseded['packet_sha256']}`" not in old_packet_body:
        raise SystemExit("superseded packet body does not bind its packet digest")

    old_started = captured["superseded_approval_task_started"]
    require_equal("superseded approval-start record type", old_started["type"], "event_msg")
    old_started_payload = old_started["payload"]
    assert isinstance(old_started_payload, dict)
    require_equal("superseded approval-start event", old_started_payload["type"], "task_started")
    require_equal("superseded approval-start turn", old_started_payload["turn_id"], superseded["approval_turn_id"])
    require_equal("superseded approval-start time", old_started_payload["started_at"], superseded["approval_turn_started_at"])

    old_message = captured["superseded_approval_user_message"]
    require_equal("superseded approval-message record type", old_message["type"], "response_item")
    old_message_payload = old_message["payload"]
    assert isinstance(old_message_payload, dict)
    require_equal("superseded approval-message type", old_message_payload["type"], "message")
    require_equal("superseded approval-message actor", old_message_payload["role"], "user")
    require_equal("superseded approval-message id", old_message_payload.get("id"), superseded["user_message_id"])
    old_metadata = old_message_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(old_metadata, dict)
    require_equal("superseded approval-message turn", old_metadata["turn_id"], superseded["approval_turn_id"])
    old_content = old_message_payload["content"]
    if not isinstance(old_content, list) or len(old_content) != 1 or not isinstance(old_content[0], dict):
        raise SystemExit("superseded approval-message content is not one text item")
    require_equal("superseded approval-message content type", old_content[0]["type"], "input_text")
    old_expected_instruction = (
        f"APPROVE BUILD PACKET sha256:{superseded['packet_sha256']}. "
        "Execute the staged build plan."
    )
    require_equal(
        "superseded declared approval instruction",
        superseded["exact_instruction"],
        old_expected_instruction,
    )
    require_equal(
        "superseded external approval instruction",
        old_content[0]["text"],
        old_expected_instruction + "\n",
    )
    if int(superseded["packet_turn_completed_at"]) >= int(superseded["approval_turn_started_at"]):
        raise SystemExit("superseded approval is not causally later than its packet")

    packet_record = captured["packet_task_complete"]
    require_equal("packet capture record type", packet_record["type"], "event_msg")
    packet_payload = packet_record["payload"]
    assert isinstance(packet_payload, dict)
    require_equal("packet capture event type", packet_payload["type"], "task_complete")
    require_equal("packet capture turn", packet_payload["turn_id"], event["packet_turn_id"])
    require_equal(
        "packet capture completion time",
        packet_payload["completed_at"],
        event["packet_turn_completed_at"],
    )
    packet_body = str(packet_payload["last_agent_message"])
    require_equal(
        "published packet body identity",
        hashlib.sha256(packet_body.encode()).hexdigest(),
        event["packet_body_sha256"],
    )
    publication_kind = event.get("packet_publication_kind", "full_build_packet_v1")
    if publication_kind == "full_build_packet_v1":
        if f"External packet SHA-256:\n\n`{approval['packet_sha256']}`" not in packet_body:
            raise SystemExit("published packet body does not bind the approved packet digest")
    elif publication_kind == "reboot_device_rebind_v1":
        source_control = bindings["external_control_bindings"]["source_git_control"]
        assert isinstance(source_control, dict)
        require_equal(
            "rebound packet current device",
            event["current_device"],
            source_control["git_common_dir_device"],
        )
        token = f"APPROVE BUILD PACKET sha256:{approval['packet_sha256']}. Execute the staged build plan."
        required_fragments = (
            event["superseded_device"],
            event["current_device"],
            "with no design or scope changes",
            token,
        )
        if not all(str(fragment) in packet_body for fragment in required_fragments):
            raise SystemExit("published reboot rebind packet does not bind its exact safety delta")
    else:
        raise SystemExit(f"unknown packet publication kind: {publication_kind}")

    started_record = captured["approval_task_started"]
    require_equal("approval-start record type", started_record["type"], "event_msg")
    started_payload = started_record["payload"]
    assert isinstance(started_payload, dict)
    require_equal("approval-start event type", started_payload["type"], "task_started")
    require_equal("approval-start turn", started_payload["turn_id"], event["approval_turn_id"])
    require_equal(
        "approval-start time",
        started_payload["started_at"],
        event["approval_turn_started_at"],
    )

    message_record = captured["approval_user_message"]
    require_equal("approval-message record type", message_record["type"], "response_item")
    message_payload = message_record["payload"]
    assert isinstance(message_payload, dict)
    require_equal("approval-message payload type", message_payload["type"], "message")
    require_equal("approval-message actor", message_payload["role"], event["actor_role"])
    require_equal("approval-message id", message_payload.get("id"), event.get("user_message_id"))
    metadata = message_payload["internal_chat_message_metadata_passthrough"]
    assert isinstance(metadata, dict)
    require_equal("approval-message turn", metadata["turn_id"], event["approval_turn_id"])
    content = message_payload["content"]
    if not isinstance(content, list) or len(content) != 1 or not isinstance(content[0], dict):
        raise SystemExit("approval-message content is not the exact single text item")
    require_equal("approval-message content type", content[0]["type"], "input_text")
    if int(event["packet_turn_completed_at"]) >= int(event["approval_turn_started_at"]):
        raise SystemExit("approval event is not causally later than packet publication")
    if int(event["recipient_thread_created_at"]) >= int(event["packet_turn_completed_at"]):
        raise SystemExit("recipient task incarnation does not predate the packet")
    if event["packet_turn_id"] == event["approval_turn_id"]:
        raise SystemExit("approval event reused the packet turn")
    expected = f"APPROVE BUILD PACKET sha256:{approval['packet_sha256']}. Execute the staged build plan."
    require_equal("exact external approval instruction", event["exact_instruction"], expected)
    completion = event.get("completion_instruction")
    expected_content = expected + "\n"
    if completion is not None:
        require_equal("completion instruction", completion, "Approve go until done")
        expected_content += completion
    require_equal("captured external approval instruction", content[0]["text"], expected_content)


def verify_source_inputs(bindings: dict[str, object], source: Path) -> None:
    canonical_inputs = bindings["canonical_source_inputs"]
    current_inputs = bindings["current_source_inputs"]
    provenance = bindings["provenance_evidence_inputs"]
    assert isinstance(canonical_inputs, dict)
    assert isinstance(current_inputs, dict)
    assert isinstance(provenance, dict)
    require_equal(
        "canonical source input keys",
        set(canonical_inputs),
        {"card_sha256", "design_sha256", "decisions_sha256"},
    )
    require_equal(
        "provenance evidence classification",
        provenance.get("classification"),
        "non_authoritative_coverage_evidence_only",
    )
    require_equal(
        "provenance evidence packet binding",
        provenance.get("packet_binding"),
        "excluded_by_design_canonical_card_design_decisions_are_authority",
    )
    evidence_inputs = provenance.get("artifacts")
    assert isinstance(evidence_inputs, dict)
    require_equal(
        "provenance evidence input keys",
        set(evidence_inputs),
        {"final_main_sha256", "post_refoundation_brainstorm_sha256"},
    )
    canonical_paths = {
        "card_sha256": source
        / ".maestro/cards/maestro-whole-flow-architecture-refoundation/card.yaml",
        "design_sha256": source
        / ".maestro/cards/maestro-whole-flow-architecture-refoundation/design.md",
        "decisions_sha256": source
        / ".maestro/cards/maestro-whole-flow-architecture-refoundation/decisions.yaml",
    }
    evidence_paths = {
        "final_main_sha256": source / "FINAL-MAIN.md",
        "post_refoundation_brainstorm_sha256": source
        / "SPEC-MAESTRO-VNEXT-POST-REFOUNDATION-BRAINSTORM.md",
    }
    for key, path in canonical_paths.items():
        if not path.is_file():
            raise SystemExit(f"{key}: missing regular file {path}")
        require_equal(key, sha256(path), current_inputs[key])
    for key, path in evidence_paths.items():
        if not path.is_file():
            raise SystemExit(f"{key}: missing regular file {path}")
        require_equal(key, sha256(path), evidence_inputs[key])


def verify_decision_inventory(bindings: dict[str, object], source: Path) -> None:
    decisions = source / (
        ".maestro/cards/maestro-whole-flow-architecture-refoundation/decisions.yaml"
    )
    text = decisions.read_text(encoding="utf-8")
    statuses = re.findall(r"^  status: (locked|superseded|open)$", text, re.MULTILINE)
    expected = bindings["decision_inventory"]
    assert isinstance(expected, dict)
    require_equal("decision total", len(statuses), expected["total"])
    for status in ("locked", "superseded", "open"):
        require_equal(
            f"decision {status}", statuses.count(status), expected[status]
        )

    head = str(expected["effective_composite_head"])
    if not re.search(rf"^  id: {re.escape(head)}$", text, re.MULTILINE):
        raise SystemExit(f"effective composite head is absent: {head}")


def verify_successor_source_closure(
    bindings: dict[str, object], packet_files: dict[str, bytes]
) -> None:
    packet_name = "replacement-build-approval-packet.v1.json"
    manifest_name = "successor-decision-store-manifest.v1.txt"
    if packet_name not in packet_files or manifest_name not in packet_files:
        raise SystemExit("successor packet source closure is incomplete")
    packet = json.loads(packet_files[packet_name])
    candidate_records = packet["candidate_input"]["records"]
    assert isinstance(candidate_records, dict)
    canonical = bindings["canonical_source_inputs"]
    current = bindings["current_source_inputs"]
    inventory = bindings["decision_inventory"]
    baseline = bindings["baseline"]
    assert isinstance(canonical, dict)
    assert isinstance(current, dict)
    assert isinstance(inventory, dict)
    assert isinstance(baseline, dict)
    expected_source = {
        "card_sha256": "2cdf1f74843a6eca926ff3bc48e060654350e6a03b65342f8d7be48d111379b4",
        "design_sha256": "9d5bda2be6274351ff7afba7f396595d80f9d560622991de1c8214aae0b8fc1b",
        "decisions_sha256": "18f14bce862e15be09c9d88155d62627582df50c7754e2e8e1d6f6bee8f7d522",
    }
    require_equal("successor canonical source inputs", canonical, expected_source)
    require_equal("successor current source inputs", current, expected_source)
    for key, value in expected_source.items():
        require_equal(f"successor packet {key}", candidate_records[key], value)
    require_equal(
        "successor baseline",
        baseline,
        {
            "commit": "6182853d1cfaca16159af428503802707607068e",
            "tree": "ee71253923248688ad8f0668a2cf9ecdec2ea5cb",
        },
    )
    require_equal(
        "successor Decision inventory",
        {key: inventory[key] for key in ("total", "locked", "superseded", "open")},
        {"total": 213, "locked": 117, "superseded": 96, "open": 0},
    )
    rows = [
        line.split("\t")
        for line in packet_files[manifest_name].decode("utf-8").splitlines()
    ]
    if len(rows) != 213 or any(len(row) != 4 for row in rows):
        raise SystemExit("successor Decision-store manifest is incomplete")
    if len({row[0] for row in rows}) != 213:
        raise SystemExit("successor Decision-store manifest has duplicate ids")
    counts = {
        status: sum(row[1] == status for row in rows)
        for status in ("locked", "superseded", "open")
    }
    require_equal(
        "successor Decision-store terminal counts",
        counts,
        {"locked": 117, "superseded": 96, "open": 0},
    )


def verify_nonpromotion(bindings: dict[str, object]) -> None:
    require_equal(
        "canonical role",
        bindings["canonical_role"],
        "external_non_promoting_provenance",
    )
    approval = bindings["external_approval"]
    assert isinstance(approval, dict)
    required_refusals = {
        "candidate_contract_root",
        "canonical_build_handoff",
        "manifest_identity",
        "mandate",
        "action_request",
        "receipt",
    }
    require_equal(
        "external approval nonpromotion set",
        set(approval["must_not_be_used_as"]),
        required_refusals,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bindings", type=Path, default=DEFAULT_BINDINGS)
    parser.add_argument("--source", type=Path)
    args = parser.parse_args()

    bindings_path = args.bindings.resolve(strict=True)
    bindings = json.loads(bindings_path.read_text(encoding="utf-8"))
    require_equal(
        "binding schema",
        bindings["schema"],
        "maestro.vnext.stage0-input-bindings.v1",
    )
    source = (args.source or Path(str(bindings["source_repository_realpath"]))).resolve(
        strict=True
    )
    require_equal(
        "source repository realpath",
        str(source),
        bindings["source_repository_realpath"],
    )
    verify_nonpromotion(bindings)
    verify_external_control_bindings(bindings, source)
    verify_external_candidate_commitment(bindings)
    verify_baseline_objects(bindings, source)
    verify_external_packet(bindings)
    successor_packet_files = verify_external_approval_event(bindings)
    if successor_approval(bindings):
        if successor_packet_files is None:
            raise SystemExit("successor packet capture is unavailable")
        verify_successor_source_closure(bindings, successor_packet_files)
    else:
        verify_current_control_rebind(bindings)
        verify_external_build_plan(bindings, source)
        verify_post_approval_execution_plan_revisions(bindings)
        verify_source_inputs(bindings, source)
        verify_decision_inventory(bindings, source)
    print(
        json.dumps(
            {
                "schema": "maestro.vnext.stage0-input-verification.v1",
                "bindings_sha256": sha256(bindings_path),
                "feature_id": bindings["feature_id"],
                "decision_total": bindings["decision_inventory"]["total"],
                "result": "verified_non_promoting",
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )


if __name__ == "__main__":
    main()
