from __future__ import annotations

import hashlib
import json
import unittest

from tools.vnext_contracts.stage5.evidence_gates import consensus, harness


class Stage5ConsensusHarnessContractTests(unittest.TestCase):
    def test_consensus_pins_the_exact_frozen_harness_manifest(self) -> None:
        expected_count = 63
        expected_identity = (
            "sha256:0264ac4154824568e121ceb41ea6d9b7b6e23b69ae576251d382a9d751b5117c"
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
