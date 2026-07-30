"""Effect-inert tests for Stage12LegacyCutCoordinatorV3."""

from __future__ import annotations

import copy
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[2]
PACKET_ROOT = Path("/private/tmp/maestro-vnext-loss-root-successor-packet-v8")
sys.path.insert(0, str(ROOT))

import coordinator_v3  # type: ignore[import-not-found]  # noqa: E402


class Stage12LegacyCutCoordinatorV3Tests(unittest.TestCase):
    def setUp(self) -> None:
        self.contract = coordinator_v3.load_contract(
            coordinator_v3.DEFAULT_CONTRACT
        )

    def test_exact_contract_binds_v8_and_only_one_named_candidate_ref(self) -> None:
        coordinator_v3.validate_contract(self.contract)
        self.assertEqual(self.contract["design"], coordinator_v3.DESIGN)
        self.assertEqual(
            self.contract["implementation_preimage"],
            coordinator_v3.IMPLEMENTATION_PREIMAGE,
        )
        self.assertEqual(
            self.contract["approved_packet_identity"],
            coordinator_v3.PACKET_IDENTITY,
        )
        self.assertEqual(
            self.contract["candidate_ref"]["ref"],
            coordinator_v3.CANDIDATE_REF,
        )
        self.assertFalse(self.contract["protected_primary"]["candidate_target"])
        self.assertFalse(
            self.contract["effect_boundary"]["proof_runner_candidate_ref_write"]
        )
        self.assertTrue(
            self.contract["effect_boundary"]["coordinator_candidate_ref_cas_only"]
        )

    def test_v3_v4_gate_order_and_zero_closures_are_exact(self) -> None:
        observed = [
            (row["kind"], row["state"]) for row in self.contract["retained_inputs"]
        ]
        self.assertEqual(observed, list(coordinator_v3.GATE_ORDER))
        self.assertIn(
            ("foundation_legacy_quarantine_closure_v2", "closed_current"),
            observed,
        )
        self.assertIn(
            ("unavailable_preexisting_loss_manifest_v4", "closed_current"),
            observed,
        )
        self.assertIn(
            (
                "unavailable_preexisting_loss_audit_v4",
                "durable_custody_current",
            ),
            observed,
        )
        self.assertIn(("legacy_quarantine_epoch_v4", "sealed_current"), observed)
        self.assertIn(
            ("legacy_removal_expected_old_binding_v3", "bound_current"),
            observed,
        )
        self.assertIn(("legacy_removal_guard_v3", "minted_current"), observed)
        for row in self.contract["retained_inputs"]:
            if "count" in row:
                self.assertEqual(row["count"], 0)

    def test_packet_primary_ref_gate_and_historical_version_mutants_refuse(self) -> None:
        mutants = []
        packet = copy.deepcopy(self.contract)
        packet["approved_packet_identity"] = "sha256:" + "f" * 64
        mutants.append(packet)
        packet_binding = copy.deepcopy(self.contract)
        packet_binding["packet_bindings"]["approval"]["byte_length"] += 1
        mutants.append(packet_binding)
        primary = copy.deepcopy(self.contract)
        primary["candidate_ref"]["repository_realpath"] = primary[
            "protected_primary"
        ]["checkout_realpath"]
        mutants.append(primary)
        primary_ref = copy.deepcopy(self.contract)
        primary_ref["candidate_ref"]["ref"] = "refs/heads/main"
        mutants.append(primary_ref)
        historical_loss = copy.deepcopy(self.contract)
        historical_loss["retained_inputs"][5][
            "kind"
        ] = "unavailable_preexisting_loss_manifest_v3"
        mutants.append(historical_loss)
        historical_epoch = copy.deepcopy(self.contract)
        historical_epoch["retained_inputs"][9][
            "kind"
        ] = "legacy_quarantine_epoch_v3"
        mutants.append(historical_epoch)
        positive_consumer = copy.deepcopy(self.contract)
        positive_consumer["retained_inputs"][14]["count"] = 1
        mutants.append(positive_consumer)
        for mutant in mutants:
            with self.subTest(index=mutants.index(mutant)), self.assertRaises(
                coordinator_v3.CoordinatorError
            ):
                coordinator_v3.validate_contract(mutant)

    def _materialize_artifacts(
        self, value: dict[str, object], artifact_root: Path
    ) -> None:
        packet_dir = artifact_root / "packet"
        packet_dir.mkdir(parents=True)
        for binding in value["packet_bindings"].values():
            target = artifact_root / binding["path"]
            shutil.copyfile(PACKET_ROOT / target.name, target)
            raw = target.read_bytes()
            self.assertEqual(len(raw), binding["byte_length"])
            self.assertEqual(coordinator_v3.digest(raw), binding["sha256"])
        for index, gate in enumerate(value["retained_inputs"]):
            binding = gate["evidence"]
            target = artifact_root / binding["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            raw = f"retained-v8-gate-{index}\n".encode()
            target.write_bytes(raw)
            binding["byte_length"] = len(raw)
            binding["sha256"] = coordinator_v3.digest(raw)

    def test_bound_packet_semantics_and_all_retained_bytes_are_required(self) -> None:
        if not PACKET_ROOT.is_dir():
            self.skipTest("authoritative V8 packet is unavailable")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value = copy.deepcopy(self.contract)
            self._materialize_artifacts(value, root)
            coordinator_v3.validate_bound_artifacts(value, root)
            target = (
                root
                / value["retained_inputs"][5]["evidence"]["path"]
            )
            target.write_text("drift\n", encoding="utf-8")
            with self.assertRaisesRegex(
                coordinator_v3.CoordinatorError, "bytes differ"
            ):
                coordinator_v3.validate_bound_artifacts(value, root)

    def test_packet_semantic_substitution_refuses_even_with_rebound_bytes(self) -> None:
        if not PACKET_ROOT.is_dir():
            self.skipTest("authoritative V8 packet is unavailable")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value = copy.deepcopy(self.contract)
            self._materialize_artifacts(value, root)
            approval = root / value["packet_bindings"]["approval"]["path"]
            payload = json.loads(approval.read_text(encoding="utf-8"))
            payload["shared_file_authority"] = "ExternalProofControl"
            raw = coordinator_v3.canonical_bytes(payload)
            approval.write_bytes(raw)
            value["packet_bindings"]["approval"]["byte_length"] = len(raw)
            value["packet_bindings"]["approval"]["sha256"] = coordinator_v3.digest(
                raw
            )
            with self.assertRaises(coordinator_v3.CoordinatorError):
                coordinator_v3.validate_contract(value)

    def test_executor_calls_one_cas_boundary_without_performing_a_real_cas(self) -> None:
        preimage = copy.deepcopy(self.contract)
        preimage["cas_observation"]["state"] = "exact_expected_preimage"
        postimage = copy.deepcopy(self.contract)
        postimage["cas_observation"] = {
            "state": "exact_declared_postimage",
            "observed_commit": self.contract["candidate_ref"]["declared_postimage"][
                "commit"
            ],
            "observed_tree": self.contract["candidate_ref"]["declared_postimage"][
                "tree"
            ],
        }
        with mock.patch.object(
            coordinator_v3,
            "observe_candidate_ref",
            side_effect=[preimage, postimage],
        ), mock.patch.object(
            coordinator_v3,
            "_candidate_repository",
            return_value=Path("/test-only/isolated-successor"),
        ), mock.patch.object(
            coordinator_v3, "verify_protected_primary_currentness"
        ) as currentness, mock.patch.object(
            coordinator_v3, "_update_candidate_ref_once"
        ) as update:
            result = coordinator_v3.execute_isolated_candidate_ref_cas(
                self.contract,
                Path("/test-only/artifacts"),
                Path("/test-only/isolated-successor"),
            )
        currentness.assert_called_once()
        update.assert_called_once_with(
            Path("/test-only/isolated-successor"),
            coordinator_v3.CANDIDATE_REF,
            self.contract["candidate_ref"]["declared_postimage"]["commit"],
            self.contract["candidate_ref"]["expected_preimage"]["commit"],
        )
        self.assertEqual(
            result["cas_observation"]["state"], "exact_declared_postimage"
        )

    def test_postimage_replay_has_no_second_cas_call(self) -> None:
        postimage = copy.deepcopy(self.contract)
        postimage["cas_observation"] = {
            "state": "exact_declared_postimage",
            "observed_commit": self.contract["candidate_ref"]["declared_postimage"][
                "commit"
            ],
            "observed_tree": self.contract["candidate_ref"]["declared_postimage"][
                "tree"
            ],
        }
        with mock.patch.object(
            coordinator_v3, "observe_candidate_ref", return_value=postimage
        ), mock.patch.object(
            coordinator_v3, "_update_candidate_ref_once"
        ) as update:
            result = coordinator_v3.execute_isolated_candidate_ref_cas(
                self.contract,
                Path("/test-only/artifacts"),
                Path("/test-only/isolated-successor"),
            )
        update.assert_not_called()
        self.assertEqual(result, postimage)

    def test_source_has_one_fixed_ref_write_and_no_product_or_seal_effects(self) -> None:
        source = (ROOT / "coordinator_v3.py").read_text(encoding="utf-8")
        self.assertEqual(source.count('"update-ref"'), 1)
        for token in (
            "LegacyRemovalGuardV3(",
            "os.unlink",
            "shutil.rmtree",
            '"checkout"',
            '"merge"',
            "shell=True",
            "seal.execute",
            "receipt.publish",
            "installation.prune",
        ):
            self.assertNotIn(token, source)


if __name__ == "__main__":
    unittest.main()
