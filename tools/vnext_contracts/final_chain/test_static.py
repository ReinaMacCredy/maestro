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
            "cohort-observation.v1.schema.json",
            "fanout-edge-sweep.v1.schema.json",
            "fault-observation.v1.schema.json",
            "final-cumulative-closure-snapshot.v1.schema.json",
            "proof-ledger.v1.schema.json",
            "proof-registry.v1.schema.json",
            "semantic-artifact-readback.v1.schema.json",
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
            "ancestry-constant-pass.v1.json",
            "cohort-metadata-echo.v1.json",
            "control-root-write.v1.json",
            "duplicate-proof-id.v1.json",
            "engine-coverage-gap.v1.json",
            "fault-schedules.v1.json",
            "foreign-receipt.v1.json",
            "incomplete-cargo-closure.v1.json",
            "inferred-proof-kind.v1.json",
            "migration-cohorts.v1.json",
            "network-sandbox-unavailable.v1.json",
            "omitted-input-row.v1.json",
            "packet-byte-substitution.v1.json",
            "protected-primary-write.v1.json",
            "publication-custody-swap.v1.json",
            "publication-generation-skip.v1.json",
            "readback-substring-proxy.v1.json",
            "schedule-metadata-echo.v1.json",
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
                "harness": {
                    "protocol": "command-exit-v1",
                    "required_receipt": "none",
                },
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
                "vnext-final-proof-registry.v1",
                "vnext-final-fault-observation.v1",
                "vnext-final-cohort-observation.v1",
                "vnext-final-fanout-edge-sweep.v1",
                "vnext-final-semantic-artifact-readback.v1",
                "vnext-stage12-semantic-readback-plan.v1",
                "vnext-final-toolchain.v1",
            ):
                self.assertIn(schema, source)

    def test_registry_is_explicit_complete_and_bound_without_inference(self) -> None:
        registry = json.loads(
            (CONTRACTS / "proof-registry.v1.json").read_text(encoding="utf-8")
        )
        proofs = registry["proofs"]
        self.assertEqual({row["stage"] for row in proofs}, set(range(13)))
        self.assertEqual(len({row["kind"] for row in proofs}), 14)
        self.assertEqual(len({row["proof_id"] for row in proofs}), len(proofs))
        for row in proofs:
            self.assertEqual(row["engines"], ["python", "rust", "ruby"])
            self.assertIn("protocol", row["harness"])
            if row["command"]["argv"][0] == "{tool:cargo}":
                self.assertIn("--offline", row["command"]["argv"])
                self.assertIn("--frozen", row["command"]["argv"])
        by_kind = {kind: [] for kind in {"race", "crash_replay", "migration", "rollback", "ancestry"}}
        for row in proofs:
            if row["kind"] in by_kind:
                by_kind[row["kind"]].append(row)
        for kind in ("race", "crash_replay"):
            self.assertEqual(by_kind[kind][0]["harness"]["protocol"], "command-exit-v1")
        for kind in ("migration", "rollback"):
            self.assertEqual(by_kind[kind][0]["harness"]["protocol"], "cohort-observation-v1")
        self.assertEqual(by_kind["ancestry"][0]["harness"]["protocol"], "fanout-edge-sweep-v1")
        generator = (ROOT / "generate.py").read_text(encoding="utf-8")
        self.assertNotIn("def stage_for", generator)
        self.assertNotIn("def proof_kind", generator)
        self.assertNotIn('rows[index]["kind"]', generator)
        self.assertIn("registry_identity", generator)

    def test_stage4_registry_rejects_zero_test_and_source_mutant_proxies(self) -> None:
        registry = json.loads(
            (CONTRACTS / "proof-registry.v1.json").read_text(encoding="utf-8")
        )
        rows = {
            row["proof_id"]: row
            for row in registry["proofs"]
            if row["proof_id"]
            in {
                "s4-run-set-one-winner-race",
                "s4-ceremony-crash-replay-cuts",
            }
        }
        expected = {
            "s4-run-set-one-winner-race": (
                "src/domain/vnext/execution/store.rs",
                "domain::vnext::execution::store::tests::"
                "step_submission_and_renewal_race_has_one_atomic_winner",
            ),
            "s4-ceremony-crash-replay-cuts": (
                "src/domain/vnext/execution/ceremony.rs",
                "domain::vnext::execution::ceremony::tests::"
                "protected_ceremony_has_one_winner_durable_full_history_replay_and_owner_refusal",
            ),
        }

        def validate_owner_unit_routes(stage4_rows: dict[str, object]) -> None:
            self.assertEqual(set(stage4_rows), set(expected))
            for proof_id, (source, exact_filter) in expected.items():
                row = stage4_rows[proof_id]
                self.assertEqual(
                    row["command"]["argv"],
                    [
                        "{tool:cargo}",
                        "test",
                        "--offline",
                        "--frozen",
                        "--lib",
                        exact_filter,
                        "--",
                        "--exact",
                        "--test-threads=1",
                    ],
                )
                self.assertEqual(row["input_paths"], [source])
                self.assertEqual(
                    row["harness"],
                    {"protocol": "command-exit-v1", "required_receipt": "none"},
                )

        validate_owner_unit_routes(rows)
        old_zero_test = copy.deepcopy(rows)
        old_zero_test["s4-run-set-one-winner-race"]["command"]["argv"] = [
            "{tool:cargo}",
            "test",
            "--offline",
            "--frozen",
            "--test",
            "vnext_stage4_contracts",
            "stage4_callable_run_set_cas_and_submission_fence_are_atomic",
            "--",
            "--exact",
            "--test-threads=1",
        ]
        with self.assertRaises(AssertionError):
            validate_owner_unit_routes(old_zero_test)

        old_source_mutant = copy.deepcopy(rows)
        old_source_mutant["s4-ceremony-crash-replay-cuts"]["command"]["argv"] = [
            "{tool:cargo}",
            "test",
            "--offline",
            "--frozen",
            "--test",
            "vnext_stage4_contracts",
            "stage4_regenerated_ceremony_replay_mutant_fails_compiled_contract",
            "--",
            "--exact",
            "--test-threads=1",
        ]
        with self.assertRaises(AssertionError):
            validate_owner_unit_routes(old_source_mutant)

    def test_all_engines_route_optional_receipts_only_by_declared_protocol(self) -> None:
        sources = {
            "python": (ROOT / "engine_python.py").read_text(encoding="utf-8"),
            "ruby": (ROOT / "engine_ruby.rb").read_text(encoding="utf-8"),
            "rust": (ROOT / "engine_rust.rs").read_text(encoding="utf-8"),
        }
        self.assertEqual(
            sources["python"].count(
                'if harness["protocol"] == "fault-observation-v1":'
            ),
            2,
        )
        self.assertEqual(
            sources["python"].count(
                'if harness["protocol"] == "cohort-observation-v1":'
            ),
            2,
        )
        self.assertIn(
            'if harness.get("protocol") == "fault-observation-v1":',
            sources["python"],
        )
        self.assertIn(
            'if harness.get("protocol") == "cohort-observation-v1":',
            sources["python"],
        )
        self.assertNotIn(
            'if row["kind"] in {"race", "crash_replay"}', sources["python"]
        )
        self.assertNotIn(
            'if row["kind"] in {"migration", "rollback"}', sources["python"]
        )

        self.assertEqual(
            sources["ruby"].count(
                'if harness["protocol"] == "fault-observation-v1"'
            ),
            3,
        )
        self.assertEqual(
            sources["ruby"].count(
                'if harness["protocol"] == "cohort-observation-v1"'
            ),
            3,
        )
        self.assertNotIn(
            '%w[race crash_replay].include?(row["kind"])', sources["ruby"]
        )
        self.assertNotIn(
            '%w[migration rollback].include?(row["kind"])', sources["ruby"]
        )

        self.assertEqual(
            sources["rust"].count('if protocol == "fault-observation-v1"'), 3
        )
        self.assertEqual(
            sources["rust"].count('if protocol == "cohort-observation-v1"'), 3
        )
        self.assertNotIn(
            'if matches!(kind, "race" | "crash_replay")', sources["rust"]
        )
        self.assertNotIn(
            'if matches!(kind, "migration" | "rollback")', sources["rust"]
        )

    def test_runtime_evidence_is_typed_and_semantic_not_substring_counted(self) -> None:
        sources = [
            (ROOT / name).read_text(encoding="utf-8")
            for name in ("engine_python.py", "engine_ruby.rb", "engine_rust.rs")
        ]
        for source in sources:
            for schema in (
                "vnext-final-fault-observation.v1",
                "vnext-final-cohort-observation.v1",
                "vnext-final-fanout-edge-sweep.v1",
                "vnext-final-semantic-artifact-readback.v1",
                "vnext-final-fault-point-observation.v1",
                "vnext-final-cohort-route-observation.v1",
                "vnext-final-canonical-read-observation.v1",
                "vnext-final-negative-route-observation.v1",
            ):
                self.assertIn(schema, source)
            for proxy in ("count_literals", "scan_counts", "byte_contains"):
                self.assertNotIn(proxy, source)
        plan_schema = json.loads(
            (CONTRACTS / "stage12-semantic-readback.v1.schema.json").read_text(
                encoding="utf-8"
            )
        )
        properties = plan_schema["properties"]["checks"]["items"]["properties"]
        self.assertIn("required_artifact_kinds", properties)
        self.assertIn("minimum_canonical_reads", properties)
        self.assertIn("minimum_negative_routes", properties)
        fault_schema = json.loads(
            (CONTRACTS / "fault-observation.v1.schema.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertIn("point_receipts", fault_schema["required"])
        cohort_schema = json.loads(
            (CONTRACTS / "cohort-observation.v1.schema.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertIn("executables", cohort_schema["required"])
        self.assertIn(
            "observation", cohort_schema["$defs"]["outcome"]["required"]
        )
        artifact_schema = json.loads(
            (CONTRACTS / "semantic-artifact-readback.v1.schema.json").read_text(
                encoding="utf-8"
            )
        )
        for field in ("canonical_reads", "negative_routes"):
            self.assertIn(
                "observation",
                artifact_schema["properties"][field]["items"]["required"],
            )
        self.assertIn(
            "semantic readback consensus differs",
            (ROOT / "runner.py").read_text(encoding="utf-8"),
        )

    def test_cargo_closure_sandbox_and_publication_custody_are_fail_closed(self) -> None:
        generator = (ROOT / "generate.py").read_text(encoding="utf-8")
        for closure in ("registry/index", "registry/cache", "registry/src"):
            self.assertIn(closure, generator)
        self.assertIn('for engine in ENGINE_IDS:', generator)
        self.assertIn('-complete-cargo-native-closure"', generator)
        runner_source = (ROOT / "runner.py").read_text(encoding="utf-8")
        self.assertNotIn("(allow file-read*)", runner_source)
        self.assertIn("sandbox protected-primary read denial probe failed", runner_source)
        self.assertIn("sandbox immutable-root write probe failed", runner_source)
        for token in (
            "os.O_NOFOLLOW",
            "dir_fd=",
            "expected_generation",
            "publication root identity or mount custody changed",
            "os.fsync",
        ):
            self.assertIn(token, runner_source)

    def test_v4_fanout_authenticates_thirteen_checkpoints_and_stage5_only_seams(self) -> None:
        source = (REPOSITORY / "tools/vnext_contracts/fanout/validate.py").read_text(
            encoding="utf-8"
        )
        self.assertIn("len(rows) != 13", source)
        self.assertIn("direct first-parent successor", source)
        self.assertIn("Stage5 post-fanout correction", source)
        self.assertIn("reused a Stage5 inherited seam exception", source)
        self.assertIn('os.environ["MAESTRO_FINAL_PROOF_RECEIPT"]', source)
        self.assertIn("write_new_receipt", source)
        self.assertNotIn('"merge-base"', source)

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
