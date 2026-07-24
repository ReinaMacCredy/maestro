from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import build


PINNED_CANONICAL_DESIGN_SHA256 = (
    "9d5bda2be6274351ff7afba7f396595d80f9d560622991de1c8214aae0b8fc1b"
)


class EffectHomeBuildTest(unittest.TestCase):
    def test_expected_source_inputs_match_pinned_bindings(self) -> None:
        self.assertEqual(
            PINNED_CANONICAL_DESIGN_SHA256,
            build.EXPECTED_INPUTS["design"],
        )
        self.assertEqual(
            "18f14bce862e15be09c9d88155d62627582df50c7754e2e8e1d6f6bee8f7d522",
            build.EXPECTED_INPUTS["decisions"],
        )
        self.assertNotIn("EXPECTED_SOURCE_BINDINGS_SHA256", vars(build))
        self.assertIn(
            "verify_input_bindings.py",
            Path(build.__file__).read_text(encoding="utf-8"),
        )


if __name__ == "__main__":
    unittest.main()
