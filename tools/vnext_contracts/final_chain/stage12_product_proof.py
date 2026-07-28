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
    "11dca539193e9a6c3e3346786c69d8d4bad386e8"
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


def proof_commits(snapshot: dict[str, Any]) -> tuple[str, str]:
    reviewed = snapshot.get("stage12_reviewed_candidate")
    integration = snapshot.get("final_integration")
    if (
        snapshot.get("schema_version") != SNAPSHOT_SCHEMA
        or snapshot.get("state") != "frozen"
        or not isinstance(reviewed, dict)
        or not isinstance(reviewed.get("commit"), str)
        or FULL_COMMIT_RE.fullmatch(reviewed["commit"]) is None
        or not isinstance(reviewed.get("tree"), str)
        or FULL_COMMIT_RE.fullmatch(reviewed["tree"]) is None
        or not isinstance(integration, dict)
        or not isinstance(integration.get("commit"), str)
        or FULL_COMMIT_RE.fullmatch(integration["commit"]) is None
        or not isinstance(integration.get("tree"), str)
        or FULL_COMMIT_RE.fullmatch(integration["tree"]) is None
        or reviewed["tree"] != integration["tree"]
    ):
        raise ProofError(
            "final-chain snapshot does not bind one frozen reviewed candidate "
            "and final integration"
        )
    return cast(str, reviewed["commit"]), cast(str, integration["commit"])


def require_stage12_ancestry(
    ancestry_repository: Path, reviewed_commit: str, final_commit: str
) -> None:
    ancestor = subprocess.run(
        [
            "git",
            "-C",
            str(ancestry_repository),
            "merge-base",
            "--is-ancestor",
            STAGE12_PRODUCT_CORRECTION_COMMIT,
            reviewed_commit,
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if ancestor.returncode != 0:
        raise ProofError(
            "Stage12Product correction is not an ancestor of the reviewed candidate"
        )
    parent_row = subprocess.run(
        [
            "git",
            "-C",
            str(ancestry_repository),
            "rev-list",
            "--parents",
            "--max-count=1",
            final_commit,
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    parents = parent_row.stdout.split()
    if (
        parent_row.returncode != 0
        or len(parents) != 3
        or parents[0] != final_commit
        or parents[2] != reviewed_commit
    ):
        raise ProofError(
            "reviewed candidate is not the direct second parent of final integration"
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
        reviewed_commit, integration_commit = proof_commits(
            load_snapshot(snapshot_path)
        )
        require_stage12_ancestry(ancestry, reviewed_commit, integration_commit)
        result = subprocess.run(
            [
                sys.executable,
                str(validator),
                "--ancestry-repository",
                str(ancestry),
                "--snapshot-root",
                str(snapshot_root),
                "--final-ref",
                reviewed_commit,
            ],
            check=False,
        )
    except (OSError, ProofError) as error:
        print(json.dumps({"status": "error", "error": str(error)}, sort_keys=True))
        return 1
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
