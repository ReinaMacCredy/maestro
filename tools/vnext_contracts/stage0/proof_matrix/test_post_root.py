from __future__ import annotations

import copy
import hashlib
import json
import sys
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import post_root


PASS_RECONSTRUCTION = {
    "python_reconstruction_status": "pass",
    "ruby_reconstruction_status": "pass",
    "rust_reconstruction_status": "pass",
}


def identifier(seed: int) -> str:
    return f"sha256:{seed:064x}"


def digest(seed: int) -> str:
    return f"{seed:064x}"


def write_json(path: Path, document: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )


def write_stage0_commitment(
    path: Path, schema: str, canonical_value: object, **fields: object
) -> dict[str, object]:
    raw = json.dumps(
        [schema, canonical_value], sort_keys=True, separators=(",", ":")
    ).encode("ascii")
    identity = hashlib.sha256(raw).hexdigest()
    document: dict[str, object] = {
        "schema": schema,
        "identity_protocol": "Stage0CanonicalCommitmentV1",
        "identity_scope": "canonical_commitment_envelope_only",
        "identity": f"sha256:{identity}",
        "canonical_commitment_envelope": [schema, canonical_value],
        "canonical_value": canonical_value,
        "canonical_cbor_sha256": identity,
        "canonical_cbor_byte_length": len(raw),
        "canonical_cbor_hex": raw.hex(),
        "candidate_only": True,
        "runtime_activation": False,
        **fields,
    }
    write_json(path, document)
    path.with_suffix(".cbor").write_bytes(raw)
    return document


