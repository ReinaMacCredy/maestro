from __future__ import annotations

import importlib.util
import ast
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
STAGE0 = Path(__file__).resolve().parent
DESIGN = "9d5bda2be6274351ff7afba7f396595d80f9d560622991de1c8214aae0b8fc1b"
DECISIONS = "18f14bce862e15be09c9d88155d62627582df50c7754e2e8e1d6f6bee8f7d522"
CARD = "2cdf1f74843a6eca926ff3bc48e060654350e6a03b65342f8d7be48d111379b4"


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class SuccessorProducerPinsTest(unittest.TestCase):
    def test_public_identity_and_dispatch_bind_successor_sources(self) -> None:
        public_build = load("successor_public_build", STAGE0 / "public_identity/build.py")
        public_validate = load(
            "successor_public_validate", STAGE0 / "public_identity/validate.py"
        )
        dispatch = load("successor_dispatch_build", STAGE0 / "dispatch_cutover/build.py")
        expected = {
            ".maestro/cards/maestro-whole-flow-architecture-refoundation/design.md": DESIGN,
            ".maestro/cards/maestro-whole-flow-architecture-refoundation/decisions.yaml": DECISIONS,
            ".maestro/cards/maestro-whole-flow-architecture-refoundation/card.yaml": CARD,
        }
        self.assertEqual(public_build.EXPECTED_AUTHORITATIVE_HASHES, expected)
        self.assertEqual(public_validate.EXPECTED_AUTHORITATIVE_HASHES, expected)
        self.assertEqual(dispatch.DESIGN_SHA256, DESIGN)
        self.assertEqual(dispatch.DECISIONS_SHA256, DECISIONS)
        self.assertEqual(dispatch.CARD_SHA256, CARD)

    def test_effect_home_binds_dynamic_verified_input_and_h3_source(self) -> None:
        source = (STAGE0 / "effect_home/build.py").read_text(encoding="utf-8")
        independent = (STAGE0 / "effect_home/validate.py").read_text(encoding="utf-8")
        self.assertNotIn("EXPECTED_SOURCE_BINDINGS_SHA256", source)
        self.assertIn("verify_input_bindings.py", source)
        self.assertIn("h3_withdrawal_publication.rs", source)
        self.assertIn("h3_withdrawal_publication.rs", independent)

    def test_nested_stage0_interpreters_are_not_ambient(self) -> None:
        offenders: list[str] = []
        for path in STAGE0.rglob("*.py"):
            tree = ast.parse(path.read_text(encoding="utf-8"))
            if any(
                isinstance(node, ast.List)
                and node.elts
                and isinstance(node.elts[0], ast.Constant)
                and node.elts[0].value in {"ruby", "python3"}
                for node in ast.walk(tree)
            ):
                offenders.append(str(path.relative_to(ROOT)))
        self.assertEqual(offenders, [])


if __name__ == "__main__":
    unittest.main()
