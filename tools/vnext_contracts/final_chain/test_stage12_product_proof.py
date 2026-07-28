"""Behavioral tests for the frozen Stage12Product proof wrapper."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator
from unittest import mock


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[2]
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

    def _linear_v8_repository(
        self, root: Path
    ) -> tuple[Path, list[dict[str, str]], list[dict[str, str]]]:
        repository = root / "v8-ancestry"
        self._git(root, "init", str(repository))
        self._git(repository, "config", "user.name", "Proof Test")
        self._git(repository, "config", "user.email", "proof@example.invalid")
        ledger: list[dict[str, str]] = []
        checkpoints: list[dict[str, str]] = []
        for index, name in enumerate(stage12_product_proof.V8_CHECKPOINT_NAMES):
            if index > 0:
                correction = self._commit(
                    repository,
                    f"correction-{index}.txt",
                    f"{name} correction\n",
                    f"{name} correction",
                )
                ledger.append(
                    {
                        "commit": correction,
                        "tree": self._git(
                            repository, "rev-parse", f"{correction}^{{tree}}"
                        ),
                    }
                )
            commit = self._commit(
                repository,
                f"checkpoint-{index}.txt",
                f"{name}\n",
                name,
            )
            tree = self._git(repository, "rev-parse", f"{commit}^{{tree}}")
            ledger.append({"commit": commit, "tree": tree})
            checkpoints.append(
                {
                    "name": name,
                    "tip_commit": commit,
                    "tip_tree": tree,
                }
            )
        return repository, ledger, checkpoints

    def _v8_snapshot(
        self,
        ledger: list[dict[str, str]],
        checkpoints: list[dict[str, str]],
    ) -> dict[str, object]:
        return {
            "schema_version": stage12_product_proof.SNAPSHOT_SCHEMA,
            "state": "frozen",
            "v8_closure": stage12_product_proof.V8_CLOSURE,
            "v8_first_parent_ledger": ledger,
            "v8_logical_checkpoints": checkpoints,
            "historical_stage12": {
                "checkpoint": stage12_product_proof.HISTORICAL_STAGE12_CHECKPOINT,
                "tree": stage12_product_proof.HISTORICAL_STAGE12_TREE,
                "validation_ref": (
                    stage12_product_proof.HISTORICAL_STAGE12_VALIDATION_REF
                ),
            },
            "effect_boundary": {
                "coordinator_cas_execution": "not_performed_by_proof_runner",
                "live_installation_mutation": False,
                "primary_mutation": False,
                "proof_runner_effect_inert": True,
                "receipt_or_pointer_publication": False,
                "seal_execution": False,
            },
            "stage12_reviewed_candidate": {
                "commit": checkpoints[-2]["tip_commit"],
                "tree": checkpoints[-2]["tip_tree"],
            },
            "final_integration": {
                "commit": checkpoints[-1]["tip_commit"],
                "tree": checkpoints[-1]["tip_tree"],
            },
        }

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

    def test_v8_snapshot_binds_exact_linear_checkpoint_and_typed_closure(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, ledger, checkpoints = self._linear_v8_repository(
                Path(directory)
            )
            closure = {
                **stage12_product_proof.V8_CLOSURE,
                "design_commit": ledger[0]["commit"],
                "design_tree": ledger[0]["tree"],
            }
            with mock.patch.object(
                stage12_product_proof, "V8_DESIGN_COMMIT", ledger[0]["commit"]
            ), mock.patch.object(
                stage12_product_proof, "V8_DESIGN_TREE", ledger[0]["tree"]
            ), mock.patch.object(stage12_product_proof, "V8_CLOSURE", closure):
                snapshot = self._v8_snapshot(ledger, checkpoints)
                snapshot["v8_closure"] = closure
                observed_ledger, observed_checkpoints = (
                    stage12_product_proof.v8_ancestry(snapshot)
                )
                stage12_product_proof.require_v8_linear_ancestry(
                    repository, observed_ledger, observed_checkpoints
                )
                mutant = json.loads(json.dumps(snapshot))
                mutant["v8_logical_checkpoints"][3]["name"] = "HiddenLane"
                with self.assertRaisesRegex(
                    stage12_product_proof.ProofError,
                    "checkpoint order differs",
                ):
                    stage12_product_proof.v8_ancestry(mutant)

    def test_v8_linear_check_rejects_hidden_interleaved_commit(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, ledger, checkpoints = self._linear_v8_repository(
                Path(directory)
            )
            ledger.pop(5)
            with mock.patch.object(
                stage12_product_proof, "V8_DESIGN_COMMIT", ledger[0]["commit"]
            ), mock.patch.object(
                stage12_product_proof, "V8_DESIGN_TREE", ledger[0]["tree"]
            ):
                with self.assertRaisesRegex(
                    stage12_product_proof.ProofError,
                    "direct single-parent child",
                ):
                    stage12_product_proof.require_v8_linear_ancestry(
                        repository,
                        ledger,
                        stage12_product_proof.v8_checkpoints(
                            self._v8_snapshot(ledger, checkpoints)
                        ),
                    )

    def test_v8_logical_checkpoint_tips_must_be_ordered_and_non_overlapping(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            _, ledger, checkpoints = self._linear_v8_repository(Path(directory))
            snapshot = self._v8_snapshot(ledger, checkpoints)
            snapshot["v8_logical_checkpoints"][4]["tip_commit"] = checkpoints[3][
                "tip_commit"
            ]
            snapshot["v8_logical_checkpoints"][4]["tip_tree"] = checkpoints[3][
                "tip_tree"
            ]
            with mock.patch.object(
                stage12_product_proof, "V8_DESIGN_COMMIT", ledger[0]["commit"]
            ), mock.patch.object(
                stage12_product_proof, "V8_DESIGN_TREE", ledger[0]["tree"]
            ):
                with self.assertRaisesRegex(
                    stage12_product_proof.ProofError,
                    "ordered and non-overlapping",
                ):
                    stage12_product_proof.v8_ancestry(snapshot)

    def test_release_inputs_bind_complete_known_prefix_without_self_reference(
        self,
    ) -> None:
        release_inputs = json.loads(
            (
                REPOSITORY
                / "tests/fixtures/vnext/stage12/release-proof-inputs.v1.json"
            ).read_text(encoding="utf-8")
        )
        materialization = release_inputs["v8_snapshot_materialization"]
        ledger = materialization["known_first_parent_ledger"]
        expected_commits = [
            "bb7b1ee0e51fa591b21943e8c7d50844cb4d0b05",
            "23c8689d0d6f14eb8a237481a1a64c83b42be996",
            "5807457f5916e965bd53a1c0a8a9b12a6f4676e1",
            "d004b05a6da118ba3709fbdb895655aae16d7bd1",
            "9f26743a52f02d1785a52d8cb3d13e3c3b4f31b9",
            "80dd8de02d13e967b238c6fd3b82e9b72ea0f18b",
            "90beb203a5f8490e89a8fda07dbccd01cb947548",
            "fb0a32426e34e60ca8849016173a906a0cfd7d7d",
            "b97070f896b3b42a1db95fe9dcd9e952fdb2ca39",
            "d14c3647af91ed2007aef250561e4956759f3565",
            "ba1b8898b1292b53059819fe9f3d39e240b10dbc",
            "69c49d3f8a175d8f3506c48d0b390121a23767c8",
            "2690ac5a243d6821c318b8985532aa514c400274",
            "97465f7121b629c5b898df4b11eb405032053f71",
            "92eb07a5a25f152688fda3d002c3ecf9bb916b82",
            "82cc9bec06a7185100a78162f74f8516fd45b2a1",
            "268f7decc2ba5478899d9a13dc500ccc4d9ea3d6",
            "7fcc67df28ed497d02d84dc21089c7bd775ffb68",
            "92e7b5dc55e9c24c815314540755aaed926050f7",
            "e627a62579308196e478c2ac28947eab862cbdc5",
            "a836a5f1d96c9de5c164df8f421d3bdaf03146e4",
            "7d7f342883892e4a366b3cad645044fdecc29b70",
            "7ec45cd182488bec40ae96a5102f9ab4c3cb912b",
            "849a59d9c1290fbc0235b0c92fa1325923e65b08",
        ]
        self.assertEqual(
            [row["commit"] for row in ledger],
            expected_commits,
        )
        checkpoints = [
            {
                "name": row["name"],
                "commit": row["tip_commit"],
                "tree": row["tip_tree"],
            }
            for row in materialization["known_logical_checkpoint_tips"]
        ]
        stage12_product_proof.require_v8_linear_ancestry(
            REPOSITORY, ledger, checkpoints
        )
        self.assertEqual(
            materialization["unmaterialized_logical_checkpoints"],
            ["ExternalProofControl", "MainIntegrationFinalClosure"],
        )
        self.assertFalse(
            materialization["canonical_regeneration"][
                "repository_correction_commit"
            ]
        )
        self.assertNotIn("stage12_reviewed_candidate", materialization)
        self.assertNotIn("final_integration", materialization)

    def test_v8_closure_runs_historical_v4_and_coordinator_validators_only(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            historical = root / "historical"
            snapshot = root / "snapshot"
            historical_validator = (
                snapshot / "tools/vnext_contracts/stage10/validate.py"
            )
            paths = [
                historical_validator,
                snapshot / "tools/vnext_contracts/stage11/validate_v4.py",
                snapshot / "tools/vnext_contracts/stage12/coordinator_v3.py",
                snapshot
                / "tests/fixtures/vnext/stage11/live_set_v4_contract.v1.json",
                snapshot / "tests/fixtures/vnext/stage11/root-universe.v1.json",
                snapshot
                / "tests/fixtures/vnext/stage12/stage12-legacy-cut-coordinator.v3.json",
            ]
            for path in paths:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("proof input\n", encoding="utf-8")

            @contextmanager
            def frozen_history(_: Path) -> Iterator[Path]:
                yield historical

            with mock.patch.object(
                stage12_product_proof,
                "historical_stage12_snapshot",
                frozen_history,
            ), mock.patch.object(
                stage12_product_proof,
                "_run_validator",
                side_effect=[0, 0, 0],
            ) as run_validator:
                self.assertEqual(
                    stage12_product_proof.run_v8_closure(root, snapshot), 0
                )
            self.assertEqual(run_validator.call_count, 3)
            historical_args = run_validator.call_args_list[0].args[0]
            self.assertEqual(
                historical_args[-1],
                stage12_product_proof.HISTORICAL_STAGE12_VALIDATION_REF,
            )
            v4_args = run_validator.call_args_list[1].args[0]
            self.assertIn("--source-root", v4_args)
            self.assertEqual(v4_args[-1], str(snapshot))
            coordinator_args = run_validator.call_args_list[2].args[0]
            self.assertEqual(coordinator_args[2], "--contract")

    def test_v8_proof_source_has_no_ref_cas_or_product_effect(self) -> None:
        source = Path(stage12_product_proof.__file__).read_text(encoding="utf-8")
        self.assertNotIn("update" + "-ref", source)
        self.assertNotIn("execute_isolated_candidate_ref_cas", source)
        self.assertNotIn("--mutant-suite", source)


if __name__ == "__main__":
    unittest.main()