@contextmanager
def closed_fixture():
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        proof_dir = root / "contracts/vnext/stage0/proof-matrix"
        candidate_dir = root / "contracts/vnext/stage0/candidate-root"
        resource_dir = root / "contracts/vnext/stage0/resource-release"
        proof_manifest = proof_dir / "stage0-proof-manifest.v1.json"
        candidate_root = candidate_dir / "candidate-contract-root.v1.json"
        finalization = candidate_dir / "design-finalization-manifest.v1.json"
        handoff = candidate_dir / "canonical-build-handoff.v1.json"
        bindings = candidate_dir / "decision-root-bindings.v1.json"
        decision_closure = root / "decision-closure.v1.json"
        resource_release = resource_dir / "resource-release.v1.json"
        expected_delta = resource_dir / "expected-delta-successor.v1.json"
        embedded_release = resource_dir / "embedded-release-bundle.v1.json"
        release_census = resource_dir / "release-resource-census.v1.json"
        input_bindings = root / "input-bindings.json"

        proof_id = identifier(1)
        root_id = identifier(2)
        finalization_id = identifier(3)
        handoff_id = identifier(4)
        decision_id = identifier(8)
        materialization_count = 3
        aggregate_component_count = 2
        finalization_input_count = 4
        proof = {
            "schema": "maestro.vnext.stage0-proof-manifest.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "identity": proof_id,
            "gate_count": len(post_root.REQUIRED_PROOF_GATE_NAMES),
            "gates": [
                {
                    "tag": index,
                    "name": name,
                    "result": "passed",
                    "result_class": (
                        "verified_non_promoting"
                        if name == "external_input_authorization"
                        else "verified"
                    ),
                }
                for index, name in enumerate(post_root.REQUIRED_PROOF_GATE_NAMES, start=1)
            ],
        }
        migration_gate = proof["gates"][
            post_root.REQUIRED_PROOF_GATE_NAMES.index("migration_rollback")
        ]
        migration_gate.update(
            {
                "assertions": {
                    "runtime_proof_complete": False,
                    "stage": "stage0_candidate_only",
                    "status": "requirements_complete_runtime_proof_pending",
                    "passed_claim": "requirements_frozen_not_runtime_complete",
                    "pending_obligation_stage": "Stage11",
                    "proof_status": "pending_stage0_execution_and_rehearsal",
                    "stage0_execution_complete": False,
                    "stage0_rehearsal_complete": False,
                },
                "semantic_counts": [
                    {"name": "pending_runtime_proof_count", "value": 2},
                    {"name": "requirement_row_count", "value": 4},
                ],
            }
        )
        write_json(proof_manifest, proof)
        proof_sha = hashlib.sha256(proof_manifest.read_bytes()).hexdigest()
        proof_binding = {
            "identity": proof_id,
            "artifact_sha256": proof_sha,
            "gate_count": len(post_root.REQUIRED_PROOF_GATE_NAMES),
        }
        materialization_ids = [digest(1000 + index) for index in range(materialization_count)]
        normative_component_ids = [
            identifier(2000 + index) for index in range(materialization_count)
        ]
        aggregate_components = [
            {"kind_tag": index + 1, "component_id": identifier(2500 + index)}
            for index in range(aggregate_component_count)
        ]
        components = [
            {"kind_tag": 12, "component_id": component_id}
            for component_id in normative_component_ids
        ] + aggregate_components
        write_json(
            decision_closure,
            {
                "schema": "maestro.vnext.decision-closure.v1",
                "identity": decision_id,
                "materializations": [
                    {
                        "id": materialization_id,
                        "materialization_base": {
                            "kind": "initial_external_design_closure",
                            "decision_closure_id": decision_id,
                        },
                    }
                    for materialization_id in reversed(materialization_ids)
                ],
            },
        )
        write_json(
            candidate_root,
            {
                "schema": "maestro.vnext.candidate-contract-root.v1",
                "candidate_only": True,
                "runtime": "inactive",
                "identity": root_id,
                "component_count": len(components),
                "components": components,
            },
        )
        write_json(
            finalization,
            {
                "schema": "maestro.vnext.design-finalization-manifest.v1",
                "candidate_only": True,
                "runtime": "inactive",
                "identity": finalization_id,
                "decision_closure_id": decision_id,
                "candidate_contract_root_id": root_id,
                "stage0_proof_manifest": proof_binding,
                "pinned_inputs": [
                    {"kind_tag": index}
                    for index in range(1, finalization_input_count + 1)
                ],
            },
        )
        write_json(
            handoff,
            {
                "schema": "maestro.vnext.canonical-build-handoff.v1",
                "candidate_only": True,
                "runtime": "inactive",
                "identity": handoff_id,
                "candidate_contract_root_id": root_id,
                "finalization_manifest_id": finalization_id,
                "stage0_proof_manifest": proof_binding,
                "component_count": len(components),
                "pinned_input_count": finalization_input_count,
            },
        )
        write_json(
            bindings,
            {
                "schema": "maestro.vnext.exact-decision-root-bindings.v1",
                "candidate_only": True,
                "runtime": "inactive",
                "decision_closure_id": decision_id,
                "bindings": [
                    {
                        "materialization_id": f"sha256:{materialization_ids[index]}",
                        "component_id": normative_component_ids[index],
                        "materialization_base": {
                            "kind": "initial_external_design_closure",
                            "decision_closure_id": decision_id,
                        },
                        "after_root_id": root_id,
                        "finalization_manifest_id": finalization_id,
                    }
                    for index in range(materialization_count)
                ],
            },
        )

        release_raw = b"fixture-release-manifest-cbor"
        release_digest = hashlib.sha256(release_raw).hexdigest()
        release_id = f"sha256:{release_digest}"
        release_envelope = ["release-domain", "release-schema", "membership-schema", [1], []]
        release_document = {
            "schema": "maestro.vnext.embedded-release-bundle.manifest.v1",
            "identity_protocol": "ManifestIdentityV1",
            "release_id": release_digest,
            "identity": release_id,
            "manifest_identity_envelope": release_envelope,
            "canonical_value": release_envelope[3:5],
            "canonical_cbor_sha256": release_digest,
            "canonical_cbor_byte_length": len(release_raw),
            "canonical_cbor_hex": release_raw.hex(),
            "candidate_only": True,
            "runtime_activation": False,
            "bundle_ids": [digest(4000 + index) for index in range(8)],
            "census_id": digest(4999),
            "sole_release_root": True,
        }
        write_json(embedded_release, release_document)
        embedded_release.with_suffix(".cbor").write_bytes(release_raw)

        obligations = [
            {
                "identity_kind": kind,
                "logical_key": logical_key,
                "predecessor_identity": None,
                "successor_identity": None,
                "disposition": "Introduce",
                "depends_on_release_identity": release_id,
                "status": "pending_downstream_stage0_producer",
                "owner": "candidate-root-worker",
            }
            for kind, logical_key in post_root.REQUIRED_POST_ROOT_KEYS
        ]
        successor_bindings = [
            {"slot_name": name, "successor_identity": identifier(20 + index)}
            for index, name in enumerate(post_root.RESOURCE_SUCCESSOR_SLOTS)
        ]
        successor_bindings[
            post_root.RESOURCE_SUCCESSOR_SLOTS.index("release_binding")
        ]["successor_identity"] = release_id
        entry_kinds = [
            kind
            for kind, count in post_root.THROUGH_RELEASE_IDENTITY_COUNTS.items()
            for _ in range(count)
        ]
        through_release_entries = [
            {
                "identity_kind": kind,
                "logical_key": f"{kind.lower()}:fixture:{index:03d}",
                "predecessor_identity": None,
                "successor_identity": identifier(10_000 + index),
                "disposition": "Introduce",
                "source_artifact": "generated:fixture",
                "source_artifact_sha256": digest(20_000 + index),
            }
            for index, kind in enumerate(entry_kinds)
        ]
        for index, binding in enumerate(
            row for row in successor_bindings if row["slot_name"] != "release_binding"
        ):
            through_release_entries[index]["successor_identity"] = binding["successor_identity"]
        next(
            row for row in through_release_entries if row["identity_kind"] == "Release"
        )["successor_identity"] = release_id
        delta = write_stage0_commitment(
            expected_delta,
            "maestro.vnext.migration-cutover-expected-delta-successor.v1",
            [1],
            publication_status="resolved_through_release_downstream_obligations_pending",
            resolved_entry_count=len(through_release_entries),
            blocked_dependency_count=len(obligations),
            unresolved_obligation_count=len(obligations),
            entries=through_release_entries,
            downstream_obligations=obligations,
            exact_identity_kind_counts=post_root.THROUGH_RELEASE_IDENTITY_COUNTS,
        )
        delta_id = str(delta["identity"])
        resources = [{"resource_id": identifier(30_000 + index)} for index in range(412)]
        bundles = [{"bundle_id": identifier(40_000 + index)} for index in range(8)]
        consumer_edges = [
            [identifier(50_000 + index), resources[index]["resource_id"]]
            for index in range(411)
        ]
        census_envelope = ["census-domain", "census-schema", "entry-schema", [1], []]
        census = {
            "schema": "maestro.vnext.release-resource-census.manifest.v1",
            "identity_protocol": "ManifestIdentityV1",
            "census_id": digest(4999),
            "identity": identifier(4999),
            "manifest_identity_envelope": census_envelope,
            "canonical_value": census_envelope[3:5],
            "candidate_only": True,
            "runtime_activation": False,
            "consumer_edges": consumer_edges,
        }
        write_json(release_census, census)
        resource_counts = {
            "resource_count": 412,
            "bundle_count": 8,
            "consumer_edge_count": 411,
            "downstream_obligation_count": 3,
        }
        resource = write_stage0_commitment(
            resource_release,
            "maestro.vnext.stage0.resource-release.v1",
            [1],
            source_publication=False,
            runtime_registration=False,
            installation=False,
            resource_count=412,
            resources=resources,
            bundle_count=8,
            bundles=bundles,
            declared_successor_slot_count=len(successor_bindings),
            resolved_successor_slot_count=len(successor_bindings),
            blocked_successor_slot_count=0,
            null_successor_identity_count=0,
            resolved_successor_bindings=successor_bindings,
            resolved_expected_delta_commitment_id=delta_id,
            expected_delta=delta,
            downstream_delta_obligations=obligations,
            release_resource_census=census,
            embedded_release_bundle=release_document,
            post_root_delta_identity=None,
            post_root_union_identity=None,
            post_root_status="pending_root_worker_noncanonical_delta_and_union",
            post_root_identity_feedback_into_resource_bundle_census_release=False,
            external_approval_token_input_identities=[],
        )
        resource_id = str(resource["identity"])
        resource_gate = proof["gates"][
            post_root.REQUIRED_PROOF_GATE_NAMES.index("resource_release")
        ]
        resource_source_paths = (
            post_root.RESOURCE_DESCRIPTOR_LOGICAL,
            *post_root.RESOURCE_BUNDLE_LOGICALS,
            post_root.RELEASE_CENSUS_LOGICAL,
            post_root.EMBEDDED_RELEASE_LOGICAL,
            post_root.EXPECTED_DELTA_LOGICAL,
            post_root.RESOURCE_RELEASE_LOGICAL,
        )
        for logical in (
            post_root.RESOURCE_DESCRIPTOR_LOGICAL,
            *post_root.RESOURCE_BUNDLE_LOGICALS,
        ):
            write_json(root / logical, {"fixture": logical})
        resource_gate.update(
            {
                "source_artifacts": sorted(
                    [
                        {
                            "path": logical,
                            "sha256": hashlib.sha256((root / logical).read_bytes()).hexdigest(),
                        }
                        for logical in resource_source_paths
                    ],
                    key=lambda row: row["path"],
                ),
                "assertions": {
                    "expected_delta_commitment_id": delta_id,
                    "release_id": release_id.removeprefix("sha256:"),
                    "resource_release_commitment_id": resource_id,
                },
                "semantic_counts": [
                    {"name": name, "value": value}
                    for name, value in sorted(resource_counts.items())
                ],
            }
        )
        write_json(proof_manifest, proof)
        proof_sha = hashlib.sha256(proof_manifest.read_bytes()).hexdigest()
        proof_binding = {
            "identity": proof_id,
            "artifact_sha256": proof_sha,
            "gate_count": len(post_root.REQUIRED_PROOF_GATE_NAMES),
        }
        for path in (finalization, handoff):
            document = json.loads(path.read_text(encoding="utf-8"))
            document["stage0_proof_manifest"] = proof_binding
            write_json(path, document)
        write_json(
            input_bindings,
            {
                "external_approval": {
                    "token": "sha256:" + "ab" * 32,
                    "recipient_task_id": "fixture-task",
                },
                "external_approval_event": {
                    "event_id": "fixture-event",
                    "task_incarnation": 3,
                },
            },
        )

        replacements = {
            "WORKSPACE": root,
            "OUTPUT": proof_dir,
            "PROOF_MANIFEST": proof_manifest,
            "CANDIDATE_ROOT": candidate_root,
            "FINALIZATION": finalization,
            "HANDOFF": handoff,
            "DECISION_BINDINGS": bindings,
            "DECISION_CLOSURE": decision_closure,
            "RESOURCE_RELEASE": resource_release,
            "EXPECTED_DELTA": expected_delta,
            "EMBEDDED_RELEASE": embedded_release,
            "INPUT_BINDINGS": input_bindings,
        }
        with mock.patch.multiple(post_root, **replacements):
            yield {
                "proof_sha": proof_sha,
                "proof_id": proof_id,
                "root_id": root_id,
                "finalization_id": finalization_id,
                "handoff_id": handoff_id,
                "resource_id": resource_id,
                "delta_id": delta_id,
                "release_id": release_id,
                "successor_bindings": successor_bindings,
                "binding_count": materialization_count,
                "resource_counts": resource_counts,
                "proof_dir": proof_dir,
            }


