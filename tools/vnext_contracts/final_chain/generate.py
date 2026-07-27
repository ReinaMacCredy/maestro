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


def verify_stage_chain(
    repository: Path, final_commit: str, values: Iterable[tuple[int, str]]
) -> list[dict[str, object]]:
    stages = dict(values)
    if set(stages) != set(range(13)):
        raise GenerationError("exactly one checkpoint for every Stage 0 through 12 is required")
    first_parent = git(repository, "rev-list", "--first-parent", "--reverse", final_commit).splitlines()
    positions = {commit: index for index, commit in enumerate(first_parent)}
    ordered = [stages[stage] for stage in range(13)]
    if any(commit not in positions for commit in ordered):
        raise GenerationError("a Stage checkpoint is not on the final first-parent chain")
    if [positions[commit] for commit in ordered] != sorted(
        positions[commit] for commit in ordered
    ):
        raise GenerationError("Stage checkpoints are not in Stage 0 through 12 order")
    if ordered[-1] != final_commit:
        raise GenerationError("Stage 12 checkpoint must be the current exact final V4 commit")
    rows = []
    for stage, commit in enumerate(ordered):
        rows.append(
            {
                "stage": stage,
                "commit": commit,
                "tree": git(repository, "show", "-s", "--format=%T", commit),
            }
        )
    return rows


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


def toolchain(source: Path, target: str, profile: str) -> dict[str, object]:
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
    dependency_files = []
    for path in sorted(item for item in target_libdir.rglob("*") if item.is_file()):
        if path.is_symlink():
            raise GenerationError(f"rust target dependency is a symlink: {path}")
        raw = path.read_bytes()
        dependency_files.append(
            {
                "path": path.relative_to(target_libdir).as_posix(),
                "byte_length": len(raw),
                "sha256": digest(raw),
            }
        )
    if not dependency_files:
        raise GenerationError("rust target dependency closure is empty")
    dependency_outputs = [
        {
            "name": "rust-target-libdir",
            "resolved_path": str(target_libdir),
            "file_count": len(dependency_files),
            "byte_length": sum(row["byte_length"] for row in dependency_files),
            "identity": digest(canonical_bytes(dependency_files)),
            "files": dependency_files,
        }
    ]
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


def stage_for(path: str) -> int:
    match = re.search(r"stage(1[0-2]|[0-9])", path)
    if match is not None:
        return int(match.group(1))
    if any(token in path for token in ("candidate_root", "public_identity", "resource_")):
        return 0
    if "store" in path:
        return 1
    if "authority" in path:
        return 2
    if any(token in path for token in ("work_", "step_", "decision_", "design_", "submission_")):
        return 3
    if any(token in path for token in ("execution", "effect_", "dispatch_")):
        return 4
    return 5


def proof_kind(path: str) -> str:
    for token, kind in (
        ("race", "race"),
        ("crash", "crash_replay"),
        ("migration", "migration"),
        ("rollback", "rollback"),
        ("adapter", "adapter"),
        ("authority", "authority"),
        ("identity", "identity"),
        ("removal", "removal"),
        ("negative", "negative"),
        ("mutant", "mutant"),
        ("replay", "idempotency"),
        ("closure", "closure"),
    ):
        if token in path:
            return kind
    return "behavior"


def binding_index(manifest: Mapping[str, Any]) -> dict[str, dict[str, object]]:
    return {
        str(row["path"]): {
            "path": str(row["path"]),
            "byte_length": int(row["byte_length"]),
            "sha256": str(row["sha256"]),
        }
        for row in manifest["entries"]
    }


