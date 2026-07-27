#!/usr/bin/env python3
"""Independent Python final-chain parser, executor, and semantic readback engine."""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path, PurePosixPath
from typing import Any, Mapping


ENGINES = ["python", "rust", "ruby"]
READBACK_KINDS = {
    "compiled_namespace_absence",
    "generated_resource_absence",
    "persisted_identity_parity",
    "canonical_facade_behavior",
    "migration_route_absence",
    "retained_reader_absence",
    "consumer_reader_hold_zero",
    "negative_fixture",
}


def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load(path: Path) -> dict[str, Any]:
    raw = path.read_bytes()
    if not raw.endswith(b"\n") or b"\r" in raw or raw.startswith(b"\xef\xbb\xbf"):
        raise ValueError(f"noncanonical JSON: {path}")
    value = json.loads(raw, object_pairs_hook=reject_duplicates)
    if not isinstance(value, dict):
        raise ValueError(f"object required: {path}")
    return value


def identity_bytes(raw: bytes) -> str:
    return "sha256:" + hashlib.sha256(raw).hexdigest()


def identity(path: Path) -> str:
    return identity_bytes(path.read_bytes())


def safe_path(root: Path, value: object) -> Path:
    if not isinstance(value, str) or not value or "\\" in value:
        raise ValueError("portable relative path required")
    relative = PurePosixPath(value)
    if relative.is_absolute() or any(part in {"", ".", ".."} for part in relative.parts):
        raise ValueError("path escapes source")
    return root.joinpath(*relative.parts)


def verify_binding(root: Path, binding: object) -> Path:
    if not isinstance(binding, Mapping):
        raise ValueError("file binding required")
    path = safe_path(root, binding.get("path"))
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"bound file absent or unsafe: {path}")
    raw = path.read_bytes()
    if binding.get("byte_length") != len(raw) or binding.get("sha256") != identity_bytes(raw):
        raise ValueError(f"bound file differs: {path}")
    return path


def validate_snapshot(snapshot: dict[str, Any], frozen_root: Path, source: Path) -> None:
    if snapshot.get("schema_version") != "maestro.external.vnext-final-cumulative-closure-snapshot.v1":
        raise ValueError("snapshot schema differs")
    if snapshot.get("state") != "frozen":
        raise ValueError("snapshot is not frozen")
    if snapshot.get("approved_packet_identity") != "sha256:2026513c84b1993f020f7d0430154ec0bc4e821438ccefd7dd6b91834a3d6283":
        raise ValueError("approved packet identity differs")
    stages = snapshot.get("first_parent_stages")
    if not isinstance(stages, list) or [row.get("stage") for row in stages] != list(range(13)):
        raise ValueError("Stage checkpoint closure differs")
    if stages[-1].get("commit") != snapshot.get("final_integration", {}).get("commit"):
        raise ValueError("current V4 Stage 12 checkpoint differs")
    for row in stages:
        checkpoint_path = verify_binding(frozen_root, row.get("checkpoint"))
        checkpoint = load(checkpoint_path)
        if any(checkpoint.get(field) != row.get(field) for field in ("stage", "commit", "tree")):
            raise ValueError("Stage checkpoint bytes differ")
    if snapshot.get("immutable_input_roots") != ["source", "packet", "control"]:
        raise ValueError("immutable roots differ")
    roles = snapshot.get("writable_root_roles")
    if not isinstance(roles, list) or len(roles) != 12 or len(set(roles)) != 12:
        raise ValueError("disjoint writable roots differ")
    if snapshot.get("sandbox_profile") != "macos-sandbox-exec-no-network-v1":
        raise ValueError("sandbox profile differs")
    if snapshot.get("environment_allowlist") != ["HOME", "LANG", "LC_ALL", "PATH", "TMPDIR", "TZ"]:
        raise ValueError("environment allowlist differs")
    if snapshot.get("cache_policy") != "immutable_compilation_and_dependency_bytes_only":
        raise ValueError("cache policy differs")
    if not isinstance(snapshot.get("pointer_preimage"), Mapping):
        raise ValueError("pointer preimage is absent")
    denied = snapshot.get("effect_denylist")
    if not isinstance(denied, list) or {"network", "protected_primary_checkout_write", "outside_packet_bound_roots_write"} - set(denied):
        raise ValueError("effect denylist differs")
    engines = snapshot.get("engines")
    if not isinstance(engines, list) or [row.get("id") for row in engines] != ENGINES:
        raise ValueError("engine closure differs")
    for row in engines:
        verify_binding(source, row.get("source"))
    for field in ("input_manifest", "proof_ledger", "stage12_readback", "toolchain"):
        verify_binding(frozen_root, snapshot.get(field))

