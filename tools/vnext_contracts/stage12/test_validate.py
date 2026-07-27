#!/usr/bin/env python3
"""Focused tests for the Stage 12 read-only candidate tools."""

from __future__ import annotations

import copy
import sys
import tempfile
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[2]
sys.dont_write_bytecode = True
sys.path.insert(0, str(TOOLS))

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
    RELEASE_PATH,
    ValidationError,
    load_fixture,
    mutant_suite,
    require_census_sight,
    validate_negative_fixture,
    validate_policy,
    validate_release_inputs,
)


class Stage12CandidateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.policy = load_fixture(POLICY_PATH)
        self.negative = load_fixture(NEGATIVE_PATH)
        self.release = load_fixture(RELEASE_PATH)

    def test_census_is_deterministic_and_literal_only(self) -> None:
        first = build_census(WORKSPACE, self.policy)
        second = build_census(WORKSPACE, self.policy)
        self.assertEqual(canonical_json(first), canonical_json(second))
        self.assertFalse(first["closed_world"])
        self.assertFalse(first["release_claim"])

    def test_census_weakening_mutant_is_rejected(self) -> None:
        mutant = copy.deepcopy(self.policy)
        for rule in mutant["rules"]:
            if rule["id"] == "temporary_domain_module_export":
                rule["values"] = ["pub mod vnext_never_present;"]
        with self.assertRaises(ValidationError):
            require_census_sight(build_census(WORKSPACE, mutant))
        require_census_sight(build_census(WORKSPACE, self.policy))

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


if __name__ == "__main__":
    unittest.main()