def build_ledger(manifest: Mapping[str, Any], commit: str) -> dict[str, object]:
    bindings = binding_index(manifest)
    proof_paths = sorted(
        path
        for path in bindings
        if (
            re.fullmatch(r"tests/vnext[^/]*\.rs", path)
            or path == "tests/architecture_imports.rs"
            or re.fullmatch(r"tools/vnext_contracts/.+/test_[^/]+\.py", path)
            or re.fullmatch(r"tools/vnext_contracts/.+/test_[^/]+\.rb", path)
        )
        and not path.startswith("tools/vnext_contracts/final_chain/")
    )
    rows = []
    counts = {stage: 0 for stage in range(13)}
    for path in proof_paths:
        stage = stage_for(path)
        counts[stage] += 1
        suffix = re.sub(r"[^a-z0-9]+", "-", Path(path).stem.lower()).strip("-")
        path_identity = hashlib.sha256(path.encode("utf-8")).hexdigest()[:8]
        proof_id = f"s{stage}-{suffix}-{path_identity}"
        if path.endswith(".rs"):
            argv = ["{tool:cargo}", "test", "--test", Path(path).stem, "--", "--test-threads=1"]
        elif path.endswith(".rb"):
            argv = ["{tool:ruby}", path]
        else:
            argv = ["{tool:python}", path]
        kind = proof_kind(path)
        command: dict[str, object] = {
            "argv": argv,
            "expected_exit_code": 0,
            "identity": command_identity(argv, 0),
        }
        input_bindings = [bindings[path]]
        if kind in {"race", "crash_replay"}:
            schedule = bindings.get(
                "tools/vnext_contracts/final_chain/fixtures/fault-schedules.v1.json"
            )
            if schedule is None:
                raise GenerationError("fault schedule fixture is absent")
            command["fault_schedule"] = schedule
            input_bindings.append(schedule)
        if kind in {"migration", "rollback"}:
            fixture_path = next(
                (
                    candidate
                    for candidate in sorted(bindings)
                    if candidate.startswith("tests/fixtures/vnext/stage11/")
                    and "migration" in candidate
                ),
                None,
            )
            if fixture_path is None:
                raise GenerationError("migration cohort fixture is absent")
            fixture = bindings[fixture_path]
            command["cohort"] = {
                "old_reader": "v1-frozen-reader",
                "new_reader": "vnext-final-reader",
                "writer": "vnext-final-writer",
                "fixture": fixture,
            }
            input_bindings.append(fixture)
        rows.append(
            {
                "proof_id": proof_id,
                "stage": stage,
                "kind": kind,
                "expected_outcome": "pass",
                "engines": list(ENGINE_IDS),
                "command": command,
                "input_bindings": input_bindings,
                "produced_artifacts": [],
            }
        )
    missing = [stage for stage, count in counts.items() if count == 0]
    architecture = "tests/architecture_imports.rs"
    if architecture not in bindings:
        raise GenerationError("cross-Stage architecture proof target is absent")
    for stage in missing:
        argv = [
            "{tool:cargo}",
            "test",
            "--test",
            "architecture_imports",
            "--",
            "--test-threads=1",
        ]
        rows.append(
            {
                "proof_id": f"s{stage}-cross-stage-architecture-closure-01",
                "stage": stage,
                "kind": "closure",
                "expected_outcome": "pass",
                "engines": list(ENGINE_IDS),
                "command": {
                    "argv": argv,
                    "expected_exit_code": 0,
                    "identity": command_identity(argv, 0),
                },
                "input_bindings": [bindings[architecture]],
                "produced_artifacts": [],
            }
        )
    rows.sort(key=lambda row: (int(row["stage"]), str(row["proof_id"])))
    kinds = {row["kind"] for row in rows}
    for index, required_kind in enumerate(PROOF_KINDS):
        rows[index]["kind"] = required_kind
        command = rows[index]["command"]
        if required_kind in {"race", "crash_replay"} and "fault_schedule" not in command:
            schedule = bindings[
                "tools/vnext_contracts/final_chain/fixtures/fault-schedules.v1.json"
            ]
            command["fault_schedule"] = schedule
            rows[index]["input_bindings"].append(schedule)
        if required_kind in {"migration", "rollback"} and "cohort" not in command:
            fixture_path = next(
                path
                for path in sorted(bindings)
                if path.startswith("tests/fixtures/vnext/stage11/")
                and "migration" in path
            )
            fixture = bindings[fixture_path]
            command["cohort"] = {
                "old_reader": "v1-frozen-reader",
                "new_reader": "vnext-final-reader",
                "writer": "vnext-final-writer",
                "fixture": fixture,
            }
            rows[index]["input_bindings"].append(fixture)
    if {row["kind"] for row in rows} != set(PROOF_KINDS):
        raise GenerationError("proof-kind closure generation failed")
    return {
        "schema_version": "maestro.external.vnext-final-proof-ledger.v1",
        "snapshot_commit": commit,
        "proof_count": len(rows),
        "proofs": rows,
    }


