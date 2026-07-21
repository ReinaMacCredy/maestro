from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest import mock

from tools.vnext_contracts.stage5.evidence_gates import behavior, consensus


class Stage5ConsensusTests(unittest.TestCase):
    def test_engine_local_binary_hashes_are_validated_before_semantic_consensus(self) -> None:
        runs = self.behavior_runs("a" * 64)
        self.assertEqual(
            consensus.semantic_behavior_runs(runs),
            consensus.semantic_behavior_runs(self.behavior_runs("b" * 64)),
        )
        for label, mutation in (
            ("uppercase", "A" * 64),
            ("short", "a" * 63),
            ("non-hex", "g" * 64),
        ):
            with self.subTest(label=label), self.assertRaises(RuntimeError):
                consensus.semantic_behavior_runs(self.behavior_runs(mutation))
        runs[-1]["binary_sha256"] = "b" * 64
        with self.assertRaises(RuntimeError):
            consensus.semantic_behavior_runs(runs)

    def test_semantic_consensus_excludes_only_engine_local_binary_hashes(self) -> None:
        expected = consensus.semantic_behavior_runs(self.behavior_runs("a" * 64))
        for field, value in (
            ("command", ["maestro", "other", "--exact", "--nocapture"]),
            ("name", "other"),
            ("result", "fail"),
        ):
            runs = self.behavior_runs("b" * 64)
            runs[0]["tests"][0][field] = value
            with self.subTest(field=field):
                self.assertNotEqual(consensus.semantic_behavior_runs(runs), expected)
        for field, run_value in (("passed", 0), ("label", "other")):
            runs = self.behavior_runs("b" * 64)
            runs[0][field] = run_value
            with self.subTest(field=field):
                self.assertNotEqual(consensus.semantic_behavior_runs(runs), expected)
        for field, mutant_value in (
            ("command", ["maestro", "other", "--exact", "--nocapture"]),
            ("passed", 1),
            ("rejected", False),
            ("result", "pass"),
            ("substituted_for", "other"),
        ):
            runs = self.behavior_runs("b" * 64)
            runs[-1][field] = mutant_value
            with self.subTest(field=field):
                self.assertNotEqual(consensus.semantic_behavior_runs(runs), expected)

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
                    "substitution",
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
        sources = [
            [
                "tools/vnext_contracts/stage5/evidence_gates/build.py",
                1,
                "a" * 64,
            ]
        ]
        artifact = {"source_closure": sources}
        value = {
            "artifact_id": "artifact",
            "artifact_sha256": "b" * 64,
            "behavior_manifest_identity": consensus.EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
            "behavior_passed": 55,
            "behavior_runs": [],
            "builder_sha256": "a" * 64,
            "publication_state": "inactive_candidate",
            "schema_version": "maestro.vnext.stage5.python-builder-receipt.v1",
            "source_closure_sha256": consensus.sha256(consensus.canonical_json(sources)),
        }
        receipt = {
            **value,
            "receipt_identity": f"sha256:{consensus.sha256(consensus.canonical_json(value))}",
        }
        self.assertTrue(consensus.validate_engine_receipt("builder", receipt, artifact))
        receipt["unbound_extension"] = "self-consistent-substitution"
        receipt["receipt_identity"] = f"sha256:{consensus.sha256(consensus.canonical_json({key: item for key, item in receipt.items() if key != 'receipt_identity'}))}"
        self.assertTrue(consensus.validate_receipt_identity(receipt))
        self.assertFalse(consensus.validate_engine_receipt("builder", receipt, artifact))

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
            source_archive = b"exact-stage4-source"
            predecessor: dict[str, Any] = {
                "files": rows,
                "historical_receipt_validation": {
                    "archive_matches_source_commit": True,
                    "canonical_files_match_archive": True,
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
