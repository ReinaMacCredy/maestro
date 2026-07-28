#!/usr/bin/env python3
"""Focused tests for the Stage 12 canonical-promotion checkpoint tools."""

from __future__ import annotations

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[2]
sys.dont_write_bytecode = True
sys.path.insert(0, str(TOOLS))

import census as census_module  # type: ignore[import-not-found]  # noqa: E402
import namespace_promotion as promotion_module  # type: ignore[import-not-found]  # noqa: E402
import validate as validate_module  # type: ignore[import-not-found]  # noqa: E402

from architecture_guard import (  # type: ignore[import-not-found]  # noqa: E402
    ArchitectureGuardError,
    evaluate,
)
from census import (  # type: ignore[import-not-found]  # noqa: E402
    CensusError,
    build_census,
    canonical_json,
    load_json,
)
from validate import (  # type: ignore[import-not-found]  # noqa: E402
    NEGATIVE_PATH,
    POLICY_PATH,
    PROMOTION_PLAN_PATH,
    RELEASE_PATH,
    ValidationError,
    load_fixture,
    mutant_suite,
    require_census_sight,
    validate_negative_fixture,
    validate_policy,
    validate_promotion_plan,
    validate_release_inputs,
)


class Stage12CandidateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.policy = load_fixture(POLICY_PATH)
        self.negative = load_fixture(NEGATIVE_PATH)
        self.release = load_fixture(RELEASE_PATH)
        self.promotion_plan = load_fixture(PROMOTION_PLAN_PATH)

    def test_inputs_bind_the_exact_v4_design_and_fanout_manifest(self) -> None:
        expected_design = (
            "5092ff84ac3bca050802ea81858375d328e1d0ffe678a71ef2f8dae65ed00a18"
        )
        expected_manifest = (
            "e299556c31c6a788285d984f9cd3040cfde200ba24e7ed5a5d90caff96ee5954"
        )
        expected_decisions = {
            "dec-canonical-authority-materialization-df3b": (
                "0d7c406f68f04fdf47ce00d56e8189b54159f164323c9511504790b941f715d0"
            ),
            "dec-canonical-execution-h3-verified-0939": (
                "b5935c389182a7f3ec6447fb2a13dcb70e912108b399d0b1d25fee5f132186a7"
            ),
            "dec-canonical-final-cumulative-stage-0-1652": (
                "214bb83b8d0d13315250b7330ec4f44d520efcfb0b2d0011fa5cd268f4d48114"
            ),
            "dec-canonical-foundation-descriptor-a128": (
                "17fb79ef9bc74cf3838d869bf5fb3b0ae0e9ae017670ca7cb207aeb8105c234e"
            ),
            "dec-canonical-foundation-owned-admitted-d215": (
                "f3e19535a81d5b6eb11836d4b90bbd01c339cee8f9e964bf33e702d90f55d20f"
            ),
            "dec-canonical-installation-consumer-c1fe": (
                "aaba56a8f34fb293a68f26743fbf4ef879d9f5a399a4eb45da74eed70a509e53"
            ),
            "dec-canonical-pre-candidate-protected-370d": (
                "3f2d88bd1659f1f6622d405e6a63158a230bd766ff091990c69bd56bdccfd6fc"
            ),
        }
        for fixture in (self.policy, self.negative, self.release):
            self.assertEqual(fixture["design_sha256"], expected_design)
            self.assertEqual(
                fixture["fanout_manifest_schema"],
                "maestro.external.vnext-successor-fanout.v4",
            )
            self.assertEqual(fixture["fanout_manifest_sha256"], expected_manifest)
            self.assertEqual(fixture["materialization_decisions"], expected_decisions)
        for module in (census_module, validate_module):
            self.assertEqual(module.DESIGN_SHA256, expected_design)
            self.assertEqual(
                module.FANOUT_MANIFEST_SCHEMA,
                "maestro.external.vnext-successor-fanout.v4",
            )
            self.assertEqual(module.FANOUT_MANIFEST_SHA256, expected_manifest)
            self.assertEqual(module.MATERIALIZATION_DECISIONS, expected_decisions)

    def test_census_is_deterministic_and_literal_only(self) -> None:
        first = build_census(WORKSPACE, self.policy)
        second = build_census(WORKSPACE, self.policy)
        self.assertEqual(canonical_json(first), canonical_json(second))
        self.assertFalse(first["closed_world"])
        self.assertFalse(first["release_claim"])

    def test_namespace_promotion_manifest_is_exact_and_collision_explicit(
        self,
    ) -> None:
        first = promotion_module.build_manifest(WORKSPACE)
        second = promotion_module.build_manifest(WORKSPACE)
        self.assertEqual(first, second)
        self.assertEqual(
            first,
            promotion_module.build_manifest(WORKSPACE, WORKSPACE),
        )
        self.assertTrue(first["closed_world"])
        self.assertEqual(first["entry_count"], 210)
        self.assertEqual(first["collision_count"], 10)
        self.assertEqual(
            first["namespace_counts"],
            {
                "src/domain/vnext": 186,
                "src/interfaces/vnext": 8,
                "src/operations/vnext": 16,
            },
        )
        self.assertEqual(first["postconditions"]["temporary_namespace_count"], 0)
        self.assertFalse(first["legacy_pruning_authorized"])
        entries = first["entries"]
        self.assertEqual(
            len({entry["source"] for entry in entries}),
            first["entry_count"],
        )
        self.assertEqual(
            len({entry["destination"] for entry in entries}),
            first["entry_count"],
        )
        self.assertTrue(
            sum(bool(entry["collision"]) for entry in entries) == 10
        )

    def test_namespace_promotion_separates_ancestry_from_snapshot_bytes(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(
                promotion_module.NamespacePromotionError,
                "unsafe or missing canonical destination",
            ):
                promotion_module.build_manifest(
                    WORKSPACE,
                    Path(directory),
                )

    def test_census_requires_namespace_zero_and_exact_legacy_blocker(self) -> None:
        census = build_census(WORKSPACE, self.policy)
        require_census_sight(census)

        mutant = copy.deepcopy(self.policy)
        for rule in mutant["rules"]:
            if rule["id"] == "legacy_skill_surface":
                rule["values"] = ["pub mod vnext_never_present;"]
        with self.assertRaises(ValidationError):
            require_census_sight(build_census(WORKSPACE, mutant))

        historical = copy.deepcopy(census)
        historical["rule_counts"]["legacy_skill_surface"] = 239
        historical["row_count"] = 349
        with self.assertRaises(ValidationError):
            require_census_sight(historical)

        off_by_one = copy.deepcopy(census)
        off_by_one["row_count"] = 383
        with self.assertRaises(ValidationError):
            require_census_sight(off_by_one)

        wrong_scan = copy.deepcopy(census)
        wrong_scan["scan_sha256"] = "0" * 64
        with self.assertRaises(ValidationError):
            require_census_sight(wrong_scan)

    def test_release_preflight_fails_closed_on_temporary_namespace(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "src/domain/vnext").mkdir(parents=True)
            (repo / "src/domain/vnext/mod.rs").write_text(
                "use crate::domain::vnext::work::WorkIdV1;\n", encoding="utf-8"
            )
            (repo / "src/domain/mod.rs").write_text("pub mod vnext;\n", encoding="utf-8")
            payload, exit_code = evaluate(
                repo,
                self.policy,
                self.release,
                {},
                release_preflight=True,
            )
        self.assertEqual(exit_code, 2)
        self.assertEqual(payload["status"], "blocked")
        blocker_ids = {blocker["id"] for blocker in payload["blockers"]}
        self.assertIn("consumer_rows_nonzero", blocker_ids)
        self.assertIn("missing_external_input", blocker_ids)

    def test_claim_and_compatibility_mutants_are_rejected(self) -> None:
        self.assertEqual(mutant_suite(self.policy, self.negative, self.release)["status"], "pass")
        mutant = copy.deepcopy(self.release)
        mutant["claims"]["release_ready"] = True
        with self.assertRaises(ValidationError):
            validate_release_inputs(mutant)
        with self.assertRaises(ArchitectureGuardError):
            evaluate(
                WORKSPACE,
                self.policy,
                mutant,
                {},
                release_preflight=False,
            )

        with tempfile.TemporaryDirectory() as directory:
            duplicate = Path(directory) / "duplicate.json"
            duplicate.write_text(
                '{"schema_version":"first","schema_version":"second"}\n',
                encoding="utf-8",
            )
            with self.assertRaises(CensusError):
                load_json(duplicate)

        validate_policy(self.policy)
        validate_negative_fixture(self.negative)
        validate_promotion_plan(self.promotion_plan)

    def _receipt(
        self,
        path: Path,
        slot: str,
        payload: dict[str, object],
    ) -> Path:
        value = {
            "currentness_id": "2" * 64,
            "payload": payload,
            "proof_id": "3" * 64,
            "schema_version": "maestro.external.stage12-evidence-receipt.v1",
            "slot": slot,
            "status": "satisfied",
            "subject_id": "1" * 64,
        }
        path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="utf-8")
        return path

    def test_release_preflight_rejects_false_consumer_reader_and_hold_zero(self) -> None:
        zero_slots = {
            "stage11_active_consumer_closure": "active_consumer_count",
            "stage11_retained_reader_manifest": "sealed_reader_count",
            "stage11_retention_hold_manifest": "retention_hold_count",
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for slot, field in zero_slots.items():
                receipt = self._receipt(root / f"{slot}.json", slot, {field: 1})
                payload, _ = evaluate(
                    WORKSPACE,
                    self.policy,
                    self.release,
                    {slot: receipt},
                    release_preflight=True,
                )
                invalid = [
                    blocker
                    for blocker in payload["blockers"]
                    if blocker["id"] == "invalid_external_evidence_receipt"
                    and blocker["slot"] == slot
                ]
                self.assertEqual(len(invalid), 1)

    def test_release_preflight_rejects_false_move_and_facade_parity(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            move = self._receipt(
                root / "move.json",
                "source_move_identity_parity",
                {
                    "collision_count": 10,
                    "destination_set_sha256": "6" * 64,
                    "entry_count": 210,
                    "manifest_sha256": "4" * 64,
                    "mismatched_paths": ["src/domain/work/mod.rs"],
                    "namespace_counts": {
                        "src/domain/vnext": 186,
                        "src/interfaces/vnext": 8,
                        "src/operations/vnext": 16,
                    },
                },
            )
            facade = self._receipt(
                root / "facade.json",
                "stage12_frozen_interface_readback",
                {
                    "facade_mismatch_count": 1,
                    "interface_manifest_sha256": "5" * 64,
                },
            )
            payload, _ = evaluate(
                WORKSPACE,
                self.policy,
                self.release,
                {
                    "source_move_identity_parity": move,
                    "stage12_frozen_interface_readback": facade,
                },
                release_preflight=True,
            )
            invalid_slots = {
                blocker["slot"]
                for blocker in payload["blockers"]
                if blocker["id"] == "invalid_external_evidence_receipt"
            }
            self.assertEqual(
                invalid_slots,
                {
                    "source_move_identity_parity",
                    "stage12_frozen_interface_readback",
                },
            )

    def test_release_preflight_rejects_untyped_file_as_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "opaque.txt"
            path.write_text("not a receipt\n", encoding="utf-8")
            payload, _ = evaluate(
                WORKSPACE,
                self.policy,
                self.release,
                {"stage11_active_consumer_closure": path},
                release_preflight=True,
            )
            self.assertTrue(
                any(
                    blocker["id"] == "invalid_external_evidence_receipt"
                    and blocker["slot"] == "stage11_active_consumer_closure"
                    for blocker in payload["blockers"]
                )
            )


if __name__ == "__main__":
    unittest.main()