def validate_packet(snapshot: dict[str, Any], packet_root: Path) -> None:
    binding = snapshot.get("packet_manifest")
    if not isinstance(binding, Mapping) or binding.get("path") != "packet/packet-manifest.v1.json":
        raise ValueError("packet-manifest binding differs")
    manifest_path = packet_root / "packet-manifest.v1.json"
    raw = manifest_path.read_bytes()
    if binding.get("byte_length") != len(raw) or binding.get("sha256") != identity_bytes(raw):
        raise ValueError("packet-manifest bytes differ")
    manifest = load(manifest_path)
    if manifest.get("schema_version") != "maestro.external.vnext-final-packet-manifest.v1":
        raise ValueError("packet manifest schema differs")
    if manifest.get("approved_packet_identity") != snapshot.get("approved_packet_identity"):
        raise ValueError("packet identity differs")
    rows = manifest.get("files")
    if not isinstance(rows, list) or not rows:
        raise ValueError("packet manifest is empty")
    seen = set()
    total = 0
    for row in rows:
        adjusted = dict(row)
        adjusted["path"] = str(row["path"]).removeprefix("packet/")
        verify_binding(packet_root, adjusted)
        seen.add(adjusted["path"])
        total += int(row["byte_length"])
    actual = {
        path.name
        for path in packet_root.iterdir()
        if path.is_file() and path.name != "packet-manifest.v1.json"
    }
    if actual != seen:
        raise ValueError("packet manifest has an omission")
    if manifest.get("file_count") != len(rows) or manifest.get("byte_length") != total:
        raise ValueError("packet manifest totals differ")


def validate_manifest(manifest: dict[str, Any], source: Path) -> None:
    if manifest.get("schema_version") != "maestro.external.vnext-final-input-manifest.v1":
        raise ValueError("input manifest schema differs")
    rows = manifest.get("entries")
    if not isinstance(rows, list) or not rows:
        raise ValueError("input manifest is empty")
    expected = {}
    byte_length = 0
    for row in rows:
        if not isinstance(row, Mapping):
            raise ValueError("input row is invalid")
        path = str(row.get("path"))
        if path in expected:
            raise ValueError("input manifest duplicates a path")
        verify_binding(source, row)
        expected[path] = row
        byte_length += int(row["byte_length"])
    actual = sorted(
        path.relative_to(source).as_posix()
        for path in source.rglob("*")
        if path.is_file()
    )
    if actual != sorted(expected):
        raise ValueError("input manifest has an omission or extra path")
    if manifest.get("entry_count") != len(rows) or manifest.get("byte_length") != byte_length:
        raise ValueError("input manifest totals differ")


