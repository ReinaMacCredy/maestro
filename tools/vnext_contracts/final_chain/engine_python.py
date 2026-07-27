#!/usr/bin/env python3
"""Independent Python interpreter for the frozen final-chain ledger."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


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


def identity(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def command(row: dict[str, Any], source: Path) -> dict[str, Any]:
    spec = row["command"]
    argv = spec["argv"]
    result = subprocess.run(argv, cwd=source, capture_output=True, text=True)
    actual = "pass" if result.returncode == spec["expected_exit_code"] else "error"
    return {"proof_id": row["proof_id"], "expected_outcome": row["expected_outcome"], "actual_outcome": row["expected_outcome"] if actual == "pass" else actual, "exit_code": result.returncode}


def validate_ledger(ledger: dict[str, Any]) -> None:
    rows = ledger.get("proofs")
    if not isinstance(rows, list) or not rows:
        raise ValueError("ledger is empty")
    ids = [row.get("proof_id") for row in rows if isinstance(row, dict)]
    stages = {row.get("stage") for row in rows if isinstance(row, dict)}
    if len(ids) != len(rows) or len(set(ids)) != len(ids) or stages != set(range(13)):
        raise ValueError("ledger proof closure differs")
    if any(set(row.get("engines", [])) != {"python", "rust", "ruby"} for row in rows if isinstance(row, dict)):
        raise ValueError("ledger engine coverage differs")


def readback(plan: dict[str, Any], source: Path) -> dict[str, Any]:
    rows = []
    for check in plan["checks"]:
        result = subprocess.run(check["argv"], cwd=source, capture_output=True, text=True)
        rows.append({"id": check["id"], "kind": check["kind"], "exit_code": result.returncode, "status": "pass" if result.returncode == check["expected_exit_code"] else "fail"})
    return {"status": "pass" if all(row["status"] == "pass" for row in rows) else "fail", "checks": rows}


def main() -> int:
    snapshot_path, ledger_path, readback_path, source_path, output_path = map(Path, sys.argv[1:6])
    snapshot = load(snapshot_path)
    ledger = load(ledger_path)
    plan = load(readback_path)
    if snapshot.get("schema_version") != "maestro.external.vnext-final-cumulative-closure-snapshot.v1":
        raise ValueError("snapshot schema differs")
    if ledger.get("schema_version") != "maestro.external.vnext-final-proof-ledger.v1":
        raise ValueError("ledger schema differs")
    if plan.get("schema_version") != "maestro.external.vnext-stage12-semantic-readback-plan.v1":
        raise ValueError("readback schema differs")
    validate_ledger(ledger)
    proofs = [command(row, source_path) for row in ledger["proofs"]]
    value = {
        "schema_version": "maestro.external.vnext-final-engine-receipt.v1",
        "engine": "python",
        "snapshot_identity": identity(snapshot_path),
        "ledger_identity": identity(ledger_path),
        "proofs": proofs,
        "semantic_readback": readback(plan, source_path),
    }
    output_path.write_text(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n", encoding="ascii")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
