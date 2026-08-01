from __future__ import annotations

import copy
import hashlib
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from . import build
from . import validate


def synthetic_manifest() -> tuple[dict[str, object], bytes]:
    component_tags = build.rust_enum_tags(
        "src/domain/contract/component_kind.rs",
        "ContractComponentKindV1",
    )
    finalization_tags = build.rust_enum_tags(
        "src/domain/contract/finalization.rs",
        "FinalizationInputKindV1",
    )
    assertions: dict[int, dict[str, object]] = {
        4: {
            "d0aa": {
                "body_sha256": "85870762931cc790a0dd16e5e4b7c55c56c871fe500106274472d2308fe7d72a",
                "symbol_count": 150,
            },
            "d116": {
                "body_sha256": "593ee2afa0356819033aa2e2d955b2fbf38a2cc2af7e23844a94159085ef37f7",
                "role_count": 7,
                "route_count": 109,
            },
            "d70b": {
                "body_sha256": "2ed739642474a92b110002a224b7f36fa39867244d6368d1904fd78de24e3a80",
                "symbol_count": 147,
            },
            "e346_disposition": "separate_catalog_predecessor_gate",
        },
        5: {
            "publish_observation": {
                "current_descriptor_id": "descriptor",
                "current_manifest_id": "manifest",
                "current_tag": 39,
                "predecessor_tag": 30,
            }
        },
        14: {
            "pending_obligation_stage": "Stage11",
            "passed_claim": "requirements_frozen_not_runtime_complete",
            "proof_status": "pending_stage0_execution_and_rehearsal",
            "runtime_proof_complete": False,
            "stage": "stage0_candidate_only",
            "stage0_execution_complete": False,
            "stage0_rehearsal_complete": False,
            "status": "requirements_complete_runtime_proof_pending",
        },
        15: {
            "component_kind_tags": component_tags,
            "finalization_input_kind_tags": finalization_tags,
        },
    }
    counts: dict[int, dict[str, int]] = {
        14: {"pending_runtime_proof_count": 1},
        15: {
            "component_kind_count": len(component_tags),
            "finalization_input_kind_count": len(finalization_tags),
        },
    }
    gates = []
    canonical_gates = []
    for tag, name in enumerate(build.GATE_NAMES, start=1):
        validator_path = f"validators/{tag}.py"
        validator_sha = hashlib.sha256(validator_path.encode("ascii")).hexdigest()
        validator_rows = [{"path": validator_path, "sha256": validator_sha}]
        gate_assertions = assertions.get(tag, {})
        semantic_counts = [
            {"name": count_name, "value": count}
            for count_name, count in sorted(counts.get(tag, {}).items())
        ]
        result_class = build.VERIFIED_NON_PROMOTING if tag == 1 else "verified"
        result_sha = (
            hashlib.sha256(build.VERIFIED_NON_PROMOTING.encode("ascii")).hexdigest()
            if tag == 1
            else build.result_hash(
                name,
                [],
                validator_rows,
                [],
                [(row["name"], row["value"]) for row in semantic_counts],
                gate_assertions,
            )
        )
        gate = {
            "tag": tag,
            "name": name,
            "source_artifacts": [],
            "validator_artifacts": validator_rows,
            "input_artifacts": [],
            "result": "passed",
            "result_class": result_class,
            "result_sha256": result_sha,
            "semantic_counts": semantic_counts,
        }
        if gate_assertions:
            gate["assertions"] = gate_assertions
        gates.append(gate)
        canonical_gates.append(
            [
                tag,
                name,
                [],
                [[validator_path, build.Bytes(bytes.fromhex(validator_sha))]],
                [],
                1,
                result_class,
                build.Bytes(bytes.fromhex(result_sha)),
                [[row["name"], row["value"]] for row in semantic_counts],
            ]
        )
    canonical = [1, canonical_gates]
    encoded = build.cbor(canonical)
    document: dict[str, object] = {
        "schema": build.DOMAIN,
        "candidate_only": True,
        "runtime_activation": False,
        "identity": build.identity(canonical),
        "gate_count": len(gates),
        "gates": gates,
        "canonical_value": build.json_value(canonical),
        "canonical_cbor_sha256": hashlib.sha256(encoded).hexdigest(),
        "canonical_cbor_byte_length": len(encoded),
    }
    return document, encoded


