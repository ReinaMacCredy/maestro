from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import build


PINNED_CANONICAL_DESIGN_SHA256 = (
    "abdf9d500d8418a9c8fae247f70af167fdc41de22a7043808b3207ab6c1d5be6"
)


class EffectHomeBuildTest(unittest.TestCase):
    def test_expected_source_inputs_match_pinned_bindings(self) -> None:
        bindings = json.loads(build.SOURCE_BINDINGS.read_text(encoding="ascii"))
        current_source_inputs = bindings["current_source_inputs"]

        self.assertEqual(
            PINNED_CANONICAL_DESIGN_SHA256,
            current_source_inputs["design_sha256"],
        )
        self.assertEqual(
            {
                "design_sha256": build.EXPECTED_INPUTS["design"],
                "decisions_sha256": build.EXPECTED_INPUTS["decisions"],
                "card_sha256": build.EXPECTED_INPUTS["card"],
            },
            current_source_inputs,
        )
        self.assertEqual(
            PINNED_CANONICAL_DESIGN_SHA256,
            build.frozen_source_hashes()["design"],
        )


if __name__ == "__main__":
    unittest.main()
