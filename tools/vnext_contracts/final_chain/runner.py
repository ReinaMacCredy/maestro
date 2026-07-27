#!/usr/bin/env python3
"""Run and publish one V4 seal from a previously frozen closure."""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import shutil
import stat
import subprocess
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any, Mapping


ENGINE_IDS = ("python", "rust", "ruby")
SCHEMA = "maestro.external.vnext-final-cumulative-seal-receipt.v1"
POINTER_SCHEMA = "maestro.external.vnext-final-cumulative-seal-pointer.v1"
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


class FinalChainError(RuntimeError):
    pass


def canonical_bytes(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
        + "\n"
    ).encode("ascii")


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            raise FinalChainError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path: Path) -> dict[str, Any]:
    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.startswith(b"\xef\xbb\xbf") or b"\r" in raw:
        raise FinalChainError(f"noncanonical JSON: {path}")
    try:
        value = json.loads(raw, object_pairs_hook=reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise FinalChainError(f"invalid JSON {path}: {error}") from error
    if not isinstance(value, dict):
        raise FinalChainError(f"JSON object required: {path}")
    return value


def safe_relative(value: object) -> PurePosixPath:
    if not isinstance(value, str) or not value or "\\" in value:
        raise FinalChainError("portable relative path required")
    path = PurePosixPath(value)
    if path.is_absolute() or any(part in {"", ".", ".."} for part in path.parts):
        raise FinalChainError(f"unsafe relative path: {value!r}")
    return path


def bound_file(root: Path, binding: object, label: str) -> Path:
    if not isinstance(binding, Mapping):
        raise FinalChainError(f"{label} binding is absent")
    path = root.joinpath(*safe_relative(binding.get("path")).parts)
    if path.is_symlink() or not path.is_file():
        raise FinalChainError(f"{label} is absent or unsafe: {path}")
    raw = path.read_bytes()
    if binding.get("byte_length") != len(raw) or binding.get("sha256") != digest(raw):
        raise FinalChainError(f"{label} bytes differ: {path}")
    return path


def ensure_readonly_tree(root: Path) -> None:
    if root.is_symlink() or not root.is_dir():
        raise FinalChainError(f"immutable root is absent or unsafe: {root}")
    for path in root.rglob("*"):
        if path.is_symlink():
            raise FinalChainError(f"immutable root contains a symlink: {path}")
        if path.stat().st_mode & 0o222:
            raise FinalChainError(f"immutable root remains writable: {path}")


def validate_packet(closure: Path, binding: object) -> None:
    manifest_path = bound_file(closure, binding, "packet manifest")
    manifest = read_json(manifest_path)
    if manifest.get("schema_version") != "maestro.external.vnext-final-packet-manifest.v1":
        raise FinalChainError("packet manifest schema differs")
    if manifest.get("approved_packet_identity") != (
        "sha256:2026513c84b1993f020f7d0430154ec0bc4e821438ccefd7dd6b91834a3d6283"
    ):
        raise FinalChainError("packet identity differs")
    rows = manifest.get("files")
    if not isinstance(rows, list) or not rows:
        raise FinalChainError("packet manifest is empty")
    seen = set()
    total = 0
    for row in rows:
        path = bound_file(closure, row, "packet artifact")
        if path.name in seen:
            raise FinalChainError("packet manifest duplicates a file")
        seen.add(path.name)
        total += int(row["byte_length"])
    required = {
        "replacement-build-approval-packet.v4.json",
        "proof-inputs.v4.json",
        "fanout-manifest.v4.json",
        "integration-ancestry.v4.txt",
        "external-build-plan-handoff.v4.json",
        "independent-verification.v4.json",
    }
    if not required.issubset(seen):
        raise FinalChainError("packet manifest omits a required V4 artifact")
    if manifest.get("file_count") != len(rows) or manifest.get("byte_length") != total:
        raise FinalChainError("packet manifest totals differ")
    actual = {
        path.name
        for path in (closure / "packet").iterdir()
        if path.is_file() and path.name != "packet-manifest.v1.json"
    }
    if actual != seen:
        raise FinalChainError("packet directory differs from its byte-total manifest")


def validate_manifest(path: Path, source: Path, commit: str, tree: str) -> dict[str, Any]:
    manifest = read_json(path)
    if manifest.get("schema_version") != "maestro.external.vnext-final-input-manifest.v1":
        raise FinalChainError("input manifest schema differs")
    if manifest.get("commit") != commit or manifest.get("tree") != tree:
        raise FinalChainError("input manifest binds another final integration")
    rows = manifest.get("entries")
    if not isinstance(rows, list) or not rows:
        raise FinalChainError("input manifest is empty")
    seen = set()
    total = 0
    for row in rows:
        if not isinstance(row, Mapping):
            raise FinalChainError("input manifest row is invalid")
        path = str(row.get("path"))
        if path in seen:
            raise FinalChainError("input manifest duplicates a path")
        seen.add(path)
        bound_file(source, row, "input manifest row")
        total += int(row["byte_length"])
    actual = {
        path.relative_to(source).as_posix()
        for path in source.rglob("*")
        if path.is_file()
    }
    if actual != seen:
        raise FinalChainError("input manifest has an omission or extra path")
    if manifest.get("entry_count") != len(rows) or manifest.get("byte_length") != total:
        raise FinalChainError("input manifest totals differ")
    return manifest


def stream_identity(raw: bytes) -> dict[str, object]:
    return {"byte_length": len(raw), "sha256": digest(raw)}


def validate_toolchain(path: Path, source: Path | None = None) -> dict[str, Any]:
    value = read_json(path)
    if value.get("schema_version") != "maestro.external.vnext-final-toolchain.v1":
        raise FinalChainError("toolchain schema differs")
    tools = value.get("tools")
    if not isinstance(tools, Mapping) or set(tools) != {
        "python",
        "rust",
        "ruby",
        "cargo",
        "git",
    }:
        raise FinalChainError("toolchain closure differs")
    for name, row in tools.items():
        if not isinstance(row, Mapping):
            raise FinalChainError(f"tool row is invalid: {name}")
        executable = Path(str(row.get("resolved_path")))
        if executable.is_symlink() or not executable.is_file():
            raise FinalChainError(f"tool is absent or unsafe: {name}")
        raw = executable.read_bytes()
        if row.get("byte_length") != len(raw) or row.get("sha256") != digest(raw):
            raise FinalChainError(f"tool bytes differ: {name}")
        result = subprocess.run(
            row["probe_argv"], stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        if (
            result.returncode != row.get("probe_exit_code")
            or stream_identity(result.stdout) != row.get("probe_stdout")
            or stream_identity(result.stderr) != row.get("probe_stderr")
        ):
            raise FinalChainError(f"tool probe differs: {name}")
    if value.get("environment") != {"LC_ALL": "C", "LANG": "C", "TZ": "UTC"}:
        raise FinalChainError("toolchain environment differs")
    if not isinstance(value.get("target"), str) or not isinstance(value.get("profile"), str):
        raise FinalChainError("toolchain target or profile is absent")
    lockfiles = value.get("lockfiles")
    if not isinstance(lockfiles, list) or not lockfiles:
        raise FinalChainError("lockfile closure is absent")
    if source is not None:
        for lockfile in lockfiles:
            bound_file(source, lockfile, "toolchain lockfile")
    dependencies = value.get("dependency_outputs")
    if not isinstance(dependencies, list) or not dependencies:
        raise FinalChainError("dependency-output closure is absent")
    for dependency in dependencies:
        if not isinstance(dependency, Mapping):
            raise FinalChainError("dependency-output row is invalid")
        root = Path(str(dependency.get("resolved_path")))
        if root.is_symlink() or not root.is_dir():
            raise FinalChainError("dependency-output root is absent or unsafe")
        expected = dependency.get("files")
        if not isinstance(expected, list) or not expected:
            raise FinalChainError("dependency-output file closure is empty")
        rows = []
        for row in expected:
            relative = safe_relative(row.get("path"))
            candidate = root.joinpath(*relative.parts)
            if candidate.is_symlink() or not candidate.is_file():
                raise FinalChainError("dependency-output file is absent or unsafe")
            raw = candidate.read_bytes()
            actual = {
                "path": str(row["path"]),
                "byte_length": len(raw),
                "sha256": digest(raw),
            }
            if actual != row:
                raise FinalChainError("dependency-output bytes differ")
            rows.append(actual)
        actual_paths = {
            item.relative_to(root).as_posix()
            for item in root.rglob("*")
            if item.is_file()
        }
        if actual_paths != {str(row["path"]) for row in rows}:
            raise FinalChainError("dependency-output manifest has an omission")
        if (
            dependency.get("file_count") != len(rows)
            or dependency.get("byte_length")
            != sum(int(row["byte_length"]) for row in rows)
            or dependency.get("identity") != digest(canonical_bytes(rows))
        ):
            raise FinalChainError("dependency-output identity differs")
    return value


def validate_ledger(path: Path, source: Path, commit: str) -> list[dict[str, Any]]:
    ledger = read_json(path)
    if ledger.get("schema_version") != "maestro.external.vnext-final-proof-ledger.v1":
        raise FinalChainError("proof ledger schema differs")
    if ledger.get("snapshot_commit") != commit:
        raise FinalChainError("proof ledger binds another final commit")
    rows = ledger.get("proofs")
    if not isinstance(rows, list) or ledger.get("proof_count") != len(rows):
        raise FinalChainError("proof ledger count differs")
    ids = set()
    stages = set()
    kinds = set()
    for row in rows:
        if not isinstance(row, dict) or row.get("proof_id") in ids:
            raise FinalChainError("proof row or identifier differs")
        ids.add(row["proof_id"])
        stages.add(row.get("stage"))
        kinds.add(row.get("kind"))
        if row.get("engines") != list(ENGINE_IDS):
            raise FinalChainError("proof engine coverage differs")
        for binding in row.get("input_bindings", []):
            bound_file(source, binding, "proof input")
        command = row.get("command")
        if not isinstance(command, Mapping):
            raise FinalChainError("proof command is absent")
        identity = digest(
            canonical_bytes(
                {
                    "argv": command.get("argv"),
                    "expected_exit_code": command.get("expected_exit_code"),
                }
            )
        )
        if command.get("identity") != identity:
            raise FinalChainError("proof command identity differs")
        if row.get("kind") in {"race", "crash_replay"}:
            bound_file(source, command.get("fault_schedule"), "fault schedule")
        if row.get("kind") in {"migration", "rollback"}:
            cohort = command.get("cohort")
            if not isinstance(cohort, Mapping):
                raise FinalChainError("migration cohort is absent")
            bound_file(source, cohort.get("fixture"), "migration cohort fixture")
    if stages != set(range(13)) or len(kinds) != 13:
        raise FinalChainError("proof Stage or kind closure differs")
    return rows


def validate_readback(path: Path, commit: str) -> dict[str, Any]:
    value = read_json(path)
    if value.get("schema_version") != "maestro.external.vnext-stage12-semantic-readback-plan.v1":
        raise FinalChainError("readback schema differs")
    if value.get("snapshot_commit") != commit:
        raise FinalChainError("readback binds another final commit")
    checks = value.get("checks")
    required = {
        "compiled_namespace_absence",
        "generated_resource_absence",
        "persisted_identity_parity",
        "canonical_facade_behavior",
        "migration_route_absence",
        "retained_reader_absence",
        "consumer_reader_hold_zero",
        "negative_fixture",
    }
    if not isinstance(checks, list) or {row.get("kind") for row in checks} != required:
        raise FinalChainError("semantic readback closure differs")
    for check in checks:
        if check.get("expected_counts") != {"consumers": 0, "readers": 0, "holds": 0}:
            raise FinalChainError("semantic readback does not require exact zero counts")
        identity = digest(
            canonical_bytes(
                {
                    "argv": check.get("argv"),
                    "expected_exit_code": check.get("expected_exit_code"),
                }
            )
        )
        if check.get("command_identity") != identity:
            raise FinalChainError("readback command identity differs")
        roots = check.get("scan_roots")
        if not isinstance(roots, list) or set(roots) != {
            "source:src",
            "source:embedded",
            "target:release",
        }:
            raise FinalChainError("readback scan-root closure differs")
        literals = check.get("count_literals")
        if not isinstance(literals, Mapping) or set(literals) != {
            "consumers",
            "readers",
            "holds",
        }:
            raise FinalChainError("readback count-literal closure differs")
    return value


def validate_snapshot(closure: Path) -> tuple[dict[str, Any], dict[str, Path]]:
    snapshot_path = closure / "control/final-cumulative-closure-snapshot.v1.json"
    snapshot = read_json(snapshot_path)
    if snapshot.get("schema_version") != "maestro.external.vnext-final-cumulative-closure-snapshot.v1":
        raise FinalChainError("snapshot schema differs")
    if snapshot.get("state") != "frozen":
        raise FinalChainError("snapshot is not frozen")
    if snapshot.get("approved_packet_identity") != (
        "sha256:2026513c84b1993f020f7d0430154ec0bc4e821438ccefd7dd6b91834a3d6283"
    ):
        raise FinalChainError("snapshot packet identity differs")
    final = snapshot.get("final_integration")
    if not isinstance(final, Mapping):
        raise FinalChainError("final integration identity is absent")
    commit = str(final.get("commit"))
    tree = str(final.get("tree"))
    stages = snapshot.get("first_parent_stages")
    if not isinstance(stages, list) or [row.get("stage") for row in stages] != list(range(13)):
        raise FinalChainError("Stage checkpoint closure differs")
    if stages[-1].get("commit") != commit or stages[-1].get("tree") != tree:
        raise FinalChainError("current V4 Stage 12 checkpoint differs")
    for row in stages:
        checkpoint = bound_file(closure, row.get("checkpoint"), "Stage checkpoint")
        value = read_json(checkpoint)
        if value.get("stage") != row.get("stage") or value.get("commit") != row.get("commit") or value.get("tree") != row.get("tree"):
            raise FinalChainError("Stage checkpoint bytes differ from snapshot")
    if snapshot.get("immutable_input_roots") != ["source", "packet", "control"]:
        raise FinalChainError("immutable root roles differ")
    if snapshot.get("environment_allowlist") != [
        "HOME",
        "LANG",
        "LC_ALL",
        "PATH",
        "TMPDIR",
        "TZ",
    ]:
        raise FinalChainError("environment allowlist differs")
    roles = snapshot.get("writable_root_roles")
    if not isinstance(roles, list) or len(roles) != 12 or len(set(roles)) != 12:
        raise FinalChainError("writable-root roles are not disjoint")
    if snapshot.get("cache_policy") != "immutable_compilation_and_dependency_bytes_only":
        raise FinalChainError("cache policy admits verdict reuse")
    if snapshot.get("sandbox_profile") != "macos-sandbox-exec-no-network-v1":
        raise FinalChainError("sandbox profile differs")
    if snapshot.get("effect_denylist") != EFFECT_DENYLIST:
        raise FinalChainError("effect denylist differs")
    protected_primary = snapshot.get("protected_primary_checkout")
    if not isinstance(protected_primary, str) or not Path(protected_primary).is_absolute():
        raise FinalChainError("protected primary identity is absent")
    if not isinstance(snapshot.get("pointer_preimage"), Mapping):
        raise FinalChainError("pointer preimage is absent")
    engines = snapshot.get("engines")
    if not isinstance(engines, list) or [row.get("id") for row in engines] != list(ENGINE_IDS):
        raise FinalChainError("engine closure differs")
    for row in engines:
        bound_file(closure / "source", row.get("source"), "engine source")
    validate_packet(closure, snapshot.get("packet_manifest"))
    paths = {
        "snapshot": snapshot_path,
        "packet_manifest": bound_file(
            closure, snapshot.get("packet_manifest"), "packet manifest"
        ),
        "manifest": bound_file(closure, snapshot.get("input_manifest"), "input manifest"),
        "ledger": bound_file(closure, snapshot.get("proof_ledger"), "proof ledger"),
        "readback": bound_file(closure, snapshot.get("stage12_readback"), "readback plan"),
        "toolchain": bound_file(closure, snapshot.get("toolchain"), "toolchain"),
    }
    validate_manifest(paths["manifest"], closure / "source", commit, tree)
    validate_toolchain(paths["toolchain"], closure / "source")
    validate_ledger(paths["ledger"], closure / "source", commit)
    validate_readback(paths["readback"], commit)
    for root_name in ("source", "packet", "control"):
        ensure_readonly_tree(closure / root_name)
    return snapshot, paths


def copy_source(source: Path, destination: Path) -> None:
    shutil.copytree(source, destination, symlinks=False, copy_function=shutil.copyfile)
    for path in destination.rglob("*"):
        if path.is_symlink():
            raise FinalChainError(f"engine source contains a symlink: {path}")
        path.chmod(
            stat.S_IRUSR
            | (stat.S_IXUSR if path.is_dir() or os.access(path, os.X_OK) else 0)
        )
    destination.chmod(stat.S_IRUSR | stat.S_IXUSR)


def sandbox_profile(writable_root: Path) -> str:
    escaped = str(writable_root).replace("\\", "\\\\").replace('"', '\\"')
    return (
        '(version 1)\n'
        '(deny default)\n'
        '(allow process*)\n'
        '(allow sysctl-read)\n'
        '(allow file-read*)\n'
        f'(allow file-write* (subpath "{escaped}"))\n'
        '(deny network*)\n'
    )


def verify_sandbox(sandbox_exec: Path, run_root: Path, protected_primary: Path) -> None:
    if sandbox_exec != Path("/usr/bin/sandbox-exec") or not sandbox_exec.is_file():
        raise FinalChainError("required no-network sandbox is unavailable")
    probe_root = run_root / "sandbox-probe"
    probe_root.mkdir()
    profile = probe_root / "profile.sb"
    profile.write_text(sandbox_profile(probe_root), encoding="utf-8")
    network = subprocess.run(
        [
            str(sandbox_exec),
            "-f",
            str(profile),
            "/usr/bin/python3",
            "-c",
            (
                "import errno,socket,sys\n"
                "try:\n"
                " s=socket.socket(); s.bind(('127.0.0.1',0))\n"
                "except OSError as e:\n"
                " sys.exit(0 if e.errno in (errno.EPERM,errno.EACCES) else 3)\n"
                "sys.exit(4)\n"
            ),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if network.returncode != 0:
        raise FinalChainError("sandbox network denial probe was not a policy denial")
    outside = protected_primary / ".final-chain-sandbox-write-probe"
    write = subprocess.run(
        [str(sandbox_exec), "-f", str(profile), "/usr/bin/touch", str(outside)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if write.returncode == 0 or outside.exists():
        raise FinalChainError("sandbox protected-primary write denial probe failed")


def engine_environment(root: Path, toolchain: Mapping[str, Any]) -> dict[str, str]:
    tools = toolchain["tools"]
    path = os.pathsep.join(
        sorted({str(Path(row["resolved_path"]).parent) for row in tools.values()})
    )
    return {
        "HOME": str(root / "temp/home"),
        "TMPDIR": str(root / "temp"),
        "CARGO_HOME": str(root / "deps/cargo-home"),
        "CARGO_TARGET_DIR": str(root / "target"),
        "PATH": path,
        "LC_ALL": "C",
        "LANG": "C",
        "TZ": "UTC",
    }


def run_engine(
    engine: str,
    closure: Path,
    paths: Mapping[str, Path],
    run_root: Path,
    sandbox_exec: Path,
    toolchain: Mapping[str, Any],
) -> dict[str, Any]:
    root = run_root / engine
    for role in ("temp", "target", "deps", "output"):
        (root / role).mkdir(parents=True, exist_ok=False)
    source = root / "source"
    copy_source(closure / "source", source)
    packet = root / "packet"
    copy_source(closure / "packet", packet)
    controls = root / "control"
    controls.mkdir()
    copied = {}
    for name, path in paths.items():
        target = controls / path.name
        shutil.copyfile(path, target)
        target.chmod(stat.S_IRUSR)
        copied[name] = target
    profile = root / "sandbox.sb"
    profile.write_text(sandbox_profile(root), encoding="utf-8")
    profile.chmod(stat.S_IRUSR)
    engine_rows = {
        row["id"]: row for row in read_json(paths["snapshot"])["engines"]
    }
    source_binding = engine_rows[engine]["source"]
    engine_source = bound_file(source, source_binding, f"{engine} engine")
    output = root / "output/engine-ledger.v1.json"
    if engine == "python":
        executable = toolchain["tools"]["python"]["resolved_path"]
        command = [executable, str(engine_source)]
    elif engine == "ruby":
        executable = toolchain["tools"]["ruby"]["resolved_path"]
        command = [executable, str(engine_source)]
    else:
        executable = toolchain["tools"]["rust"]["resolved_path"]
        binary = root / "target/rust-final-chain-engine"
        compile_result = subprocess.run(
            [
                str(sandbox_exec),
                "-f",
                str(profile),
                executable,
                "--edition=2021",
                str(engine_source),
                "-O",
                "-o",
                str(binary),
            ],
            cwd=source,
            env=engine_environment(root, toolchain),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if compile_result.returncode != 0:
            raise FinalChainError(
                f"Rust engine compilation failed: "
                f"{compile_result.stderr.decode('utf-8', 'replace')}"
            )
        command = [str(binary)]
    result = subprocess.run(
        [
            str(sandbox_exec),
            "-f",
            str(profile),
            *command,
            str(copied["snapshot"]),
            str(copied["manifest"]),
            str(copied["ledger"]),
            str(copied["readback"]),
            str(copied["toolchain"]),
            str(packet),
            str(source),
            str(output),
        ],
        cwd=source,
        env=engine_environment(root, toolchain),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise FinalChainError(
            f"{engine} engine refused: "
            f"{(result.stderr or result.stdout).decode('utf-8', 'replace')}"
        )
    value = read_json(output)
    raw = output.read_bytes()
    value["_engine_ledger_file"] = {
        "byte_length": len(raw),
        "sha256": digest(raw),
    }
    value["_produced_roots"] = [
        produced_root_identity(root / role, f"{engine}-{role}")
        for role in ("target", "deps", "output")
    ]
    return value


def produced_root_identity(root: Path, role: str) -> dict[str, object]:
    rows = []
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        if path.is_symlink():
            raise FinalChainError(f"produced root contains a symlink: {path}")
        raw = path.read_bytes()
        rows.append(
            {
                "path": path.relative_to(root).as_posix(),
                "byte_length": len(raw),
                "sha256": digest(raw),
            }
        )
    return {
        "role": role,
        "file_count": len(rows),
        "byte_length": sum(int(row["byte_length"]) for row in rows),
        "identity": digest(canonical_bytes(rows)),
    }


def consensus(
    snapshot: Mapping[str, Any],
    paths: Mapping[str, Path],
    proofs: list[dict[str, Any]],
    receipts: list[dict[str, Any]],
) -> dict[str, Any]:
    by_engine = {receipt.get("engine"): receipt for receipt in receipts}
    if set(by_engine) != set(ENGINE_IDS):
        raise FinalChainError("engine ledgers are not one per independent engine")
    identities = {
        "snapshot_identity": digest(paths["snapshot"].read_bytes()),
        "input_manifest_identity": digest(paths["manifest"].read_bytes()),
        "ledger_identity": digest(paths["ledger"].read_bytes()),
        "readback_plan_identity": digest(paths["readback"].read_bytes()),
        "toolchain_identity": digest(paths["toolchain"].read_bytes()),
    }
    expected_ids = [row["proof_id"] for row in proofs]
    consensus_rows = []
    engine_ledgers = []
    produced = []
    for engine in ENGINE_IDS:
        receipt = by_engine[engine]
        ledger_file = receipt.pop("_engine_ledger_file", None)
        produced_roots = receipt.pop("_produced_roots", None)
        if not isinstance(ledger_file, Mapping):
            raise FinalChainError(f"{engine} engine-ledger file identity is absent")
        if not isinstance(produced_roots, list) or len(produced_roots) != 3:
            raise FinalChainError(f"{engine} produced-root identity is absent")
        produced.extend(produced_roots)
        if any(receipt.get(key) != value for key, value in identities.items()):
            raise FinalChainError(f"{engine} binds another frozen control input")
        rows = receipt.get("proofs")
        if not isinstance(rows, list) or [row.get("proof_id") for row in rows] != expected_ids:
            raise FinalChainError(f"{engine} proof rows differ")
        if receipt.get("semantic_readback", {}).get("status") != "pass":
            raise FinalChainError(f"{engine} semantic readback failed")
        counts = receipt["semantic_readback"]
        if (
            counts.get("consumer_count"),
            counts.get("reader_count"),
            counts.get("hold_count"),
        ) != (0, 0, 0):
            raise FinalChainError(f"{engine} semantic zero counts differ")
        engine_ledgers.append(
            {
                "engine": engine,
                "byte_length": ledger_file["byte_length"],
                "sha256": ledger_file["sha256"],
            }
        )
    for index, proof in enumerate(proofs):
        rows = [by_engine[engine]["proofs"][index] for engine in ENGINE_IDS]
        typed = [
            {
                key: row.get(key)
                for key in (
                    "proof_id",
                    "stage",
                    "kind",
                    "command_identity",
                    "expected_outcome",
                    "actual_outcome",
                    "exit_code",
                    "produced_artifacts",
                    "fault_schedule_identity",
                    "injection_points_reached",
                    "cohort_identity",
                )
            }
            for row in rows
        ]
        if typed[1:] != typed[:-1]:
            raise FinalChainError(f"typed row consensus differs: {proof['proof_id']}")
        if typed[0]["actual_outcome"] != proof["expected_outcome"]:
            raise FinalChainError(f"proof outcome differs: {proof['proof_id']}")
        if proof["kind"] in {"race", "crash_replay"} and not typed[0][
            "injection_points_reached"
        ]:
            raise FinalChainError(
                f"fault schedule reachability is absent: {proof['proof_id']}"
            )
        if proof["kind"] in {"migration", "rollback"} and not typed[0][
            "cohort_identity"
        ]:
            raise FinalChainError(f"cohort identity is absent: {proof['proof_id']}")
        consensus_rows.append(typed[0])
        produced.extend(typed[0]["produced_artifacts"])
    return {
        "schema_version": SCHEMA,
        **identities,
        "final_integration": snapshot["final_integration"],
        "proof_count": len(proofs),
        "consensus_rows": consensus_rows,
        "engine_ledgers": engine_ledgers,
        "produced_artifacts": produced,
        "semantic_readback": {
            "status": "pass",
            "consumer_count": 0,
            "reader_count": 0,
            "hold_count": 0,
        },
        "ancestry_closure": "pass",
        "final_edge_sweep": "pass",
        "verdict": "pass",
    }


def pointer_state(path: Path) -> dict[str, object]:
    if not path.exists():
        return {"state": "absent"}
    if path.is_symlink() or not path.is_file():
        raise FinalChainError("final pointer is unsafe")
    return {"state": "present", "sha256": digest(path.read_bytes())}


def immutable_payload(
    receipt: Mapping[str, Any], paths: Mapping[str, Path]
) -> dict[str, bytes]:
    payload = {"final-cumulative-seal-receipt.v1.json": canonical_bytes(receipt)}
    for name, path in paths.items():
        payload[path.name] = path.read_bytes()
    manifest_rows = [
        {"path": name, "byte_length": len(raw), "sha256": digest(raw)}
        for name, raw in sorted(payload.items())
    ]
    payload["release-manifest.v1.json"] = canonical_bytes(
        {
            "schema_version": "maestro.external.vnext-final-release-manifest.v1",
            "file_count": len(manifest_rows),
            "byte_length": sum(row["byte_length"] for row in manifest_rows),
            "files": manifest_rows,
        }
    )
    return payload


def publish(
    receipt: Mapping[str, Any],
    paths: Mapping[str, Path],
    publication_root: Path,
    pointer_preimage: Mapping[str, Any],
) -> str:
    payload = immutable_payload(receipt, paths)
    release_identity = digest(
        canonical_bytes(
            {
                "files": [
                    {"path": name, "byte_length": len(raw), "sha256": digest(raw)}
                    for name, raw in sorted(payload.items())
                ]
            }
        )
    )
    objects = publication_root / "objects"
    objects.mkdir(parents=True, exist_ok=True)
    object_root = objects / release_identity.removeprefix("sha256:")
    if object_root.exists():
        if object_root.is_symlink() or not object_root.is_dir():
            raise FinalChainError("pre-existing release object is unsafe")
        if any(path.is_symlink() or not path.is_file() for path in object_root.iterdir()):
            raise FinalChainError("pre-existing release object has an unsafe entry")
        actual = {
            path.name: path.read_bytes()
            for path in object_root.iterdir()
            if path.is_file() and not path.is_symlink()
        }
        if actual != payload:
            raise FinalChainError("pre-existing release object bytes differ")
    else:
        temporary = Path(tempfile.mkdtemp(prefix=".final-chain-object-", dir=objects))
        try:
            for name, raw in payload.items():
                path = temporary / name
                path.write_bytes(raw)
                path.chmod(stat.S_IRUSR | stat.S_IRGRP)
            temporary.chmod(stat.S_IRUSR | stat.S_IXUSR | stat.S_IRGRP | stat.S_IXGRP)
            os.rename(temporary, object_root)
        finally:
            if temporary.exists():
                shutil.rmtree(temporary)
    if any(path.is_symlink() or not path.is_file() for path in object_root.iterdir()):
        raise FinalChainError("release object has an unsafe entry")
    actual = {
        path.name: path.read_bytes()
        for path in object_root.iterdir()
        if path.is_file() and not path.is_symlink()
    }
    if actual != payload:
        raise FinalChainError("release-object semantic readback differs")
    pointer = {
        "schema_version": POINTER_SCHEMA,
        "object": f"objects/{release_identity.removeprefix('sha256:')}",
        "release_identity": release_identity,
    }
    pointer_path = publication_root / "current.json"
    lock_path = publication_root / ".current.lock"
    with lock_path.open("a+") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        desired = canonical_bytes(pointer)
        current = pointer_state(pointer_path)
        if current == {"state": "present", "sha256": digest(desired)}:
            return release_identity
        if current != dict(pointer_preimage):
            raise FinalChainError("final pointer advanced after snapshot freeze")
        descriptor, temporary_name = tempfile.mkstemp(
            prefix=".current-", dir=publication_root
        )
        temporary = Path(temporary_name)
        try:
            with os.fdopen(descriptor, "wb") as output:
                output.write(desired)
                output.flush()
                os.fsync(output.fileno())
            os.replace(temporary, pointer_path)
        finally:
            if temporary.exists():
                temporary.unlink()
        fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
    return release_identity


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--closure-root", type=Path, required=True)
    parser.add_argument("--run-root", type=Path, required=True)
    parser.add_argument("--publication-root", type=Path, required=True)
    parser.add_argument("--sandbox-exec", type=Path, default=Path("/usr/bin/sandbox-exec"))
    args = parser.parse_args()
    closure = args.closure_root.resolve()
    run_root = args.run_root.resolve()
    publication = args.publication_root.resolve()
    if run_root.exists():
        raise FinalChainError(f"run root already exists: {run_root}")
    snapshot, paths = validate_snapshot(closure)
    protected = Path(snapshot["protected_primary_checkout"]).resolve()
    for path, label in (
        (closure, "closure"),
        (run_root, "run"),
        (publication, "publication"),
    ):
        if path == protected or path.is_relative_to(protected):
            raise FinalChainError(f"{label} root enters the protected primary checkout")
    roots = [closure, run_root, publication]
    for index, root in enumerate(roots):
        if any(
            root == other or root.is_relative_to(other) or other.is_relative_to(root)
            for other in roots[index + 1 :]
        ):
            raise FinalChainError("closure, run, and publication roots are not disjoint")
    run_root.mkdir(parents=True)
    verify_sandbox(args.sandbox_exec.resolve(), run_root, protected)
    toolchain = validate_toolchain(paths["toolchain"], closure / "source")
    proofs = validate_ledger(
        paths["ledger"], closure / "source", snapshot["final_integration"]["commit"]
    )
    receipts = [
        run_engine(
            engine,
            closure,
            paths,
            run_root,
            args.sandbox_exec.resolve(),
            toolchain,
        )
        for engine in ENGINE_IDS
    ]
    receipt = consensus(snapshot, paths, proofs, receipts)
    release_identity = publish(
        receipt, paths, publication, snapshot["pointer_preimage"]
    )
    print(release_identity)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except FinalChainError as error:
        print(f"final-chain seal refused: {error}", file=os.sys.stderr)
        raise SystemExit(2)
