"""Behavioral tests for the frozen Stage12Product proof wrapper."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

import stage12_product_proof  # type: ignore[import-not-found]  # noqa: E402


class Stage12ProductProofBehaviorTests(unittest.TestCase):
    def _git(self, repository: Path, *arguments: str) -> str:
        result = subprocess.run(
            ["git", "-C", str(repository), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        return result.stdout.strip()

    def _commit(self, repository: Path, path: str, contents: str, message: str) -> str:
        target = repository / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(contents, encoding="utf-8")
        self._git(repository, "add", path)
        self._git(repository, "commit", "-m", message)
        return self._git(repository, "rev-parse", "HEAD")

    def _repository(self, root: Path, mixed: bool) -> tuple[Path, str, str, str]:
        repository = root / "ancestry"
        self._git(root, "init", str(repository))
        self._git(repository, "config", "user.name", "Proof Test")
        self._git(repository, "config", "user.email", "proof@example.invalid")
        owner = self._commit(
            repository,
            "tools/vnext_contracts/stage12/owner.txt",
            "owner\n",
            "owner checkpoint",
        )
        self._commit(
            repository,
            "tools/vnext_contracts/final_chain/lane.txt",
            "external\n",
            "external lane",
        )
        if mixed:
            self._commit(repository, "unowned.txt", "mixed\n", "unowned mutation")
            (repository / "unowned.txt").unlink()
            self._git(repository, "add", "-u", "unowned.txt")
            self._git(repository, "commit", "-m", "restore final tree")
        reviewed = self._commit(
            repository,
            "tools/vnext_contracts/stage12/product.txt",
            "reviewed\n",
            "reviewed candidate",
        )
        reviewed_tree = self._git(repository, "rev-parse", f"{reviewed}^{{tree}}")
        stage11 = self._git(
            repository,
            "commit-tree",
            self._git(repository, "rev-parse", f"{owner}^{{tree}}"),
            "-p",
            owner,
            "-m",
            "synthetic stage 11",
        )
        final = self._git(
            repository,
            "commit-tree",
            reviewed_tree,
            "-p",
            stage11,
            "-p",
            reviewed,
            "-m",
            "synthetic final integration",
        )
        return repository, owner, reviewed, final

    def _snapshot_root(
        self, root: Path, owner: str, reviewed: str, final: str
    ) -> tuple[Path, Path]:
        source = root / "gitless-snapshot"
        validator = source / "tools/vnext_contracts/stage10/validate.py"
        validator.parent.mkdir(parents=True)
        validator.write_text(
            f"""\
import argparse
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("--ancestry-repository", required=True)
parser.add_argument("--snapshot-root", required=True)
parser.add_argument("--final-ref", required=True)
args = parser.parse_args()
commits = subprocess.run(
    ["git", "-C", args.ancestry_repository, "rev-list", "--reverse", "{owner}.." + args.final_ref],
    check=True,
    stdout=subprocess.PIPE,
    text=True,
).stdout.splitlines()
for commit in commits:
    paths = subprocess.run(
        ["git", "-C", args.ancestry_repository, "diff-tree", "--no-commit-id", "--name-only", "-r", commit],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.splitlines()
    if "unowned.txt" in paths:
        raise SystemExit(9)
raise SystemExit(0)
""",
            encoding="utf-8",
        )
        snapshot = {
            "schema_version": stage12_product_proof.SNAPSHOT_SCHEMA,
            "state": "frozen",
            "stage12_reviewed_candidate": {
                "commit": reviewed,
                "tree": "a" * 40,
            },
            "final_integration": {
                "commit": final,
                "tree": "a" * 40,
            },
        }
        snapshot_path = source / "snapshot.json"
        snapshot_path.write_text(json.dumps(snapshot), encoding="utf-8")
        return source, snapshot_path

    def _run_case(self, root: Path, mixed: bool) -> int:
        repository, owner, reviewed, final = self._repository(root, mixed)
        source, snapshot_path = self._snapshot_root(root, owner, reviewed, final)
        argv = [
            "stage12_product_proof.py",
            "--ancestry-repository",
            str(repository),
            "--snapshot",
            str(snapshot_path),
            "--snapshot-root",
            str(source),
        ]
        with mock.patch.object(
            stage12_product_proof,
            "STAGE12_PRODUCT_CORRECTION_COMMIT",
            owner,
        ), mock.patch.object(sys, "argv", argv):
            return stage12_product_proof.main()

    def test_interleaved_lane_pure_history_is_validated_through_reviewed_candidate(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(self._run_case(Path(directory), mixed=False), 0)

    def test_mixed_then_restored_history_is_not_truncated_at_owner_checkpoint(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(self._run_case(Path(directory), mixed=True), 9)

    def test_reviewed_candidate_must_be_final_integrations_direct_second_parent(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, owner, reviewed, final = self._repository(
                Path(directory), mixed=False
            )
            with mock.patch.object(
                stage12_product_proof,
                "STAGE12_PRODUCT_CORRECTION_COMMIT",
                owner,
            ):
                stage12_product_proof.require_stage12_ancestry(
                    repository, reviewed, final
                )
                with self.assertRaisesRegex(
                    stage12_product_proof.ProofError,
                    "direct second parent",
                ):
                    stage12_product_proof.require_stage12_ancestry(
                        repository, owner, final
                    )


if __name__ == "__main__":
    unittest.main()