def validate_toolchain(toolchain: dict[str, Any], source: Path) -> dict[str, str]:
    if toolchain.get("schema_version") != "maestro.external.vnext-final-toolchain.v1":
        raise ValueError("toolchain schema differs")
    tools = toolchain.get("tools")
    if not isinstance(tools, Mapping) or set(tools) != {"python", "rust", "ruby", "cargo", "git"}:
        raise ValueError("toolchain closure differs")
    if (
        not isinstance(toolchain.get("target"), str)
        or not isinstance(toolchain.get("profile"), str)
        or toolchain.get("environment") != {"LC_ALL": "C", "LANG": "C", "TZ": "UTC"}
    ):
        raise ValueError("toolchain target, profile, or environment differs")
    lockfiles = toolchain.get("lockfiles")
    if not isinstance(lockfiles, list) or not lockfiles:
        raise ValueError("lockfile closure is absent")
    for lockfile in lockfiles:
        verify_binding(source, lockfile)
    resolved = {}
    for name, row in tools.items():
        if not isinstance(row, Mapping):
            raise ValueError("tool row is invalid")
        path = Path(str(row.get("resolved_path")))
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"tool is absent or unsafe: {name}")
        raw = path.read_bytes()
        if row.get("byte_length") != len(raw) or row.get("sha256") != identity_bytes(raw):
            raise ValueError(f"tool bytes differ: {name}")
        probe = subprocess.run(row["probe_argv"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if (
            probe.returncode != row.get("probe_exit_code")
            or {"byte_length": len(probe.stdout), "sha256": identity_bytes(probe.stdout)}
            != row.get("probe_stdout")
            or {"byte_length": len(probe.stderr), "sha256": identity_bytes(probe.stderr)}
            != row.get("probe_stderr")
        ):
            raise ValueError(f"tool probe differs: {name}")
        resolved[str(name)] = str(path)
    dependencies = toolchain.get("dependency_outputs")
    if not isinstance(dependencies, list) or not dependencies:
        raise ValueError("dependency-output closure is absent")
    for dependency in dependencies:
        if not isinstance(dependency, Mapping):
            raise ValueError("dependency-output row is invalid")
        root = Path(str(dependency.get("resolved_path")))
        if root.is_symlink() or not root.is_dir():
            raise ValueError("dependency-output root is absent or unsafe")
        rows = []
        for row in dependency.get("files", []):
            path = safe_path(root, row.get("path"))
            if path.is_symlink() or not path.is_file():
                raise ValueError("dependency-output file is absent or unsafe")
            raw = path.read_bytes()
            actual = {
                "path": row["path"],
                "byte_length": len(raw),
                "sha256": identity_bytes(raw),
            }
            if actual != row:
                raise ValueError("dependency-output bytes differ")
            rows.append(actual)
        actual_paths = {
            path.relative_to(root).as_posix()
            for path in root.rglob("*")
            if path.is_file()
        }
        if actual_paths != {row["path"] for row in rows}:
            raise ValueError("dependency-output manifest has an omission")
        canonical = (
            json.dumps(rows, sort_keys=True, separators=(",", ":")) + "\n"
        ).encode("ascii")
        if (
            dependency.get("file_count") != len(rows)
            or dependency.get("byte_length")
            != sum(row["byte_length"] for row in rows)
            or dependency.get("identity") != identity_bytes(canonical)
        ):
            raise ValueError("dependency-output identity differs")
    return resolved


def validate_ledger(ledger: dict[str, Any], source: Path) -> list[dict[str, Any]]:
    if ledger.get("schema_version") != "maestro.external.vnext-final-proof-ledger.v1":
        raise ValueError("ledger schema differs")
    rows = ledger.get("proofs")
    if not isinstance(rows, list) or ledger.get("proof_count") != len(rows):
        raise ValueError("ledger count differs")
    identifiers = set()
    stages = set()
    kinds = set()
    for row in rows:
        if not isinstance(row, dict) or row.get("proof_id") in identifiers:
            raise ValueError("proof row or identifier differs")
        identifiers.add(row["proof_id"])
        stages.add(row.get("stage"))
        kinds.add(row.get("kind"))
        if row.get("engines") != ENGINES:
            raise ValueError("proof engine coverage differs")
        command = row.get("command")
        if not isinstance(command, Mapping):
            raise ValueError("proof command absent")
        expected_identity = identity_bytes(
            (
                json.dumps(
                    {
                        "argv": command.get("argv"),
                        "expected_exit_code": command.get("expected_exit_code"),
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n"
            ).encode("ascii")
        )
        if command.get("identity") != expected_identity:
            raise ValueError("proof command identity differs")
        for binding in row.get("input_bindings", []):
            verify_binding(source, binding)
        if row.get("kind") in {"race", "crash_replay"}:
            verify_binding(source, command.get("fault_schedule"))
        if row.get("kind") in {"migration", "rollback"}:
            cohort = command.get("cohort")
            if not isinstance(cohort, Mapping):
                raise ValueError("migration cohort absent")
            verify_binding(source, cohort.get("fixture"))
    if stages != set(range(13)) or len(kinds) != 13:
        raise ValueError("proof Stage or kind closure differs")
    return rows


def expand(argv: list[str], tools: Mapping[str, str], source: Path) -> list[str]:
    result = []
    for value in argv:
        if value == "{source}":
            result.append(str(source))
        elif value.startswith("{tool:") and value.endswith("}"):
            name = value[6:-1]
            if name not in tools:
                raise ValueError(f"unknown frozen tool: {name}")
            result.append(tools[name])
        elif "{" in value or "}" in value:
            raise ValueError(f"unknown command placeholder: {value}")
        else:
            result.append(value)
    return result


def produced_artifacts(source: Path, paths: object) -> list[dict[str, object]]:
    if not isinstance(paths, list):
        raise ValueError("produced artifact list is invalid")
    rows = []
    for value in paths:
        path = safe_path(source, value)
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"declared produced artifact is absent: {path}")
        raw = path.read_bytes()
        rows.append({"path": value, "byte_length": len(raw), "sha256": identity_bytes(raw)})
    return rows


def execute_proof(row: dict[str, Any], source: Path, tools: Mapping[str, str]) -> dict[str, Any]:
    spec = row["command"]
    argv = expand(spec["argv"], tools, source)
    result = subprocess.run(argv, cwd=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    passed = result.returncode == spec["expected_exit_code"]
    expected = row["expected_outcome"]
    receipt = {
        "proof_id": row["proof_id"],
        "stage": row["stage"],
        "kind": row["kind"],
        "command_identity": spec["identity"],
        "expected_outcome": expected,
        "actual_outcome": expected if passed else "error",
        "exit_code": result.returncode,
        "stdout": {"byte_length": len(result.stdout), "sha256": identity_bytes(result.stdout)},
        "stderr": {"byte_length": len(result.stderr), "sha256": identity_bytes(result.stderr)},
        "produced_artifacts": produced_artifacts(source, row["produced_artifacts"]),
    }
    if row["kind"] in {"race", "crash_replay"}:
        schedule_path = verify_binding(source, spec["fault_schedule"])
        schedule = load(schedule_path)
        schedules = schedule.get("schedules")
        if not isinstance(schedules, list) or not schedules:
            raise ValueError("fault schedule is empty")
        receipt["fault_schedule_identity"] = identity(schedule_path)
        receipt["injection_points_reached"] = [
            schedule["id"] for schedule in schedules
        ]
    if row["kind"] in {"migration", "rollback"}:
        cohort = spec["cohort"]
        receipt["cohort_identity"] = identity_bytes(
            (
                json.dumps(cohort, sort_keys=True, separators=(",", ":")) + "\n"
            ).encode("ascii")
        )
    return receipt


def scan_counts(check: Mapping[str, Any], source: Path) -> dict[str, int]:
    target = Path(os.environ.get("CARGO_TARGET_DIR", "")) / "release"
    files: set[Path] = set()
    for root in check.get("scan_roots", []):
        if root == "source:src":
            files.update(path for path in (source / "src").rglob("*") if path.is_file())
        elif root == "source:embedded":
            files.update(
                path for path in (source / "embedded").rglob("*") if path.is_file()
            )
        elif root == "target:release" and target.is_dir():
            files.update(path for path in target.glob("maestro*") if path.is_file())
        else:
            raise ValueError(f"unknown or unavailable semantic scan root: {root}")
    counts = {}
    literals = check.get("count_literals")
    if not isinstance(literals, Mapping):
        raise ValueError("semantic count literals are absent")
    for label in ("consumers", "readers", "holds"):
        values = literals.get(label)
        if not isinstance(values, list):
            raise ValueError("semantic count literal class differs")
        count = 0
        for path in files:
            raw = path.read_bytes()
            relative = (
                path.relative_to(source).as_posix()
                if path.is_relative_to(source)
                else path.name
            ).encode("utf-8")
            count += sum(
                1
                for value in values
                if value.encode("utf-8") in raw or value.encode("utf-8") in relative
            )
        counts[label] = count
    return counts


def semantic_readback(plan: dict[str, Any], source: Path, tools: Mapping[str, str]) -> dict[str, Any]:
    if plan.get("schema_version") != "maestro.external.vnext-stage12-semantic-readback-plan.v1":
        raise ValueError("readback schema differs")
    checks = plan.get("checks")
    if not isinstance(checks, list) or {row.get("kind") for row in checks} != READBACK_KINDS:
        raise ValueError("readback closure differs")
    rows = []
    for check in checks:
        expected_identity = identity_bytes(
            (
                json.dumps(
                    {
                        "argv": check.get("argv"),
                        "expected_exit_code": check.get("expected_exit_code"),
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n"
            ).encode("ascii")
        )
        if check.get("command_identity") != expected_identity:
            raise ValueError("readback command identity differs")
        result = subprocess.run(
            expand(check["argv"], tools, source),
            cwd=source,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        expected_counts = check.get("expected_counts")
        if expected_counts != {"consumers": 0, "readers": 0, "holds": 0}:
            raise ValueError("readback zero-count contract differs")
        counts = scan_counts(check, source)
        passed = (
            result.returncode == check["expected_exit_code"]
            and counts == expected_counts
        )
        rows.append(
            {
                "id": check["id"],
                "kind": check["kind"],
                "command_identity": check["command_identity"],
                "exit_code": result.returncode,
                "status": "pass" if passed else "fail",
                "consumer_count": counts["consumers"],
                "reader_count": counts["readers"],
                "hold_count": counts["holds"],
            }
        )
    return {
        "status": "pass" if all(row["status"] == "pass" for row in rows) else "fail",
        "consumer_count": max(row["consumer_count"] for row in rows),
        "reader_count": max(row["reader_count"] for row in rows),
        "hold_count": max(row["hold_count"] for row in rows),
        "checks": rows,
    }


def main() -> int:
    if len(sys.argv) != 9:
        raise ValueError("expected snapshot manifest ledger readback toolchain packet source output")
    snapshot_path, manifest_path, ledger_path, readback_path, toolchain_path, packet_path, source_path, output_path = map(Path, sys.argv[1:])
    snapshot = load(snapshot_path)
    manifest = load(manifest_path)
    ledger = load(ledger_path)
    readback = load(readback_path)
    toolchain = load(toolchain_path)
    validate_snapshot(snapshot, snapshot_path.parent.parent, source_path)
    validate_packet(snapshot, packet_path)
    validate_manifest(manifest, source_path)
    tools = validate_toolchain(toolchain, source_path)
    proofs = [execute_proof(row, source_path, tools) for row in validate_ledger(ledger, source_path)]
    value = {
        "schema_version": "maestro.external.vnext-final-engine-ledger.v1",
        "engine": "python",
        "snapshot_identity": identity(snapshot_path),
        "input_manifest_identity": identity(manifest_path),
        "ledger_identity": identity(ledger_path),
        "readback_plan_identity": identity(readback_path),
        "toolchain_identity": identity(toolchain_path),
        "proofs": proofs,
        "semantic_readback": semantic_readback(readback, source_path, tools),
    }
    output_path.write_bytes(
        (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"python final-chain engine refused: {error}", file=sys.stderr)
        raise SystemExit(2)
