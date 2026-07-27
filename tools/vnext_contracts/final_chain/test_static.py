"""Static V4 namespace guards; this module never executes a final-chain run."""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
CONTRACTS = ROOT.parents[2] / "contracts/vnext/final-chain"
FIXTURES = ROOT.parents[2] / "tests/fixtures/vnext/final-chain"


class FinalChainStaticTests(unittest.TestCase):
    def test_contract_schemas_and_hostile_fixtures_are_present(self) -> None:
        schemas = {
            "final-cumulative-closure-snapshot.v1.schema.json",
            "proof-ledger.v1.schema.json",
            "stage12-semantic-readback.v1.schema.json",
            "toolchain.v1.schema.json",
            "final-cumulative-seal-receipt.v1.schema.json",
            "final-pointer.v1.schema.json",
            "input-manifest.v1.schema.json",
        }
        self.assertEqual({path.name for path in CONTRACTS.glob("*.schema.json")}, schemas)
        self.assertEqual(
            {path.name for path in FIXTURES.glob("*.json")},
            {
                "duplicate-proof-id.v1.json",
                "engine-coverage-gap.v1.json",
                "foreign-receipt.v1.json",
                "semantic-readback-false-success.v1.json",
                "stale-pointer.v1.json",
            },
        )
        for path in CONTRACTS.glob("*.json"):
            json.loads(path.read_text(encoding="utf-8"))

    def test_engines_do_not_import_or_launch_each_other(self) -> None:
        python = (ROOT / "engine_python.py").read_text(encoding="utf-8")
        ruby = (ROOT / "engine_ruby.rb").read_text(encoding="utf-8")
        rust = (ROOT / "engine_rust.rs").read_text(encoding="utf-8")
        for source in (python, ruby, rust):
            self.assertNotIn("runner.py", source)
            self.assertNotIn("engine_python", source.replace("engine_python.py", ""))
            self.assertNotIn("engine_ruby", source.replace("engine_ruby.rb", ""))
            self.assertNotIn("engine_rust", source.replace("engine_rust.rs", ""))


if __name__ == "__main__":
    unittest.main()
