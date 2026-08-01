from __future__ import annotations

import hashlib
import json
import unittest

from tools.vnext_contracts.stage5.evidence_gates import consensus, harness


class Stage5ConsensusHarnessContractTests(unittest.TestCase):
    def test_consensus_pins_the_exact_frozen_harness_manifest(self) -> None:
        expected_count = 67
        expected_identity = (
            "sha256:703c5bd549cf77954b5950b00106aca4678ad70e85e99446a020f303b0d06b05"
        )
        canonical_tests = (
            json.dumps(
                list(harness.EXPECTED_TESTS),
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
        derived_identity = f"sha256:{hashlib.sha256(canonical_tests).hexdigest()}"

        self.assertEqual(len(harness.EXPECTED_TESTS), expected_count)
        self.assertEqual(harness.EXPECTED_TEST_MANIFEST_IDENTITY, expected_identity)
        self.assertEqual(derived_identity, expected_identity)
        self.assertEqual(consensus.EXPECTED_PROOF_HARNESS_TESTS, expected_count)
        self.assertEqual(
            consensus.EXPECTED_PROOF_HARNESS_MANIFEST_IDENTITY,
            expected_identity,
        )


if __name__ == "__main__":
    unittest.main()
