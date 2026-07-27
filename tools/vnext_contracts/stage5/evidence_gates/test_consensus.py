from __future__ import annotations

import copy
import io
import json
import re
import tarfile
import tempfile
import unittest
from collections.abc import Iterator
from pathlib import Path
from typing import Any
from unittest import mock

from tools.vnext_contracts.stage5.evidence_gates import (
    behavior,
    build as stage5_build,
    consensus,
    harness,
    validate,
)


class Stage5ConsensusTests(unittest.TestCase):
    def test_all_independent_engines_bind_the_exact_same_source_set(self) -> None:
        ruby_source = (
            Path(__file__).with_name("verify.rb").read_text(encoding="utf-8")
        )
        match = re.search(r"SOURCE_PATHS = %w\[(.*?)\]\.freeze", ruby_source, re.S)
        self.assertIsNotNone(match)
        ruby_paths = tuple(sorted(match.group(1).split()))
        expected = tuple(sorted(stage5_build.SOURCE_PATHS))
        self.assertEqual(expected, tuple(sorted(validate.SOURCE_PATHS)))
        self.assertEqual(expected, tuple(sorted(consensus.ARTIFACT_SOURCE_PATHS)))
        self.assertEqual(expected, ruby_paths)
        self.assertIn("src/foundation/mod.rs", expected)
        self.assertIn("src/foundation/core/mod.rs", expected)
        self.assertIn("src/domain/contract/mod.rs", expected)
        self.assertIn("src/domain/persistence/tests/mod.rs", expected)
        self.assertIn(
            "src/domain/installation/consumer_snapshot_stage11_seed.rs",
            expected,
        )

    def test_engine_local_binary_hashes_are_validated_before_semantic_consensus(self) -> None:
        runs = self.behavior_runs("a" * 64)
        self.assertEqual(
            self.semantic_runs(runs),
            self.semantic_runs(self.behavior_runs("b" * 64)),
        )
        for label, mutation in (
            ("uppercase", "A" * 64),
            ("short", "a" * 63),
            ("non-hex", "g" * 64),
        ):
            with self.subTest(label=label), self.assertRaises(RuntimeError):
                self.semantic_runs(self.behavior_runs(mutation))
        runs[-1]["binary_sha256"] = "b" * 64
        with self.assertRaises(RuntimeError):
            self.semantic_runs(runs)

    def test_semantic_consensus_excludes_only_engine_local_binary_hashes(self) -> None:
        self.semantic_runs(self.behavior_runs("a" * 64))
        for field, value in (
            ("command", ["maestro", "other", "--exact", "--nocapture"]),
            ("name", "other"),
            ("result", "fail"),
        ):
            runs = self.behavior_runs("b" * 64)
            runs[0]["tests"][0][field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.semantic_runs(runs)
        runs = self.behavior_runs("b" * 64)
        runs[0]["label"] = "other"
        with self.assertRaises(RuntimeError):
            self.semantic_runs(runs)
        for field, run_value in (
            ("passed", 0),
            ("label", ""),
            ("label", "Invalid Label"),
        ):
            runs = self.behavior_runs("b" * 64)
            runs[0][field] = run_value
            with self.subTest(field=field, value=run_value), self.assertRaises(
                RuntimeError
            ):
                self.semantic_runs(runs)
        runs = self.behavior_runs("b" * 64)
        runs[0]["tests"] = []
        with self.assertRaises(RuntimeError):
            self.semantic_runs(runs)
        runs = self.behavior_runs("b" * 64)
        runs.insert(1, copy.deepcopy(runs[0]))
        runs[1]["tests"][0]["name"] = "other_exact"
        runs[1]["tests"][0]["command"][1] = "other_exact"
        with self.assertRaises(RuntimeError):
            self.semantic_runs(runs, expected_passes=2)
        with self.assertRaises(RuntimeError):
            self.semantic_runs(self.behavior_runs("b" * 64), expected_passes=2)
        for field, mutant_value in (
            ("label", "other"),
            ("command", ["maestro", "other", "--exact", "--nocapture"]),
            ("passed", 1),
            ("rejected", False),
            ("result", "pass"),
            ("substituted_for", "other"),
        ):
            runs = self.behavior_runs("b" * 64)
            runs[-1][field] = mutant_value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.semantic_runs(runs)

    @staticmethod
    def semantic_runs(
        runs: list[dict[str, Any]], *, expected_passes: int = 1
    ) -> list[dict[str, Any]]:
        with mock.patch.object(
            consensus, "EXPECTED_BEHAVIOR_TESTS", expected_passes
        ), mock.patch.object(
            consensus, "EXPECTED_NORMAL_RUNS", (("behavior", "maestro", 1),)
        ):
            return consensus.semantic_behavior_runs(runs)

    @staticmethod
    def behavior_runs(binary_sha256: str) -> list[dict[str, Any]]:
        return [
            {
                "binary_sha256": binary_sha256,
                "label": "behavior",
                "passed": 1,
                "tests": [
                    {
                        "command": ["maestro", "exact", "--exact", "--nocapture"],
                        "name": "exact",
                        "result": "pass",
                    }
                ],
            },
            {
                "binary_sha256": binary_sha256,
                "command": [
                    "maestro",
                    "exact_same_count_substitution_mutant",
                    "--exact",
                    "--nocapture",
                ],
                "label": "same-count-substitution-mutant",
                "passed": 0,
                "rejected": True,
                "result": "rejected",
                "substituted_for": "exact",
            },
        ]

    def test_receipt_identity_rejects_a_self_consistent_payload_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            stage5_build.build(output, Path("/bin/true"), Path("/bin/true"), False)
            artifact: dict[str, Any] = json.loads(
                (output / "evidence-gates.v1.json").read_text(encoding="ascii")
            )
        catalog = json.loads(
            (
                stage5_build.WORKSPACE
                / "contracts/vnext/catalogs/generated/catalog-01-observation.json"
            ).read_text(encoding="ascii")
        )
        observations = validate.observation_rows(catalog)
        sources = [validate.row(path) for path in sorted(validate.SOURCE_PATHS)]
        predecessors = [validate.row(path) for path in validate.PREDECESSOR_PATHS]
        encoded = bytes.fromhex(artifact["cbor_hex"])

        def validator_accepts(value: object) -> bool:
            return validate.exact_artifact_grammar(
                value,
                catalog_manifest_id=catalog["manifest_id"],
                observations=observations,
                sources=sources,
                predecessors=predecessors,
                encoded=encoded,
            )
        self.assertTrue(stage5_build.exact_behavior(artifact["behavior"]))
        self.assertTrue(validator_accepts(artifact))
        self.assertTrue(consensus.validate_artifact_grammar(artifact, require_full=False))
        for mutant in self.recursive_shape_mutants(artifact["behavior"]):
            preflight_mutant = copy.deepcopy(artifact)
            preflight_mutant["behavior"] = mutant
            self.assertFalse(stage5_build.exact_behavior(mutant))
            self.assertFalse(validate.exact_behavior(mutant))
            self.assertFalse(validator_accepts(preflight_mutant))
            self.assertFalse(
                consensus.validate_artifact_grammar(preflight_mutant, require_full=False)
            )

        runs = self.full_behavior_runs()
        artifact["behavior"] = {"passed": behavior.EXPECTED_TESTS, "runs": runs}
        self.assertTrue(stage5_build.exact_behavior(artifact["behavior"]))
        self.assertTrue(validator_accepts(artifact))
        self.assertTrue(consensus.validate_artifact_grammar(artifact, require_full=True))
        for mutant in self.recursive_shape_mutants(artifact["behavior"]):
            self.assertFalse(stage5_build.exact_behavior(mutant))
            self.assertFalse(validate.exact_behavior(mutant))
        for mutant in self.recursive_shape_mutants(artifact):
            self.assertFalse(validator_accepts(mutant))
            self.assertFalse(consensus.validate_artifact_grammar(mutant, require_full=True))

        value = {
            "artifact_id": artifact["artifact_id"],
            "artifact_sha256": consensus.sha256(consensus.pretty_json(artifact)),
            "behavior_manifest_identity": consensus.EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
            "behavior_passed": behavior.EXPECTED_TESTS,
            "behavior_runs": runs,
            "builder_sha256": dict(
                (row[0], row[2]) for row in artifact["source_closure"]
            )["tools/vnext_contracts/stage5/evidence_gates/build.py"],
            "diagnostic_proof_claim": consensus.DIAGNOSTIC_PROOF_CLAIM,
            "publication_state": "inactive_candidate",
            "schema_version": "maestro.vnext.stage5.python-builder-receipt.v1",
            "source_closure_sha256": consensus.sha256(
                consensus.canonical_json(artifact["source_closure"])
            ),
        }
        receipt: dict[str, Any] = {
            **value,
            "receipt_identity": f"sha256:{consensus.sha256(consensus.canonical_json(value))}",
        }
        self.assertTrue(consensus.validate_engine_receipt("builder", receipt, artifact))
        for mutant in self.recursive_shape_mutants(receipt):
            if isinstance(mutant, dict) and "receipt_identity" in mutant:
                identity_value = {
                    key: item for key, item in mutant.items() if key != "receipt_identity"
                }
                mutant["receipt_identity"] = (
                    f"sha256:{consensus.sha256(consensus.canonical_json(identity_value))}"
                )
            if mutant == receipt:
                continue
            self.assertFalse(consensus.validate_engine_receipt("builder", mutant, artifact))

        harness_receipt: dict[str, Any] = {
            "diagnostic_proof_claim": consensus.DIAGNOSTIC_PROOF_CLAIM,
            "manifest_identity": harness.EXPECTED_TEST_MANIFEST_IDENTITY,
            "passed": len(harness.EXPECTED_TESTS),
            "schema_version": "maestro.vnext.stage5.proof-harness-receipt.v1",
            "tests": list(harness.EXPECTED_TESTS),
        }
        self.assertTrue(consensus.validate_harness_receipt(harness_receipt))
        for mutant in self.recursive_shape_mutants(harness_receipt):
            self.assertFalse(consensus.validate_harness_receipt(mutant))

    @staticmethod
    def full_behavior_runs() -> list[dict[str, Any]]:
        runs = [
            {
                "binary_sha256": "a" * 64,
                "label": label,
                "passed": len(tests),
                "tests": [
                    {
                        "command": [target, test, "--exact", "--nocapture"],
                        "name": test,
                        "result": "pass",
                    }
                    for test in tests
                ],
            }
            for label, target, tests in behavior.EXPECTED_RUNS
        ]
        first_target = behavior.EXPECTED_RUNS[0][1]
        first_test = behavior.EXPECTED_RUNS[0][2][0]
        runs.append(
            {
                "binary_sha256": "a" * 64,
                "command": [
                    first_target,
                    f"{first_test}_same_count_substitution_mutant",
                    "--exact",
                    "--nocapture",
                ],
                "label": "same-count-substitution-mutant",
                "passed": 0,
                "rejected": True,
                "result": "rejected",
                "substituted_for": first_test,
            }
        )
        return runs

    @classmethod
    def recursive_shape_mutants(cls, value: Any) -> Iterator[Any]:
        if isinstance(value, dict):
            yield {**copy.deepcopy(value), "__unexpected_claim__": "prod-host-verified"}
            for key, child in value.items():
                dict_missing = copy.deepcopy(value)
                del dict_missing[key]
                yield dict_missing
                renamed = copy.deepcopy(value)
                renamed[f"__renamed_{key}"] = renamed.pop(key)
                yield renamed
                for mutated_child in cls.recursive_shape_mutants(child):
                    mutant: Any = copy.deepcopy(value)
                    mutant[key] = mutated_child
                    yield mutant
            yield [copy.deepcopy(value)]
        elif isinstance(value, list):
            yield [*copy.deepcopy(value), {"proof": {"claim": "live-host-certified"}}]
            for index, child in enumerate(value):
                list_missing = list(copy.deepcopy(value))
                del list_missing[index]
                yield list_missing
                for mutated_child in cls.recursive_shape_mutants(child):
                    mutant = list(copy.deepcopy(value))
                    mutant[index] = mutated_child
                    yield mutant
            yield {"substituted": copy.deepcopy(value)}
        elif isinstance(value, str):
            yield f"{value}-restore-freshness-guaranteed"
            yield {"proof": {"claim": value}}
        elif type(value) is int:
            yield value + 1
            yield {"substituted": value}
        elif isinstance(value, bool):
            yield not value
            yield {"substituted": value}

    def test_semantic_behavior_receipt_excludes_duration_only_diagnostics(self) -> None:
        first = behavior.semantic_test_receipt(
            "maestro",
            "exact_test",
            b"test result: ok. 1 passed; 0 failed; finished in 0.01s\n",
            b"",
            0,
        )
        second = behavior.semantic_test_receipt(
            "maestro",
            "exact_test",
            b"test result: ok. 1 passed; 0 failed; finished in 9.99s\n",
            b"noncanonical timing detail\n",
            0,
        )
        self.assertEqual(first, second)

    def test_frozen_behavior_manifest_rejects_a_real_passing_test_substitution(self) -> None:
        runs: list[dict[str, Any]] = []
        for label, target, tests in behavior.EXPECTED_RUNS:
            runs.append(
                {
                    "label": label,
                    "passed": len(tests),
                    "tests": [
                        {
                            "command": [target, test, "--exact", "--nocapture"],
                            "name": test,
                            "result": "pass",
                        }
                        for test in tests
                    ],
                }
            )
        runs.append(
            {
                "label": "same-count-substitution-mutant",
                "passed": 0,
                "rejected": True,
                "result": "rejected",
            }
        )
        rows = consensus.behavior_manifest_rows(runs)
        self.assertEqual(
            f"sha256:{consensus.sha256(consensus.canonical_json(rows))}",
            consensus.EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
        )
        replacement = "foundation::core::secure_fs::tests::regular_file_bindings_refuse_symlinks_and_hard_links"
        runs[0]["tests"][0] = {
            "command": ["maestro", replacement, "--exact", "--nocapture"],
            "name": replacement,
            "result": "pass",
        }
        substituted = consensus.behavior_manifest_rows(runs)
        self.assertNotEqual(
            f"sha256:{consensus.sha256(consensus.canonical_json(substituted))}",
            consensus.EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
        )

    def test_toolchain_identity_rejects_file_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "out"
            binary = output / "toolchain/bin/rustc"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"rustc-v1")
            binary.chmod(0o755)
            rows = [["toolchain/bin/rustc", 8, consensus.sha256(b"rustc-v1"), True]]
            receipt = {
                "files": rows,
                "identity": f"sha256:{consensus.sha256(consensus.canonical_json(rows))}",
                "schema_version": "maestro.vnext.stage5.rust-toolchain-closure.v1",
                "target": "aarch64-apple-darwin",
            }
            receipt_path = output / "rust-toolchain-closure.v1.json"
            receipt_path.write_bytes(consensus.canonical_json(receipt))
            self.assertTrue(
                consensus.validate_toolchain(receipt, receipt_path, "aarch64-apple-darwin")
            )
            binary.write_bytes(b"rustc-v2")
            self.assertFalse(
                consensus.validate_toolchain(receipt, receipt_path, "aarch64-apple-darwin")
            )

    def test_toolchain_identity_rejects_extra_file_and_duplicate_row(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "out"
            binary = output / "toolchain/bin/rustc"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"rustc-v1")
            binary.chmod(0o755)
            row = ["toolchain/bin/rustc", 8, consensus.sha256(b"rustc-v1"), True]
            receipt = {
                "files": [row],
                "identity": f"sha256:{consensus.sha256(consensus.canonical_json([row]))}",
                "schema_version": "maestro.vnext.stage5.rust-toolchain-closure.v1",
                "target": "aarch64-apple-darwin",
            }
            receipt_path = output / "rust-toolchain-closure.v1.json"
            receipt_path.write_bytes(consensus.canonical_json(receipt))
            extra = output / "toolchain/bin/substituted"
            extra.write_bytes(b"extra")
            extra.chmod(0o755)
            self.assertFalse(
                consensus.validate_toolchain(receipt, receipt_path, "aarch64-apple-darwin")
            )
            extra.unlink()
            receipt["files"] = [row, row]
            receipt["identity"] = f"sha256:{consensus.sha256(consensus.canonical_json(receipt['files']))}"
            self.assertFalse(
                consensus.validate_toolchain(receipt, receipt_path, "aarch64-apple-darwin")
            )

    def test_snapshot_identity_is_recomputed_from_exact_tree_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source.txt"
            source.write_bytes(b"source\n")
            with (
                mock.patch.object(consensus, "WORKSPACE", root),
                mock.patch.object(consensus, "SNAPSHOT_PATHS", ("source.txt",)),
            ):
                source_rows = consensus.source_rows(root)
                manifest = {
                    "schema_version": "maestro.vnext.stage5.immutable-workspace-snapshot.v1",
                    "snapshot_identity": f"sha256:{consensus.sha256(consensus.canonical_json(consensus.snapshot_rows(root)))}",
                    "source_identity": f"sha256:{consensus.sha256(consensus.canonical_json(source_rows))}",
                    "source_rows": source_rows,
                }
                manifest_bytes = consensus.canonical_json(manifest)
                self.assertTrue(consensus.validate_snapshot_manifest(manifest, manifest_bytes))
                manifest["snapshot_identity"] = "sha256:" + "0" * 64
                self.assertFalse(
                    consensus.validate_snapshot_manifest(
                        manifest, consensus.canonical_json(manifest)
                    )
                )

    def test_predecessor_rows_are_recomputed_instead_of_trusted(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = root / "first.json"
            second = root / "second.cbor"
            first.write_bytes(b"{}\n")
            second.write_bytes(b"canonical-predecessor")
            paths = ("first.json", "second.cbor")
            expected = {
                relative: consensus.sha256((root / relative).read_bytes()) for relative in paths
            }
            identity = f"sha256:{expected['second.cbor']}"
            rows = [
                [relative, (root / relative).stat().st_size, expected[relative]]
                for relative in paths
            ]
            archive_buffer = io.BytesIO()
            with tarfile.open(fileobj=archive_buffer, mode="w:gz") as archive:
                for relative in paths:
                    data = (root / relative).read_bytes()
                    member = tarfile.TarInfo(relative)
                    member.size = len(data)
                    archive.addfile(member, io.BytesIO(data))
            source_archive = archive_buffer.getvalue()
            predecessor: dict[str, Any] = {
                "current_dependency_differs_from_history": False,
                "current_dependency_files": rows,
                "files": rows,
                "historical_receipt_validation": {
                    "archive_matches_source_commit": True,
                    "current_dependency_rows_bound_separately": True,
                    "mode": "read_only_commit_tree_content_and_receipt_equality",
                    "receipt_count": 4,
                    "receipts_report_pass": True,
                    "source_commit": "stage4-commit",
                    "source_tree": "stage4-tree",
                },
                "identity": identity,
                "source_archive_byte_length": len(source_archive),
                "source_archive_sha256": consensus.sha256(source_archive),
                "source_commit": "stage4-commit",
                "source_tree": "stage4-tree",
            }
            with (
                mock.patch.object(consensus, "WORKSPACE", root),
                mock.patch.object(consensus, "PREDECESSOR_PATHS", paths),
                mock.patch.object(consensus, "EXPECTED_PREDECESSOR_SHA256", expected),
                mock.patch.object(consensus, "EXPECTED_STAGE4_IDENTITY", identity),
                mock.patch.object(
                    consensus, "EXPECTED_STAGE4_SOURCE_ARCHIVE_LENGTH", len(source_archive)
                ),
                mock.patch.object(
                    consensus,
                    "EXPECTED_STAGE4_SOURCE_ARCHIVE_SHA256",
                    consensus.sha256(source_archive),
                ),
                mock.patch.object(
                    consensus, "EXPECTED_STAGE4_SOURCE_COMMIT", "stage4-commit"
                ),
                mock.patch.object(consensus, "EXPECTED_STAGE4_SOURCE_TREE", "stage4-tree"),
            ):
                self.assertTrue(consensus.validate_predecessor(predecessor, source_archive))
                predecessor["files"][0][1] += 1
                self.assertFalse(consensus.validate_predecessor(predecessor, source_archive))


if __name__ == "__main__":
    unittest.main()
