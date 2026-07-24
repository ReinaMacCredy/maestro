from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import build
import validate as candidate_validate


def write_closed_proof_manifest(directory: Path) -> Path:
    proof_manifest = directory / "stage0-proof-manifest.v1.json"
    proof_cbor = proof_manifest.with_suffix(".cbor")
    gates = []
    canonical_gates = []
    for tag, name in enumerate(build.REQUIRED_PROOF_GATES, start=1):
        validator_path = f"validators/{name}.py"
        validator_hash = build.hashlib.sha256(validator_path.encode("ascii")).digest()
        result_hash = build.hashlib.sha256(
            (
                "verified_non_promoting"
                if name == "external_input_authorization"
                else f"{name}:passed"
            ).encode("ascii")
        ).digest()
        gates.append(
            {
                "tag": tag,
                "name": name,
                "result": "passed",
                "result_class": (
                    "verified_non_promoting"
                    if name == "external_input_authorization"
                    else "verified"
                ),
                "validator_artifacts": [
                    {"path": validator_path, "sha256": validator_hash.hex()}
                ],
            }
        )
        canonical_gates.append(
            [
                tag,
                name,
                [],
                [[validator_path, build.Bytes(validator_hash)]],
                [],
                1,
                (
                    "verified_non_promoting"
                    if name == "external_input_authorization"
                    else "verified"
                ),
                build.Bytes(result_hash),
                [],
            ]
        )
    canonical = [1, canonical_gates]
    encoded = build.cbor(canonical)
    proof_manifest.write_text(
        json.dumps(
            {
                "schema": "maestro.vnext.stage0-proof-manifest.v1",
                "candidate_only": True,
                "runtime_activation": False,
                "identity": build.rendered(
                    build.digest(build.STAGE0_PROOF_MANIFEST_DOMAIN, canonical)
                ),
                "gate_count": len(gates),
                "gates": gates,
                "canonical_value": build.json_value(canonical),
                "canonical_cbor_sha256": build.hashlib.sha256(encoded).hexdigest(),
            },
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n",
        encoding="utf-8",
    )
    proof_cbor.write_bytes(encoded)
    return proof_manifest


class CandidateRootBuildTest(unittest.TestCase):
    def test_cbor_keeps_booleans_distinct_from_integers(self) -> None:
        self.assertEqual(build.cbor(False), b"\xf4")
        self.assertEqual(build.cbor(True), b"\xf5")
        self.assertEqual(build.cbor(0), b"\x00")

    def test_successor_external_provenance_is_forbidden_from_root_values(self) -> None:
        approval = {
            "packet_sha256": "fb33b048b59c66df9858558a2c80e59a478d101465761f902366c9a00751cbc5",
            "candidate_input_commitment": "c180b31a6b2649ed416eeeaa614e98960ec3eef030b69ff90a0e0f63ed1ff93e",
            "build_plan_handoff": "bd1e6e55ff473250557be13bb8332df97a2c60eac62e964702e5eca67991b352",
        }
        event = {
            "recipient_thread_id": "019f4d99-a48a-7390-9560-7c7a9dc63a8d",
            "approval_turn_id": "019f9154-6de5-7612-9d3f-136c4c1324fd",
            "user_message_id": "msg_019f9154-6e72-7d00-8d41-f3ef5e7917dc",
        }
        forbidden = build.forbidden_promotion_values(approval, event)
        for value in (
            *approval.values(),
            *event.values(),
        ):
            with self.assertRaises(ValueError):
                build.scan_forbidden({"candidate_root_field": value}, forbidden)
        build.scan_forbidden(
            {"candidate_root_field": "sha256:" + "ab" * 32},
            forbidden,
        )

    def test_closed_fixture_materializes_every_required_surface(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            resource_release = directory / "resource-release.v1.json"
            embedded_release = directory / "embedded-release-bundle.v1.json"
            effect_finalization = directory / "effect-finalization-receipt.v1.json"
            resource_successor = directory / "expected-delta-successor.v1.json"
            proof_manifest = write_closed_proof_manifest(directory)
            resource_schema = "maestro.vnext.stage0.resource-release.v1"
            resource_value = [1, False]
            resource_cbor = build.cbor([resource_schema, resource_value])
            effect = build.load(build.EFFECT_HOME)
            h2_identity = "sha256:" + "03" * 32
            h3_identity = "sha256:" + "04" * 32
            successor_schema = "maestro.vnext.migration-cutover-expected-delta-successor.v1"
            successor_value = [1]
            successor_cbor = build.cbor([successor_schema, successor_value])
            successor_identity = build.rendered(build.hashlib.sha256(successor_cbor).digest())
            release_envelope = [
                "maestro.vnext.release.manifest.v1",
                "release-schema",
                "membership-schema",
                [1],
                [],
            ]
            release_cbor = build.cbor(release_envelope)
            release_digest = build.hashlib.sha256(release_cbor).hexdigest()
            release_identity = f"sha256:{release_digest}"
            release_document = {
                "schema": "maestro.vnext.embedded-release-bundle.manifest.v1",
                "identity_protocol": "ManifestIdentityV1",
                "release_id": release_digest,
                "identity": release_identity,
                "manifest_identity_envelope": release_envelope,
                "canonical_value": release_envelope[3:5],
                "canonical_cbor_sha256": release_digest,
                "canonical_cbor_byte_length": len(release_cbor),
                "canonical_cbor_hex": release_cbor.hex(),
                "candidate_only": True,
                "runtime_activation": False,
                "bundle_ids": [f"{index:064x}" for index in range(1, 9)],
                "census_id": "09" * 32,
                "sole_release_root": True,
            }
            embedded_release.write_text(json.dumps(release_document), encoding="utf-8")
            embedded_release.with_suffix(".cbor").write_bytes(release_cbor)
            slot_names = list(build.RESOURCE_SUCCESSOR_SLOTS)
            slot_ids = [
                "sha256:" + build.hashlib.sha256(name.encode("ascii")).hexdigest()
                for name in slot_names
            ]
            slot_ids[slot_names.index("effect_control_h2")] = h2_identity
            slot_ids[slot_names.index("local_withdrawal_h3")] = h3_identity
            slot_ids[slot_names.index("release_binding")] = release_identity
            downstream_obligations = [
                {
                    "identity_kind": identity_kind,
                    "logical_key": logical_key,
                    "predecessor_identity": None,
                    "successor_identity": None,
                    "disposition": "Introduce",
                    "depends_on_release_identity": release_identity,
                    "status": "pending_downstream_stage0_producer",
                    "owner": "candidate-root-worker",
                }
                for identity_kind, logical_key in (
                    ("RootInput", "candidate-root"),
                    ("RootInput", "candidate-finalization"),
                    ("HandoffInput", "candidate-handoff"),
                )
            ]
            successor_document = {
                "schema": successor_schema,
                "identity_protocol": "Stage0CanonicalCommitmentV1",
                "candidate_only": True,
                "publication_status": "resolved_through_release_downstream_obligations_pending",
                "runtime_activation": False,
                "blocked_dependency_count": len(downstream_obligations),
                "unresolved_obligation_count": len(downstream_obligations),
                "resolved_entry_count": sum(build.THROUGH_RELEASE_IDENTITY_COUNTS.values()),
                "identity": successor_identity,
                "identity_scope": "canonical_commitment_envelope_only",
                "canonical_commitment_envelope": [successor_schema, successor_value],
                "canonical_value": successor_value,
                "canonical_cbor_sha256": build.hashlib.sha256(successor_cbor).hexdigest(),
                "canonical_cbor_byte_length": len(successor_cbor),
                "canonical_cbor_hex": successor_cbor.hex(),
                "entries": [],
                "downstream_obligations": downstream_obligations,
                "exact_identity_kind_counts": build.THROUGH_RELEASE_IDENTITY_COUNTS,
            }
            entry_kinds = [
                kind
                for kind, count in build.THROUGH_RELEASE_IDENTITY_COUNTS.items()
                for _ in range(count)
            ]
            for index, kind in enumerate(entry_kinds):
                successor_document["entries"].append(
                    {
                        "identity_kind": kind,
                        "logical_key": f"{kind.lower()}:fixture:{index:03d}",
                        "predecessor_identity": None,
                        "successor_identity": "sha256:"
                        + build.hashlib.sha256(f"delta-entry:{index}".encode("ascii")).hexdigest(),
                        "disposition": "Introduce",
                        "source_artifact": "fixture.json",
                        "source_artifact_sha256": "07" * 32,
                    }
                )
            non_release_slots = [
                slot_id
                for slot_name, slot_id in zip(slot_names, slot_ids, strict=True)
                if slot_name != "release_binding"
            ]
            for index, slot_id in enumerate(non_release_slots):
                successor_document["entries"][index]["successor_identity"] = slot_id
            release_entry = next(
                item
                for item in successor_document["entries"]
                if item["identity_kind"] == "Release"
            )
            release_entry["successor_identity"] = release_identity
            resource_successor.write_text(
                json.dumps(successor_document),
                encoding="utf-8",
            )
            resource_successor.with_suffix(".cbor").write_bytes(successor_cbor)
            effect_body = {
                "schema_version": "maestro.vnext.stage0.effect-home-finalization-receipt.v1",
                "finalization_state": "final",
                "candidate_only": True,
                "runtime": "inactive",
                "runtime_activation": False,
                "expected_delta_manifest_id": effect["identity"],
                "encoder_receipt_sha256": build.artifact_hash(build.EFFECT_RECEIPT),
                "unresolved_actual_semantic_consumers": 0,
                "h2_manifest_identity": h2_identity,
                "h3_withdrawal_identity": h3_identity,
            }
            effect_finalization.write_text(
                json.dumps(
                    {
                        **effect_body,
                        "identity": build.rendered(
                            build.hashlib.sha256(
                                json.dumps(
                                    effect_body, sort_keys=True, separators=(",", ":"), ensure_ascii=True
                                ).encode("ascii")
                            ).digest()
                        ),
                    }
                ),
                encoding="utf-8",
            )
            resource_release.write_text(
                json.dumps(
                    {
                        "schema": resource_schema,
                        "identity_protocol": "Stage0CanonicalCommitmentV1",
                        "identity_scope": "canonical_commitment_envelope_only",
                        "candidate_only": True,
                        "source_publication": False,
                        "runtime_activation": False,
                        "runtime_registration": False,
                        "installation": False,
                        "identity": build.rendered(build.hashlib.sha256(resource_cbor).digest()),
                        "canonical_commitment_envelope": [resource_schema, resource_value],
                        "canonical_value": resource_value,
                        "canonical_cbor_sha256": build.hashlib.sha256(resource_cbor).hexdigest(),
                        "canonical_cbor_byte_length": len(resource_cbor),
                        "canonical_cbor_hex": resource_cbor.hex(),
                        "declared_successor_slot_count": len(slot_names),
                        "resolved_successor_slot_count": len(slot_names),
                        "blocked_successor_slot_count": 0,
                        "null_successor_identity_count": 0,
                        "resolved_successor_bindings": [
                            {"slot_name": name, "successor_identity": identity}
                            for name, identity in zip(slot_names, slot_ids, strict=True)
                        ],
                        "resolved_expected_delta_commitment_id": successor_identity,
                        "expected_delta": successor_document,
                        "downstream_delta_obligations": downstream_obligations,
                        "embedded_release_bundle": release_document,
                        "resource_count": 377,
                        "resources": [{} for _ in range(377)],
                        "bundle_count": 8,
                        "bundles": [{} for _ in range(8)],
                        "effect_home_finalization_receipt_sha256": build.artifact_hash(
                            effect_finalization
                        ),
                        "effect_home_finalization_identity": json.loads(
                            effect_finalization.read_text()
                        )["identity"],
                        "effect_home_expected_delta_manifest_id": effect["identity"],
                    }
                ),
                encoding="utf-8",
            )
            resource_release.with_suffix(".cbor").write_bytes(resource_cbor)
            original_resource = build.RESOURCE_RELEASE
            original_embedded_release = build.EMBEDDED_RELEASE
            original_effect = build.EFFECT_FINALIZATION
            original_successor = build.RESOURCE_SUCCESSOR_DELTA
            original_proof_manifest = build.PROOF_MANIFEST
            original_proof_cbor = build.PROOF_MANIFEST_CBOR
            original_verify = build.verify_closed_sources
            original_artifact_hash = build.artifact_hash
            source_inputs = build.load(build.INPUT_BINDINGS)["current_source_inputs"]

            def fixture_artifact_hash(path: Path) -> str:
                if not path.exists() and path.name in {"card.yaml", "design.md"}:
                    return source_inputs[f"{path.stem}_sha256"]
                return original_artifact_hash(path)

            try:
                build.RESOURCE_RELEASE = resource_release
                build.EMBEDDED_RELEASE = embedded_release
                build.EFFECT_FINALIZATION = effect_finalization
                build.RESOURCE_SUCCESSOR_DELTA = resource_successor
                build.PROOF_MANIFEST = proof_manifest
                build.PROOF_MANIFEST_CBOR = proof_manifest.with_suffix(".cbor")
                build.verify_closed_sources = lambda: None
                build.artifact_hash = fixture_artifact_hash
                result = build.build()
            finally:
                build.RESOURCE_RELEASE = original_resource
                build.EMBEDDED_RELEASE = original_embedded_release
                build.EFFECT_FINALIZATION = original_effect
                build.RESOURCE_SUCCESSOR_DELTA = original_successor
                build.PROOF_MANIFEST = original_proof_manifest
                build.PROOF_MANIFEST_CBOR = original_proof_cbor
                build.verify_closed_sources = original_verify
                build.artifact_hash = original_artifact_hash

            output = directory / "candidate-root"
            output.mkdir()
            for name, (document, canonical) in result["generated"].items():
                build.write_artifact(output / name, document, canonical)
            (output / "decision-root-bindings.v1.json").write_text(
                json.dumps(
                    {
                        "schema": "maestro.vnext.exact-decision-root-bindings.v1",
                        "candidate_only": True,
                        "runtime": "inactive",
                        "decision_closure_id": build.rendered(result["sources"]["decision_id"]),
                        "bindings": result["bindings"],
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                )
                + "\n",
                encoding="utf-8",
            )
            build.PROOF_MANIFEST = proof_manifest
            build.PROOF_MANIFEST_CBOR = proof_manifest.with_suffix(".cbor")
            try:
                candidate_validate.validate(output)
                candidate_validate.ruby_equality(output)
                candidate_validate.mutant_rejections(output)
            finally:
                build.PROOF_MANIFEST = original_proof_manifest
                build.PROOF_MANIFEST_CBOR = original_proof_cbor

        generated = result["generated"]
        root_document, _ = generated["candidate-contract-root.v1.json"]
        manifest_document, _ = generated["design-finalization-manifest.v1.json"]
        handoff_document, _ = generated["canonical-build-handoff.v1.json"]
        expected_component_count = len(result["bindings"]) + len(build.COMPONENT_KINDS) - 1
        self.assertEqual(root_document["component_count"], expected_component_count)
        self.assertEqual(len(result["bindings"]), len(result["sources"]["materials"]))
        self.assertEqual(len(manifest_document["pinned_inputs"]), len(build.FINALIZATION_KINDS))
        self.assertEqual(handoff_document["component_count"], expected_component_count)
        self.assertEqual(
            manifest_document["stage0_proof_manifest"],
            handoff_document["stage0_proof_manifest"],
        )
        literal_schema_closure = next(
            component
            for component in root_document["components"]
            if component["kind_tag"] == 17
        )
        self.assertEqual(
            {
                "schema_id": literal_schema_closure["owned_commitments"]["submission_claim_set_schema_id"],
                "artifact_sha256": literal_schema_closure["owned_commitments"]["submission_claim_set_artifact_sha256"],
            },
            generated["design-revision.v1.json"][0]["submission_claim_set"],
        )

    def test_candidate_root_refuses_incomplete_or_failed_proof_gate_sets(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            proof_manifest = write_closed_proof_manifest(directory)
            original_proof_manifest = build.PROOF_MANIFEST
            original_proof_cbor = build.PROOF_MANIFEST_CBOR
            try:
                build.PROOF_MANIFEST = proof_manifest
                build.PROOF_MANIFEST_CBOR = proof_manifest.with_suffix(".cbor")
                build.stage0_proof_manifest(set())

                document = json.loads(proof_manifest.read_text(encoding="utf-8"))
                document["gates"].pop()
                proof_manifest.write_text(json.dumps(document), encoding="utf-8")
                with self.assertRaises(build.Blocked):
                    build.stage0_proof_manifest(set())

                write_closed_proof_manifest(directory)
                document = json.loads(proof_manifest.read_text(encoding="utf-8"))
                document["gates"][0]["result"] = "failed"
                proof_manifest.write_text(json.dumps(document), encoding="utf-8")
                with self.assertRaises(build.Blocked):
                    build.stage0_proof_manifest(set())
            finally:
                build.PROOF_MANIFEST = original_proof_manifest
                build.PROOF_MANIFEST_CBOR = original_proof_cbor

    def test_successor_decision_manifest_binds_every_record_before_candidate_root(self) -> None:
        manifest_path = Path(
            "/private/tmp/maestro-vnext-materialization-successor-packet/"
            "successor-decision-store-manifest.v1.txt"
        )
        records = [
            {
                "id": decision_id,
                "terminal_status": terminal_status,
                "raw_record_sha256": raw_record_sha256,
                "raw_body_sha256": raw_body_sha256,
            }
            for decision_id, terminal_status, raw_record_sha256, raw_body_sha256 in (
                line.split("\t")
                for line in manifest_path.read_text(encoding="ascii").splitlines()
            )
        ]
        decision = {
            "source_provenance_excluded_from_identity": {
                "decisions_sha256": build.SUCCESSOR_DECISION_STORE_MANIFEST_SHA256
            },
            "records": records,
            "materializations": [{"decision_sources": [records[0].copy()]}],
        }
        build.validate_successor_decision_manifest(decision)

        mutant = json.loads(json.dumps(decision))
        mutant["records"][0]["raw_body_sha256"] = "00" * 32
        mutant["materializations"][0]["decision_sources"][0][
            "raw_body_sha256"
        ] = "00" * 32
        with self.assertRaises(ValueError):
            build.validate_successor_decision_manifest(mutant)


if __name__ == "__main__":
    unittest.main()