class PostRootTest(unittest.TestCase):
    def test_build_closes_the_three_exact_resource_obligations(self) -> None:
        with closed_fixture() as fixture:
            delta, receipt = post_root.build(PASS_RECONSTRUCTION)

            self.assertEqual(
                [(row["identity_kind"], row["logical_key"]) for row in delta["rows"]],
                list(post_root.REQUIRED_POST_ROOT_KEYS),
            )
            self.assertEqual(
                [row["successor_identity"] for row in delta["rows"]],
                [fixture["root_id"], fixture["finalization_id"], fixture["handoff_id"]],
            )
            self.assertTrue(all(row["predecessor_identity"] is None for row in delta["rows"]))
            self.assertTrue(all(row["disposition"] == "Introduce" for row in delta["rows"]))
            self.assertTrue(
                all(
                    row["depends_on_release_identity"] == fixture["release_id"]
                    for row in delta["rows"]
                )
            )
            self.assertEqual(delta["artifact_class"], "NONCANONICAL")
            self.assertNotIn("identity", delta)
            self.assertNotIn("canonical_cbor_sha256", delta)

            self.assertEqual(
                receipt["stage0_proof_manifest"],
                {
                    "identity": fixture["proof_id"],
                    "json_artifact_sha256": fixture["proof_sha"],
                    "gate_count": len(post_root.REQUIRED_PROOF_GATE_NAMES),
                },
            )
            self.assertEqual(receipt["candidate_contract_root_id"], fixture["root_id"])
            self.assertEqual(receipt["design_finalization_manifest_id"], fixture["finalization_id"])
            self.assertEqual(receipt["canonical_build_handoff_id"], fixture["handoff_id"])
            self.assertEqual(
                receipt["decision_root_bindings"]["binding_count"], fixture["binding_count"]
            )
            self.assertEqual(
                receipt["resource_release_commitment_id"], fixture["resource_id"]
            )
            self.assertEqual(
                receipt["expected_delta_commitment_id"], fixture["delta_id"]
            )
            self.assertEqual(receipt["release_id"], fixture["release_id"])
            self.assertEqual(
                receipt["compatibility_successor_bindings"], fixture["successor_bindings"]
            )
            self.assertEqual(receipt["resource_exact_counts"], fixture["resource_counts"])
            self.assertEqual(
                receipt["through_release_compatibility_row_count"],
                len(fixture["successor_bindings"]),
            )
            self.assertEqual(
                receipt["closed_downstream_row_count"],
                len(fixture["successor_bindings"]) + len(post_root.REQUIRED_POST_ROOT_KEYS),
            )
            self.assertEqual(receipt["union_set_equality_status"], "pass")
            self.assertEqual(receipt["stage0_requirement_freeze_status"], "pass")
            self.assertEqual(receipt["later_runtime_proof_status"], "pending_stage_11")
            self.assertEqual(receipt["runtime_status"], "inactive")
            self.assertEqual(receipt["external_approval_exclusion_status"], "pass")
            self.assertEqual(receipt["post_root_identity_count"], 0)
            self.assertNotIn("identity", receipt)
            post_root.validate(delta, receipt)

    def test_all_semantic_mutants_are_rejected(self) -> None:
        with closed_fixture():
            delta, receipt = post_root.build(PASS_RECONSTRUCTION)
            rejected = post_root.mutant_rejections(delta, receipt)

        self.assertEqual(
            rejected,
            (
                "omitted_row",
                "reordered_rows",
                "substituted_successor",
                "approval_promotion",
                "extra_field",
            ),
        )

    def test_reconstruction_statuses_must_all_pass(self) -> None:
        with closed_fixture():
            statuses = dict(PASS_RECONSTRUCTION)
            statuses["rust_reconstruction_status"] = "failed"
            with self.assertRaisesRegex(post_root.ContractError, "reconstruction"):
                post_root.build(statuses)

    def test_resource_obligation_key_set_is_exact(self) -> None:
        with closed_fixture():
            resource = post_root.read_json(post_root.RESOURCE_RELEASE)
            delta = post_root.read_json(post_root.EXPECTED_DELTA)
            delta["downstream_obligations"][0]["logical_key"] = "candidate-unknown"
            resource["downstream_delta_obligations"] = delta["downstream_obligations"]
            resource["expected_delta"] = delta
            write_json(post_root.EXPECTED_DELTA, delta)
            write_json(post_root.RESOURCE_RELEASE, resource)
            with self.assertRaisesRegex(post_root.ContractError, "obligation order or set"):
                post_root.build(PASS_RECONSTRUCTION)

    def test_check_mode_rejects_drift_without_rewriting_it(self) -> None:
        with closed_fixture() as fixture, mock.patch.object(
            post_root, "run_reconstruction_checks", return_value=PASS_RECONSTRUCTION
        ):
            built = post_root.execute(check=False, mutants=False)
            self.assertEqual(built["status"], "built")
            checked = post_root.execute(check=True, mutants=True)
            self.assertEqual(checked["status"], "checked")
            self.assertEqual(checked["semantic_mutants"], 5)

            path = fixture["proof_dir"] / post_root.POST_ROOT_DELTA_NAME
            stale = json.loads(path.read_text(encoding="utf-8"))
            stale["rows"].reverse()
            write_json(path, stale)
            stale_bytes = path.read_bytes()
            with self.assertRaisesRegex(post_root.ContractError, "artifact drift"):
                post_root.execute(check=True, mutants=False)
            self.assertEqual(path.read_bytes(), stale_bytes)

    def test_approval_value_promotion_is_refused(self) -> None:
        with closed_fixture():
            delta, receipt = post_root.build(PASS_RECONSTRUCTION)
            receipt = copy.deepcopy(receipt)
            receipt["promoted_token"] = post_root.read_json(post_root.INPUT_BINDINGS)[
                "external_approval"
            ]["token"]
            with self.assertRaisesRegex(post_root.ContractError, "approval"):
                post_root.validate(delta, receipt)

    def test_compatibility_binding_must_match_one_expected_delta_row(self) -> None:
        with closed_fixture():
            resource = post_root.read_json(post_root.RESOURCE_RELEASE)
            resource["resolved_successor_bindings"][0]["successor_identity"] = identifier(9999)
            write_json(post_root.RESOURCE_RELEASE, resource)
            with self.assertRaisesRegex(post_root.ContractError, "exactly one through-Release"):
                post_root.build(PASS_RECONSTRUCTION)

    def test_schema_specific_inactive_fields_are_required(self) -> None:
        with closed_fixture():
            resource = post_root.read_json(post_root.RESOURCE_RELEASE)
            resource["runtime"] = "inactive"
            write_json(post_root.RESOURCE_RELEASE, resource)
            with self.assertRaisesRegex(post_root.ContractError, "synthetic runtime"):
                post_root.build(PASS_RECONSTRUCTION)

    def test_resource_obligation_extra_field_is_refused(self) -> None:
        with closed_fixture():
            resource = post_root.read_json(post_root.RESOURCE_RELEASE)
            delta = post_root.read_json(post_root.EXPECTED_DELTA)
            delta["downstream_obligations"][0]["unexpected"] = "smuggled"
            resource["downstream_delta_obligations"] = delta["downstream_obligations"]
            resource["expected_delta"] = delta
            write_json(post_root.EXPECTED_DELTA, delta)
            write_json(post_root.RESOURCE_RELEASE, resource)
            with self.assertRaisesRegex(post_root.ContractError, "missing or extra fields"):
                post_root.build(PASS_RECONSTRUCTION)

    def test_candidate_identity_cannot_feed_back_into_resource(self) -> None:
        with closed_fixture() as fixture:
            resource = post_root.read_json(post_root.RESOURCE_RELEASE)
            resource["candidate_root_backreference"] = fixture["root_id"]
            write_json(post_root.RESOURCE_RELEASE, resource)
            with self.assertRaisesRegex(post_root.ContractError, "fed backward"):
                post_root.build(PASS_RECONSTRUCTION)


if __name__ == "__main__":
    unittest.main()
