#!/usr/bin/env python3
"""Run the Stage12Product validator against one frozen final-chain snapshot."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, cast


SNAPSHOT_SCHEMA = "maestro.external.vnext-final-cumulative-closure-snapshot.v1"
STAGE12_PRODUCT_CORRECTION_COMMIT = (
    "673605c630db2112b5ff66ded919a6cd2d4a3558"
)
FULL_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")


class ProofError(RuntimeError):
    """The frozen Stage12Product proof inputs are absent or inconsistent."""


def load_snapshot(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise ProofError("final-chain snapshot is absent or unsafe")
    try:
        value = json.loads(path.read_bytes())
    except (OSError, json.JSONDecodeError) as error:
        raise ProofError(f"cannot read final-chain snapshot: {error}") from error
    if not isinstance(value, dict):
        raise ProofError("final-chain snapshot must be one object")
    return cast(dict[str, Any], value)


def final_commit(snapshot: dict[str, Any]) -> str:
    integration = snapshot.get("final_integration")
    if (
        snapshot.get("schema_version") != SNAPSHOT_SCHEMA
        or snapshot.get("state") != "frozen"
        or not isinstance(integration, dict)
        or not isinstance(integration.get("commit"), str)
        or FULL_COMMIT_RE.fullmatch(integration["commit"]) is None
    ):
        raise ProofError("final-chain snapshot does not bind one frozen final commit")
    return cast(str, integration["commit"])


def require_stage12_ancestor(
    ancestry_repository: Path, snapshot_commit: str
) -> None:
    result = subprocess.run(
        [
            "git",
            "-C",
            str(ancestry_repository),
            "merge-base",
            "--is-ancestor",
            STAGE12_PRODUCT_CORRECTION_COMMIT,
            snapshot_commit,
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise ProofError(
            "Stage12Product correction is not an ancestor of the frozen final commit"
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--ancestry-repository", type=Path, required=True)
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--snapshot-root", type=Path, required=True)
    args = parser.parse_args()
    try:
        ancestry = args.ancestry_repository.resolve(strict=True)
        snapshot_path = args.snapshot.resolve(strict=True)
        snapshot_root = args.snapshot_root.resolve(strict=True)
        if not ancestry.is_dir() or not snapshot_root.is_dir():
            raise ProofError("ancestry repository or snapshot root is not a directory")
        validator = snapshot_root / "tools/vnext_contracts/stage10/validate.py"
        if validator.is_symlink() or not validator.is_file():
            raise ProofError("frozen Stage12Product validator is absent or unsafe")
        snapshot_commit = final_commit(load_snapshot(snapshot_path))
        require_stage12_ancestor(ancestry, snapshot_commit)
        result = subprocess.run(
            [
                sys.executable,
                str(validator),
                "--ancestry-repository",
                str(ancestry),
                "--snapshot-root",
                str(snapshot_root),
                "--final-ref",
                STAGE12_PRODUCT_CORRECTION_COMMIT,
            ],
            check=False,
        )
    except (OSError, ProofError) as error:
        print(json.dumps({"status": "error", "error": str(error)}, sort_keys=True))
        return 1
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
