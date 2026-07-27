#!/usr/bin/env python3
"""Freeze the exact V4 Stage 0-12 final-chain inputs without executing a seal."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Mapping


SCHEMA = "maestro.external.vnext-final-cumulative-closure-snapshot.v1"
PACKET_IDENTITY = "sha256:2026513c84b1993f020f7d0430154ec0bc4e821438ccefd7dd6b91834a3d6283"
FANOUT_IDENTITY = "sha256:e299556c31c6a788285d984f9cd3040cfde200ba24e7ed5a5d90caff96ee5954"
REQUIRED_PACKET_FILES = (
    "replacement-build-approval-packet.v4.json",
    "proof-inputs.v4.json",
    "fanout-manifest.v4.json",
    "integration-ancestry.v4.txt",
    "external-build-plan-handoff.v4.json",
    "independent-verification.v4.json",
)
ENGINE_IDS = ("python", "rust", "ruby")
HISTORICAL_STAGE_CHECKPOINTS = {
    0: "66a34db6a28ff3f3ee178f644b645bb6ea60681e",
    1: "bebc2e35314741c3b053901fd5040b323ee2c924",
    2: "602302c69df22e96f319b1451c14d341fdde14cd",
    3: "ad8f3d88bf647031d415aa3aed0998ca7a9097d7",
    4: "9f3cc73b2199c5b2be78dcea8852cbdcafaaafc2",
}
PROVISIONAL_STAGE5_SOURCE = "7080fb6cd1e286998ff47fb6205e90dca990ba40"
OVERLAY_DISALLOWED_EXACT = {
    "AGENTS.md",
    "Cargo.lock",
    "Cargo.toml",
    "build.rs",
    "src/lib.rs",
    "src/main.rs",
}
OVERLAY_DISALLOWED_PREFIXES = (
    ".git/",
    ".maestro/",
    "contracts/vnext/catalogs/",
    "contracts/vnext/public/",
    "contracts/vnext/stage0/",
    "contracts/vnext/stage2/",
    "contracts/vnext/stage3/",
    "contracts/vnext/stage4/",
    "contracts/vnext/stage5/",
    "src/domain/vnext/authority/",
    "src/domain/vnext/contract/",
    "src/domain/vnext/design/",
    "src/domain/vnext/execution/",
    "src/domain/vnext/gate/",
    "src/domain/vnext/identity/",
    "src/domain/vnext/integration/",
    "src/domain/vnext/persistence/",
    "src/domain/vnext/repository/",
    "src/domain/vnext/step/",
    "src/domain/vnext/work/",
    "src/foundation/",
)
TOOL_NAMES = {
    "python": ("python3", ("--version",)),
    "rust": ("rustc", ("-vV",)),
    "ruby": ("ruby", ("--version",)),
    "cargo": ("cargo", ("-vV",)),
    "git": ("git", ("--version",)),
}
EFFECT_DENYLIST = [
    "install",
    "publish",
    "activate",
    "release",
    "push",
    "tag",
    "network",
    "remote_connector",
    "live_external_system",
    "protected_primary_checkout_write",
    "outside_packet_bound_roots_write",
]
READBACK_KINDS = (
    "compiled_namespace_absence",
    "generated_resource_absence",
    "persisted_identity_parity",
    "canonical_facade_behavior",
    "migration_route_absence",
    "retained_reader_absence",
    "consumer_reader_hold_zero",
    "negative_fixture",
)
PROOF_KINDS = (
    "behavior",
    "negative",
    "mutant",
    "authority",
    "race",
    "crash_replay",
    "idempotency",
    "migration",
    "rollback",
    "adapter",
    "identity",
    "removal",
    "ancestry",
    "closure",
)


class GenerationError(RuntimeError):
    pass


def canonical_bytes(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("ascii")


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise GenerationError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def load_json(path: Path) -> dict[str, Any]:
    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.startswith(b"\xef\xbb\xbf") or b"\r" in raw:
        raise GenerationError(f"noncanonical JSON: {path}")
    try:
        value = json.loads(raw, object_pairs_hook=reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise GenerationError(f"invalid JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise GenerationError(f"JSON object required: {path}")
    return value


def run(repository: Path, *argv: str) -> bytes:
    result = subprocess.run(
        argv, cwd=repository, stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )
    if result.returncode != 0:
        raise GenerationError(
            f"command failed ({' '.join(argv)}): "
            f"{result.stderr.decode('utf-8', 'replace').strip()}"
        )
    return result.stdout


def git(repository: Path, *argv: str) -> str:
    return run(repository, "git", *argv).decode("ascii").strip()


def safe_relative(value: str) -> PurePosixPath:
    path = PurePosixPath(value)
    if (
        not value
        or "\\" in value
        or path.is_absolute()
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise GenerationError(f"unsafe relative path: {value!r}")
    return path


def bound_file(path: Path, relative: str) -> dict[str, object]:
    if path.is_symlink() or not path.is_file():
        raise GenerationError(f"bound input is absent or unsafe: {path}")
    raw = path.read_bytes()
    return {"path": relative, "byte_length": len(raw), "sha256": digest(raw)}


def require_empty_destination(path: Path) -> None:
    if path.exists():
        raise GenerationError(f"output root already exists: {path}")
    if path.resolve().is_relative_to(Path("/Users/reinamaccredy/Code/maestro")):
        raise GenerationError("generation output may not enter the protected primary checkout")


def verify_packet(packet_root: Path, destination: Path) -> dict[str, object]:
    for name in REQUIRED_PACKET_FILES:
        if not (packet_root / name).is_file() or (packet_root / name).is_symlink():
            raise GenerationError(f"required V4 packet file is absent or unsafe: {name}")
    approval = load_json(packet_root / REQUIRED_PACKET_FILES[0])
    proof_inputs = load_json(packet_root / REQUIRED_PACKET_FILES[1])
    fanout = load_json(packet_root / REQUIRED_PACKET_FILES[2])
    handoff = load_json(packet_root / REQUIRED_PACKET_FILES[4])
    verification = load_json(packet_root / REQUIRED_PACKET_FILES[5])
    if approval.get("packet_sha256") != PACKET_IDENTITY.removeprefix("sha256:"):
        raise GenerationError("approved packet identity differs")
    if verification.get("packet_sha256") != PACKET_IDENTITY.removeprefix("sha256:"):
        raise GenerationError("independent verification binds another packet")
    if verification.get("status") != "verified":
        raise GenerationError("V4 packet is not independently verified")
    if fanout.get("schema") != "maestro.external.vnext-successor-fanout.v4":
        raise GenerationError("V4 fanout schema differs")
    if digest((packet_root / REQUIRED_PACKET_FILES[2]).read_bytes()) != FANOUT_IDENTITY:
        raise GenerationError("V4 fanout byte identity differs")
    if fanout.get("canonical_integration_order") != list(range(6, 13)):
        raise GenerationError("V4 canonical integration order differs")
    if fanout.get("orchestrator_owned_prefixes") != [
        "contracts/vnext/final-chain/",
        "tools/vnext_contracts/final_chain/",
        "tools/vnext_contracts/fanout/",
    ]:
        raise GenerationError("V4 orchestrator ownership differs")
    if proof_inputs.get("schema") != "maestro.external.successor-proof-inputs.v4":
        raise GenerationError("V4 proof inputs schema differs")
    final_runner = proof_inputs.get("final_runner")
    if not isinstance(final_runner, Mapping) or final_runner != {
        "namespace": "tools/vnext_contracts/final_chain",
        "artifact_namespace": "contracts/vnext/final-chain",
        "snapshot": "FinalCumulativeClosureSnapshotV1",
        "receipt": "FinalCumulativeSealReceiptV1",
        "engines": ["python", "rust", "ruby"],
        "engine_roots": "disjoint",
        "durable_receipt_count": 1,
        "durable_pointer_count": 1,
        "verdict_cache": "forbidden",
        "semantic_stage12_readback": "required",
    }:
        raise GenerationError("V4 final runner contract differs")
    if proof_inputs.get("historical_evidence_policy") != (
        "stage0_stage2_stage3_stage4_stage5_receipts_are_immutable_"
        "predecessor_evidence_not_final_verdicts"
    ):
        raise GenerationError("historical evidence policy differs")
    if proof_inputs.get("final_effect_policy") != "effect_inert":
        raise GenerationError("V4 final effect policy differs")
    if handoff.get("fanout_manifest_sha256") != FANOUT_IDENTITY.removeprefix("sha256:"):
        raise GenerationError("V4 handoff fanout identity differs")
    artifact_hashes = approval.get("artifact_sha256")
    if not isinstance(artifact_hashes, Mapping):
        raise GenerationError("approval packet lacks its artifact map")
    expected_names = set(artifact_hashes) | {
        "replacement-build-approval-packet.v4.json",
        "independent-verification.v4.json",
    }
    present_names = {
        path.name
        for path in packet_root.iterdir()
        if path.is_file() and not path.is_symlink()
    }
    if not expected_names.issubset(present_names):
        raise GenerationError("V4 packet artifact set is incomplete")
    destination.mkdir()
    rows = []
    for name in sorted(expected_names):
        source = packet_root / safe_relative(name)
        actual = hashlib.sha256(source.read_bytes()).hexdigest()
        if name in artifact_hashes and artifact_hashes[name] != actual:
            raise GenerationError(f"V4 packet artifact digest differs: {name}")
        target = destination / name
        shutil.copyfile(source, target)
        target.chmod(stat.S_IRUSR)
        rows.append(bound_file(target, f"packet/{name}"))
    manifest = {
        "schema_version": "maestro.external.vnext-final-packet-manifest.v1",
        "approved_packet_identity": PACKET_IDENTITY,
        "file_count": len(rows),
        "byte_length": sum(int(row["byte_length"]) for row in rows),
        "files": rows,
    }
    manifest_path = destination / "packet-manifest.v1.json"
    manifest_path.write_bytes(canonical_bytes(manifest))
    manifest_path.chmod(stat.S_IRUSR)
    return bound_file(manifest_path, "packet/packet-manifest.v1.json")


def parse_stage(value: str) -> tuple[int, str]:
    match = re.fullmatch(r"(0|[1-9]|1[0-2])=([0-9a-f]{40})", value)
    if match is None:
        raise argparse.ArgumentTypeError("stage checkpoint must be STAGE=40_HEX_COMMIT")
    return int(match.group(1)), match.group(2)


def commit_row(repository: Path, commit: str) -> dict[str, object]:
    return {
        "commit": commit,
        "tree": git(repository, "show", "-s", "--format=%T", commit),
        "parents": git(repository, "show", "-s", "--format=%P", commit).split(),
    }


def first_parent_path(
    repository: Path, descendant: str, ancestor: str
) -> list[dict[str, object]]:
    rows = []
    current = descendant
    while True:
        row = commit_row(repository, current)
        rows.append(row)
        if current == ancestor:
            return rows
        parents = row["parents"]
        if not parents:
            raise GenerationError(
                "Stage 5 second-parent ancestry does not reach the V4 provisional source"
            )
        current = str(parents[0])


def changed_paths(repository: Path, parent: str, child: str) -> set[str]:
    raw = run(
        repository,
        "git",
        "diff",
        "--name-only",
        "-z",
        "--no-renames",
        parent,
        child,
    )
    return {value.decode("utf-8") for value in raw.split(b"\0") if value}


def verify_stage_chain(
    repository: Path,
    final_commit: str,
    reviewed_candidate: str,
    required_stage5_seeds: set[str],
    values: Iterable[tuple[int, str]],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    stages = dict(values)
    if set(stages) != set(range(13)):
        raise GenerationError("exactly one checkpoint for every Stage 0 through 12 is required")
    ordered = [stages[stage] for stage in range(13)]
    if len(set(ordered)) != 13:
        raise GenerationError("Stage checkpoints must be thirteen distinct commits")
    for stage, expected in HISTORICAL_STAGE_CHECKPOINTS.items():
        if ordered[stage] != expected:
            raise GenerationError(f"historical Stage {stage} checkpoint differs")
    if ordered[-1] != final_commit:
        raise GenerationError("Stage 12 checkpoint must be the current exact final V4 commit")
    rows = [{"stage": stage, **commit_row(repository, commit)} for stage, commit in enumerate(ordered)]
    stage5_parents = rows[5]["parents"]
    if len(stage5_parents) != 2 or stage5_parents[0] != ordered[4]:
        raise GenerationError("Stage 5 must be an exact two-parent merge with Stage 4 first")
    stage5_ancestry = first_parent_path(
        repository, str(stage5_parents[1]), PROVISIONAL_STAGE5_SOURCE
    )
    if changed_paths(repository, ordered[4], ordered[5]) != required_stage5_seeds:
        raise GenerationError("Stage 5 merge tree delta is not the exact required seed set")
    for stage in range(6, 12):
        if rows[stage]["parents"] != [ordered[stage - 1]]:
            raise GenerationError(
                f"Stage {stage} must be the sole direct first-parent successor"
            )
    if rows[12]["parents"] != [ordered[11], reviewed_candidate]:
        raise GenerationError(
            "Stage 12 must merge Stage 11 with the exact reviewed candidate second"
        )
    reviewed_tree = git(repository, "show", "-s", "--format=%T", reviewed_candidate)
    if rows[12]["tree"] != reviewed_tree:
        raise GenerationError("Stage 12 and reviewed candidate trees differ")
    return rows, stage5_ancestry


def overlay_entries(repository: Path, stage11: str, stage12: str) -> list[dict[str, object]]:
    raw = run(
        repository,
        "git",
        "diff",
        "--raw",
        "--abbrev=40",
        "-z",
        "--no-renames",
        stage11,
        stage12,
    )
    parts = raw.split(b"\0")
    rows = []
    index = 0
    while index < len(parts) and parts[index]:
        header = parts[index].decode("ascii").split()
        path = parts[index + 1].decode("utf-8")
        index += 2
        if len(header) != 5 or not header[0].startswith(":"):
            raise GenerationError("Stage 12 overlay raw diff is malformed")
        old_mode = header[0][1:]
        new_mode, _old_oid, new_oid, status = header[1:]
        safe_relative(path)
        if (
            status not in {"A", "M"}
            or new_mode not in {"100644", "100755"}
            or (status == "M" and old_mode not in {"100644", "100755"})
        ):
            raise GenerationError("Stage 12 overlay contains a delete, rename, or unsafe mode")
        if any(part.startswith(".") for part in PurePosixPath(path).parts):
            raise GenerationError("Stage 12 overlay contains a hidden path")
        if path in OVERLAY_DISALLOWED_EXACT or any(
            path.startswith(prefix) for prefix in OVERLAY_DISALLOWED_PREFIXES
        ):
            raise GenerationError(f"Stage 12 overlay enters a protected root: {path}")
        blob = run(repository, "git", "cat-file", "blob", new_oid)
        if not blob:
            raise GenerationError("Stage 12 overlay contains an empty or missing blob")
        rows.append(
            {
                "status": status,
                "path": path,
                "mode": new_mode,
                "byte_length": len(blob),
                "sha256": digest(blob),
            }
        )
    if not rows:
        raise GenerationError("Stage 12 overlay is empty")
    return sorted(rows, key=lambda row: str(row["path"]))


def verify_overlay_manifest(
    repository: Path,
    path: Path,
    stage11: str,
    reviewed_candidate: str,
    stage12: str,
) -> dict[str, Any]:
    value = load_json(path)
    reviewed_tree = git(repository, "show", "-s", "--format=%T", reviewed_candidate)
    stage12_tree = git(repository, "show", "-s", "--format=%T", stage12)
    expected_entries = overlay_entries(repository, stage11, stage12)
    expected = {
        "schema_version": "maestro.external.vnext-final-stage12-overlay.v1",
        "stage11_commit": stage11,
        "reviewed_candidate_commit": reviewed_candidate,
        "reviewed_candidate_tree": reviewed_tree,
        "stage12_commit": stage12,
        "stage12_tree": stage12_tree,
        "entry_count": len(expected_entries),
        "byte_length": sum(int(row["byte_length"]) for row in expected_entries),
        "entries": expected_entries,
    }
    if value != expected:
        raise GenerationError(
            "Stage 12 overlay manifest differs from exact path, mode, byte, parent, or tree facts"
        )
    return value


def verify_promotion_prerequisites(
    path: Path, stage11: str, reviewed_candidate: str
) -> dict[str, Any]:
    value = load_json(path)
    if (
        value.get("schema_version")
        != "maestro.external.vnext-final-promotion-prerequisites.v1"
        or value.get("stage11_commit") != stage11
        or value.get("stage12_reviewed_candidate") != reviewed_candidate
        or value.get("legacy_prune_gate", {}).get("status") != "pass"
        or value.get("legacy_prune_gate", {}).get("observed_legacy_row_count") != 0
        or {
            key: value.get("consumer_reader_hold", {}).get(key)
            for key in ("consumer_count", "reader_count", "hold_count")
        }
        != {"consumer_count": 0, "reader_count": 0, "hold_count": 0}
        or {
            key: value.get("promotion_parity", {}).get(key)
            for key in ("source_file_count", "promoted_file_count", "mismatch_count")
        }
        != {"source_file_count": 210, "promoted_file_count": 210, "mismatch_count": 0}
    ):
        raise GenerationError(
            "promotion prerequisites are absent, nonzero, stale, or noncanonical"
        )
    for section in ("legacy_prune_gate", "consumer_reader_hold", "promotion_parity"):
        binding = value[section].get("receipt")
        if (
            not isinstance(binding, Mapping)
            or not isinstance(binding.get("path"), str)
            or not isinstance(binding.get("byte_length"), int)
            or not isinstance(binding.get("sha256"), str)
        ):
            raise GenerationError(f"{section} receipt binding is absent")
        receipt = path.parent.joinpath(*safe_relative(str(binding.get("path"))).parts)
        actual = bound_file(receipt, str(binding["path"]))
        if actual != binding:
            raise GenerationError(f"{section} receipt bytes differ")
    return value


def archive_commit(repository: Path, commit: str, destination: Path) -> None:
    archive = run(repository, "git", "archive", "--format=tar", commit)
    destination.mkdir()
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as tar:
        members = tar.getmembers()
        for member in members:
            relative = safe_relative(member.name.rstrip("/"))
            target = destination.joinpath(*relative.parts)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            if not member.isfile() or member.issym() or member.islnk():
                raise GenerationError(f"unsupported Git archive entry: {member.name}")
            target.parent.mkdir(parents=True, exist_ok=True)
            extracted = tar.extractfile(member)
            if extracted is None:
                raise GenerationError(f"cannot extract Git archive entry: {member.name}")
            target.write_bytes(extracted.read())
            target.chmod(stat.S_IRUSR | (stat.S_IXUSR if member.mode & 0o111 else 0))


def ancestry_pack(repository: Path, commit: str) -> bytes:
    result = subprocess.run(
        ["git", "--no-replace-objects", "pack-objects", "--stdout", "--revs"],
        cwd=repository,
        input=f"{commit}\n".encode("ascii"),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0 or not result.stdout.startswith(b"PACK"):
        raise GenerationError(
            "exact ancestry object-pack construction failed: "
            + result.stderr.decode("utf-8", "replace").strip()
        )
    return result.stdout


def classify(path: str) -> str:
    lower = path.lower()
    if path in {"Cargo.lock", "Cargo.toml"} or lower.endswith((".lock", ".toml")):
        return "lockfile"
    if path.startswith("tests/fixtures/"):
        if "migration" in lower:
            return "migration"
        if "rollback" in lower:
            return "rollback"
        if "consumer" in lower:
            return "consumer_manifest"
        if "reader" in lower:
            return "reader_manifest"
        if "hold" in lower:
            return "hold_manifest"
        return "fixture"
    if path.startswith("tests/") or "/test_" in lower or lower.endswith("_test.rb"):
        return "test"
    if "mutation" in lower or "mutant" in lower:
        return "mutation"
    if "crash" in lower or "fault_schedule" in lower or "fault-schedule" in lower:
        return "crash_schedule"
    if "migration" in lower:
        return "migration"
    if "rollback" in lower:
        return "rollback"
    if "adapter" in lower:
        return "adapter"
    if "removal" in lower:
        return "removal"
    if "consumer" in lower:
        return "consumer_manifest"
    if "reader" in lower:
        return "reader_manifest"
    if "hold" in lower:
        return "hold_manifest"
    if path.startswith("contracts/vnext/stage"):
        return "predecessor"
    if path.startswith("tools/vnext_contracts/"):
        return "validator"
    if path.startswith("contracts/vnext/final-chain/"):
        return "proof_control"
    return "source"


def input_manifest(source: Path, commit: str, tree: str) -> dict[str, Any]:
    rows = []
    for path in sorted(item for item in source.rglob("*") if item.is_file()):
        if path.is_symlink():
            raise GenerationError(f"snapshot source contains a symlink: {path}")
        relative = path.relative_to(source).as_posix()
        raw = path.read_bytes()
        rows.append(
            {
                "kind": classify(relative),
                "path": relative,
                "mode": "100755" if os.access(path, os.X_OK) else "100644",
                "byte_length": len(raw),
                "sha256": digest(raw),
            }
        )
    return {
        "schema_version": "maestro.external.vnext-final-input-manifest.v1",
        "commit": commit,
        "tree": tree,
        "entry_count": len(rows),
        "byte_length": sum(row["byte_length"] for row in rows),
        "entries": rows,
    }


def stream_identity(data: bytes) -> dict[str, object]:
    return {"byte_length": len(data), "sha256": digest(data)}


def materialize_dependency_tree(source: Path, destination: Path) -> None:
    if source.is_symlink() or not source.is_dir():
        raise GenerationError(f"dependency source root is absent or unsafe: {source}")
    for path in source.rglob("*"):
        if path.is_symlink():
            raise GenerationError(f"dependency source contains a symlink: {path}")
    shutil.copytree(source, destination)


def dependency_output(root: Path, name: str) -> dict[str, object]:
    rows = []
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        if path.is_symlink():
            raise GenerationError(f"materialized dependency is a symlink: {path}")
        raw = path.read_bytes()
        rows.append(
            {
                "path": path.relative_to(root).as_posix(),
                "byte_length": len(raw),
                "sha256": digest(raw),
            }
        )
    if not rows:
        raise GenerationError(f"materialized dependency closure is empty: {name}")
    return {
        "name": name,
        "resolved_path": str(root.resolve()),
        "file_count": len(rows),
        "byte_length": sum(int(row["byte_length"]) for row in rows),
        "identity": digest(canonical_bytes(rows)),
        "files": rows,
    }


def toolchain(
    source: Path, target: str, profile: str, dependency_root: Path
) -> dict[str, object]:
    tools: dict[str, object] = {}
    for name, (executable, probe) in TOOL_NAMES.items():
        resolved = shutil.which(executable)
        if resolved is None:
            raise GenerationError(f"required final-chain tool is unavailable: {executable}")
        path = Path(resolved).resolve()
        raw = path.read_bytes()
        result = subprocess.run(
            [str(path), *probe], stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        tools[name] = {
            "resolved_path": str(path),
            "byte_length": len(raw),
            "sha256": digest(raw),
            "probe_argv": [str(path), *probe],
            "probe_exit_code": result.returncode,
            "probe_stdout": stream_identity(result.stdout),
            "probe_stderr": stream_identity(result.stderr),
        }
    lockfiles = []
    for relative in ("Cargo.toml", "Cargo.lock"):
        lockfiles.append(bound_file(source / relative, relative))
    rustc = Path(str(tools["rust"]["resolved_path"]))
    target_libdir_result = subprocess.run(
        [str(rustc), "--print", "target-libdir", "--target", target],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if target_libdir_result.returncode != 0:
        raise GenerationError("rustc target dependency closure probe failed")
    target_libdir = Path(
        target_libdir_result.stdout.decode("utf-8").strip()
    ).resolve()
    if target_libdir.is_symlink() or not target_libdir.is_dir():
        raise GenerationError("rust target dependency root is absent or unsafe")
    cargo_home = Path(
        os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))
    ).resolve()
    required_cargo_roots = [
        cargo_home / "registry/index",
        cargo_home / "registry/cache",
        cargo_home / "registry/src",
    ]
    if not all(path.is_dir() and not path.is_symlink() for path in required_cargo_roots):
        raise GenerationError(
            "complete Cargo registry index/cache/source closure is unavailable"
        )
    dependency_outputs = []
    for engine in ENGINE_IDS:
        engine_root = dependency_root / engine
        cargo_destination = engine_root / "cargo-home"
        cargo_destination.mkdir(parents=True)
        for relative in ("registry/index", "registry/cache", "registry/src"):
            materialize_dependency_tree(
                cargo_home / relative, cargo_destination / relative
            )
        for optional in ("git/db", "git/checkouts"):
            candidate = cargo_home / optional
            if candidate.is_dir() and not candidate.is_symlink():
                materialize_dependency_tree(candidate, cargo_destination / optional)
        (cargo_destination / "config.toml").write_text(
            "[net]\noffline = true\nretry = 0\n",
            encoding="utf-8",
        )
        materialize_dependency_tree(
            target_libdir, engine_root / "rust-target-libdir"
        )
        probe_argv = [
            str(tools["cargo"]["resolved_path"]),
            "fetch",
            "--offline",
            "--frozen",
            "--locked",
            "--target",
            target,
            "--manifest-path",
            str(source / "Cargo.toml"),
        ]
        probe = subprocess.run(
            probe_argv,
            env={
                **os.environ,
                "CARGO_HOME": str(cargo_destination),
                "CARGO_TARGET_DIR": str(engine_root / "probe-target"),
                "LC_ALL": "C",
                "LANG": "C",
                "TZ": "UTC",
            },
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if probe.returncode != 0:
            raise GenerationError(
                f"{engine} Cargo dependency closure completeness probe failed"
            )
        output = dependency_output(
            engine_root, f"{engine}-complete-cargo-native-closure"
        )
        output["completeness_probe"] = {
            "argv": [
                "{tool:cargo}",
                "fetch",
                "--offline",
                "--frozen",
                "--locked",
                "--target",
                target,
                "--manifest-path",
                "{source}/Cargo.toml",
            ],
            "exit_code": probe.returncode,
            "stdout": stream_identity(probe.stdout),
            "stderr": stream_identity(probe.stderr),
        }
        dependency_outputs.append(output)
    return {
        "schema_version": "maestro.external.vnext-final-toolchain.v1",
        "target": target,
        "profile": profile,
        "environment": {"LC_ALL": "C", "LANG": "C", "TZ": "UTC"},
        "lockfiles": lockfiles,
        "dependency_outputs": dependency_outputs,
        "tools": tools,
    }


def command_identity(argv: list[str], expected_exit_code: int) -> str:
    return digest(
        canonical_bytes({"argv": argv, "expected_exit_code": expected_exit_code})
    )


def binding_index(manifest: Mapping[str, Any]) -> dict[str, dict[str, object]]:
    return {
        str(row["path"]): {
            "path": str(row["path"]),
            "byte_length": int(row["byte_length"]),
            "sha256": str(row["sha256"]),
        }
        for row in manifest["entries"]
    }


def build_ledger(
    manifest: Mapping[str, Any],
    registry: Mapping[str, Any],
    registry_binding: Mapping[str, object],
    commit: str,
) -> dict[str, object]:
    bindings = binding_index(manifest)
    if (
        registry.get("schema_version")
        != "maestro.external.vnext-final-proof-registry.v1"
        or registry.get("registry_identity_policy")
        != "canonical-bytes-bound-no-inference-no-reassignment"
    ):
        raise GenerationError("normative proof registry identity policy differs")
    registry_rows = registry.get("proofs")
    if not isinstance(registry_rows, list) or not registry_rows:
        raise GenerationError("normative proof registry is empty")
    rows: list[dict[str, object]] = []
    identifiers: set[str] = set()
    for registry_row in registry_rows:
        if not isinstance(registry_row, Mapping):
            raise GenerationError("normative proof registry row is invalid")
        proof_id = str(registry_row.get("proof_id"))
        if proof_id in identifiers:
            raise GenerationError(f"duplicate normative proof id: {proof_id}")
        identifiers.add(proof_id)
        stage = registry_row.get("stage")
        kind = registry_row.get("kind")
        if stage not in range(13) or kind not in PROOF_KINDS:
            raise GenerationError(f"normative proof row classification differs: {proof_id}")
        rotation_slot = registry_row.get("rotation_slot")
        if rotation_slot is not None:
            slot_id = (
                rotation_slot.get("slot_id")
                if isinstance(rotation_slot, Mapping)
                else proof_id
            )
            raise GenerationError(f"promotion rotation slot is unresolved: {slot_id}")
        if registry_row.get("engines") != list(ENGINE_IDS):
            raise GenerationError(f"normative proof engine coverage differs: {proof_id}")
        specification = registry_row.get("command")
        if not isinstance(specification, Mapping):
            raise GenerationError(f"normative proof command is absent: {proof_id}")
        argv = specification.get("argv")
        expected_exit_code = specification.get("expected_exit_code")
        if (
            not isinstance(argv, list)
            or not argv
            or not all(isinstance(value, str) and value for value in argv)
            or not isinstance(expected_exit_code, int)
        ):
            raise GenerationError(f"normative proof command differs: {proof_id}")
        command: dict[str, object] = {
            "argv": list(argv),
            "expected_exit_code": expected_exit_code,
            "identity": command_identity(list(argv), expected_exit_code),
        }
        input_paths = registry_row.get("input_paths")
        if not isinstance(input_paths, list) or not input_paths:
            raise GenerationError(f"normative proof inputs are absent: {proof_id}")
        try:
            input_bindings = [bindings[str(path)] for path in input_paths]
        except KeyError as error:
            raise GenerationError(
                f"normative proof input is absent from final tree: {error.args[0]}"
            ) from error
        harness = registry_row.get("harness")
        if not isinstance(harness, Mapping):
            raise GenerationError(f"normative proof harness is absent: {proof_id}")
        harness_value: dict[str, object] = {
            "protocol": harness.get("protocol"),
            "required_receipt": harness.get("required_receipt"),
        }
        if harness.get("fault_schedule_path") is not None:
            fault_path = str(harness["fault_schedule_path"])
            harness_value["fault_schedule"] = bindings[fault_path]
        if harness.get("cohort_path") is not None:
            cohort_path = str(harness["cohort_path"])
            harness_value["cohort"] = bindings[cohort_path]
        rows.append(
            {
                "proof_id": proof_id,
                "stage": stage,
                "kind": kind,
                "expected_outcome": registry_row.get("expected_outcome"),
                "engines": list(ENGINE_IDS),
                "command": command,
                "input_bindings": input_bindings,
                "harness": harness_value,
                "produced_artifacts": [],
            }
        )
    if {int(row["stage"]) for row in rows} != set(range(13)):
        raise GenerationError("normative proof registry does not cover every Stage")
    if {str(row["kind"]) for row in rows} != set(PROOF_KINDS):
        raise GenerationError("normative proof registry does not cover every proof kind")
    rows.sort(key=lambda row: (int(row["stage"]), str(row["proof_id"])))
    return {
        "schema_version": "maestro.external.vnext-final-proof-ledger.v1",
        "snapshot_commit": commit,
        "registry_identity": dict(registry_binding),
        "proof_count": len(rows),
        "proofs": rows,
    }


def readback_plan(
    registry: Mapping[str, Any], commit: str
) -> dict[str, object]:
    registry_rows = {
        str(row.get("proof_id")): row
        for row in registry.get("proofs", [])
        if isinstance(row, Mapping)
    }
    command_rows = {}
    for proof_id in (
        "s12-post-promotion-readback",
        "s12-post-promotion-removal",
    ):
        row = registry_rows.get(proof_id)
        command = row.get("command") if isinstance(row, Mapping) else None
        argv = command.get("argv") if isinstance(command, Mapping) else None
        expected_exit_code = (
            command.get("expected_exit_code")
            if isinstance(command, Mapping)
            else None
        )
        if (
            not isinstance(argv, list)
            or not all(isinstance(value, str) and value for value in argv)
            or argv[:4] != ["{tool:cargo}", "test", "--offline", "--frozen"]
            or "--lib" not in argv
            or "--exact" not in argv
            or expected_exit_code != 0
        ):
            raise GenerationError(
                f"Stage 12 semantic readback command is unresolved or inexact: {proof_id}"
            )
        command_rows[proof_id] = (list(argv), expected_exit_code)
    requirements = {
        "compiled_namespace_absence": ["compiled"],
        "generated_resource_absence": ["resource"],
        "persisted_identity_parity": ["schema", "persisted"],
        "canonical_facade_behavior": ["compiled", "exported"],
        "migration_route_absence": ["persisted", "reader"],
        "retained_reader_absence": ["reader"],
        "consumer_reader_hold_zero": ["consumer", "reader", "hold"],
        "negative_fixture": ["compiled", "resource"],
    }
    rows = []
    for kind in READBACK_KINDS:
        proof_id = (
            "s12-post-promotion-readback"
            if kind in {"canonical_facade_behavior", "negative_fixture"}
            else "s12-post-promotion-removal"
        )
        argv, expected_exit_code = command_rows[proof_id]
        rows.append(
            {
                "id": kind.replace("_", "-"),
                "kind": kind,
                "argv": argv,
                "expected_exit_code": expected_exit_code,
                "command_identity": command_identity(argv, expected_exit_code),
                "required_artifact_kinds": requirements[kind],
                "minimum_canonical_reads": 1,
                "minimum_negative_routes": 16 if kind == "negative_fixture" else 1,
            }
        )
    return {
        "schema_version": "maestro.external.vnext-stage12-semantic-readback-plan.v1",
        "snapshot_commit": commit,
        "checks": rows,
    }


def chmod_readonly(root: Path) -> None:
    for path in sorted(root.rglob("*"), reverse=True):
        if path.is_symlink():
            raise GenerationError(f"frozen closure contains a symlink: {path}")
        path.chmod(
            stat.S_IRUSR
            | (stat.S_IXUSR if path.is_dir() or os.access(path, os.X_OK) else 0)
        )
    root.chmod(stat.S_IRUSR | stat.S_IXUSR)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--packet-root", type=Path, required=True)
    parser.add_argument("--final-ref", required=True)
    parser.add_argument("--stage12-reviewed-candidate", required=True)
    parser.add_argument("--stage12-overlay-manifest", type=Path, required=True)
    parser.add_argument("--promotion-prerequisites", type=Path, required=True)
    parser.add_argument("--stage-checkpoint", action="append", type=parse_stage, required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--publication-root", type=Path, required=True)
    parser.add_argument("--protected-primary", type=Path, required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--profile", required=True)
    args = parser.parse_args()

    repository = args.repository.resolve()
    packet_root = args.packet_root.resolve()
    output = args.output_root.resolve()
    publication_argument = args.publication_root.absolute()
    if publication_argument.is_symlink():
        raise GenerationError("publication root may not be a symlink")
    publication = publication_argument.resolve(strict=True)
    protected_primary = args.protected_primary.resolve()
    require_empty_destination(output)
    if repository == protected_primary:
        raise GenerationError("final candidate repository may not be the protected primary checkout")
    if publication == protected_primary or publication.is_relative_to(protected_primary):
        raise GenerationError("publication root may not enter the protected primary checkout")
    status = git(repository, "status", "--porcelain=v1", "--untracked-files=all")
    if status:
        raise GenerationError("final candidate worktree is not clean")
    final_commit = git(repository, "rev-parse", "--verify", f"{args.final_ref}^{{commit}}")
    if not re.fullmatch(r"[0-9a-f]{40}", final_commit):
        raise GenerationError("final ref did not resolve to one commit")
    final_tree = git(repository, "show", "-s", "--format=%T", final_commit)
    reviewed_candidate = git(
        repository,
        "rev-parse",
        "--verify",
        f"{args.stage12_reviewed_candidate}^{{commit}}",
    )
    fanout = load_json(packet_root / "fanout-manifest.v4.json")
    required_stage5_seeds = {
        str(value) for value in fanout.get("required_stage5_seeds", [])
    }
    if not required_stage5_seeds:
        raise GenerationError("V4 required Stage 5 seed set is absent")
    rows, stage5_ancestry = verify_stage_chain(
        repository,
        final_commit,
        reviewed_candidate,
        required_stage5_seeds,
        args.stage_checkpoint,
    )
    overlay = verify_overlay_manifest(
        repository,
        args.stage12_overlay_manifest.resolve(strict=True),
        str(rows[11]["commit"]),
        reviewed_candidate,
        final_commit,
    )
    promotion_prerequisites_path = args.promotion_prerequisites.resolve(strict=True)
    promotion_prerequisites = verify_promotion_prerequisites(
        promotion_prerequisites_path,
        str(rows[11]["commit"]),
        reviewed_candidate,
    )
    if publication.is_symlink() or not publication.is_dir():
        raise GenerationError(
            "publication root must pre-exist as a non-symlink directory"
        )
    publication_stat = publication.stat(follow_symlinks=False)
    publication_identity = {
        "path": str(publication),
        "device": publication_stat.st_dev,
        "inode": publication_stat.st_ino,
        "mount_device": publication_stat.st_dev,
        "mode": stat.S_IMODE(publication_stat.st_mode),
        "link_count": publication_stat.st_nlink,
        "ctime_ns": publication_stat.st_ctime_ns,
    }
    pointer_path = publication / "current.json"
    pointer_preimage: dict[str, object]
    if pointer_path.exists():
        if pointer_path.is_symlink() or not pointer_path.is_file():
            raise GenerationError("final pointer is unsafe")
        pointer_value = load_json(pointer_path)
        generation = pointer_value.get("generation")
        if not isinstance(generation, int) or generation < 1:
            raise GenerationError("final pointer generation is invalid")
        pointer_preimage = {
            "state": "present",
            "generation": generation,
            "sha256": digest(pointer_path.read_bytes()),
        }
    else:
        pointer_preimage = {"state": "absent", "generation": 0}

    temporary = Path(tempfile.mkdtemp(prefix=".final-chain-generate-", dir=output.parent))
    try:
        source = temporary / "source"
        packet = temporary / "packet"
        control = temporary / "control"
        dependencies = temporary / "dependencies"
        control.mkdir()
        dependencies.mkdir()
        archive_commit(repository, final_commit, source)
        packet_binding = verify_packet(packet_root, packet)
        manifest = input_manifest(source, final_commit, final_tree)
        registry_path = source / "contracts/vnext/final-chain/proof-registry.v1.json"
        registry = load_json(registry_path)
        registry_by_id = {
            str(row.get("proof_id")): row for row in registry.get("proofs", [])
        }
        expected_filters = {
            "stage11_migration_filter": "s11-frozen-cohort-migration",
            "stage11_rollback_filter": "s11-frozen-cohort-rollback",
            "stage12_readback_filter": "s12-post-promotion-readback",
            "stage12_removal_filter": "s12-post-promotion-removal",
        }
        for field, proof_id in expected_filters.items():
            command = registry_by_id.get(proof_id, {}).get("command", {})
            argv = command.get("argv", [])
            if (
                not isinstance(argv, list)
                or "--lib" not in argv
                or promotion_prerequisites[field] not in argv
                or "--exact" not in argv
            ):
                raise GenerationError(
                    f"promotion prerequisite filter is not the exact resolved registry command: {field}"
                )
        registry_binding = bound_file(
            registry_path, "contracts/vnext/final-chain/proof-registry.v1.json"
        )
        ledger = build_ledger(manifest, registry, registry_binding, final_commit)
        readback = readback_plan(registry, final_commit)
        toolchain_value = toolchain(source, args.target, args.profile, dependencies)
        for dependency in toolchain_value["dependency_outputs"]:
            engine = str(dependency["name"]).split("-", 1)[0]
            dependency["resolved_path"] = str(output / "dependencies" / engine)
        overlay_path = control / "stage12-overlay.v1.json"
        overlay_path.write_bytes(canonical_bytes(overlay))
        ancestry_pack_path = control / "ancestry-objects.pack"
        ancestry_pack_path.write_bytes(ancestry_pack(repository, final_commit))
        promotion_root = control / "promotion"
        promotion_root.mkdir()
        promotion_path = promotion_root / "promotion-prerequisites.v1.json"
        promotion_path.write_bytes(canonical_bytes(promotion_prerequisites))
        for section in ("legacy_prune_gate", "consumer_reader_hold", "promotion_parity"):
            binding = promotion_prerequisites[section]["receipt"]
            relative = safe_relative(str(binding["path"]))
            target = promotion_root.joinpath(*relative.parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(
                promotion_prerequisites_path.parent.joinpath(*relative.parts),
                target,
            )
        for name, value in (
            ("input-manifest.v1.json", manifest),
            ("proof-ledger.v1.json", ledger),
            ("stage12-semantic-readback.v1.json", readback),
            ("toolchain.v1.json", toolchain_value),
        ):
            (control / name).write_bytes(canonical_bytes(value))
        for row in rows:
            checkpoint = {
                "schema_version": "maestro.external.vnext-final-stage-checkpoint.v1",
                **row,
            }
            checkpoint_path = control / f"stage-{row['stage']}-checkpoint.v1.json"
            checkpoint_path.write_bytes(canonical_bytes(checkpoint))
            row["checkpoint"] = bound_file(
                checkpoint_path, f"control/{checkpoint_path.name}"
            )
        engines = []
        for engine, filename in (
            ("python", "engine_python.py"),
            ("rust", "engine_rust.rs"),
            ("ruby", "engine_ruby.rb"),
        ):
            binding = next(
                entry
                for entry in manifest["entries"]
                if entry["path"] == f"tools/vnext_contracts/final_chain/{filename}"
            )
            engines.append(
                {
                    "id": engine,
                    "language": engine,
                    "source": {
                        "path": binding["path"],
                        "byte_length": binding["byte_length"],
                        "sha256": binding["sha256"],
                    },
                }
            )
        snapshot = {
            "schema_version": SCHEMA,
            "state": "frozen",
            "approved_packet_identity": PACKET_IDENTITY,
            "packet_manifest": packet_binding,
            "final_integration": {"commit": final_commit, "tree": final_tree},
            "first_parent_stages": rows,
            "provisional_stage5_source_commit": PROVISIONAL_STAGE5_SOURCE,
            "stage5_second_parent_ancestry": stage5_ancestry,
            "stage12_reviewed_candidate": {
                "commit": reviewed_candidate,
                "tree": overlay["reviewed_candidate_tree"],
            },
            "stage12_overlay": bound_file(
                overlay_path, "control/stage12-overlay.v1.json"
            ),
            "ancestry_pack": bound_file(
                ancestry_pack_path, "control/ancestry-objects.pack"
            ),
            "promotion_prerequisites": bound_file(
                promotion_path,
                "control/promotion/promotion-prerequisites.v1.json",
            ),
            "input_manifest": bound_file(
                control / "input-manifest.v1.json",
                "control/input-manifest.v1.json",
            ),
            "proof_ledger": bound_file(
                control / "proof-ledger.v1.json", "control/proof-ledger.v1.json"
            ),
            "proof_registry": registry_binding,
            "stage12_readback": bound_file(
                control / "stage12-semantic-readback.v1.json",
                "control/stage12-semantic-readback.v1.json",
            ),
            "toolchain": bound_file(
                control / "toolchain.v1.json", "control/toolchain.v1.json"
            ),
            "environment_allowlist": ["HOME", "LANG", "LC_ALL", "PATH", "TMPDIR", "TZ"],
            "immutable_input_roots": ["source", "packet", "control", "dependencies"],
            "writable_root_roles": [
                f"{engine}-{role}"
                for engine in ENGINE_IDS
                for role in ("temp", "target", "deps", "output")
            ],
            "protected_primary_checkout": str(protected_primary),
            "publication_root_identity": publication_identity,
            "expected_generation": pointer_preimage["generation"],
            "cache_policy": "immutable_compilation_and_dependency_bytes_only",
            "sandbox_profile": "macos-sandbox-exec-no-network-v1",
            "pointer_preimage": pointer_preimage,
            "engines": engines,
            "effect_denylist": EFFECT_DENYLIST,
        }
        (control / "final-cumulative-closure-snapshot.v1.json").write_bytes(
            canonical_bytes(snapshot)
        )
        chmod_readonly(source)
        chmod_readonly(packet)
        chmod_readonly(control)
        chmod_readonly(dependencies)
        os.rename(temporary, output)
    finally:
        if temporary.exists():
            shutil.rmtree(temporary)
    print(output / "control/final-cumulative-closure-snapshot.v1.json")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GenerationError as error:
        print(f"final-chain generation refused: {error}", file=os.sys.stderr)
        raise SystemExit(2)
