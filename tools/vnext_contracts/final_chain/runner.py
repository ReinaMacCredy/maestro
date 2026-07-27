#!/usr/bin/env python3
"""Build one V4 final-chain receipt only from a frozen explicit snapshot."""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any, Mapping


ROOT = Path(__file__).resolve().parent
SCHEMA = "maestro.external.vnext-final-cumulative-seal-receipt.v1"
POINTER_SCHEMA = "maestro.external.vnext-final-cumulative-seal-pointer.v1"
ENGINE_IDS = ("python", "rust", "ruby")


class FinalChainError(RuntimeError):
    pass


def canonical_bytes(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n").encode("ascii")


def digest(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise FinalChainError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def read_json(path: Path) -> dict[str, Any]:
    data = path.read_bytes()
    if b"\r" in data or data.startswith(b"\xef\xbb\xbf") or not data.endswith(b"\n"):
        raise FinalChainError(f"JSON input must be UTF-8 LF terminated without BOM: {path}")
    try:
        value = json.loads(data, object_pairs_hook=reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise FinalChainError(f"invalid JSON input {path}: {error}") from error
    if not isinstance(value, dict):
        raise FinalChainError(f"JSON input must be one object: {path}")
    return value


def safe_relative(value: object, label: str) -> PurePosixPath:
    if not isinstance(value, str) or not value or "\\" in value:
        raise FinalChainError(f"{label} must be a portable relative path")
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or "." in path.parts:
        raise FinalChainError(f"{label} escapes its declared root: {value!r}")
    return path


def bound_file(source_root: Path, binding: object, label: str) -> Path:
    if not isinstance(binding, Mapping):
        raise FinalChainError(f"{label} is not a file binding")
    path = source_root / safe_relative(binding.get("path"), f"{label}.path")
    if path.is_symlink() or not path.is_file():
        raise FinalChainError(f"{label} is absent or unsafe: {path}")
    actual = digest(path.read_bytes())
    if binding.get("sha256") != actual:
        raise FinalChainError(f"{label} digest differs: {path}")
    return path


def validate_snapshot(snapshot: Mapping[str, Any], source_root: Path) -> None:
    if snapshot.get("schema_version") != "maestro.external.vnext-final-cumulative-closure-snapshot.v1":
        raise FinalChainError("snapshot schema differs")
    if snapshot.get("state") != "frozen":
        raise FinalChainError("snapshot is not frozen")
    stages = snapshot.get("first_parent_stages")
    if not isinstance(stages, list) or [row.get("stage") for row in stages if isinstance(row, dict)] != list(range(13)):
        raise FinalChainError("snapshot does not bind exact Stage 0 through 12 first-parent order")
    for row in stages:
        if not isinstance(row, Mapping) or not all(re.fullmatch(r"[0-9a-f]{40}", str(row.get(field, ""))) for field in ("commit", "tree")):
            raise FinalChainError("snapshot has malformed Stage ancestry identity")
        bound_file(source_root, row.get("checkpoint"), f"snapshot.stage{row['stage']}.checkpoint")
    final = snapshot.get("final_integration")
    if not isinstance(final, Mapping) or not all(re.fullmatch(r"[0-9a-f]{40}", str(final.get(field, ""))) for field in ("commit", "tree")):
        raise FinalChainError("snapshot final integration identity is incomplete")
    engines = snapshot.get("engines")
    if not isinstance(engines, list) or tuple(row.get("id") for row in engines if isinstance(row, dict)) != ENGINE_IDS:
        raise FinalChainError("snapshot engine closure differs")
    for field in ("input_manifest", "proof_ledger", "stage12_readback", "toolchain"):
        bound_file(source_root, snapshot.get(field), f"snapshot.{field}")
    if snapshot.get("cache_policy") != "immutable_compilation_and_dependency_bytes_only":
        raise FinalChainError("snapshot cache policy admits proof-result reuse")
    if not isinstance(snapshot.get("environment_allowlist"), list) or not isinstance(snapshot.get("immutable_input_roots"), list):
        raise FinalChainError("snapshot runtime closure is incomplete")
    writable = snapshot.get("writable_roots")
    if not isinstance(writable, list) or len(writable) < 4 or len(writable) != len(set(writable)):
        raise FinalChainError("snapshot engine writable-root isolation is incomplete")
    if not isinstance(snapshot.get("sandbox_profile"), str) or not snapshot["sandbox_profile"]:
        raise FinalChainError("snapshot sandbox profile is absent")
    if not isinstance(snapshot.get("pointer_preimage"), Mapping):
        raise FinalChainError("snapshot pointer preimage is absent")
    for engine in engines:
        if not isinstance(engine, Mapping):
            raise FinalChainError("snapshot engine row is invalid")
        bound_file(source_root, engine.get("source"), f"snapshot.engine.{engine.get('id')}")
    denied = snapshot.get("effect_denylist")
    required_denials = {"install", "publish", "activate", "release", "push", "tag", "remote_connector", "primary_checkout_write"}
    if not isinstance(denied, list) or not required_denials.issubset(set(denied)):
        raise FinalChainError("snapshot does not deny every final-chain external effect")


def engine_sources(snapshot: Mapping[str, Any], source_root: Path) -> dict[str, Path]:
    engines = snapshot["engines"]
    return {
        str(engine["id"]): bound_file(source_root, engine["source"], f"snapshot.engine.{engine['id']}")
        for engine in engines
        if isinstance(engine, Mapping)
    }


def validate_input_manifest(path: Path, source_root: Path) -> None:
    manifest = read_json(path)
    if manifest.get("schema_version") != "maestro.external.vnext-final-input-manifest.v1":
        raise FinalChainError("input manifest schema differs")
    entries = manifest.get("entries")
    required = {
        "source", "test", "validator", "fixture", "mutation", "crash_schedule",
        "migration", "rollback", "adapter", "removal", "consumer_manifest",
        "reader_manifest", "hold_manifest", "predecessor",
    }
    if not isinstance(entries, list) or {row.get("kind") for row in entries if isinstance(row, Mapping)} != required:
        raise FinalChainError("input manifest closure differs")
    seen: set[tuple[str, str]] = set()
    for row in entries:
        if not isinstance(row, Mapping):
            raise FinalChainError("input manifest row is invalid")
        key = (str(row.get("kind")), str(row.get("path")))
        if key in seen:
            raise FinalChainError("input manifest duplicates an owned input")
        seen.add(key)
        bound_file(source_root, row, f"input_manifest.{key[0]}")


def validate_toolchain(toolchain_path: Path) -> None:
    manifest = read_json(toolchain_path)
    if manifest.get("schema_version") != "maestro.external.vnext-final-toolchain.v1":
        raise FinalChainError("toolchain manifest schema differs")
    tools = manifest.get("tools")
    if not isinstance(tools, Mapping) or set(tools) != set(ENGINE_IDS):
        raise FinalChainError("toolchain manifest closure differs")
    executable_names = {"python": "python3", "rust": "rustc", "ruby": "ruby"}
    for engine, executable_name in executable_names.items():
        expected = tools[engine]
        if not isinstance(expected, Mapping) or not isinstance(expected.get("sha256"), str):
            raise FinalChainError(f"toolchain manifest lacks {engine} executable identity")
        resolved = shutil.which(executable_name)
        if resolved is None or digest(Path(resolved).read_bytes()) != expected["sha256"]:
            raise FinalChainError(f"{engine} executable differs from frozen toolchain")


def validate_ledger(ledger: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    if ledger.get("schema_version") != "maestro.external.vnext-final-proof-ledger.v1":
        raise FinalChainError("proof ledger schema differs")
    proofs = ledger.get("proofs")
    if not isinstance(proofs, list) or not proofs:
        raise FinalChainError("proof ledger is empty")
    identifiers: set[str] = set()
    covered_stages: set[int] = set()
    required_kinds = {"race", "crash_replay", "migration", "rollback", "adapter", "removal", "closure"}
    kinds: set[str] = set()
    for row in proofs:
        if not isinstance(row, Mapping):
            raise FinalChainError("proof ledger row is not an object")
        proof_id = row.get("proof_id")
        if not isinstance(proof_id, str) or proof_id in identifiers:
            raise FinalChainError("proof ledger has an absent or duplicate proof id")
        identifiers.add(proof_id)
        stage = row.get("stage")
        if not isinstance(stage, int) or stage not in range(13):
            raise FinalChainError(f"proof {proof_id} has invalid stage")
        covered_stages.add(stage)
        if row.get("expected_outcome") not in {"pass", "refuse"}:
            raise FinalChainError(f"proof {proof_id} has invalid expected outcome")
        if set(row.get("engines", [])) != set(ENGINE_IDS):
            raise FinalChainError(f"proof {proof_id} lacks independent engine coverage")
        command = row.get("command")
        if not isinstance(command, Mapping) or not isinstance(command.get("argv"), list) or not command["argv"]:
            raise FinalChainError(f"proof {proof_id} lacks an executable command")
        if row.get("kind") in {"race", "crash_replay"} and not command.get("fault_schedule"):
            raise FinalChainError(f"proof {proof_id} lacks an exact fault schedule")
        if row.get("kind") in {"migration", "rollback"} and not command.get("cohort"):
            raise FinalChainError(f"proof {proof_id} lacks its reader-writer cohort")
        kinds.add(str(row.get("kind")))
    if covered_stages != set(range(13)) or not required_kinds.issubset(kinds):
        raise FinalChainError("proof ledger omits required final-chain coverage")
    return proofs


def validate_readback(plan: Mapping[str, Any]) -> None:
    if plan.get("schema_version") != "maestro.external.vnext-stage12-semantic-readback-plan.v1":
        raise FinalChainError("Stage 12 readback schema differs")
    checks = plan.get("checks")
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
    if not isinstance(checks, list) or {row.get("kind") for row in checks if isinstance(row, Mapping)} != required:
        raise FinalChainError("Stage 12 readback is not semantic closure")
    for row in checks:
        if not isinstance(row, Mapping) or not isinstance(row.get("argv"), list) or not row["argv"]:
            raise FinalChainError("Stage 12 readback command is incomplete")


def freeze_copy(source: Path, destination: Path) -> None:
    def ignore(directory: str, names: list[str]) -> set[str]:
        return {name for name in names if name in {".git", "target", "__pycache__"}}
    shutil.copytree(source, destination, symlinks=False, ignore=ignore)
    for root, directories, files in os.walk(destination):
        for name in directories + files:
            path = Path(root) / name
            if path.is_symlink():
                raise FinalChainError(f"source materialization contains symlink: {path}")
            path.chmod(stat.S_IRUSR | (stat.S_IXUSR if path.is_dir() else 0))


def run_engine(engine: str, engine_source: Path, snapshot: Path, ledger: Path, readback: Path, source: Path, root: Path) -> dict[str, Any]:
    root.mkdir(parents=True, exist_ok=False)
    inputs = root / "inputs"
    inputs.mkdir()
    for item in (snapshot, ledger, readback):
        shutil.copy2(item, inputs / item.name)
    isolated_source = root / "source"
    freeze_copy(source, isolated_source)
    output = root / "output"
    output.mkdir()
    receipt = output / "engine-receipt.json"
    if engine == "python":
        command = ["python3", str(root / "engine.py")]
    elif engine == "ruby":
        command = ["ruby", str(root / "engine.rb")]
    else:
        binary = root / "rust-engine"
        command = [str(binary)]
    copied_engine = root / ("engine.rs" if engine == "rust" else "engine.py" if engine == "python" else "engine.rb")
    shutil.copy2(engine_source, copied_engine)
    if engine == "rust":
        compile_result = subprocess.run(["rustc", "--edition=2021", str(copied_engine), "-O", "-o", str(binary)], capture_output=True, text=True)
        if compile_result.returncode != 0:
            raise FinalChainError(f"Rust engine compilation failed: {compile_result.stderr}")
    environment = {"PATH": os.environ.get("PATH", ""), "HOME": str(root / "home"), "LC_ALL": "C", "TZ": "UTC"}
    result = subprocess.run(command + [str(inputs / snapshot.name), str(inputs / ledger.name), str(inputs / readback.name), str(isolated_source), str(receipt)], cwd=isolated_source, env=environment, capture_output=True, text=True)
    if result.returncode != 0:
        raise FinalChainError(f"{engine} engine failed: {result.stderr or result.stdout}")
    return read_json(receipt)


def consensus(snapshot_identity: str, ledger_identity: str, proofs: list[Mapping[str, Any]], receipts: list[Mapping[str, Any]]) -> dict[str, Any]:
    by_engine = {receipt.get("engine"): receipt for receipt in receipts}
    if set(by_engine) != set(ENGINE_IDS):
        raise FinalChainError("engine receipts are not one-per-independent-engine")
    expected_ids = [row["proof_id"] for row in proofs]
    engine_ledgers: list[dict[str, Any]] = []
    for engine in ENGINE_IDS:
        receipt = by_engine[engine]
        if receipt.get("snapshot_identity") != snapshot_identity or receipt.get("ledger_identity") != ledger_identity:
            raise FinalChainError(f"{engine} receipt binds another snapshot or ledger")
        rows = receipt.get("proofs")
        if not isinstance(rows, list) or [row.get("proof_id") for row in rows if isinstance(row, Mapping)] != expected_ids:
            raise FinalChainError(f"{engine} proof coverage differs")
        if any(row.get("actual_outcome") != row.get("expected_outcome") for row in rows if isinstance(row, Mapping)):
            raise FinalChainError(f"{engine} does not match frozen expected outcomes")
        if receipt.get("semantic_readback", {}).get("status") != "pass":
            raise FinalChainError(f"{engine} semantic Stage 12 readback failed")
        engine_ledgers.append({"engine": engine, "sha256": digest(canonical_bytes(receipt))})
    return {
        "schema_version": SCHEMA,
        "snapshot_identity": snapshot_identity,
        "ledger_identity": ledger_identity,
        "engine_ledgers": engine_ledgers,
        "semantic_readback": {"status": "pass", "engine_count": 3},
        "verdict": "pass",
    }


def pointer_state(path: Path) -> dict[str, object]:
    if not path.exists():
        return {"state": "absent"}
    if path.is_symlink() or not path.is_file():
        raise FinalChainError(f"final pointer is unsafe: {path}")
    return {"state": "present", "sha256": digest(path.read_bytes())}


def publish(receipt: Mapping[str, Any], snapshot: Path, ledger: Path, readback: Path, publication_root: Path, pointer_preimage: Mapping[str, Any]) -> None:
    payload = {"receipt": receipt, "snapshot_sha256": digest(snapshot.read_bytes()), "ledger_sha256": digest(ledger.read_bytes()), "readback_sha256": digest(readback.read_bytes())}
    release_identity = digest(canonical_bytes(payload))
    objects = publication_root / "objects"
    objects.mkdir(parents=True, exist_ok=True)
    object_root = objects / release_identity.removeprefix("sha256:")
    if not object_root.exists():
        temporary = Path(tempfile.mkdtemp(prefix=".final-chain-", dir=objects))
        try:
            (temporary / "payload.json").write_bytes(canonical_bytes(payload))
            (temporary / "receipt.json").write_bytes(canonical_bytes(receipt))
            for path in (temporary, temporary / "payload.json", temporary / "receipt.json"):
                path.chmod(stat.S_IRUSR | stat.S_IRGRP | (stat.S_IXUSR | stat.S_IXGRP if path.is_dir() else 0))
            os.rename(temporary, object_root)
        finally:
            if temporary.exists():
                shutil.rmtree(temporary)
    pointer = {"schema_version": POINTER_SCHEMA, "object": f"objects/{release_identity.removeprefix('sha256:')}", "release_identity": release_identity}
    pointer_path = publication_root / "current.json"
    lock = publication_root / ".current.lock"
    with lock.open("a+") as handle:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
        desired = canonical_bytes(pointer)
        current = pointer_state(pointer_path)
        if current == {"state": "present", "sha256": digest(desired)}:
            return
        if current != dict(pointer_preimage):
            raise FinalChainError("final pointer advanced after the frozen snapshot")
        temporary = pointer_path.with_suffix(".tmp")
        temporary.write_bytes(desired)
        os.replace(temporary, pointer_path)
        fcntl.flock(handle.fileno(), fcntl.LOCK_UN)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--ledger", type=Path, required=True)
    parser.add_argument("--readback", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--run-root", type=Path, required=True)
    parser.add_argument("--publication-root", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        source = arguments.source_root.resolve(strict=True)
        snapshot = read_json(arguments.snapshot)
        ledger = read_json(arguments.ledger)
        readback = read_json(arguments.readback)
        validate_snapshot(snapshot, source)
        if snapshot["proof_ledger"]["sha256"] != digest(arguments.ledger.read_bytes()) or snapshot["stage12_readback"]["sha256"] != digest(arguments.readback.read_bytes()):
            raise FinalChainError("snapshot input binding differs")
        proofs = validate_ledger(ledger)
        validate_readback(readback)
        validate_input_manifest(bound_file(source, snapshot["input_manifest"], "snapshot.input_manifest"), source)
        validate_toolchain(bound_file(source, snapshot["toolchain"], "snapshot.toolchain"))
        if arguments.run_root.exists():
            raise FinalChainError("run root must be absent to prevent output reuse")
        arguments.run_root.mkdir(parents=True)
        sources = engine_sources(snapshot, source)
        receipts = [run_engine(engine, sources[engine], arguments.snapshot, arguments.ledger, arguments.readback, source, arguments.run_root / engine) for engine in ENGINE_IDS]
        receipt = consensus(digest(arguments.snapshot.read_bytes()), digest(arguments.ledger.read_bytes()), proofs, receipts)
        publish(receipt, arguments.snapshot, arguments.ledger, arguments.readback, arguments.publication_root, snapshot["pointer_preimage"])
        print(json.dumps(receipt, sort_keys=True))
        return 0
    except (FinalChainError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps({"status": "error", "error": str(error)}, sort_keys=True))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
