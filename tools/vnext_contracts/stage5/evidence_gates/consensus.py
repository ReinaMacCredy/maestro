#!/usr/bin/env python3
"""Require exact Stage 5 predecessor and three-engine agreement before publication."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
from pathlib import Path
from typing import Any


EXPECTED_STAGE4_IDENTITY = "sha256:462d821152e1f621073276d8403ad0ea89d9ec66227cd8b3067cf956bdfaa077"
EXPECTED_STAGE4_SOURCE_COMMIT = "9f3cc73b2199c5b2be78dcea8852cbdcafaaafc2"
EXPECTED_STAGE4_SOURCE_TREE = "2f832a04c7109e17b4b298e40b4827c1ced2d527"
EXPECTED_STAGE4_SOURCE_ARCHIVE_LENGTH = 16_486_231
EXPECTED_STAGE4_SOURCE_ARCHIVE_SHA256 = (
    "347eaf928f81d9ce6e07e3767f0cdaf2cde23cd98d13bad41b745d5fbc359910"
)
EXPECTED_BEHAVIOR_TESTS = 55
EXPECTED_PROOF_HARNESS_TESTS = 63
EXPECTED_BEHAVIOR_MANIFEST_IDENTITY = (
    "sha256:a45a1774976a2ad7d3e9cf9702ea78bb5bbae33a9deca7a06d5127c451477f12"
)
EXPECTED_OBSERVATION_CONTRACT_TABLE_IDENTITY = (
    "sha256:a5f0e9137c091972802cb7084d86070a930091f0570cefcc7df445074478a676"
)
EXPECTED_PROOF_HARNESS_MANIFEST_IDENTITY = (
    "sha256:0264ac4154824568e121ceb41ea6d9b7b6e23b69ae576251d382a9d751b5117c"
)
ENGINE_RECEIPT_CONTRACTS = {
    "builder": (
        "maestro.vnext.stage5.python-builder-receipt.v1",
        "builder_sha256",
        "tools/vnext_contracts/stage5/evidence_gates/build.py",
    ),
    "validator": (
        "maestro.vnext.stage5.semantic-validation-receipt.v1",
        "validator_sha256",
        "tools/vnext_contracts/stage5/evidence_gates/validate.py",
    ),
    "ruby": (
        "maestro.vnext.stage5.ruby-verification-receipt.v1",
        "verifier_sha256",
        "tools/vnext_contracts/stage5/evidence_gates/verify.rb",
    ),
}
WORKSPACE = Path(__file__).resolve().parents[4]
PREDECESSOR_PATHS = (
    "contracts/vnext/stage4/execution/execution-effects.v1.json",
    "contracts/vnext/stage4/execution/execution-effects.v1.cbor",
    "contracts/vnext/stage4/execution/behavioral-proof-receipt.v1.json",
    "contracts/vnext/stage4/execution/python-encoder-receipt.v1.json",
    "contracts/vnext/stage4/execution/semantic-validation-receipt.v1.json",
    "contracts/vnext/stage4/execution/ruby-verification-receipt.v1.json",
)
EXPECTED_PREDECESSOR_SHA256 = {
    "contracts/vnext/stage4/execution/execution-effects.v1.json": "18b215280ea9aeab3a7bb6edf15214950d35343e6d15be89fef54031c9a51e3b",
    "contracts/vnext/stage4/execution/execution-effects.v1.cbor": "462d821152e1f621073276d8403ad0ea89d9ec66227cd8b3067cf956bdfaa077",
    "contracts/vnext/stage4/execution/behavioral-proof-receipt.v1.json": "ead17b652be513d2bbb6cf8460676c38609ffaec9bee9ac1818d83be454cb3ac",
    "contracts/vnext/stage4/execution/python-encoder-receipt.v1.json": "c806b4fe97ecb9374adf1ae7401fb86081230644a444ca4a77ff37c881e04f51",
    "contracts/vnext/stage4/execution/semantic-validation-receipt.v1.json": "5fd6437350350691ee7b623fb3a0b8750b43b16fd3a7719cd9d7e8713d3756c4",
    "contracts/vnext/stage4/execution/ruby-verification-receipt.v1.json": "e9a9e882decfc91a23ae5d2a47fef5b976b42583ae1b2b565ce7e2f2fab9103b",
}
SNAPSHOT_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "src",
    "embedded",
    "tests",
    "tools/vnext_contracts",
    "contracts/vnext/catalogs",
    "contracts/vnext/stage0",
    "contracts/vnext/stage2",
    "contracts/vnext/stage3",
    "contracts/vnext/stage4/execution",
    "predecessors/stage4-source.tar.gz",
)


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")


def pretty_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode("ascii")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_json(path: Path) -> tuple[dict[str, Any], bytes]:
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"consensus input is absent or unsafe: {path}")
    data = path.read_bytes()
    value = json.loads(data)
    if not isinstance(value, dict):
        raise RuntimeError(f"consensus input is not an object: {path}")
    return value, data


def read_regular(path: Path) -> tuple[bytes, bool]:
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode):
        raise RuntimeError(f"consensus input closure contains an unsafe file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        opened = os.fstat(descriptor)
        chunks: list[bytes] = []
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.append(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    binding = (opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns, opened.st_ctime_ns)
    if binding != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
        raise RuntimeError(f"consensus input closure changed while read: {path}")
    data = b"".join(chunks)
    if len(data) != opened.st_size:
        raise RuntimeError(f"consensus input closure length changed while read: {path}")
    return data, bool(opened.st_mode & 0o111)


def source_rows(root: Path) -> list[list[object]]:
    rows: list[list[object]] = []
    for relative in SNAPSHOT_PATHS:
        path = root / relative
        if path.is_symlink() or not path.exists():
            raise RuntimeError(f"snapshot source is absent or unsafe: {path}")
        children = [path] if path.is_file() else sorted(path.rglob("*"))
        for child in children:
            if child.is_symlink():
                raise RuntimeError(f"snapshot source contains a symlink: {child}")
            if child.is_dir() or "__pycache__" in child.parts or child.suffix == ".pyc":
                continue
            data, executable = read_regular(child)
            rows.append(
                [child.relative_to(root).as_posix(), len(data), sha256(data), executable]
            )
    rows.sort(key=lambda row: str(row[0]))
    return rows


def snapshot_rows(root: Path) -> list[list[object]]:
    rows: list[list[object]] = []
    for child in sorted(root.rglob("*")):
        if child.is_symlink():
            raise RuntimeError(f"immutable snapshot contains a symlink: {child}")
        if child.is_dir() or child.name == "snapshot-manifest.v1.json":
            continue
        data, executable = read_regular(child)
        rows.append(
            [child.relative_to(root).as_posix(), len(data), sha256(data), executable]
        )
    return rows


def validate_predecessor(predecessor: dict[str, Any], source_archive: bytes) -> bool:
    rows = []
    for relative in PREDECESSOR_PATHS:
        data, _ = read_regular(WORKSPACE / relative)
        digest = sha256(data)
        if EXPECTED_PREDECESSOR_SHA256.get(relative) != digest:
            return False
        rows.append([relative, len(data), digest])
    historical = predecessor.get("historical_receipt_validation")
    return (
        predecessor.get("files") == rows
        and predecessor.get("identity") == EXPECTED_STAGE4_IDENTITY
        and predecessor.get("source_commit") == EXPECTED_STAGE4_SOURCE_COMMIT
        and predecessor.get("source_tree") == EXPECTED_STAGE4_SOURCE_TREE
        and predecessor.get("source_archive_byte_length")
        == EXPECTED_STAGE4_SOURCE_ARCHIVE_LENGTH
        and predecessor.get("source_archive_sha256")
        == EXPECTED_STAGE4_SOURCE_ARCHIVE_SHA256
        and len(source_archive) == EXPECTED_STAGE4_SOURCE_ARCHIVE_LENGTH
        and sha256(source_archive) == EXPECTED_STAGE4_SOURCE_ARCHIVE_SHA256
        and sha256((WORKSPACE / PREDECESSOR_PATHS[1]).read_bytes())
        == EXPECTED_STAGE4_IDENTITY.removeprefix("sha256:")
        and historical
        == {
            "archive_matches_source_commit": True,
            "canonical_files_match_archive": True,
            "mode": "read_only_commit_tree_content_and_receipt_equality",
            "receipt_count": 4,
            "receipts_report_pass": True,
            "source_commit": EXPECTED_STAGE4_SOURCE_COMMIT,
            "source_tree": EXPECTED_STAGE4_SOURCE_TREE,
        }
    )


def validate_snapshot_manifest(
    manifest: dict[str, Any], manifest_bytes: bytes
) -> bool:
    return (
        set(manifest) == {"schema_version", "snapshot_identity", "source_identity", "source_rows"}
        and manifest.get("schema_version")
        == "maestro.vnext.stage5.immutable-workspace-snapshot.v1"
        and manifest_bytes == canonical_json(manifest)
        and manifest.get("source_rows") == source_rows(WORKSPACE)
        and manifest.get("source_identity")
        == f"sha256:{sha256(canonical_json(manifest.get('source_rows')))}"
        and manifest.get("snapshot_identity")
        == f"sha256:{sha256(canonical_json(snapshot_rows(WORKSPACE)))}"
    )


def validate_toolchain(toolchain: dict[str, Any], toolchain_path: Path, target: str) -> bool:
    rows = toolchain.get("files")
    if not isinstance(rows, list) or not rows:
        return False
    root = toolchain_path.parent.resolve(strict=True)
    toolchain_root = root / "toolchain"
    if toolchain_root.is_symlink() or not toolchain_root.is_dir():
        return False
    actual = []
    for path in sorted(toolchain_root.rglob("*")):
        if path.is_symlink():
            return False
        if path.is_dir():
            continue
        data, executable = read_regular(path)
        actual.append(
            [path.relative_to(root).as_posix(), len(data), sha256(data), executable]
        )
    for row in rows:
        if (
            not isinstance(row, list)
            or len(row) != 4
            or not isinstance(row[0], str)
            or not isinstance(row[1], int)
            or not isinstance(row[2], str)
            or not isinstance(row[3], bool)
        ):
            return False
        relative = Path(row[0])
        if (
            relative.is_absolute()
            or ".." in relative.parts
            or relative.parts[:1] != ("toolchain",)
        ):
            return False
    return (
        rows == sorted(actual, key=lambda row: str(row[0]))
        and len({str(row[0]) for row in rows}) == len(rows)
        and toolchain.get("schema_version")
        == "maestro.vnext.stage5.rust-toolchain-closure.v1"
        and toolchain.get("target") == target
        and toolchain.get("identity") == f"sha256:{sha256(canonical_json(rows))}"
    )


def validate_receipt_identity(receipt: dict[str, Any]) -> bool:
    value = {key: item for key, item in receipt.items() if key != "receipt_identity"}
    return receipt.get("receipt_identity") == f"sha256:{sha256(canonical_json(value))}"


def validate_engine_receipt(
    name: str, receipt: dict[str, Any], artifact: dict[str, Any]
) -> bool:
    contract = ENGINE_RECEIPT_CONTRACTS.get(name)
    sources = artifact.get("source_closure")
    if contract is None or not isinstance(sources, list):
        return False
    schema_version, engine_hash_key, engine_path = contract
    expected_keys = {
        "artifact_id",
        "artifact_sha256",
        "behavior_manifest_identity",
        "behavior_passed",
        "behavior_runs",
        engine_hash_key,
        "publication_state",
        "receipt_identity",
        "schema_version",
        "source_closure_sha256",
    }
    source_hashes = {
        row[0]: row[2]
        for row in sources
        if isinstance(row, list)
        and len(row) == 3
        and isinstance(row[0], str)
        and isinstance(row[2], str)
    }
    return (
        set(receipt) == expected_keys
        and receipt.get("schema_version") == schema_version
        and receipt.get(engine_hash_key) == source_hashes.get(engine_path)
        and receipt.get("source_closure_sha256")
        == sha256(canonical_json(sources))
        and validate_receipt_identity(receipt)
    )


def behavior_manifest_rows(runs: object) -> list[list[str]]:
    if not isinstance(runs, list) or len(runs) < 2:
        raise RuntimeError("Stage 5 behavior runs are malformed")
    rows: list[list[str]] = []
    for run in runs[:-1]:
        if not isinstance(run, dict) or not isinstance(run.get("tests"), list):
            raise RuntimeError("Stage 5 behavior run is malformed")
        for test in run["tests"]:
            if not isinstance(test, dict):
                raise RuntimeError("Stage 5 behavior test receipt is malformed")
            command = test.get("command")
            name = test.get("name")
            if (
                not isinstance(command, list)
                or len(command) != 4
                or not isinstance(command[0], str)
                or not isinstance(name, str)
                or command != [command[0], name, "--exact", "--nocapture"]
                or test.get("result") != "pass"
            ):
                raise RuntimeError("Stage 5 behavior test receipt is not exact")
            rows.append([command[0], name])
    if len(rows) != EXPECTED_BEHAVIOR_TESTS or len({tuple(row) for row in rows}) != len(rows):
        raise RuntimeError("Stage 5 behavior manifest count or uniqueness differs")
    return rows


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--builder", type=Path, required=True)
    parser.add_argument("--validator", type=Path, required=True)
    parser.add_argument("--ruby", type=Path, required=True)
    parser.add_argument("--predecessor", type=Path, required=True)
    parser.add_argument("--predecessor-source", type=Path, required=True)
    parser.add_argument("--harness", type=Path, required=True)
    parser.add_argument("--snapshot-manifest", type=Path, required=True)
    parser.add_argument("--toolchain", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    args = parser.parse_args()

    artifact, artifact_bytes = read_json(args.artifact)
    predecessor, predecessor_bytes = read_json(args.predecessor)
    predecessor_source_bytes, _ = read_regular(args.predecessor_source)
    harness, harness_bytes = read_json(args.harness)
    snapshot_manifest, snapshot_manifest_bytes = read_json(args.snapshot_manifest)
    toolchain, toolchain_bytes = read_json(args.toolchain)
    named_receipts = {
        "builder": read_json(args.builder),
        "validator": read_json(args.validator),
        "ruby": read_json(args.ruby),
    }
    artifact_id = artifact.get("artifact_id")
    artifact_sha256 = sha256(artifact_bytes)
    if (
        not isinstance(artifact_id, str)
        or artifact.get("publication_state") != "inactive_candidate"
        or artifact.get("observation_contract_table_identity")
        != EXPECTED_OBSERVATION_CONTRACT_TABLE_IDENTITY
        or artifact.get("behavior_manifest_identity")
        != EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
        or not validate_predecessor(predecessor, predecessor_source_bytes)
        or harness.get("schema_version")
        != "maestro.vnext.stage5.proof-harness-receipt.v1"
        or harness.get("passed") != EXPECTED_PROOF_HARNESS_TESTS
        or set(harness) != {"manifest_identity", "passed", "schema_version", "tests"}
        or not isinstance(harness.get("tests"), list)
        or len(harness["tests"]) != EXPECTED_PROOF_HARNESS_TESTS
        or len(set(harness["tests"])) != EXPECTED_PROOF_HARNESS_TESTS
        or harness.get("manifest_identity")
        != EXPECTED_PROOF_HARNESS_MANIFEST_IDENTITY
        or harness.get("manifest_identity")
        != f"sha256:{sha256(canonical_json(harness['tests']))}"
        or harness_bytes != canonical_json(harness)
        or not validate_snapshot_manifest(snapshot_manifest, snapshot_manifest_bytes)
        or not validate_toolchain(toolchain, args.toolchain, args.target)
    ):
        raise RuntimeError("Stage 5 artifact or exact Stage 4 predecessor differs")
    behavior_runs = None
    input_rows = []
    for name, (receipt, receipt_bytes) in named_receipts.items():
        runs = receipt.get("behavior_runs")
        if (
            receipt.get("artifact_id") != artifact_id
            or receipt.get("artifact_sha256") != artifact_sha256
            or receipt.get("behavior_manifest_identity")
            != EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
            or receipt.get("behavior_passed") != EXPECTED_BEHAVIOR_TESTS
            or receipt.get("publication_state") != "inactive_candidate"
            or not validate_engine_receipt(name, receipt, artifact)
            or not isinstance(runs, list)
            or not runs
            or runs[-1].get("label") != "same-count-substitution-mutant"
            or runs[-1].get("rejected") is not True
            or runs[-1].get("result") != "rejected"
            or any(
                "--exact" not in test.get("command", [])
                for run in runs[:-1]
                for test in run.get("tests", [])
            )
        ):
            raise RuntimeError(f"{name} Stage 5 receipt is incomplete or disagrees")
        manifest_rows = behavior_manifest_rows(runs)
        if (
            f"sha256:{sha256(canonical_json(manifest_rows))}"
            != EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
        ):
            raise RuntimeError(f"{name} Stage 5 behavior manifest differs")
        if behavior_runs is None:
            behavior_runs = runs
        elif runs != behavior_runs:
            raise RuntimeError("Stage 5 engines disagree on exact behavioral receipts")
        input_rows.append([name, len(receipt_bytes), sha256(receipt_bytes)])
    input_rows.extend(
        [
            ["artifact", len(artifact_bytes), artifact_sha256],
            ["harness", len(harness_bytes), sha256(harness_bytes)],
            ["predecessor", len(predecessor_bytes), sha256(predecessor_bytes)],
            [
                "predecessor-source",
                len(predecessor_source_bytes),
                sha256(predecessor_source_bytes),
            ],
            ["snapshot-manifest", len(snapshot_manifest_bytes), sha256(snapshot_manifest_bytes)],
            ["toolchain", len(toolchain_bytes), sha256(toolchain_bytes)],
        ]
    )
    input_rows.sort()
    value = {
        "artifact_id": artifact_id,
        "behavior_passed": EXPECTED_BEHAVIOR_TESTS,
        "behavior_manifest_identity": EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
        "exact_behavior_receipt_sha256": sha256(canonical_json(behavior_runs)),
        "inputs": input_rows,
        "predecessor_identity": EXPECTED_STAGE4_IDENTITY,
        "proof_harness_passed": EXPECTED_PROOF_HARNESS_TESTS,
        "publication_state": "inactive_candidate",
        "schema_version": "maestro.vnext.stage5.three-engine-consensus.v1",
    }
    receipt = {**value, "consensus_identity": f"sha256:{sha256(canonical_json(value))}"}
    args.output_root.mkdir(parents=True, exist_ok=True)
    (args.output_root / "three-engine-consensus-receipt.v1.json").write_bytes(
        pretty_json(receipt)
    )
    (args.output_root / "workspace-snapshot-manifest.v1.json").write_bytes(
        snapshot_manifest_bytes
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
