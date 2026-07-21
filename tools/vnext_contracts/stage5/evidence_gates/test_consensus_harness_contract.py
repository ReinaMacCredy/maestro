from __future__ import annotations

import hashlib
import json
import unittest

from tools.vnext_contracts.stage5.evidence_gates import consensus, harness


class Stage5ConsensusHarnessContractTests(unittest.TestCase):
    def test_consensus_pins_the_exact_frozen_harness_manifest(self) -> None:
        expected_count = 64
        expected_identity = (
            "sha256:953795f20001a5a7c81b4ad57fccce9adfb8d681a5a6ca852193990779481375"
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
