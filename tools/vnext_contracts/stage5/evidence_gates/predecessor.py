#!/usr/bin/env python3
"""Bind immutable Stage 4 history without reexecuting its completed seal."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import sys
import tarfile
from pathlib import Path, PurePosixPath
from typing import Any


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[3]
sys.dont_write_bytecode = True
PATHS = (
    "contracts/vnext/stage4/execution/execution-effects.v1.json",
    "contracts/vnext/stage4/execution/execution-effects.v1.cbor",
    "contracts/vnext/stage4/execution/behavioral-proof-receipt.v1.json",
    "contracts/vnext/stage4/execution/python-encoder-receipt.v1.json",
    "contracts/vnext/stage4/execution/semantic-validation-receipt.v1.json",
    "contracts/vnext/stage4/execution/ruby-verification-receipt.v1.json",
)
EXPECTED_FILE_SHA256 = {
    "contracts/vnext/stage4/execution/execution-effects.v1.json": "18b215280ea9aeab3a7bb6edf15214950d35343e6d15be89fef54031c9a51e3b",
    "contracts/vnext/stage4/execution/execution-effects.v1.cbor": "462d821152e1f621073276d8403ad0ea89d9ec66227cd8b3067cf956bdfaa077",
    "contracts/vnext/stage4/execution/behavioral-proof-receipt.v1.json": "ead17b652be513d2bbb6cf8460676c38609ffaec9bee9ac1818d83be454cb3ac",
    "contracts/vnext/stage4/execution/python-encoder-receipt.v1.json": "c806b4fe97ecb9374adf1ae7401fb86081230644a444ca4a77ff37c881e04f51",
    "contracts/vnext/stage4/execution/semantic-validation-receipt.v1.json": "5fd6437350350691ee7b623fb3a0b8750b43b16fd3a7719cd9d7e8713d3756c4",
    "contracts/vnext/stage4/execution/ruby-verification-receipt.v1.json": "e9a9e882decfc91a23ae5d2a47fef5b976b42583ae1b2b565ce7e2f2fab9103b",
}
STAGE4_SOURCE_COMMIT = "9f3cc73b2199c5b2be78dcea8852cbdcafaaafc2"
STAGE4_SOURCE_TREE = "2f832a04c7109e17b4b298e40b4827c1ced2d527"
STAGE4_SOURCE_ARCHIVE_LENGTH = 16_486_231
STAGE4_SOURCE_ARCHIVE_SHA256 = (
    "347eaf928f81d9ce6e07e3767f0cdaf2cde23cd98d13bad41b745d5fbc359910"
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def pretty_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode("ascii")


def read_archive(archive: Path) -> tuple[bytes, dict[str, bytes]]:
    if archive.is_symlink() or not archive.is_file():
        raise RuntimeError("exact Stage 4 source archive is absent or unsafe")
    archive_bytes = archive.read_bytes()
    if (
        len(archive_bytes) != STAGE4_SOURCE_ARCHIVE_LENGTH
        or sha256(archive_bytes) != STAGE4_SOURCE_ARCHIVE_SHA256
    ):
        raise RuntimeError("exact Stage 4 source archive differs")
    captured: dict[str, bytes] = {}
    seen: set[str] = set()
    with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as source:
        members = source.getmembers()
        if not members or len(members) > 100_000:
            raise RuntimeError("Stage 4 source archive member census is invalid")
        for member in members:
            relative = PurePosixPath(member.name)
            if (
                relative.is_absolute()
                or not relative.parts
                or ".." in relative.parts
                or member.name in seen
                or not (member.isdir() or member.isfile())
            ):
                raise RuntimeError("Stage 4 source archive contains an unsafe member")
            seen.add(member.name)
            if member.name not in PATHS:
                continue
            stream = source.extractfile(member)
            if stream is None:
                raise RuntimeError("Stage 4 source archive file is unreadable")
            data = stream.read()
            if len(data) != member.size:
                raise RuntimeError("Stage 4 source archive file length differs")
            captured[member.name] = data
    if set(captured) != set(PATHS):
        raise RuntimeError("Stage 4 source archive omits a canonical proof file")
    return archive_bytes, captured


def load_json(data: bytes, label: str) -> dict[str, Any]:
    value = json.loads(data)
    if not isinstance(value, dict):
        raise RuntimeError(f"{label} is not an object")
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--stage4-source", type=Path, required=True)
    parser.add_argument("--stage4-root", type=Path)
    args = parser.parse_args()

    archive_bytes, archived = read_archive(args.stage4_source)
    stage4_root = args.stage4_root
    if stage4_root is not None and (
        stage4_root.is_symlink() or not stage4_root.is_dir()
    ):
        raise RuntimeError("exact Stage 4 proof root is absent or unsafe")
    historical_files: list[list[object]] = []
    current_dependency_files: list[list[object]] = []
    for relative in PATHS:
        historical = archived[relative]
        historical_digest = sha256(historical)
        if EXPECTED_FILE_SHA256[relative] != historical_digest:
            raise RuntimeError(f"exact Stage 4 predecessor file differs: {relative}")
        historical_files.append([relative, len(historical), historical_digest])
        if stage4_root is None:
            current_dependency_files.append(
                [relative, len(historical), historical_digest]
            )
            continue
        workspace_path = stage4_root / Path(relative).name
        if workspace_path.is_symlink() or not workspace_path.is_file():
            raise RuntimeError(
                f"current Stage 4 dependency file is absent or unsafe: {relative}"
            )
        current = workspace_path.read_bytes()
        current_dependency_files.append([relative, len(current), sha256(current)])

    artifact = load_json(archived[PATHS[0]], "Stage 4 artifact")
    receipts = [load_json(archived[path], path) for path in PATHS[2:]]
    expected_identity = "sha256:462d821152e1f621073276d8403ad0ea89d9ec66227cd8b3067cf956bdfaa077"
    if (
        artifact.get("identity") != expected_identity
        or artifact.get("publication_state") != "inactive_candidate"
        or sha256(archived[PATHS[1]]) != expected_identity.removeprefix("sha256:")
        or any(receipt.get("identity") != expected_identity for receipt in receipts)
    ):
        raise RuntimeError("Stage 4 predecessor identity or publication state differs")
    behavioral = receipts[0]
    if (
        behavioral.get("result") != "pass"
        or behavioral.get("validation_mode") != "full_chain"
        or not behavioral.get("command_receipts")
        or not behavioral.get("mutant_command_receipts")
    ):
        raise RuntimeError("Stage 4 historical behavioral receipt is incomplete")
    ancestry_receipts = receipts[1:]
    chains = [receipt.get("predecessor_chain") for receipt in ancestry_receipts]
    if not chains or chains[1:] != chains[:-1]:
        raise RuntimeError("Stage 4 historical receipts disagree on ancestry")

    historical_validation = {
        "archive_matches_source_commit": True,
        "current_dependency_rows_bound_separately": True,
        "mode": "read_only_commit_tree_content_and_receipt_equality",
        "receipt_count": len(receipts),
        "receipts_report_pass": True,
        "source_commit": STAGE4_SOURCE_COMMIT,
        "source_tree": STAGE4_SOURCE_TREE,
    }
    closure = {
        "current_dependency_differs_from_history": (
            current_dependency_files != historical_files
        ),
        "current_dependency_files": current_dependency_files,
        "files": historical_files,
        "historical_receipt_validation": historical_validation,
        "identity": expected_identity,
        "predecessor_chain": chains[0],
        "schema_version": "maestro.vnext.stage5.predecessor-closure.v1",
        "source_archive_byte_length": len(archive_bytes),
        "source_archive_sha256": STAGE4_SOURCE_ARCHIVE_SHA256,
        "source_commit": STAGE4_SOURCE_COMMIT,
        "source_tree": STAGE4_SOURCE_TREE,
    }
    args.output_root.mkdir(parents=True, exist_ok=True)
    (args.output_root / "predecessor-closure.v1.json").write_bytes(pretty_json(closure))
    (args.output_root / "stage4-source.tar.gz").write_bytes(archive_bytes)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