def replace_assertions(
    document: dict[str, object],
    gate_index: int,
    assertions: dict[str, object],
) -> tuple[dict[str, object], bytes]:
    gate = document["gates"][gate_index]
    gate["assertions"] = assertions
    result_sha = build.result_hash(
        gate["name"],
        gate["source_artifacts"],
        gate["validator_artifacts"],
        gate["input_artifacts"],
        [(row["name"], row["value"]) for row in gate["semantic_counts"]],
        assertions,
    )
    gate["result_sha256"] = result_sha
    document["canonical_value"][1][gate_index][7] = {"bytes": result_sha}
    return validate.resigned(document)


class ProofMatrixValidationTest(unittest.TestCase):
    def test_exact_manifest_and_all_omission_failure_mutants(self) -> None:
        document, encoded = synthetic_manifest()
        summary = validate.validate_document(document, encoded, verify_files=False)
        self.assertEqual(summary["gate_count"], len(build.GATE_NAMES))
        self.assertEqual(len(validate.mutant_rejections(document)), 2 * len(build.GATE_NAMES))

    def test_external_gate_refuses_promoting_inputs(self) -> None:
        document, _ = synthetic_manifest()
        promoted = copy.deepcopy(document)
        row = {"path": "external/approval.json", "sha256": "00" * 32}
        promoted["gates"][0]["input_artifacts"] = [row]
        promoted["canonical_value"][1][0][4] = [
            [row["path"], {"bytes": row["sha256"]}]
        ]
        promoted, encoded = validate.resigned(promoted)
        with self.assertRaises(validate.ProofValidationError):
            validate.validate_document(promoted, encoded, verify_files=False)

    def test_independent_ruby_encoder_reproduces_synthetic_bytes(self) -> None:
        document, encoded = synthetic_manifest()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "stage0-proof-manifest.v1.json").write_bytes(
                build.json_bytes(document)
            )
            (root / "stage0-proof-manifest.v1.cbor").write_bytes(encoded)
            process = subprocess.run(
                ["/usr/bin/ruby", str(validate.TOOLS / "encode.rb")],
                cwd=validate.WORKSPACE,
                env={**os.environ, "STAGE0_PROOF_MATRIX_ROOT": str(root)},
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(process.returncode, 0, process.stderr)
        receipt = json.loads(process.stdout)
        self.assertEqual(receipt["status"], "pass")
        self.assertEqual(receipt["canonical_cbor_sha256"], hashlib.sha256(encoded).hexdigest())

    def test_direct_transitive_and_keyed_approval_values_are_rejected(self) -> None:
        bindings = json.loads(validate.INPUT_BINDINGS.read_text(encoding="utf-8"))
        packet_sha = bindings["external_approval"]["packet_sha256"]
        values = (
            packet_sha,
            f"sha256:{hashlib.sha256(packet_sha.encode('ascii')).hexdigest()}",
        )
        for value in values:
            with self.subTest(value=value):
                document, _ = synthetic_manifest()
                assertions = {"forbidden": value}
                document, encoded = replace_assertions(document, 1, assertions)
                with self.assertRaises(validate.ProofValidationError):
                    validate.validate_document(document, encoded, verify_files=False)

        bindings_sha = hashlib.sha256(validate.INPUT_BINDINGS.read_bytes()).hexdigest()
        keyed_assertions = (
            {packet_sha: "forbidden_key"},
            {"nested": {bindings_sha: "forbidden_nested_key"}},
        )
        for assertions in keyed_assertions:
            with self.subTest(assertions=assertions):
                document, _ = synthetic_manifest()
                document, encoded = replace_assertions(document, 1, assertions)
                with self.assertRaises(validate.ProofValidationError):
                    validate.validate_document(document, encoded, verify_files=False)

    def test_migration_gate_cannot_claim_runtime_completion(self) -> None:
        document, _ = synthetic_manifest()
        assertions = copy.deepcopy(document["gates"][13]["assertions"])
        assertions["runtime_proof_complete"] = True
        document, encoded = replace_assertions(document, 13, assertions)
        with self.assertRaises(validate.ProofValidationError):
            validate.validate_document(document, encoded, verify_files=False)


if __name__ == "__main__":
    unittest.main()
