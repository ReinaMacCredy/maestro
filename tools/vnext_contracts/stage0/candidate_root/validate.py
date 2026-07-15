#!/usr/bin/env python3
"""Direct semantic validation for the Stage-0 candidate-root closure."""

from __future__ import annotations

import argparse
import copy
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import build


OUTPUT = build.OUTPUT
ROOT_FILE = "candidate-contract-root.v1.json"
FINALIZATION_FILE = "design-finalization-manifest.v1.json"
HANDOFF_FILE = "canonical-build-handoff.v1.json"
DESIGN_FILE = "design-revision.v1.json"
BINDINGS_FILE = "decision-root-bindings.v1.json"
SCHEMAS_FILE = "candidate-root-schema-descriptors.v1.json"
STAGE_PROOF_COMPONENT_KIND = next(
    kind
    for kind, fields in build.FACET_FIELDS.items()
    if any(field_name == "stage0_proof_manifest_id" for field_name, _, _ in fields)
)
STAGE_PROOF_INPUT_KIND = next(
    kind
    for kind, name in build.FINALIZATION_SCHEMA_NAMES.items()
    if name == "StageProofMatrixFinalizationInputV1"
)


def load(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as file:
        return json.load(file)


def verify_artifact(path: Path, domain: str, schema: str) -> dict[str, Any]:
    document = load(path)
    if document.get("schema") != schema:
        raise ValueError(f"{path.name} schema drifted")
    value = build.value_from_json(document["canonical_value"])
    encoded = build.cbor(value)
    if encoded.hex() != document["canonical_cbor_hex"]:
        raise ValueError(f"{path.name} canonical bytes drifted")
    if build.hashlib.sha256(encoded).hexdigest() != document["canonical_cbor_sha256"]:
        raise ValueError(f"{path.name} canonical hash drifted")
    if build.rendered(build.digest(domain, value)) != document["identity"]:
        raise ValueError(f"{path.name} identity drifted")
    if document.get("candidate_only") is not True or document.get("runtime") != "inactive":
        raise ValueError(f"{path.name} is not inactive candidate-only material")
    return document


def validate(output: Path = OUTPUT) -> None:
    documents = {
        SCHEMAS_FILE: verify_artifact(
            output / SCHEMAS_FILE,
            build.CANDIDATE_SCHEMA_CLOSURE_DOMAIN,
            "maestro.vnext.candidate-root-schema-closure.v1",
        ),
        DESIGN_FILE: verify_artifact(
            output / DESIGN_FILE,
            build.DESIGN_REVISION_DOMAIN,
            "maestro.vnext.design-revision.v1",
        ),
        ROOT_FILE: verify_artifact(
            output / ROOT_FILE,
            build.ROOT_DOMAIN,
            "maestro.vnext.candidate-contract-root.v1",
        ),
        FINALIZATION_FILE: verify_artifact(
            output / FINALIZATION_FILE,
            build.FINALIZATION_DOMAIN,
            "maestro.vnext.design-finalization-manifest.v1",
        ),
        HANDOFF_FILE: verify_artifact(
            output / HANDOFF_FILE,
            build.HANDOFF_DOMAIN,
            "maestro.vnext.canonical-build-handoff.v1",
        ),
        BINDINGS_FILE: load(output / BINDINGS_FILE),
    }
    root = documents[ROOT_FILE]
    components = root["components"]
    binding = documents[BINDINGS_FILE]
    if binding.get("schema") != "maestro.vnext.exact-decision-root-bindings.v1":
        raise ValueError("decision root binding schema drifted")
    expected_binding_count = len(build.load(build.DECISION).get("materializations", []))
    if len(binding.get("bindings", [])) != expected_binding_count:
        raise ValueError("decision root binding envelope does not match the exact Decision closure")
    expected_component_count = expected_binding_count + len(build.COMPONENT_KINDS) - 1
    if root["component_count"] != expected_component_count or len(components) != expected_component_count:
        raise ValueError("candidate root does not match its exact derived component set")
    kind_counts: dict[int, int] = {}
    for component in components:
        kind_counts[component["kind_tag"]] = kind_counts.get(component["kind_tag"], 0) + 1
    if set(kind_counts) != set(build.COMPONENT_KINDS) or kind_counts[build.NORMATIVE_INPUTS_KIND] != expected_binding_count:
        raise ValueError("candidate root does not cover every component kind with its exact Decision slots")
    if any(kind_counts[kind] != 1 for kind in build.COMPONENT_KINDS if kind != build.NORMATIVE_INPUTS_KIND):
        raise ValueError("candidate root aggregate component cardinality drifted")
    descriptor_rows = documents[SCHEMAS_FILE]["descriptors"]
    expected_descriptor_names = {"NormativeInputsDecisionMaterializationV1"}
    expected_descriptor_names.update(
        f"CandidateRootFacet{kind}V1"
        for kind in build.COMPONENT_KINDS
        if kind != build.NORMATIVE_INPUTS_KIND
    )
    expected_descriptor_names.update(build.FINALIZATION_SCHEMA_NAMES.values())
    if {row["schema_name"] for row in descriptor_rows} != expected_descriptor_names:
        raise ValueError("candidate root schema descriptor set is missing or contains an unknown descriptor")
    descriptor_ids = set()
    for descriptor in descriptor_rows:
        actual = build.rendered(
            build.digest(build.SCHEMA_DOMAIN, build.value_from_json(descriptor["canonical_value"]))
        )
        if descriptor["schema_id"] != actual:
            raise ValueError("candidate root schema descriptor identity drifted")
        descriptor_ids.add(actual)
    if len(descriptor_ids) != len(expected_descriptor_names):
        raise ValueError("candidate root schema descriptor identities are not distinct")
    for component in components:
        if component["schema_id"] not in descriptor_ids:
            raise ValueError("candidate component references a schema outside its closure")
        if component["kind_tag"] != build.NORMATIVE_INPUTS_KIND:
            expected = {field_name for field_name, _, _ in build.FACET_FIELDS[component["kind_tag"]]}
            if set(component["owned_commitments"]) != expected:
                raise ValueError("facet contains commitments outside its ownership boundary")
    if binding.get("candidate_only") is not True or binding.get("runtime") != "inactive":
        raise ValueError("decision root binding envelope is incomplete")
    if binding["decision_closure_id"] != documents[FINALIZATION_FILE]["decision_closure_id"]:
        raise ValueError("decision root binding closure drifted")
    for item in binding["bindings"]:
        if item["materialization_base"] != {"kind": "initial_external_design_closure", "decision_closure_id": binding["decision_closure_id"]}:
            raise ValueError("decision root binding has a fabricated prior root")
        if item["after_root_id"] != root["identity"] or item["finalization_manifest_id"] != documents[FINALIZATION_FILE]["identity"]:
            raise ValueError("decision root binding does not bind the final root and manifest")
    finalization = documents[FINALIZATION_FILE]
    if len(finalization["pinned_inputs"]) != len(build.FINALIZATION_KINDS) or [item["kind_tag"] for item in finalization["pinned_inputs"]] != list(build.FINALIZATION_KINDS):
        raise ValueError("finalization manifest does not pin the exact input-kind set")
    if finalization["candidate_contract_root_id"] != root["identity"]:
        raise ValueError("finalization root mismatch")
    aggregate_ids = {
        component["kind_tag"]: component["component_id"]
        for component in components
        if component["kind_tag"] != build.NORMATIVE_INPUTS_KIND
    }
    binding_signatures = set()
    for input_row in finalization["pinned_inputs"]:
        expected_facet_ids = [aggregate_ids[facet_kind] for facet_kind in build.FINALIZATION_FACETS[input_row["kind_tag"]]]
        if input_row.get("owner_facet_component_ids") != expected_facet_ids:
            raise ValueError("finalization input does not pin its exact owned root facets")
        canonical = build.value_from_json(input_row["canonical_value"])
        canonical_facet_ids = [build.rendered(value.value) for value in canonical[5]]
        if canonical_facet_ids != expected_facet_ids:
            raise ValueError("finalization input canonical value does not bind its owned root facets")
        signature = tuple(expected_facet_ids)
        if signature in binding_signatures:
            raise ValueError("finalization inputs cannot relabel an identical owner-facet binding")
        binding_signatures.add(signature)
    handoff = documents[HANDOFF_FILE]
    if handoff["candidate_contract_root_id"] != root["identity"] or handoff["finalization_manifest_id"] != finalization["identity"]:
        raise ValueError("canonical handoff pin mismatch")
    stage_proof_component = next(
        component
        for component in components
        if component["kind_tag"] == STAGE_PROOF_COMPONENT_KIND
    )
    expected_proof_binding = {
        "identity": stage_proof_component["owned_commitments"]["stage0_proof_manifest_id"],
        "artifact_sha256": stage_proof_component["owned_commitments"]["stage0_proof_manifest_artifact_sha256"],
        "gate_count": stage_proof_component["owned_commitments"]["stage0_proof_gate_count"],
    }
    if finalization.get("stage0_proof_manifest") != expected_proof_binding:
        raise ValueError("finalization does not bind the exact Stage0ProofManifest")
    if handoff.get("stage0_proof_manifest") != expected_proof_binding:
        raise ValueError("handoff does not bind the exact Stage0ProofManifest")
    proof_input = next(
        item
        for item in finalization["pinned_inputs"]
        if item["kind_tag"] == STAGE_PROOF_INPUT_KIND
    )
    proof_canonical = build.value_from_json(proof_input["canonical_value"])
    if [build.rendered(item.value) for item in proof_canonical[6:8]] != [
        expected_proof_binding["identity"],
        "sha256:" + expected_proof_binding["artifact_sha256"],
    ] or proof_canonical[8] != expected_proof_binding["gate_count"]:
        raise ValueError("StageProofMatrix finalization input does not bind the proof manifest")
    proof_document = build.load(build.PROOF_MANIFEST)
    if (
        proof_document["identity"] != expected_proof_binding["identity"]
        or build.artifact_hash(build.PROOF_MANIFEST) != expected_proof_binding["artifact_sha256"]
        or proof_document["gate_count"] != expected_proof_binding["gate_count"]
    ):
        raise ValueError("Stage0ProofManifest artifact binding drifted")
    publication = next(component for component in components if component["kind_tag"] == 10)
    if set(publication["owned_commitments"]) != {"decision_closure_id"}:
        raise ValueError("publication authority contains non-publication commitments")
    schema_closure = next(component for component in components if component["kind_tag"] == 17)
    if schema_closure["owned_commitments"].get("submission_claim_set_schema_id") != documents[DESIGN_FILE]["submission_claim_set"]["schema_id"] or schema_closure["owned_commitments"].get("submission_claim_set_artifact_sha256") != documents[DESIGN_FILE]["submission_claim_set"]["artifact_sha256"]:
        raise ValueError("submission-claim set is not bound through LiteralSchemaClosure")
    bindings = build.load(build.INPUT_BINDINGS)
    forbidden = build.forbidden_promotion_values(
        bindings["external_approval"], bindings["external_approval_event"]
    )
    bindings_hash = build.artifact_hash(build.INPUT_BINDINGS)
    forbidden.update((bindings_hash, f"sha256:{bindings_hash}"))
    for document in documents.values():
        build.scan_forbidden(document, forbidden)


def write_documents(path: Path, documents: dict[str, dict[str, Any]]) -> None:
    path.mkdir(parents=True, exist_ok=True)
    for name, document in documents.items():
        (path / name).write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")


def mutant_rejections(output: Path = OUTPUT) -> int:
    documents = {name: load(output / name) for name in (SCHEMAS_FILE, DESIGN_FILE, ROOT_FILE, FINALIZATION_FILE, HANDOFF_FILE, BINDINGS_FILE)}
    input_bindings = build.load(build.INPUT_BINDINGS)
    approval_hash = build.hashlib.sha256(
        json.dumps(
            input_bindings["external_approval"],
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
        ).encode("ascii")
    ).hexdigest()
    input_bindings_hash = build.artifact_hash(build.INPUT_BINDINGS)

    def root_proof_binding_drift(docs: dict[str, dict[str, Any]]) -> None:
        component = next(
            row
            for row in docs[ROOT_FILE]["components"]
            if row["kind_tag"] == STAGE_PROOF_COMPONENT_KIND
        )
        component["owned_commitments"]["stage0_proof_gate_count"] += 1

    def finalization_proof_binding_drift(docs: dict[str, dict[str, Any]]) -> None:
        docs[FINALIZATION_FILE]["stage0_proof_manifest"]["identity"] = (
            "sha256:" + "00" * 32
        )

    def handoff_proof_binding_drift(docs: dict[str, dict[str, Any]]) -> None:
        docs[HANDOFF_FILE]["stage0_proof_manifest"]["artifact_sha256"] = "00" * 32

    def proof_input_binding_drift(docs: dict[str, dict[str, Any]]) -> None:
        row = next(
            item
            for item in docs[FINALIZATION_FILE]["pinned_inputs"]
            if item["kind_tag"] == STAGE_PROOF_INPUT_KIND
        )
        row["canonical_value"][8] += 1

    mutations = {
        "missing_normative_input": lambda docs: docs[ROOT_FILE]["components"].pop(),
        "fabricated_prior_root": lambda docs: docs[BINDINGS_FILE]["bindings"][0]["materialization_base"].update({"kind": "prior_contract_root", "root_id": "sha256:" + "00" * 32}),
        "missing_finalization_input": lambda docs: docs[FINALIZATION_FILE]["pinned_inputs"].pop(),
        "handoff_root_mismatch": lambda docs: docs[HANDOFF_FILE].update({"candidate_contract_root_id": "sha256:" + "00" * 32}),
        "approval_promotion": lambda docs: docs[DESIGN_FILE].update({"external_approval": build.load(build.INPUT_BINDINGS)["external_approval"]}),
        "transitive_approval_hash_promotion": lambda docs: docs[DESIGN_FILE].update(
            {"promotion_sha256": approval_hash}
        ),
        "approval_hash_key_promotion": lambda docs: docs[DESIGN_FILE].update(
            {approval_hash: "forbidden_key"}
        ),
        "input_bindings_hash_key_promotion": lambda docs: docs[DESIGN_FILE].update(
            {"nested_promotion": {input_bindings_hash: "forbidden_nested_key"}}
        ),
        "root_proof_binding_drift": root_proof_binding_drift,
        "finalization_proof_binding_drift": finalization_proof_binding_drift,
        "handoff_proof_binding_drift": handoff_proof_binding_drift,
        "proof_input_binding_drift": proof_input_binding_drift,
    }
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        for name, mutate in mutations.items():
            mutant = copy.deepcopy(documents)
            mutate(mutant)
            target = root / name
            write_documents(target, mutant)
            try:
                validate(target)
            except (KeyError, ValueError):
                continue
            raise AssertionError(f"semantic mutant accepted: {name}")
    return len(mutations)


def ruby_equality(output: Path = OUTPUT) -> None:
    process = subprocess.run(
        ["ruby", str(Path(__file__).with_name("encode.rb")), str(output)],
        check=True,
        capture_output=True,
        text=True,
    )
    artifacts = json.loads(process.stdout)["artifacts"]
    for name, cbor_hex in artifacts.items():
        if load(output / name)["canonical_cbor_hex"] != cbor_hex:
            raise ValueError(f"independent Ruby CBOR mismatch: {name}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--mutants", action="store_true")
    arguments = parser.parse_args()
    validate()
    ruby_equality()
    semantic_mutants = mutant_rejections() if arguments.mutants else 0
    print(json.dumps({"status": "validated", "ruby_encoder": "pass", "semantic_mutants": semantic_mutants}))
