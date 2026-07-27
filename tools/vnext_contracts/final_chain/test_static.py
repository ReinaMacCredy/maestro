"""Source/static V4 final-chain guards; these tests never generate or seal."""

from __future__ import annotations

import copy
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[2]
CONTRACTS = REPOSITORY / "contracts/vnext/final-chain"
FIXTURES = ROOT / "fixtures"
PACKET = Path("/private/tmp/maestro-vnext-final-closure-successor-packet-v4")
sys.path.insert(0, str(ROOT))

import generate  # type: ignore[import-not-found]  # noqa: E402
import runner  # type: ignore[import-not-found]  # noqa: E402


class FinalChainStaticTests(unittest.TestCase):
    def test_contract_schemas_are_strict_and_parseable(self) -> None:
        expected = {
            "final-cumulative-closure-snapshot.v1.schema.json",
            "proof-ledger.v1.schema.json",
            "stage12-semantic-readback.v1.schema.json",
            "toolchain.v1.schema.json",
            "final-cumulative-seal-receipt.v1.schema.json",
            "final-pointer.v1.schema.json",
            "input-manifest.v1.schema.json",
        }
        self.assertEqual(
            {path.name for path in CONTRACTS.glob("*.schema.json")}, expected
        )
        for path in CONTRACTS.glob("*.json"):
            value = json.loads(path.read_text(encoding="utf-8"))
            if path.name.endswith(".schema.json"):
                self.assertFalse(value["additionalProperties"])

    def test_hostile_fixtures_live_only_in_the_owned_namespace(self) -> None:
        expected = {
            "duplicate-proof-id.v1.json",
            "engine-coverage-gap.v1.json",
            "fault-schedules.v1.json",
            "foreign-receipt.v1.json",
            "network-sandbox-unavailable.v1.json",
            "omitted-input-row.v1.json",
            "packet-byte-substitution.v1.json",
            "protected-primary-write.v1.json",
            "semantic-readback-false-success.v1.json",
            "shared-writable-root.v1.json",
            "stale-pointer.v1.json",
            "toolchain-substitution.v1.json",
        }
        self.assertEqual({path.name for path in FIXTURES.glob("*.json")}, expected)
        self.assertFalse((REPOSITORY / "tests/fixtures/vnext/final-chain").exists())
        self.assertFalse((REPOSITORY / "tests/vnext_final_chain_contracts.rs").exists())
        for path in FIXTURES.glob("*.json"):
            json.loads(path.read_text(encoding="utf-8"))

    def test_current_v4_checkpoint_is_dynamic_and_v3_is_evidence_only(self) -> None:
        dependencies = json.loads(
            (CONTRACTS / "reconstruction-dependencies.v1.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            dependencies["current_v4_checkpoint_binding"],
            "generated_from_exact_clean_final_ref_commit_and_tree",
        )
        self.assertEqual(
            dependencies["historical_predecessor_status"],
            "immutable_input_only_no_final_verdict_reuse",
        )
        source = (ROOT / "generate.py").read_text(encoding="utf-8")
        self.assertNotIn('"reconstructed_stage12_commit"', source)
        self.assertIn("Stage 12 checkpoint must be the current exact final V4 commit", source)

    @unittest.skipUnless(PACKET.is_dir(), "authoritative V4 packet is unavailable")
    def test_exact_v4_packet_is_verified_and_byte_total(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "packet"
            binding = generate.verify_packet(PACKET, destination)
            manifest = json.loads(
                (destination / "packet-manifest.v1.json").read_text(encoding="utf-8")
            )
        self.assertEqual(
            manifest["approved_packet_identity"], generate.PACKET_IDENTITY
        )
        self.assertEqual(binding["byte_length"], len(generate.canonical_bytes(manifest)))
        self.assertGreaterEqual(
            {row["path"].removeprefix("packet/") for row in manifest["files"]},
            set(generate.REQUIRED_PACKET_FILES),
        )

    @unittest.skipUnless(PACKET.is_dir(), "authoritative V4 packet is unavailable")
    def test_packet_byte_substitution_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "packet"
            shutil.copytree(PACKET, root)
            target = root / "fanout-manifest.v4.json"
            value = json.loads(target.read_text(encoding="utf-8"))
            value["state"] = "substituted"
            target.write_bytes(generate.canonical_bytes(value))
            with self.assertRaisesRegex(generate.GenerationError, "fanout byte identity"):
                generate.verify_packet(root, Path(directory) / "frozen")

    def test_manifest_omission_and_duplicate_ledger_refuse(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source"
            source.mkdir()
            (source / "one").write_bytes(b"1")
            (source / "two").write_bytes(b"22")
            manifest = generate.input_manifest(source, "a" * 40, "b" * 40)
            manifest["entries"].pop()
            manifest_path = Path(directory) / "manifest.json"
            manifest_path.write_bytes(generate.canonical_bytes(manifest))
            with self.assertRaisesRegex(runner.FinalChainError, "omission"):
                runner.validate_manifest(
                    manifest_path, source, "a" * 40, "b" * 40
                )

            argv = ["{tool:cargo}", "test"]
            command = {
                "argv": argv,
                "expected_exit_code": 0,
                "identity": generate.command_identity(argv, 0),
            }
            source_binding = generate.bound_file(source / "one", "one")
            proof = {
                "proof_id": "s0-duplicate",
                "stage": 0,
                "kind": "behavior",
                "expected_outcome": "pass",
                "engines": ["python", "rust", "ruby"],
                "command": command,
                "input_bindings": [source_binding],
                "produced_artifacts": [],
            }
            ledger = {
                "schema_version": "maestro.external.vnext-final-proof-ledger.v1",
                "snapshot_commit": "a" * 40,
                "proof_count": 2,
                "proofs": [proof, copy.deepcopy(proof)],
            }
            ledger_path = Path(directory) / "ledger.json"
            ledger_path.write_bytes(generate.canonical_bytes(ledger))
            with self.assertRaisesRegex(
                runner.FinalChainError, "row or identifier"
            ):
                runner.validate_ledger(ledger_path, source, "a" * 40)

    def test_runner_has_no_sandbox_fallback_or_live_tree_execution(self) -> None:
        source = (ROOT / "runner.py").read_text(encoding="utf-8")
        self.assertIn('sandbox_exec != Path("/usr/bin/sandbox-exec")', source)
        self.assertIn('"sandbox network denial probe was not a policy denial"', source)
        self.assertIn('"sandbox protected-primary write denial probe failed"', source)
        self.assertNotIn("shell=True", source)
        self.assertNotIn("git checkout", source)
        self.assertNotIn("fallback", source.lower())

    def test_three_engines_do_not_import_or_launch_each_other(self) -> None:
        sources = {
            "python": (ROOT / "engine_python.py").read_text(encoding="utf-8"),
            "ruby": (ROOT / "engine_ruby.rb").read_text(encoding="utf-8"),
            "rust": (ROOT / "engine_rust.rs").read_text(encoding="utf-8"),
        }
        for engine, source in sources.items():
            self.assertNotIn("runner.py", source)
            for other in set(sources) - {engine}:
                self.assertNotIn(f"engine_{other}", source)
            for schema in (
                "vnext-final-cumulative-closure-snapshot.v1",
                "vnext-final-input-manifest.v1",
                "vnext-final-packet-manifest.v1",
                "vnext-final-proof-ledger.v1",
                "vnext-stage12-semantic-readback-plan.v1",
                "vnext-final-toolchain.v1",
            ):
                self.assertIn(schema, source)

    def test_rust_engine_compiles_without_shared_dependencies(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [
                    "rustc",
                    "--edition=2021",
                    str(ROOT / "engine_rust.rs"),
                    "-o",
                    str(Path(directory) / "engine"),
                ],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_release_object_preexistence_is_byte_verified_before_pointer_cas(self) -> None:
        source = (ROOT / "runner.py").read_text(encoding="utf-8")
        byte_verify = source.index("pre-existing release object bytes differ")
        pointer_cas = source.index("final pointer advanced after snapshot freeze")
        self.assertLess(byte_verify, pointer_cas)
        self.assertIn("release-object semantic readback differs", source)


if __name__ == "__main__":
    unittest.main()
