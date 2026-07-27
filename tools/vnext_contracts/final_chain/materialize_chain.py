#!/usr/bin/env python3
"""Materialize the reviewed synthetic Stage 0-12 chain without updating refs."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path

import generate


def git(repository: Path, *argv: str, data: bytes | None = None, stage: int = 0) -> str:
    timestamp = 946684800 + stage * 60
    environment = {
        **os.environ,
        "GIT_AUTHOR_NAME": "Maestro External Orchestrator",
        "GIT_AUTHOR_EMAIL": "orchestrator@maestro.invalid",
        "GIT_AUTHOR_DATE": f"{timestamp} +0000",
        "GIT_COMMITTER_NAME": "Maestro External Orchestrator",
        "GIT_COMMITTER_EMAIL": "orchestrator@maestro.invalid",
        "GIT_COMMITTER_DATE": f"{timestamp} +0000",
    }
    result = subprocess.run(
        ["git", "--no-replace-objects", *argv],
        cwd=repository,
        env=environment,
        input=data,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        raise generate.GenerationError(
            result.stderr.decode("utf-8", "replace").strip()
        )
    return result.stdout.decode("ascii").strip()


def parse_stage(value: str) -> tuple[int, str]:
    stage, separator, commit = value.partition("=")
    if separator != "=" or not stage.isdigit() or int(stage) not in range(6, 12):
        raise argparse.ArgumentTypeError("stage candidate must be N=COMMIT for N=6..11")
    return int(stage), commit


def commit_tree(
    repository: Path, tree: str, parents: list[str], stage: int
) -> str:
    arguments = ["commit-tree", tree]
    for parent in parents:
        arguments.extend(["-p", parent])
    return git(
        repository,
        *arguments,
        data=f"maestro synthetic Stage {stage} checkpoint\n".encode("ascii"),
        stage=stage,
    )


def require_first_parent_ancestor(
    repository: Path, descendant: str, ancestor: str
) -> None:
    current = descendant
    while current != ancestor:
        parents = git(repository, "show", "-s", "--format=%P", current).split()
        if not parents:
            raise generate.GenerationError(
                "Stage 5 reviewed candidate first-parent ancestry does not reach "
                "the provisional Stage 5 source"
            )
        current = parents[0]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--stage5-reviewed-candidate", required=True)
    parser.add_argument("--stage-candidate", action="append", type=parse_stage, required=True)
    parser.add_argument("--stage12-reviewed-candidate", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--overlay-output", type=Path, required=True)
    args = parser.parse_args()
    repository = args.repository.resolve(strict=True)
    candidates = dict(args.stage_candidate)
    if len(args.stage_candidate) != 6 or set(candidates) != set(range(6, 12)):
        raise generate.GenerationError("exactly one reviewed candidate for Stage 6 through 11 is required")
    if args.output.exists() or args.overlay_output.exists():
        raise generate.GenerationError("synthetic-chain output already exists")
    stage5_candidate = git(
        repository, "rev-parse", "--verify", f"{args.stage5_reviewed_candidate}^{{commit}}"
    )
    stage12_candidate = git(
        repository, "rev-parse", "--verify", f"{args.stage12_reviewed_candidate}^{{commit}}"
    )
    require_first_parent_ancestor(
        repository,
        stage5_candidate,
        generate.PROVISIONAL_STAGE5_SOURCE,
    )
    checkpoints = dict(generate.HISTORICAL_STAGE_CHECKPOINTS)
    stage5_tree = git(repository, "show", "-s", "--format=%T", stage5_candidate)
    checkpoints[5] = commit_tree(
        repository, stage5_tree, [checkpoints[4], stage5_candidate], 5
    )
    for stage in range(6, 12):
        candidate = git(repository, "rev-parse", "--verify", f"{candidates[stage]}^{{commit}}")
        tree = git(repository, "show", "-s", "--format=%T", candidate)
        checkpoints[stage] = commit_tree(repository, tree, [checkpoints[stage - 1]], stage)
    stage12_tree = git(repository, "show", "-s", "--format=%T", stage12_candidate)
    checkpoints[12] = commit_tree(
        repository, stage12_tree, [checkpoints[11], stage12_candidate], 12
    )
    overlay_rows = generate.overlay_entries(repository, checkpoints[11], checkpoints[12])
    overlay = {
        "schema_version": "maestro.external.vnext-final-stage12-overlay.v1",
        "stage11_commit": checkpoints[11],
        "reviewed_candidate_commit": stage12_candidate,
        "reviewed_candidate_tree": stage12_tree,
        "stage12_commit": checkpoints[12],
        "stage12_tree": stage12_tree,
        "entry_count": len(overlay_rows),
        "byte_length": sum(int(row["byte_length"]) for row in overlay_rows),
        "entries": overlay_rows,
    }
    result = {
        "schema_version": "maestro.external.vnext-synthetic-final-chain.v1",
        "checkpoints": [
            {
                "stage": stage,
                "commit": checkpoints[stage],
                "tree": git(repository, "show", "-s", "--format=%T", checkpoints[stage]),
            }
            for stage in range(13)
        ],
        "stage5_reviewed_candidate": stage5_candidate,
        "stage12_reviewed_candidate": stage12_candidate,
        "overlay_manifest": overlay,
        "refs_updated": False,
    }
    args.overlay_output.write_bytes(generate.canonical_bytes(overlay))
    args.output.write_bytes(generate.canonical_bytes(result))
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except generate.GenerationError as error:
        print(f"synthetic-chain materialization refused: {error}", file=os.sys.stderr)
        raise SystemExit(2)