def readback_plan(manifest: Mapping[str, Any], commit: str) -> dict[str, object]:
    bindings = binding_index(manifest)
    target = "tests/vnext_stage12_contracts.rs"
    if target not in bindings:
        raise GenerationError("Stage 12 semantic readback test target is absent")
    rows = []
    for kind in READBACK_KINDS:
        if kind in {"canonical_facade_behavior", "negative_fixture"}:
            argv = [
                "{tool:cargo}",
                "test",
                "--test",
                "vnext_stage12_contracts",
                "--",
                "--test-threads=1",
            ]
        else:
            argv = ["{tool:cargo}", "build", "--release", "--locked"]
        count_literals = {
            "consumers": [],
            "readers": [],
            "holds": [],
        }
        if kind == "compiled_namespace_absence":
            count_literals["consumers"] = [
                "crate::domain::vnext",
                "maestro::domain::vnext",
                "src/domain/vnext/",
            ]
        elif kind == "generated_resource_absence":
            count_literals["consumers"] = [
                "embedded/vnext/",
                "maestro-work",
            ]
        elif kind == "migration_route_absence":
            count_literals["consumers"] = [
                "src/domain/vnext/migration/",
                "domain::vnext::migration",
            ]
        elif kind == "retained_reader_absence":
            count_literals["readers"] = [
                "ask-maestro",
                "maestro-audit",
                "maestro-card",
                "maestro-design",
                "maestro-research",
                "maestro-setup",
                "maestro-witness",
                "maestro-work",
            ]
        elif kind == "consumer_reader_hold_zero":
            count_literals = {
                "consumers": ["crate::domain::vnext", "maestro::domain::vnext"],
                "readers": ["v1-frozen-reader", "legacy-reader-route"],
                "holds": ["vnext-retention-hold", "legacy-retention-hold"],
            }
        rows.append(
            {
                "id": kind.replace("_", "-"),
                "kind": kind,
                "argv": argv,
                "expected_exit_code": 0,
                "command_identity": command_identity(argv, 0),
                "scan_roots": [
                    "source:src",
                    "source:embedded",
                    "target:release",
                ],
                "count_literals": count_literals,
                "expected_counts": {"consumers": 0, "readers": 0, "holds": 0},
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
    publication = args.publication_root.resolve()
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
    rows = verify_stage_chain(repository, final_commit, args.stage_checkpoint)
    pointer_path = publication / "current.json"
    pointer_preimage: dict[str, object]
    if pointer_path.exists():
        if pointer_path.is_symlink() or not pointer_path.is_file():
            raise GenerationError("final pointer is unsafe")
        pointer_preimage = {"state": "present", "sha256": digest(pointer_path.read_bytes())}
    else:
        pointer_preimage = {"state": "absent"}

    temporary = Path(tempfile.mkdtemp(prefix=".final-chain-generate-", dir=output.parent))
    try:
        source = temporary / "source"
        packet = temporary / "packet"
        control = temporary / "control"
        control.mkdir()
        archive_commit(repository, final_commit, source)
        packet_binding = verify_packet(packet_root, packet)
        manifest = input_manifest(source, final_commit, final_tree)
        ledger = build_ledger(manifest, final_commit)
        readback = readback_plan(manifest, final_commit)
        toolchain_value = toolchain(source, args.target, args.profile)
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
            "input_manifest": bound_file(
                control / "input-manifest.v1.json",
                "control/input-manifest.v1.json",
            ),
            "proof_ledger": bound_file(
                control / "proof-ledger.v1.json", "control/proof-ledger.v1.json"
            ),
            "stage12_readback": bound_file(
                control / "stage12-semantic-readback.v1.json",
                "control/stage12-semantic-readback.v1.json",
            ),
            "toolchain": bound_file(
                control / "toolchain.v1.json", "control/toolchain.v1.json"
            ),
            "environment_allowlist": ["HOME", "LANG", "LC_ALL", "PATH", "TMPDIR", "TZ"],
            "immutable_input_roots": ["source", "packet", "control"],
            "writable_root_roles": [
                f"{engine}-{role}"
                for engine in ENGINE_IDS
                for role in ("temp", "target", "deps", "output")
            ],
            "protected_primary_checkout": str(protected_primary),
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
