"""Source-only and isolated-ref tests for Stage12LegacyCutCoordinatorV2."""

from __future__ import annotations

import copy
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[2]
FIXTURE = (
    REPOSITORY
    / "tests/fixtures/vnext/stage12/stage12-legacy-cut-coordinator.v2.json"
)
SCHEMA = (
    REPOSITORY
    / "contracts/vnext/final-chain/stage12-legacy-cut-coordinator.v2.schema.json"
)
PACKET_ROOT = Path(
    "/private/tmp/maestro-vnext-host-injection-successor-packet-v7"
)
sys.path.insert(0, str(ROOT))

import coordinator  # type: ignore[import-not-found]  # noqa: E402


class Stage12LegacyCutCoordinatorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))

    def _git(self, repo: Path, *arguments: str) -> str:
        result = subprocess.run(
            ["git", "-C", str(repo), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        return result.stdout.strip()

    def _materialize_artifacts(self, value: dict[str, object], root: Path) -> None:
        packet_rows = [
            value["approved_packet"],
            value["protected_primary"]["boundary"],
            value["source_git_binding"]["artifact"],
        ]
        if not PACKET_ROOT.is_dir():
            self.skipTest("authoritative V7 packet is unavailable")
        for binding in packet_rows:
            path = root / binding["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(PACKET_ROOT / Path(binding["path"]).name, path)
            raw = path.read_bytes()
            self.assertEqual(binding["byte_length"], len(raw))
            self.assertEqual(binding["sha256"], coordinator.digest(raw))
        for index, binding in enumerate(
            gate["evidence"] for gate in value["retained_inputs"]
        ):
            path = root / binding["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            raw = f"bound-stage12-input-{index}\n".encode()
            path.write_bytes(raw)
            binding["byte_length"] = len(raw)
            binding["sha256"] = coordinator.digest(raw)

    def _repository(
        self, root: Path, value: dict[str, object]
    ) -> tuple[Path, str, str]:
        repo = root / "isolated-successor"
        self._git(root, "init", str(repo))
        self._git(repo, "config", "user.name", "Stage12 Test")
        self._git(repo, "config", "user.email", "stage12@example.invalid")
        tracked = repo / "candidate.txt"
        tracked.write_text("preimage\n", encoding="utf-8")
        self._git(repo, "add", "candidate.txt")
        self._git(repo, "commit", "-m", "preimage")
        preimage = self._git(repo, "rev-parse", "HEAD")
        preimage_tree = self._git(repo, "rev-parse", "HEAD^{tree}")
        tracked.write_text("postimage\n", encoding="utf-8")
        self._git(repo, "commit", "-am", "declared postimage")
        postimage = self._git(repo, "rev-parse", "HEAD")
        postimage_tree = self._git(repo, "rev-parse", "HEAD^{tree}")
        candidate_ref = value["candidate_ref"]
        candidate_ref["repository_realpath"] = str(repo.resolve())
        candidate_ref["git_common_dir_realpath"] = str((repo / ".git").resolve())
        value["source_git_binding"]["git_common_dir_realpath"] = str(
            (repo / ".git").resolve()
        )
        candidate_ref["expected_preimage"] = {
            "commit": preimage,
            "tree": preimage_tree,
        }
        candidate_ref["declared_postimage"] = {
            "commit": postimage,
            "tree": postimage_tree,
        }
        candidate_ref["declared_postimage_parent"] = preimage
        value["cas_observation"] = {
            "state": "not_executed",
            "observed_commit": preimage,
            "observed_tree": preimage_tree,
        }
        self._git(repo, "update-ref", "refs/heads/main", preimage)
        self._git(repo, "update-ref", candidate_ref["ref"], preimage)
        return repo, preimage, postimage

    def test_exact_fixture_and_schema_are_strict_and_aligned(self) -> None:
        coordinator.validate_contract(self.fixture)
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        self.assertFalse(schema["additionalProperties"])
        self.assertEqual(
            schema["properties"]["schema_version"]["const"], coordinator.SCHEMA
        )
        self.assertEqual(
            schema["properties"]["approved_packet_identity"]["const"],
            coordinator.PACKET_IDENTITY,
        )
        self.assertEqual(
            schema["properties"]["canonical_ancestry"]["const"],
            coordinator.CANONICAL_ANCESTRY,
        )
        self.assertEqual(
            [
                row["properties"]["kind"]["const"]
                for row in (
                    schema["$defs"][name]["allOf"][1]
                    for name in (
                        "source_cases",
                        "sightings",
                        "classifications",
                        "overlaps",
                        "loss",
                        "quarantine",
                        "epoch",
                        "activation",
                        "parity",
                        "consumer_zero",
                        "reader_zero",
                        "hold_zero",
                        "rollback",
                        "namespace",
                        "release",
                        "proof_registry",
                    )
                )
            ],
            [kind for kind, _state in coordinator.GATE_ORDER],
        )

    def test_packet_gate_order_zero_and_parity_mutants_refuse(self) -> None:
        mutants = []
        packet = copy.deepcopy(self.fixture)
        packet["approved_packet_identity"] = "sha256:" + "f" * 64
        mutants.append(packet)
        reordered = copy.deepcopy(self.fixture)
        reordered["retained_inputs"][0], reordered["retained_inputs"][1] = (
            reordered["retained_inputs"][1],
            reordered["retained_inputs"][0],
        )
        mutants.append(reordered)
        consumer = copy.deepcopy(self.fixture)
        consumer["retained_inputs"][9]["count"] = 1
        mutants.append(consumer)
        namespace = copy.deepcopy(self.fixture)
        namespace["retained_inputs"][13]["entry_count"] = 209
        mutants.append(namespace)
        primary = copy.deepcopy(self.fixture)
        primary["candidate_ref"]["ref"] = primary["protected_primary"]["ref"]
        mutants.append(primary)
        ancestry = copy.deepcopy(self.fixture)
        ancestry["canonical_ancestry"][3]["commit"] = "f" * 40
        mutants.append(ancestry)
        for mutant in mutants:
            with self.subTest(mutant=mutants.index(mutant)), self.assertRaises(
                coordinator.CoordinatorError
            ):
                coordinator.validate_contract(mutant)

    def test_exact_canonical_lane_ancestry_reaches_affected_stage12_rebind(
        self,
    ) -> None:
        self.assertEqual(
            coordinator.CANONICAL_ANCESTRY[-2],
            {
                "lane": "AuthorityOwnerModulePlacementCorrection",
                "commit": "acd2a469d058f5a17162d3f0a5a44fe394cf6676",
                "tree": "b97282eadfc10ad552cdc5b46bef7b62454367ef",
            },
        )
        self.assertEqual(
            coordinator.CANONICAL_ANCESTRY[-1],
            {
                "lane": "Stage12ProductAffectedSuffixRebind",
                "commit": "e03d21b64995a20cfda3e90d706048ca79038f30",
                "tree": "600171763b9e782d494fa0c04ba5de9a5d7fa5a4",
            },
        )
        coordinator._validate_candidate_ancestry(
            REPOSITORY, coordinator.CANONICAL_ANCESTRY[-1]["commit"]
        )

    def test_bound_artifact_bytes_and_nonregular_shapes_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value = copy.deepcopy(self.fixture)
            self._materialize_artifacts(value, root)
            coordinator.validate_bound_artifacts(value, root)
            source_mutant = copy.deepcopy(value)
            source_mutant["source_git_binding"][
                "git_common_dir_realpath"
            ] = "/foreign/git-common-dir"
            with self.assertRaisesRegex(
                coordinator.CoordinatorError,
                "V7 packet, source Git, or protected-primary binding differs",
            ):
                coordinator.validate_bound_artifacts(source_mutant, root)
            target = root / value["retained_inputs"][4]["evidence"]["path"]
            target.write_text("drift\n", encoding="utf-8")
            with self.assertRaisesRegex(coordinator.CoordinatorError, "bytes differ"):
                coordinator.validate_bound_artifacts(value, root)

    def test_one_expected_preimage_cas_updates_only_the_isolated_candidate_ref(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value = copy.deepcopy(self.fixture)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            self._materialize_artifacts(value, artifacts)
            repo, preimage, postimage = self._repository(root, value)
            with mock.patch.object(
                coordinator, "validate_bound_artifacts"
            ) as artifact_validation, mock.patch.object(
                coordinator, "_validate_candidate_ancestry"
            ) as ancestry_validation, mock.patch.object(
                coordinator, "_git", wraps=coordinator._git
            ) as git_call:
                result = coordinator.execute_isolated_candidate_ref_cas(
                    value, artifacts, repo
                )
            artifact_validation.assert_called()
            ancestry_validation.assert_called()
            updates = [
                call
                for call in git_call.call_args_list
                if len(call.args) > 1 and call.args[1] == "update-ref"
            ]
            self.assertEqual(len(updates), 1)
            self.assertEqual(
                self._git(repo, "rev-parse", value["candidate_ref"]["ref"]),
                postimage,
            )
            self.assertEqual(self._git(repo, "rev-parse", "refs/heads/main"), preimage)
            self.assertEqual(
                result["cas_observation"]["state"], "exact_declared_postimage"
            )

    def test_postimage_replay_is_recognized_without_a_second_ref_write(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value = copy.deepcopy(self.fixture)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            self._materialize_artifacts(value, artifacts)
            repo, _preimage, _postimage = self._repository(root, value)
            with mock.patch.object(
                coordinator, "validate_bound_artifacts"
            ) as artifact_validation, mock.patch.object(
                coordinator, "_validate_candidate_ancestry"
            ) as ancestry_validation:
                first = coordinator.execute_isolated_candidate_ref_cas(
                    value, artifacts, repo
                )
                with mock.patch.object(
                    coordinator, "_git", wraps=coordinator._git
                ) as git_call:
                    second = coordinator.execute_isolated_candidate_ref_cas(
                        first, artifacts, repo
                    )
            artifact_validation.assert_called()
            ancestry_validation.assert_called()
            self.assertFalse(
                any(
                    len(call.args) > 1 and call.args[1] == "update-ref"
                    for call in git_call.call_args_list
                )
            )
            self.assertEqual(
                second["cas_observation"]["state"], "exact_declared_postimage"
            )

    def test_foreign_crash_state_refuses_without_a_ref_write(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value = copy.deepcopy(self.fixture)
            artifacts = root / "artifacts"
            artifacts.mkdir()
            self._materialize_artifacts(value, artifacts)
            repo, _preimage, _postimage = self._repository(root, value)
            (repo / "candidate.txt").write_text("foreign\n", encoding="utf-8")
            self._git(repo, "commit", "-am", "foreign state")
            foreign = self._git(repo, "rev-parse", "HEAD")
            self._git(repo, "update-ref", value["candidate_ref"]["ref"], foreign)
            with mock.patch.object(
                coordinator, "validate_bound_artifacts"
            ) as artifact_validation, mock.patch.object(
                coordinator, "_validate_candidate_ancestry"
            ) as ancestry_validation, mock.patch.object(
                coordinator, "_git", wraps=coordinator._git
            ) as git_call, self.assertRaisesRegex(
                coordinator.CoordinatorError, "neither exact preimage nor postimage"
            ):
                coordinator.execute_isolated_candidate_ref_cas(
                    value, artifacts, repo
                )
            artifact_validation.assert_called()
            ancestry_validation.assert_called()
            self.assertFalse(
                any(
                    len(call.args) > 1 and call.args[1] == "update-ref"
                    for call in git_call.call_args_list
                )
            )

    def test_coordinator_source_has_one_fixed_ref_write_and_no_product_effects(
        self,
    ) -> None:
        source = (ROOT / "coordinator.py").read_text(encoding="utf-8")
        self.assertEqual(source.count('"update-ref"'), 1)
        for token in (
            "LegacyRemovalGuard",
            "os.unlink",
            "shutil.rmtree",
            '"checkout"',
            '"merge"',
            "shell=True",
        ):
            self.assertNotIn(token, source)


if __name__ == "__main__":
    unittest.main()
