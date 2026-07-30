"""Behavioral and refusal-mutant tests for the Stage-11 V4 validator."""

from __future__ import annotations

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[2]
sys.path.insert(0, str(ROOT))

import validate_v4  # type: ignore[import-not-found]  # noqa: E402


class Stage11V4ValidatorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.contract = validate_v4.load_json(validate_v4.CONTRACT)
        self.universe = validate_v4.load_json(validate_v4.ROOT_UNIVERSE)

    def test_exact_fixture_is_effect_inert_and_complete(self) -> None:
        validate_v4.validate_contract(self.contract)
        validate_v4.validate_root_universe(self.universe)
        self.assertEqual(
            set(self.contract["historical_evidence_fields"]),
            validate_v4.EVIDENCE_FIELDS,
        )
        self.assertEqual(
            {row["role"] for row in self.universe["installation"]["rows"]},
            validate_v4.INSTALLATION_ROLES,
        )
        self.assertEqual(self.universe["caller_roots"], [])
        self.assertFalse(self.universe["census_schema_delta"])
        self.assertFalse(self.universe["census_header_scope_authority"])
        self.assertEqual(
            self.contract["current_types"]["foundation_owner_evidence_mint"],
            "FoundationOwnerEvidenceMintV1",
        )
        self.assertEqual(
            self.contract["current_types"]["loss_audit_custody"],
            "QuarantineCustodyLeaseV1",
        )
        self.assertEqual(
            self.contract["current_types"]["guard_consumer_binding"],
            "LegacyRemovalConsumerBindingV3",
        )

    def test_every_required_mutant_refuses_with_its_exact_reason(self) -> None:
        observed: set[str] = set()
        for name, mutate, expected_code in validate_v4.mutant_cases(
            self.contract, self.universe
        ):
            with self.subTest(name=name):
                candidate = copy.deepcopy(self.universe)
                mutate(candidate)
                with self.assertRaises(validate_v4.ValidationError) as raised:
                    validate_v4.validate_root_universe(candidate)
                self.assertEqual(raised.exception.code, expected_code)
                observed.add(name)
        for required in (
            "post_admission_disappearance",
            "orphan_history",
            "current_absence_only",
            "wrong_owner",
            "provider_race",
            "revocation_race",
            "omitted_role",
            "duplicate_declaration",
            "role_substitution",
            "unsupported",
            "header_without_rows",
            "expected_under_declared_absent",
            "requiredness_flip",
            "foreign_absence_fence",
            "caller_root",
            "cross_root_alias",
            "a_to_b_to_a",
            "final_recheck_drift",
            "journal_unreachable",
        ):
            self.assertIn(required, observed)
        self.assertEqual(
            {
                name
                for name in observed
                if name.startswith("missing_evidence_")
            },
            {f"missing_evidence_{field}" for field in validate_v4.EVIDENCE_FIELDS},
        )

    def test_expected_source_under_unsupported_refuses_before_evidence(self) -> None:
        candidate = copy.deepcopy(self.universe)
        row = candidate["installation"]["rows"][-1]
        row["disposition"] = "Unsupported"
        row["retained_root_capability"] = None
        with self.assertRaises(validate_v4.ValidationError) as raised:
            validate_v4.validate_root_universe(candidate)
        self.assertEqual(raised.exception.code, "unsupported_production_row")

    def test_header_references_cannot_replace_owner_rows(self) -> None:
        candidate = copy.deepcopy(self.universe)
        candidate["installation"]["rows"] = []
        candidate["header_references"] = {
            "declared_root_set": "comparison-only",
            "host_adapter_set": "comparison-only",
            "legacy_locator_set": "comparison-only",
        }
        with self.assertRaises(validate_v4.ValidationError) as raised:
            validate_v4.validate_root_universe(candidate)
        self.assertEqual(raised.exception.code, "root_universe_incomplete_or_extra")

    def test_duplicate_json_keys_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "duplicate.json"
            path.write_text('{"schema_version":"a","schema_version":"b"}\n')
            with self.assertRaises(validate_v4.ValidationError) as raised:
                validate_v4.load_json(path)
            self.assertEqual(raised.exception.code, "duplicate_json_key")

    def _synthetic_source_root(self, root: Path) -> None:
        for relative, tokens in self.contract["required_source_tokens"].items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            source = "\n".join(tokens) + "\n"
            if relative == "src/domain/persistence/legacy_quarantine.rs":
                source += "self.recheck_loss_audit_custody()?\n" * 5
            path.write_text(source, encoding="utf-8")
        for relative in self.contract["forbidden_source_tokens"]:
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if not path.exists():
                path.write_text("", encoding="utf-8")
        census = root / "src/domain/installation/census.rs"
        census.parent.mkdir(parents=True, exist_ok=True)
        census.write_text(
            validate_v4.HEADER_STRUCT + "\n\n" + validate_v4.CENSUS_STRUCT + "\n",
            encoding="utf-8",
        )

    def test_source_contract_accepts_only_current_v4_tokens_and_frozen_census(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source_root = Path(directory)
            self._synthetic_source_root(source_root)
            validate_v4.validate_sources(self.contract, source_root)
            migration = (
                source_root / "src/domain/migration/runtime/live_set_v3.rs"
            )
            migration.write_text(
                migration.read_text(encoding="utf-8") + "PathBuf\n",
                encoding="utf-8",
            )
            with self.assertRaises(validate_v4.ValidationError) as raised:
                validate_v4.validate_sources(self.contract, source_root)
            self.assertEqual(
                raised.exception.code, "historical_or_locator_adapter_reachable"
            )

    def test_source_contract_rejects_census_field_drift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source_root = Path(directory)
            self._synthetic_source_root(source_root)
            census = source_root / "src/domain/installation/census.rs"
            census.write_text(
                census.read_text(encoding="utf-8").replace(
                    "pub proof_profile_id: CommitmentV1,",
                    "pub proof_profile_id: Vec<u8>,",
                ),
                encoding="utf-8",
            )
            with self.assertRaises(validate_v4.ValidationError) as raised:
                validate_v4.validate_sources(self.contract, source_root)
            self.assertEqual(
                raised.exception.code, "installation_census_struct_bytes_changed"
            )

    def test_source_contract_rejects_obsolete_external_audit_store_adapter(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source_root = Path(directory)
            self._synthetic_source_root(source_root)
            migration = source_root / "src/operations/migration/mod.rs"
            migration.write_text(
                "let persistence_store = external_store;\n",
                encoding="utf-8",
            )
            with self.assertRaises(validate_v4.ValidationError) as raised:
                validate_v4.validate_sources(self.contract, source_root)
            self.assertEqual(
                raised.exception.code, "historical_or_locator_adapter_reachable"
            )

    def test_historical_v3_and_v2_files_remain_exactly_bound(self) -> None:
        validate_v4.validate_contract(self.contract, REPOSITORY)
        candidate = copy.deepcopy(self.contract)
        candidate["historical_immutable_files"][
            "tools/vnext_contracts/stage12/coordinator.py"
        ] = "0" * 64
        with self.assertRaises(validate_v4.ValidationError) as raised:
            validate_v4.validate_contract(candidate, REPOSITORY)
        self.assertEqual(raised.exception.code, "historical_artifact_binding_differs")

    def test_contract_and_root_fixture_fields_are_exact(self) -> None:
        contract = copy.deepcopy(self.contract)
        contract["required_source_tokens"].pop(
            "src/domain/repository/root_universe.rs"
        )
        with self.assertRaises(validate_v4.ValidationError) as raised:
            validate_v4.validate_contract(contract)
        self.assertEqual(raised.exception.code, "source_token_closure_differs")

        universe = copy.deepcopy(self.universe)
        universe["ambient_roots"] = []
        with self.assertRaises(validate_v4.ValidationError) as raised:
            validate_v4.validate_root_universe(universe)
        self.assertEqual(raised.exception.code, "root_fixture_fields_differ")

    def test_mutant_suite_reports_zero_acceptance(self) -> None:
        result = validate_v4.run_mutants(self.contract, self.universe)
        self.assertEqual(result["accepted_mutants"], 0)
        self.assertGreaterEqual(result["rejected_mutants"], 39)


if __name__ == "__main__":
    unittest.main()
