#!/usr/bin/env python3
"""Run the historical Stage12 proof and V8 closure checks effect-inertly."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tarfile
import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Iterator, Mapping, cast


SNAPSHOT_SCHEMA = "maestro.external.vnext-final-cumulative-closure-snapshot.v1"
STAGE12_PRODUCT_CORRECTION_COMMIT = (
    "11dca539193e9a6c3e3346786c69d8d4bad386e8"
)
HISTORICAL_STAGE12_CHECKPOINT = "e03d21b64995a20cfda3e90d706048ca79038f30"
HISTORICAL_STAGE12_TREE = "600171763b9e782d494fa0c04ba5de9a5d7fa5a4"
V8_DESIGN_COMMIT = "bb7b1ee0e51fa591b21943e8c7d50844cb4d0b05"
V8_DESIGN_TREE = "cb6b62cc187abdecebef8f621206289029fb590b"
HISTORICAL_STAGE12_VALIDATION_REF = V8_DESIGN_COMMIT
V8_CHECKPOINT_NAMES = (
    "V8Design",
    "Stage11Owners",
    "MainIntegrationOwnerWiring",
    "FoundationMigration",
    "MainIntegrationFoundationV4Wiring",
    "Stage12Dependency",
    "MainIntegrationGuardCoordinatorWiring",
    "ExternalProofControl",
    "MainIntegrationFinalClosure",
)
V8_CLOSURE = {
    "coordinator": "Stage12LegacyCutCoordinatorV3",
    "design_commit": V8_DESIGN_COMMIT,
    "design_tree": V8_DESIGN_TREE,
    "epoch": "LegacyQuarantineEpochV4",
    "foundation_closure": "FoundationLegacyQuarantineClosureV2",
    "foundation_owner_evidence_mint": "FoundationOwnerEvidenceMintV1",
    "guard": "LegacyRemovalGuardV3",
    "guard_consumer_binding": "LegacyRemovalConsumerBindingV3",
    "implementation_preimage_commit": "1685b39138a045bcd5e87744860d95eb589999d2",
    "implementation_preimage_tree": "2daa5f8458411cf9e6d6288bf51606c98a4e31c9",
    "integration_plan_identity": "789cd36b82f4e6a0d534833446b9a2c35d6cfafcd96e1123fb9e3215a5df0f29",
    "loss_manifest": "UnavailablePreexistingLossManifestV4",
    "loss_audit_currentness": "UnavailablePreexistingLossAuditCurrentnessV4",
    "loss_audit_custody": "QuarantineCustodyLeaseV1",
    "loss_audit_gate": "UnavailablePreexistingLossAuditGateErrorV1",
    "ownership_identity": "699c6b98c8e4f1c8d92bf3a7377759fcc65e685c4f59272c36f13b65b3dc9cfd",
    "packet_sha256": "d0953ac33f361ccad2fe0c7844294324b7b33cb974e16a11639ad3aad19e40e2",
    "primary_boundary_identity": "e5b4c0592b8cf373ea68fc5e0e3f84020c14f3f422c5779e8d4a423930aa6054",
    "rollback": "LegacyRollbackAssessmentV4",
}
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
    """Retain the frozen V7 snapshot parser for historical callers."""
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


def _ledger_entry(value: object, index: int) -> dict[str, str]:
    if not isinstance(value, dict) or set(value) != {"commit", "tree"}:
        raise ProofError(f"V8 first-parent ledger entry differs at index {index}")
    commit = value.get("commit")
    tree = value.get("tree")
    if (
        not isinstance(commit, str)
        or FULL_COMMIT_RE.fullmatch(commit) is None
        or not isinstance(tree, str)
        or FULL_COMMIT_RE.fullmatch(tree) is None
    ):
        raise ProofError(f"V8 first-parent ledger identity is invalid at index {index}")
    return {"commit": commit, "tree": tree}


def _checkpoint(value: object, name: str) -> dict[str, str]:
    if (
        not isinstance(value, dict)
        or set(value) != {"name", "tip_commit", "tip_tree"}
        or value.get("name") != name
    ):
        raise ProofError(f"V8 checkpoint order differs at {name}")
    commit = value.get("tip_commit")
    tree = value.get("tip_tree")
    if (
        not isinstance(commit, str)
        or FULL_COMMIT_RE.fullmatch(commit) is None
        or not isinstance(tree, str)
        or FULL_COMMIT_RE.fullmatch(tree) is None
    ):
        raise ProofError(f"V8 checkpoint identity is invalid at {name}")
    return {"name": name, "commit": commit, "tree": tree}


def v8_ancestry(
    snapshot: Mapping[str, Any],
) -> tuple[list[dict[str, str]], list[dict[str, str]]]:
    if (
        snapshot.get("schema_version") != SNAPSHOT_SCHEMA
        or snapshot.get("state") != "frozen"
    ):
        raise ProofError("V8 final-chain snapshot is not frozen")
    ledger_values = snapshot.get("v8_first_parent_ledger")
    if not isinstance(ledger_values, list) or len(ledger_values) < len(
        V8_CHECKPOINT_NAMES
    ):
        raise ProofError("V8 first-parent ledger is incomplete")
    ledger = [
        _ledger_entry(row, index) for index, row in enumerate(ledger_values)
    ]
    if len({row["commit"] for row in ledger}) != len(ledger):
        raise ProofError("V8 first-parent ledger repeats a commit")
    rows = snapshot.get("v8_logical_checkpoints")
    if not isinstance(rows, list) or len(rows) != len(V8_CHECKPOINT_NAMES):
        raise ProofError("V8 final-chain checkpoint closure differs")
    checkpoints = [
        _checkpoint(row, name) for row, name in zip(rows, V8_CHECKPOINT_NAMES)
    ]
    if ledger[0] != {
        "commit": V8_DESIGN_COMMIT,
        "tree": V8_DESIGN_TREE,
    } or checkpoints[0] != {
        "name": "V8Design",
        **ledger[0],
    }:
        raise ProofError("V8 design checkpoint differs")
    ledger_indices = {row["commit"]: index for index, row in enumerate(ledger)}
    checkpoint_indices: list[int] = []
    for checkpoint in checkpoints:
        index = ledger_indices.get(checkpoint["commit"])
        if index is None or ledger[index]["tree"] != checkpoint["tree"]:
            raise ProofError(
                f"V8 checkpoint tip is absent from the ledger at {checkpoint['name']}"
            )
        checkpoint_indices.append(index)
    if checkpoint_indices != sorted(checkpoint_indices) or any(
        left >= right
        for left, right in zip(checkpoint_indices, checkpoint_indices[1:])
    ):
        raise ProofError("V8 logical checkpoint tips are not ordered and non-overlapping")
    if checkpoint_indices[0] != 0 or checkpoint_indices[-1] != len(ledger) - 1:
        raise ProofError("V8 logical checkpoint tips do not cover the complete ledger")
    if snapshot.get("v8_closure") != V8_CLOSURE:
        raise ProofError("V8 typed closure binding differs")
    if snapshot.get("historical_stage12") != {
        "checkpoint": HISTORICAL_STAGE12_CHECKPOINT,
        "tree": HISTORICAL_STAGE12_TREE,
        "validation_ref": HISTORICAL_STAGE12_VALIDATION_REF,
    }:
        raise ProofError("historical Stage12 checkpoint binding differs")
    if snapshot.get("effect_boundary") != {
        "coordinator_cas_execution": "not_performed_by_proof_runner",
        "live_installation_mutation": False,
        "primary_mutation": False,
        "proof_runner_effect_inert": True,
        "receipt_or_pointer_publication": False,
        "seal_execution": False,
    }:
        raise ProofError("V8 proof-runner effect boundary differs")
    reviewed = snapshot.get("stage12_reviewed_candidate")
    final = snapshot.get("final_integration")
    if reviewed != {
        "commit": checkpoints[-2]["commit"],
        "tree": checkpoints[-2]["tree"],
    }:
        raise ProofError("V8 reviewed candidate is not ExternalProofControl")
    if final != {
        "commit": checkpoints[-1]["commit"],
        "tree": checkpoints[-1]["tree"],
    }:
        raise ProofError("V8 final integration is not MainIntegrationFinalClosure")
    return ledger, checkpoints


def v8_checkpoints(snapshot: Mapping[str, Any]) -> list[dict[str, str]]:
    return v8_ancestry(snapshot)[1]


def _git(
    repository: Path, *arguments: str, text: bool = True
) -> subprocess.CompletedProcess[Any]:
    return subprocess.run(
        ["git", "-C", str(repository), *arguments],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=text,
    )


def require_v8_linear_ancestry(
    ancestry_repository: Path,
    ledger: list[dict[str, str]],
    checkpoints: list[dict[str, str]],
) -> None:
    for index, row in enumerate(ledger):
        observed_tree = _git(
            ancestry_repository, "rev-parse", f"{row['commit']}^{{tree}}"
        )
        if (
            observed_tree.returncode != 0
            or cast(str, observed_tree.stdout).strip() != row["tree"]
        ):
            raise ProofError(f"V8 first-parent ledger tree differs at index {index}")
    for parent, child in zip(ledger, ledger[1:]):
        parent_row = _git(
            ancestry_repository,
            "rev-list",
            "--parents",
            "--max-count=1",
            child["commit"],
        )
        identities = cast(str, parent_row.stdout).split()
        if (
            parent_row.returncode != 0
            or identities != [child["commit"], parent["commit"]]
        ):
            raise ProofError(
                "V8 first-parent ledger entry is not the direct single-parent "
                f"child of its predecessor: {child['commit']}"
            )
    ledger_rows = {(row["commit"], row["tree"]) for row in ledger}
    if any(
        (checkpoint["commit"], checkpoint["tree"]) not in ledger_rows
        for checkpoint in checkpoints
    ):
        raise ProofError("V8 logical checkpoint tip is outside the verified ledger")


def _safe_archive_member(member: tarfile.TarInfo) -> bool:
    path = Path(member.name)
    return (
        not path.is_absolute()
        and ".." not in path.parts
        and member.name not in {"", "."}
        and (member.isfile() or member.isdir())
    )


@contextmanager
def historical_stage12_snapshot(
    ancestry_repository: Path,
) -> Iterator[Path]:
    with tempfile.TemporaryDirectory(prefix="maestro-v8-stage12-history-") as directory:
        root = Path(directory)
        archive = root / "historical.tar"
        snapshot = root / "snapshot"
        snapshot.mkdir()
        archived = _git(
            ancestry_repository,
            "archive",
            "--format=tar",
            "--output",
            str(archive),
            HISTORICAL_STAGE12_VALIDATION_REF,
        )
        if archived.returncode != 0:
            raise ProofError("cannot archive the historical Stage12 checkpoint")
        with tarfile.open(archive, "r:") as value:
            if any(not _safe_archive_member(member) for member in value.getmembers()):
                raise ProofError("historical Stage12 archive contains an unsafe path")
            value.extractall(snapshot)
        yield snapshot


def _run_validator(arguments: list[str]) -> int:
    result = subprocess.run(arguments, check=False)
    return result.returncode


def run_v8_closure(
    ancestry_repository: Path, snapshot_root: Path
) -> int:
    with historical_stage12_snapshot(ancestry_repository) as historical_root:
        historical_validator = snapshot_root / "tools/vnext_contracts/stage10/validate.py"
        if historical_validator.is_symlink() or not historical_validator.is_file():
            raise ProofError("frozen historical Stage12 validator is absent or unsafe")
        result = _run_validator(
            [
                sys.executable,
                str(historical_validator),
                "--ancestry-repository",
                str(ancestry_repository),
                "--snapshot-root",
                str(historical_root),
                "--final-ref",
                HISTORICAL_STAGE12_VALIDATION_REF,
            ]
        )
        if result != 0:
            return result
    v4_validator = snapshot_root / "tools/vnext_contracts/stage11/validate_v4.py"
    coordinator = snapshot_root / "tools/vnext_contracts/stage12/coordinator_v3.py"
    v4_contract = (
        snapshot_root
        / "tests/fixtures/vnext/stage11/live_set_v4_contract.v1.json"
    )
    root_universe = (
        snapshot_root / "tests/fixtures/vnext/stage11/root-universe.v1.json"
    )
    coordinator_contract = (
        snapshot_root
        / "tests/fixtures/vnext/stage12/stage12-legacy-cut-coordinator.v3.json"
    )
    for path in (
        v4_validator,
        coordinator,
        v4_contract,
        root_universe,
        coordinator_contract,
    ):
        if path.is_symlink() or not path.is_file():
            raise ProofError(f"V8 proof input is absent or unsafe: {path}")
    result = _run_validator(
        [
            sys.executable,
            str(v4_validator),
            "--contract",
            str(v4_contract),
            "--root-universe",
            str(root_universe),
            "--source-root",
            str(snapshot_root),
        ]
    )
    if result != 0:
        return result
    return _run_validator(
        [
            sys.executable,
            str(coordinator),
            "--contract",
            str(coordinator_contract),
        ]
    )


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
        snapshot = load_snapshot(snapshot_path)
        if "v8_first_parent_ledger" in snapshot:
            ledger, checkpoints = v8_ancestry(snapshot)
            require_v8_linear_ancestry(ancestry, ledger, checkpoints)
            return run_v8_closure(ancestry, snapshot_root)
        validator = snapshot_root / "tools/vnext_contracts/stage10/validate.py"
        if validator.is_symlink() or not validator.is_file():
            raise ProofError("frozen Stage12Product validator is absent or unsafe")
        reviewed_commit, integration_commit = proof_commits(snapshot)
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
