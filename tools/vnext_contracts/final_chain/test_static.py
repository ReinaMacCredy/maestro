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
from unittest import mock


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[2]
CONTRACTS = REPOSITORY / "contracts/vnext/final-chain"
FIXTURES = ROOT / "fixtures"
PACKET = Path("/private/tmp/maestro-vnext-final-closure-successor-packet-v4")
STAGE12_PACKET = Path(
    "/private/tmp/maestro-vnext-host-injection-successor-packet-v7"
)
sys.path.insert(0, str(ROOT))

import generate  # type: ignore[import-not-found]  # noqa: E402
import runner  # type: ignore[import-not-found]  # noqa: E402
import stage12_product_proof  # type: ignore[import-not-found]  # noqa: E402


class FinalChainStaticTests(unittest.TestCase):
    def _assert_exact_cargo_lib_filter(
        self, row: dict[str, object], test_filter: str
    ) -> None:
        self.assertEqual(
            row["command"]["argv"],
            [
                "{tool:cargo}",
                "test",
                "--offline",
                "--frozen",
                "--lib",
                test_filter,
                "--",
                "--exact",
                "--test-threads=1",
            ],
        )

    def _materialize_stage12_bindings(
        self, value: dict[str, object], root: Path, prefix: str
    ) -> None:
        if not STAGE12_PACKET.is_dir():
            self.skipTest("authoritative V7 packet is unavailable")
        packet_bindings = [
            value["approved_packet"],
            value["protected_primary"]["boundary"],
            value["source_git_binding"]["artifact"],
        ]
        for binding in packet_bindings:
            path = root / binding["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(STAGE12_PACKET / Path(binding["path"]).name, path)
            raw = path.read_bytes()
            self.assertEqual(binding["byte_length"], len(raw))
            self.assertEqual(binding["sha256"], runner.digest(raw))
        for index, binding in enumerate(
            row["evidence"] for row in value["retained_inputs"]
        ):
            path = root / binding["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            raw = f"{prefix}-{index}\n".encode()
            path.write_bytes(raw)
            binding["byte_length"] = len(raw)
            binding["sha256"] = runner.digest(raw)

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
            "promotion-prerequisites.v1.schema.json",
            "stage12-overlay.v1.schema.json",
            "stage12-legacy-cut-coordinator.v2.schema.json",
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
        self.assertNotIn('"update-ref"', source)
        self.assertNotIn("fallback", source.lower())
        self.assertIn('"candidate_ref_write"', source)

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
            if "rotation_slot" in row:
                self.assertNotIn("command", row)
                self.assertNotIn("input_paths", row)
            elif row["command"]["argv"][0] == "{tool:cargo}":
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
        ancestry = next(row for row in proofs if row["kind"] == "ancestry")
        self.assertIn(
            "{control:ancestry-repository}", ancestry["command"]["argv"]
        )
        self.assertNotIn("{source}", ancestry["command"]["argv"])
        rows = {row["proof_id"]: row for row in proofs}
        self.assertEqual(
            rows["s0-public-contract-literals"]["command"]["argv"],
            [
                "{tool:cargo}",
                "test",
                "--offline",
                "--frozen",
                "--test",
                "vnext_public_contract_literals",
                "--",
                "--test-threads=1",
            ],
        )
        self.assertEqual(
            rows["s10-stage12-product-ownership-closure"]["command"]["argv"],
            [
                "{tool:python}",
                "tools/vnext_contracts/final_chain/stage12_product_proof.py",
                "--ancestry-repository",
                "{control:ancestry-repository}",
                "--snapshot",
                "{control:snapshot}",
                "--snapshot-root",
                "{source}",
            ],
        )
        stage12_product_proof = (
            ROOT / "stage12_product_proof.py"
        ).read_text(encoding="utf-8")
        self.assertIn(
            "11dca539193e9a6c3e3346786c69d8d4bad386e8",
            stage12_product_proof,
        )
        self.assertIn("merge-base", stage12_product_proof)

    def test_stage12_product_proof_refuses_unsafe_inputs_and_propagates_child_status(
        self,
    ) -> None:
        valid_snapshot = {
            "schema_version": stage12_product_proof.SNAPSHOT_SCHEMA,
            "state": "frozen",
            "final_integration": {
                "commit": stage12_product_proof.STAGE12_PRODUCT_CORRECTION_COMMIT
            },
        }
        self.assertEqual(
            stage12_product_proof.final_commit(valid_snapshot),
            stage12_product_proof.STAGE12_PRODUCT_CORRECTION_COMMIT,
        )
        invalid_snapshot = copy.deepcopy(valid_snapshot)
        invalid_snapshot["state"] = "open"
        with self.assertRaisesRegex(
            stage12_product_proof.ProofError,
            "does not bind one frozen final commit",
        ):
            stage12_product_proof.final_commit(invalid_snapshot)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            missing = root / "missing.json"
            with self.assertRaisesRegex(
                stage12_product_proof.ProofError,
                "absent or unsafe",
            ):
                stage12_product_proof.load_snapshot(missing)
            snapshot_path = root / "snapshot.json"
            snapshot_path.write_text(json.dumps(valid_snapshot), encoding="utf-8")
            unsafe_snapshot = root / "snapshot-link.json"
            unsafe_snapshot.symlink_to(snapshot_path)
            with self.assertRaisesRegex(
                stage12_product_proof.ProofError,
                "absent or unsafe",
            ):
                stage12_product_proof.load_snapshot(unsafe_snapshot)

            ancestry = root / "ancestry"
            ancestry.mkdir()
            source = root / "source"
            validator = source / "tools/vnext_contracts/stage10/validate.py"
            validator.parent.mkdir(parents=True)
            validator.write_text("raise SystemExit(0)\n", encoding="utf-8")
            child = mock.Mock(returncode=7)
            argv = [
                "stage12_product_proof.py",
                "--ancestry-repository",
                str(ancestry),
                "--snapshot",
                str(snapshot_path),
                "--snapshot-root",
                str(source),
            ]
            with mock.patch.object(
                stage12_product_proof,
                "require_stage12_ancestor",
            ) as ancestor_check, mock.patch.object(
                stage12_product_proof.subprocess,
                "run",
                return_value=child,
            ) as run_child, mock.patch.object(sys, "argv", argv):
                self.assertEqual(stage12_product_proof.main(), 7)
            ancestor_check.assert_called_once_with(
                ancestry.resolve(),
                stage12_product_proof.STAGE12_PRODUCT_CORRECTION_COMMIT,
            )
            child_argv = run_child.call_args.args[0]
            self.assertEqual(
                child_argv[-1],
                stage12_product_proof.STAGE12_PRODUCT_CORRECTION_COMMIT,
            )

        refused = mock.Mock(returncode=1, stdout=b"", stderr=b"not ancestor")
        with mock.patch.object(
            stage12_product_proof.subprocess,
            "run",
            return_value=refused,
        ), self.assertRaisesRegex(
            stage12_product_proof.ProofError,
            "not an ancestor",
        ):
            stage12_product_proof.require_stage12_ancestor(
                Path("."),
                "f" * 40,
            )

    def test_stage11_and_stage12_filters_are_exact_and_fully_resolved(self) -> None:
        registry = json.loads(
            (CONTRACTS / "proof-registry.v1.json").read_text(encoding="utf-8")
        )
        rows = {row["proof_id"]: row for row in registry["proofs"]}
        stage11 = {
            "s11-frozen-cohort-migration": (
                "domain::migration::runtime::consumer::cohort_observation_tests::"
                "frozen_cohort_migration_observes_real_reader_and_writer_routes"
            ),
            "s11-frozen-cohort-rollback": (
                "domain::migration::runtime::consumer::cohort_observation_tests::"
                "frozen_cohort_rollback_observes_restore_and_refusal_routes"
            ),
        }
        for proof_id, test_filter in stage11.items():
            row = rows[proof_id]
            self._assert_exact_cargo_lib_filter(row, test_filter)
            self.assertEqual(
                rows[proof_id]["harness"]["protocol"], "cohort-observation-v1"
            )
        stage12 = {
            "s12-post-promotion-readback": (
                "operations::adapters::tests::"
                "post_promotion_canonical_readback_emits_positive_semantic_receipt"
            ),
            "s12-post-promotion-removal": (
                "operations::adapters::tests::"
                "post_promotion_legacy_and_obsolete_reader_removal_emits_positive_semantic_receipt"
            ),
        }
        for proof_id, test_filter in stage12.items():
            row = rows[proof_id]
            self._assert_exact_cargo_lib_filter(row, test_filter)
            self.assertEqual(
                row["input_paths"],
                [
                    "src/operations/adapters/mod.rs",
                    "tools/vnext_contracts/stage12/namespace_promotion.py",
                ],
            )
            self.assertEqual(
                row["harness"],
                {
                    "protocol": "semantic-receipt-v1",
                    "required_receipt": "semantic-artifact-readback.v1.json",
                },
            )
        self.assertFalse(
            any("rotation_slot" in row for row in registry["proofs"])
        )
        readback = generate.readback_plan(registry, "a" * 40)
        self.assertEqual(len(readback["checks"]), 8)
        self.assertEqual(
            {row["kind"] for row in readback["checks"]},
            set(generate.READBACK_KINDS),
        )
        generator = (ROOT / "generate.py").read_text(encoding="utf-8")
        self.assertNotIn("vnext_stage12_contracts", generator)
        self.assertNotIn('"{tool:cargo}",\n                "build"', generator)
        self.assertIn("Stage 12 semantic readback command is unresolved or inexact", generator)

    def test_synthetic_topology_mutants_refuse_extra_parents_and_wrong_merges(self) -> None:
        commits = {
            **generate.HISTORICAL_STAGE_CHECKPOINTS,
            **{stage: f"{stage:040x}" for stage in range(5, 13)},
        }
        reviewed = "e" * 40
        rows = {
            commit: {
                "commit": commit,
                "tree": f"{stage + 20:040x}",
                "parents": (
                    [commits[4], "d" * 40]
                    if stage == 5
                    else [commits[11], reviewed]
                    if stage == 12
                    else []
                    if stage == 0
                    else [commits[stage - 1]]
                ),
            }
            for stage, commit in commits.items()
        }
        rows[reviewed] = {
            "commit": reviewed,
            "tree": rows[commits[12]]["tree"],
            "parents": [],
        }
        values = list(commits.items())
        required = {"seed"}

        def validate() -> None:
            with (
                mock.patch.object(generate, "commit_row", side_effect=lambda _r, c: copy.deepcopy(rows[c])),
                mock.patch.object(generate, "changed_paths", return_value=required),
                mock.patch.object(
                    generate,
                    "first_parent_path",
                    return_value=[
                        {"commit": "d" * 40, "tree": "1" * 40, "parents": [generate.PROVISIONAL_STAGE5_SOURCE]},
                        {"commit": generate.PROVISIONAL_STAGE5_SOURCE, "tree": "2" * 40, "parents": []},
                    ],
                ),
                mock.patch.object(generate, "git", return_value=rows[reviewed]["tree"]),
            ):
                generate.verify_stage_chain(
                    Path("."), commits[12], reviewed, required, values
                )

        validate()
        rows[commits[6]]["parents"].append("f" * 40)
        with self.assertRaisesRegex(generate.GenerationError, "Stage 6"):
            validate()
        rows[commits[6]]["parents"] = [commits[5]]
        rows[commits[5]]["parents"][0] = "f" * 40
        with self.assertRaisesRegex(generate.GenerationError, "Stage 5"):
            validate()
        rows[commits[5]]["parents"][0] = commits[4]
        rows[commits[12]]["parents"][1] = "f" * 40
        with self.assertRaisesRegex(generate.GenerationError, "reviewed candidate"):
            validate()
        rows[commits[12]]["parents"][1] = reviewed
        rows[commits[12]]["tree"] = "f" * 40
        with self.assertRaisesRegex(generate.GenerationError, "trees differ"):
            validate()
        with mock.patch.object(
            generate,
            "commit_row",
            return_value={"commit": "f" * 40, "tree": "1" * 40, "parents": []},
        ), self.assertRaisesRegex(generate.GenerationError, "does not reach"):
            generate.first_parent_path(
                Path("."), "f" * 40, generate.PROVISIONAL_STAGE5_SOURCE
            )

    def test_promotion_prerequisites_refuse_nonzero_legacy_count_and_missing_receipt(self) -> None:
        base = {
            "schema_version": "maestro.external.vnext-final-promotion-prerequisites.v1",
            "stage11_commit": "a" * 40,
            "stage12_reviewed_candidate": "b" * 40,
            "stage11_migration_filter": "migration",
            "stage11_rollback_filter": "rollback",
            "stage12_readback_filter": "readback",
            "stage12_removal_filter": "removal",
            "legacy_prune_gate": {
                "status": "pass",
                "observed_legacy_row_count": 0,
                "receipt": {},
            },
            "consumer_reader_hold": {
                "consumer_count": 0,
                "reader_count": 0,
                "hold_count": 0,
                "receipt": {},
            },
            "promotion_parity": {
                "source_file_count": 210,
                "promoted_file_count": 210,
                "mismatch_count": 0,
                "receipt": {},
            },
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "promotion-prerequisites.v1.json"
            blocked = copy.deepcopy(base)
            blocked["legacy_prune_gate"]["observed_legacy_row_count"] = 384
            path.write_bytes(generate.canonical_bytes(blocked))
            with self.assertRaisesRegex(
                generate.GenerationError, "absent, nonzero, stale, or noncanonical"
            ):
                generate.verify_promotion_prerequisites(
                    path, "a" * 40, "b" * 40
                )
            path.write_bytes(generate.canonical_bytes(base))
            with self.assertRaisesRegex(
                generate.GenerationError, "receipt binding is absent"
            ):
                generate.verify_promotion_prerequisites(
                    path, "a" * 40, "b" * 40
                )

    def test_overlay_refuses_delete_hidden_protected_and_unsafe_mode_mutants(self) -> None:
        cases = [
            (":100644 000000 " + "1" * 40 + " " + "0" * 40 + " D", "src/x.rs", "delete"),
            (":000000 100644 " + "0" * 40 + " " + "1" * 40 + " A", ".hidden", "hidden"),
            (":100644 100644 " + "1" * 40 + " " + "2" * 40 + " M", "Cargo.toml", "protected"),
            (":100644 120000 " + "1" * 40 + " " + "2" * 40 + " M", "src/x.rs", "unsafe mode"),
        ]
        for header, path, label in cases:
            raw = header.encode("ascii") + b"\0" + path.encode() + b"\0"
            with self.subTest(label=label), mock.patch.object(
                generate, "run", return_value=raw
            ), self.assertRaises(generate.GenerationError):
                generate.overlay_entries(Path("."), "a" * 40, "b" * 40)

    def test_materializer_is_first_parent_exact_and_never_updates_refs(self) -> None:
        source = (ROOT / "materialize_chain.py").read_text(encoding="utf-8")
        self.assertIn('"--no-replace-objects"', source)
        self.assertIn("require_first_parent_ancestor", source)
        self.assertNotIn('"merge-base"', source)
        self.assertNotIn("update-ref", source)
        self.assertNotIn("checkout", source)
        self.assertIn('"refs_updated": False', source)
        generator = (ROOT / "generate.py").read_text(encoding="utf-8")
        runner_source = (ROOT / "runner.py").read_text(encoding="utf-8")
        self.assertIn('"pack-objects", "--stdout", "--revs"', generator)
        self.assertIn('"index-pack", "--stdin", "--fix-thin"', runner_source)
        self.assertIn("ancestry proof repository", runner_source)

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
        self.assertIn("Stage 5 must be an exact two-parent merge", source)
        self.assertIn("sole direct first-parent successors", source)
        self.assertIn("Stage5 post-fanout correction", source)
        self.assertIn("reused a Stage5 inherited seam exception", source)
        self.assertIn("exact reviewed candidate with an identical tree", source)
        self.assertIn("Stage 12 overlay contains a delete, rename, or unsafe mode", source)
        self.assertIn("Stage 12 overlay enters a protected root", source)
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

    def test_v7_coordinator_is_postimage_bound_before_the_final_runner(self) -> None:
        fixture_path = (
            REPOSITORY
            / "tests/fixtures/vnext/stage12/stage12-legacy-cut-coordinator.v2.json"
        )
        value = json.loads(fixture_path.read_text(encoding="utf-8"))
        declared = value["candidate_ref"]["declared_postimage"]
        value["candidate_ref"]["git_common_dir_realpath"] = value[
            "source_git_binding"
        ]["git_common_dir_realpath"]
        value["cas_observation"] = {
            "state": "exact_declared_postimage",
            "observed_commit": declared["commit"],
            "observed_tree": declared["tree"],
        }
        with tempfile.TemporaryDirectory() as directory:
            closure = Path(directory)
            self._materialize_stage12_bindings(
                value, closure, "runner-stage12-binding"
            )
            path = closure / "control/stage12-legacy-cut-coordinator.v2.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(runner.canonical_bytes(value))
            runner.validate_stage12_coordinator(
                closure, path, declared["commit"], declared["tree"]
            )
            binding_mutant = copy.deepcopy(value)
            binding_mutant["source_git_binding"][
                "git_common_dir_realpath"
            ] = "/foreign/git-common-dir"
            binding_mutant["candidate_ref"][
                "git_common_dir_realpath"
            ] = "/foreign/git-common-dir"
            path.write_bytes(runner.canonical_bytes(binding_mutant))
            with self.assertRaisesRegex(
                runner.FinalChainError,
                "V7 packet, source Git, or protected-primary binding differs",
            ):
                runner.validate_stage12_coordinator(
                    closure, path, declared["commit"], declared["tree"]
                )
            mutant = copy.deepcopy(value)
            mutant["cas_observation"]["state"] = "exact_expected_preimage"
            path.write_bytes(runner.canonical_bytes(mutant))
            with self.assertRaisesRegex(
                runner.FinalChainError, "postimage was not observed"
            ):
                runner.validate_stage12_coordinator(
                    closure, path, declared["commit"], declared["tree"]
                )

    def test_generator_materializes_only_bound_stage12_coordinator_inputs(self) -> None:
        fixture_path = (
            REPOSITORY
            / "tests/fixtures/vnext/stage12/stage12-legacy-cut-coordinator.v2.json"
        )
        value = json.loads(fixture_path.read_text(encoding="utf-8"))
        declared = value["candidate_ref"]["declared_postimage"]
        value["candidate_ref"]["git_common_dir_realpath"] = value[
            "source_git_binding"
        ]["git_common_dir_realpath"]
        value["cas_observation"] = {
            "state": "exact_declared_postimage",
            "observed_commit": declared["commit"],
            "observed_tree": declared["tree"],
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            closure = root / "closure"
            source.mkdir()
            closure.mkdir()
            self._materialize_stage12_bindings(
                value, source, "generator-stage12-binding"
            )
            coordinator_path = source / "coordinator.json"
            coordinator_path.write_bytes(generate.canonical_bytes(value))
            binding = generate.materialize_stage12_coordinator(
                coordinator_path,
                source,
                closure,
                declared["commit"],
                declared["tree"],
            )
            self.assertEqual(
                binding["path"],
                "control/stage12-legacy-cut-coordinator.v2.json",
            )
            last_binding = value["retained_inputs"][-1]["evidence"]
            (source / last_binding["path"]).write_text("drift\n", encoding="utf-8")
            with self.assertRaisesRegex(
                generate.GenerationError, "input bytes differ"
            ):
                generate.materialize_stage12_coordinator(
                    coordinator_path,
                    source,
                    root / "second-closure",
                    declared["commit"],
                    declared["tree"],
                )


if __name__ == "__main__":
    unittest.main()
