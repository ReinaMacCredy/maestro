#!/usr/bin/env python3
"""Close Stage 0 downstream obligations after candidate-root materialization.

The two outputs are deliberately NONCANONICAL receipts. They have no identity,
canonical-value, or CBOR fields, so their hashes cannot participate in the
pre-root proof or feed back into Resource, Bundle, Release, root, finalization,
or Handoff identity.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


WORKSPACE = Path(__file__).resolve().parents[4]
OUTPUT = WORKSPACE / "contracts/vnext/stage0/proof-matrix"
PROOF_MANIFEST = OUTPUT / "stage0-proof-manifest.v1.json"
CANDIDATE_ROOT = (
    WORKSPACE / "contracts/vnext/stage0/candidate-root/candidate-contract-root.v1.json"
)
FINALIZATION = (
    WORKSPACE / "contracts/vnext/stage0/candidate-root/design-finalization-manifest.v1.json"
)
HANDOFF = WORKSPACE / "contracts/vnext/stage0/candidate-root/canonical-build-handoff.v1.json"
DECISION_BINDINGS = (
    WORKSPACE / "contracts/vnext/stage0/candidate-root/decision-root-bindings.v1.json"
)
DECISION_CLOSURE = (
    WORKSPACE / "contracts/vnext/stage0/decision-closure/decision-closure.v1.json"
)
RESOURCE_RELEASE = (
    WORKSPACE / "contracts/vnext/stage0/resource-release/resource-release.v1.json"
)
RESOURCE_RELEASE_LOGICAL = "contracts/vnext/stage0/resource-release/resource-release.v1.json"
RESOURCE_DESCRIPTOR_LOGICAL = (
    "contracts/vnext/stage0/resource-release/resource-descriptors.v1.json"
)
RESOURCE_BUNDLE_LOGICALS = tuple(
    f"contracts/vnext/stage0/resource-release/bundle-{tag:03d}-{name}.v1.json"
    for tag, name in (
        (1, "migration"),
        (2, "external-pattern-neutral"),
        (3, "external-pattern-vendor"),
        (4, "shared-contract"),
        (5, "orchestration"),
        (6, "capability"),
        (7, "adapter"),
        (8, "agent-bootstrap"),
    )
)
RELEASE_CENSUS_LOGICAL = (
    "contracts/vnext/stage0/resource-release/release-resource-census.v1.json"
)
EMBEDDED_RELEASE_LOGICAL = (
    "contracts/vnext/stage0/resource-release/embedded-release-bundle.v1.json"
)
EMBEDDED_RELEASE = WORKSPACE / EMBEDDED_RELEASE_LOGICAL
EXPECTED_DELTA = (
    WORKSPACE / "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.json"
)
EXPECTED_DELTA_LOGICAL = (
    "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.json"
)
INPUT_BINDINGS = WORKSPACE / "contracts/vnext/stage0/input-bindings.json"
CANDIDATE_VALIDATOR = WORKSPACE / "tools/vnext_contracts/stage0/candidate_root/validate.py"

POST_ROOT_DELTA_NAME = "post-root-downstream-delta.v1.json"
POST_ROOT_RECEIPT_NAME = "post-root-validation-receipt.v1.json"

REQUIRED_PROOF_GATE_NAMES = (
    "external_input_authorization",
    "decision_closure",
    "catalog_predecessor",
    "incorporated_catalog_checkpoints",
    "catalog_successor",
    "public_contracts",
    "public_identity",
    "submission_claim",
    "dispatch",
    "effect_home",
    "resource_release",
    "current_surface_consumer_census",
    "persistence_archive_golden_fixtures",
    "migration_rollback",
    "root_assembly_source_binding",
)
REQUIRED_POST_ROOT_KEYS = (
    ("RootInput", "candidate-root"),
    ("RootInput", "candidate-finalization"),
    ("HandoffInput", "candidate-handoff"),
)
OBLIGATION_FIELDS = {
    "identity_kind",
    "logical_key",
    "predecessor_identity",
    "successor_identity",
    "disposition",
    "depends_on_release_identity",
    "status",
    "owner",
}
RESOURCE_SUCCESSOR_SLOTS = (
    "public_transport_7138",
    "grammar_catalog_d116",
    "effect_control_h2",
    "local_withdrawal_h3",
    "catalog_owner_efa0",
    "resource_bundle_c868",
    "release_binding",
    "writer_compatibility",
)
THROUGH_RELEASE_IDENTITY_COUNTS = {
    "Schema": 117,
    "Manifest": 26,
    "Resource": 412,
    "Bundle": 8,
    "Census": 1,
    "Release": 1,
}
COMPATIBILITY_ROW_FIELDS = {
    "slot_name",
    "identity_kind",
    "logical_key",
    "predecessor_identity",
    "successor_identity",
    "disposition",
}
RECONSTRUCTION_FIELDS = (
    "python_reconstruction_status",
    "ruby_reconstruction_status",
    "rust_reconstruction_status",
)
APPROVAL_KEYS = {
    "external_approval",
    "external_approval_event",
    "approval_token",
    "build_approval_token",
    "external_build_approval_packet",
    "external_candidate_input_commitment",
    "external_build_plan_handoff",
}


class ContractError(ValueError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def read_json(path: Path) -> dict[str, Any]:
    try:
        with path.open(encoding="utf-8") as file:
            document = json.load(file)
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"cannot read required artifact {path}: {error}") from error
    require(isinstance(document, dict), f"required artifact is not a JSON object: {path}")
    return document


def json_bytes(document: dict[str, Any]) -> bytes:
    return (
        json.dumps(document, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n"
    ).encode("ascii")


def file_sha256(path: Path) -> str:
    try:
        return hashlib.sha256(path.read_bytes()).hexdigest()
    except OSError as error:
        raise ContractError(f"cannot hash required artifact {path}: {error}") from error


def exact_id(value: Any, label: str) -> str:
    require(isinstance(value, str) and value.startswith("sha256:"), f"{label} is not a sha256 identity")
    digest = value.removeprefix("sha256:")
    require(len(digest) == 64, f"{label} is not a 32-byte sha256 identity")
    try:
        bytes.fromhex(digest)
    except ValueError as error:
        raise ContractError(f"{label} is not lowercase hexadecimal") from error
    require(digest == digest.lower(), f"{label} is not lowercase hexadecimal")
    return value


def exact_digest(value: Any, label: str) -> str:
    require(isinstance(value, str), f"{label} is not a sha256 digest")
    digest = value.removeprefix("sha256:")
    require(len(digest) == 64, f"{label} is not a 32-byte sha256 digest")
    try:
        bytes.fromhex(digest)
    except ValueError as error:
        raise ContractError(f"{label} is not lowercase hexadecimal") from error
    require(digest == digest.lower(), f"{label} is not lowercase hexadecimal")
    return digest


def validate_stage0_commitment(
    document: dict[str, Any], path: Path, schema: str, label: str
) -> str:
    require(document.get("schema") == schema, f"{label} schema drifted")
    require(
        document.get("identity_protocol") == "Stage0CanonicalCommitmentV1"
        and document.get("identity_scope") == "canonical_commitment_envelope_only",
        f"{label} commitment protocol drifted",
    )
    envelope = document.get("canonical_commitment_envelope")
    require(
        isinstance(envelope, list)
        and len(envelope) == 2
        and envelope[0] == schema
        and envelope[1] == document.get("canonical_value"),
        f"{label} canonical commitment envelope drifted",
    )
    cbor_path = path.with_suffix(".cbor")
    require(cbor_path.is_file(), f"{label} canonical CBOR artifact is missing")
    raw = cbor_path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    require(
        document.get("canonical_cbor_hex") == raw.hex()
        and document.get("canonical_cbor_sha256") == digest
        and document.get("canonical_cbor_byte_length") == len(raw),
        f"{label} canonical CBOR receipt drifted",
    )
    identity = exact_id(document.get("identity"), f"{label} identity")
    require(identity == f"sha256:{digest}", f"{label} identity does not equal its commitment bytes")
    return identity


def validate_embedded_release(document: dict[str, Any]) -> str:
    require(
        document.get("schema") == "maestro.vnext.embedded-release-bundle.manifest.v1",
        "embedded Release schema drifted",
    )
    require(
        document.get("identity_protocol") == "ManifestIdentityV1",
        "embedded Release identity protocol drifted",
    )
    envelope = document.get("manifest_identity_envelope")
    require(
        isinstance(envelope, list)
        and len(envelope) == 5
        and document.get("canonical_value") == envelope[3:5]
        and document.get("sole_release_root") is True,
        "embedded Release five-slot ManifestIdentityV1 envelope drifted",
    )
    cbor_path = EMBEDDED_RELEASE.with_suffix(".cbor")
    require(cbor_path.is_file(), "embedded Release canonical CBOR artifact is missing")
    raw = cbor_path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    require(
        document.get("release_id") == digest
        and document.get("identity") == f"sha256:{digest}"
        and document.get("canonical_cbor_hex") == raw.hex()
        and document.get("canonical_cbor_sha256") == digest
        and document.get("canonical_cbor_byte_length") == len(raw),
        "embedded Release identity or canonical receipt drifted",
    )
    require(
        document.get("candidate_only") is True
        and document.get("runtime_activation") is False
        and "state" not in document
        and "runtime" not in document,
        "embedded Release acquired synthetic runtime state",
    )
    return f"sha256:{digest}"


def load_inputs() -> dict[str, dict[str, Any]]:
    return {
        "proof": read_json(PROOF_MANIFEST),
        "root": read_json(CANDIDATE_ROOT),
        "finalization": read_json(FINALIZATION),
        "handoff": read_json(HANDOFF),
        "bindings": read_json(DECISION_BINDINGS),
        "decision": read_json(DECISION_CLOSURE),
        "resource": read_json(RESOURCE_RELEASE),
        "expected_delta": read_json(EXPECTED_DELTA),
        "input_bindings": read_json(INPUT_BINDINGS),
    }


def require_exact_fields(
    document: dict[str, Any], expected: dict[str, Any], label: str
) -> None:
    for field, value in expected.items():
        require(field in document, f"{label} omits explicit {field}")
        require(document[field] == value, f"{label} {field} drifted")


def validate_proof_manifest(document: dict[str, Any]) -> tuple[str, int]:
    require(
        document.get("schema") == "maestro.vnext.stage0-proof-manifest.v1",
        "pre-root proof manifest schema drifted",
    )
    require_exact_fields(
        document,
        {"candidate_only": True, "runtime_activation": False},
        "pre-root proof manifest",
    )
    identity = exact_id(document.get("identity"), "pre-root proof manifest identity")
    gates = document.get("gates")
    require(isinstance(gates, list), "pre-root proof manifest gates are missing")
    require(
        document.get("gate_count") == len(REQUIRED_PROOF_GATE_NAMES) == len(gates),
        "pre-root proof gate count drifted",
    )
    require(
        [row.get("name") for row in gates if isinstance(row, dict)]
        == list(REQUIRED_PROOF_GATE_NAMES),
        "pre-root proof gate order or set drifted",
    )
    for tag, row in enumerate(gates, start=1):
        require(isinstance(row, dict), "pre-root proof gate is not an object")
        require(row.get("tag") == tag, "pre-root proof gate tags are not contiguous")
        require(row.get("result") == "passed", "pre-root proof gate did not pass")
        required_class = (
            "verified_non_promoting"
            if row["name"] == "external_input_authorization"
            else "verified"
        )
        require(
            row.get("result_class") == required_class,
            "pre-root proof gate result class drifted",
        )
    return identity, len(gates)


def validate_candidate_closure(
    documents: dict[str, dict[str, Any]], proof_id: str, proof_sha: str, proof_gate_count: int
) -> tuple[str, str, str, int]:
    root = documents["root"]
    finalization = documents["finalization"]
    handoff = documents["handoff"]
    bindings = documents["bindings"]
    decision = documents["decision"]
    schemas = {
        "root": "maestro.vnext.candidate-contract-root.v1",
        "finalization": "maestro.vnext.design-finalization-manifest.v1",
        "handoff": "maestro.vnext.canonical-build-handoff.v1",
        "bindings": "maestro.vnext.exact-decision-root-bindings.v1",
    }
    for name, schema in schemas.items():
        require(documents[name].get("schema") == schema, f"candidate {name} schema drifted")
        require_exact_fields(
            documents[name],
            {"candidate_only": True, "runtime": "inactive"},
            f"candidate {name}",
        )

    root_id = exact_id(root.get("identity"), "candidate root identity")
    finalization_id = exact_id(finalization.get("identity"), "finalization identity")
    handoff_id = exact_id(handoff.get("identity"), "Handoff identity")
    require(
        decision.get("schema") == "maestro.vnext.decision-closure.v1",
        "Decision closure schema drifted",
    )
    decision_id = exact_id(decision.get("identity"), "Decision closure identity")
    materializations = decision.get("materializations")
    require(
        isinstance(materializations, list) and materializations,
        "Decision closure materializations are missing",
    )
    materialization_ids = [
        exact_digest(row.get("id"), "Decision materialization identity")
        for row in materializations
        if isinstance(row, dict)
    ]
    require(
        len(materialization_ids) == len(materializations)
        and len(set(materialization_ids)) == len(materialization_ids),
        "Decision closure materialization identities are malformed or duplicate",
    )
    ordered_materialization_ids = sorted(materialization_ids)
    materialization_by_id = {
        exact_digest(row["id"], "Decision materialization identity"): row
        for row in materializations
    }
    components = root.get("components")
    require(isinstance(components, list) and components, "candidate root components are missing")
    require(root.get("component_count") == len(components), "candidate root component count drifted")
    component_ids = [
        exact_id(row.get("component_id"), "candidate component identity")
        for row in components
        if isinstance(row, dict)
    ]
    require(
        len(component_ids) == len(components) and len(set(component_ids)) == len(component_ids),
        "candidate root component identities are malformed or duplicate",
    )
    require(
        finalization.get("candidate_contract_root_id") == root_id,
        "finalization does not bind the candidate root",
    )
    require(
        isinstance(finalization.get("pinned_inputs"), list)
        and finalization.get("pinned_inputs"),
        "finalization inputs are missing",
    )
    require(
        handoff.get("candidate_contract_root_id") == root_id
        and handoff.get("finalization_manifest_id") == finalization_id,
        "Handoff does not bind the candidate root and finalization",
    )
    require(
        handoff.get("component_count") == len(components)
        and handoff.get("pinned_input_count") == len(finalization["pinned_inputs"]),
        "Handoff closure counts drifted",
    )
    expected_proof_binding = {
        "identity": proof_id,
        "artifact_sha256": proof_sha,
        "gate_count": proof_gate_count,
    }
    require(
        finalization.get("stage0_proof_manifest") == expected_proof_binding,
        "finalization pre-root proof binding drifted",
    )
    require(
        handoff.get("stage0_proof_manifest") == expected_proof_binding,
        "Handoff pre-root proof binding drifted",
    )

    rows = bindings.get("bindings")
    require(isinstance(rows, list), "Decision-root bindings are missing")
    require(
        len(rows) == len(materializations),
        "Decision-root binding count differs from the frozen Decision closure",
    )
    require(
        bindings.get("decision_closure_id") == decision_id
        and finalization.get("decision_closure_id") == decision_id,
        "Decision-root binding closure differs from finalization or the frozen Decision closure",
    )
    bound_materialization_ids = [
        exact_digest(row.get("materialization_id"), "Decision materialization identity")
        for row in rows
        if isinstance(row, dict)
    ]
    require(
        bound_materialization_ids == ordered_materialization_ids,
        "Decision-root bindings do not preserve the deterministic Decision materialization order and set",
    )
    bound_component_ids = [row.get("component_id") for row in rows if isinstance(row, dict)]
    require(len(set(bound_component_ids)) == len(rows), "Decision-root component bindings are duplicate")
    for row in rows:
        require(isinstance(row, dict), "Decision-root binding row is not an object")
        materialization_id = exact_digest(
            row.get("materialization_id"), "Decision materialization identity"
        )
        exact_id(row.get("component_id"), "Decision materialization component identity")
        require(
            row.get("materialization_base")
            == materialization_by_id[materialization_id].get("materialization_base"),
            "Decision-root binding materialization base differs from the frozen Decision closure",
        )
        require(
            row.get("after_root_id") == root_id
            and row.get("finalization_manifest_id") == finalization_id,
            "Decision-root binding does not bind the final root and finalization",
        )
    normative_component_ids = {
        row["component_id"]
        for row in components
        if row.get("kind_tag") == 12
    }
    require(
        set(bound_component_ids) == normative_component_ids,
        "Decision-root bindings do not equal the root NormativeInputs component set",
    )
    return root_id, finalization_id, handoff_id, len(rows)


def validate_resource_closure(
    resource: dict[str, Any], expected_delta: dict[str, Any]
) -> tuple[
    str,
    str,
    str,
    list[dict[str, Any]],
    list[dict[str, Any]],
    dict[str, int],
]:
    require_exact_fields(
        resource,
        {
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "source_publication": False,
            "installation": False,
        },
        "Resource release",
    )
    require(
        "runtime" not in resource and "release_state" not in resource,
        "Resource release acquired synthetic runtime or lifecycle state",
    )
    resource_id = validate_stage0_commitment(
        resource,
        RESOURCE_RELEASE,
        "maestro.vnext.stage0.resource-release.v1",
        "Resource release wrapper",
    )
    require_exact_fields(
        expected_delta,
        {"candidate_only": True, "runtime_activation": False},
        "through-Release expected delta",
    )
    delta_id = validate_stage0_commitment(
        expected_delta,
        EXPECTED_DELTA,
        "maestro.vnext.migration-cutover-expected-delta-successor.v1",
        "through-Release expected delta",
    )
    require(
        resource.get("resolved_expected_delta_commitment_id") == delta_id,
        "Resource release expected-delta identity binding drifted",
    )
    require(resource.get("expected_delta") == expected_delta, "Resource release expected-delta bytes drifted")

    release = resource.get("embedded_release_bundle")
    require(isinstance(release, dict), "Resource release embedded Release is missing")
    require(release == read_json(EMBEDDED_RELEASE), "Resource wrapper does not embed the exact Release artifact")
    release_id = validate_embedded_release(release)
    require(resource_id != release_id, "Stage0 wrapper commitment and ReleaseId were conflated")

    entries = expected_delta.get("entries")
    require(
        isinstance(entries, list)
        and len(entries) == sum(THROUGH_RELEASE_IDENTITY_COUNTS.values())
        and expected_delta.get("exact_identity_kind_counts")
        == THROUGH_RELEASE_IDENTITY_COUNTS,
        "through-Release expected-delta identity census drifted",
    )
    require(
        expected_delta.get("resolved_entry_count") == len(entries),
        "through-Release expected-delta entry count drifted",
    )
    entry_keys: list[tuple[str, str]] = []
    entry_successors: list[str] = []
    for row in entries:
        require(isinstance(row, dict), "through-Release expected-delta row is not an object")
        key = (row.get("identity_kind"), row.get("logical_key"))
        require(all(isinstance(item, str) and item for item in key), "through-Release delta key is malformed")
        entry_keys.append(key)
        successor = exact_id(row.get("successor_identity"), "through-Release successor identity")
        entry_successors.append(successor)
        predecessor = row.get("predecessor_identity")
        if predecessor is not None:
            exact_id(predecessor, "through-Release predecessor identity")
        disposition = row.get("disposition")
        require(disposition in {"Introduce", "Rotate", "Preserve"}, "through-Release disposition is invalid")
        require(
            (disposition != "Introduce" or predecessor is None)
            and (disposition != "Rotate" or (predecessor is not None and predecessor != successor))
            and (disposition != "Preserve" or predecessor == successor),
            "through-Release predecessor/disposition relation is invalid",
        )
    require(len(set(entry_keys)) == len(entry_keys), "through-Release expected delta has duplicate keys")
    require(
        len(set(entry_successors)) == len(entry_successors),
        "through-Release expected delta has duplicate successor identities",
    )
    require(
        {
            kind: sum(row.get("identity_kind") == kind for row in entries)
            for kind in THROUGH_RELEASE_IDENTITY_COUNTS
        }
        == THROUGH_RELEASE_IDENTITY_COUNTS,
        "through-Release expected-delta entry kinds drifted",
    )

    obligations = expected_delta.get("downstream_obligations")
    require(isinstance(obligations, list), "Resource downstream obligations are missing")
    obligation_keys = [
        (row.get("identity_kind"), row.get("logical_key"))
        for row in obligations
        if isinstance(row, dict)
    ]
    require(
        obligation_keys == list(REQUIRED_POST_ROOT_KEYS),
        "Resource downstream obligation order or set drifted",
    )
    require(
        resource.get("downstream_delta_obligations") == obligations,
        "Resource release does not preserve its exact downstream obligations",
    )
    require(
        expected_delta.get("blocked_dependency_count") == len(obligations)
        and expected_delta.get("unresolved_obligation_count") == len(obligations),
        "Resource downstream obligation counts drifted",
    )
    for row in obligations:
        require(
            set(row) == OBLIGATION_FIELDS,
            "Resource downstream obligation contains missing or extra fields",
        )
        require(
            row.get("predecessor_identity") is None
            and row.get("successor_identity") is None
            and row.get("disposition") == "Introduce"
            and row.get("depends_on_release_identity") == release_id
            and row.get("status") == "pending_downstream_stage0_producer"
            and row.get("owner") == "candidate-root-worker",
            "Resource downstream obligation fields drifted",
        )
    require(
        not set(entry_keys) & set(obligation_keys),
        "through-Release entries overlap downstream obligations",
    )

    bindings = resource.get("resolved_successor_bindings")
    require(isinstance(bindings, list), "Resource compatibility successor bindings are missing")
    require(bindings, "Resource compatibility successor binding set is empty")
    require(
        resource.get("declared_successor_slot_count") == len(bindings)
        and resource.get("resolved_successor_slot_count") == len(bindings)
        and resource.get("blocked_successor_slot_count") == 0
        and resource.get("null_successor_identity_count") == 0,
        "Resource compatibility successor closure is incomplete",
    )
    slot_names: list[str] = []
    slot_identities: list[str] = []
    for row in bindings:
        require(
            isinstance(row, dict) and set(row) == {"slot_name", "successor_identity"},
            "Resource compatibility successor binding shape drifted",
        )
        require(isinstance(row["slot_name"], str) and row["slot_name"], "Resource successor slot is empty")
        slot_names.append(row["slot_name"])
        slot_identities.append(exact_id(row["successor_identity"], "Resource successor identity"))
    require(
        slot_names == list(RESOURCE_SUCCESSOR_SLOTS),
        "Resource successor slot order or set drifted",
    )
    require(len(set(slot_identities)) == len(bindings), "Resource successor identities are duplicate")
    require(
        bindings[RESOURCE_SUCCESSOR_SLOTS.index("release_binding")]["successor_identity"]
        == release_id,
        "Resource release-binding slot does not equal the exact ReleaseId",
    )
    entries_by_successor: dict[str, list[dict[str, Any]]] = {}
    for entry in entries:
        entries_by_successor.setdefault(entry["successor_identity"], []).append(entry)
    compatibility_rows = []
    for binding in bindings:
        matches = entries_by_successor.get(binding["successor_identity"], [])
        require(
            len(matches) == 1,
            "Resource compatibility successor does not match exactly one through-Release delta row",
        )
        match = matches[0]
        compatibility_rows.append(
            {
                "slot_name": binding["slot_name"],
                "identity_kind": match["identity_kind"],
                "logical_key": match["logical_key"],
                "predecessor_identity": match["predecessor_identity"],
                "successor_identity": match["successor_identity"],
                "disposition": match["disposition"],
            }
        )
    require(
        len(
            {
                (row["identity_kind"], row["logical_key"])
                for row in compatibility_rows
            }
        )
        == len(compatibility_rows),
        "Resource compatibility bindings collapse onto duplicate expected-delta keys",
    )

    census = resource.get("release_resource_census")
    require(isinstance(census, dict), "Resource exact census is missing")
    require(
        census == read_json(WORKSPACE / RELEASE_CENSUS_LOGICAL),
        "Resource wrapper does not embed the exact ReleaseResourceCensus artifact",
    )
    require(
        census.get("identity_protocol") == "ManifestIdentityV1"
        and isinstance(census.get("manifest_identity_envelope"), list)
        and len(census["manifest_identity_envelope"]) == 5
        and census.get("canonical_value") == census["manifest_identity_envelope"][3:5],
        "ReleaseResourceCensus ManifestIdentityV1 envelope drifted",
    )
    actual_counts = {
        "resource_count": len(resource.get("resources", [])),
        "bundle_count": len(resource.get("bundles", [])),
        "consumer_edge_count": len(census.get("consumer_edges", [])),
        "downstream_obligation_count": len(obligations),
    }
    require(
        actual_counts
        == {
            "resource_count": 412,
            "bundle_count": 8,
            "consumer_edge_count": 411,
            "downstream_obligation_count": 3,
        }
        and resource.get("resource_count") == 412
        and resource.get("bundle_count") == 8
        and len(release.get("bundle_ids", [])) == 8,
        "Resource, Bundle, Census, or downstream-obligation exact counts drifted",
    )
    return (
        resource_id,
        delta_id,
        release_id,
        copy.deepcopy(bindings),
        compatibility_rows,
        actual_counts,
    )


def approval_forbidden_values(input_bindings: dict[str, Any]) -> set[str]:
    require(
        "external_approval" in input_bindings and "external_approval_event" in input_bindings,
        "external approval exclusion source is incomplete",
    )
    forbidden: set[str] = {file_sha256(INPUT_BINDINGS)}

    def visit(value: Any) -> None:
        if isinstance(value, dict):
            encoded = json.dumps(
                value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
            ).encode("ascii")
            forbidden.add(hashlib.sha256(encoded).hexdigest())
            forbidden.add("sha256:" + hashlib.sha256(encoded).hexdigest())
            for child in value.values():
                visit(child)
        elif isinstance(value, list):
            encoded = json.dumps(
                value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
            ).encode("ascii")
            forbidden.add(hashlib.sha256(encoded).hexdigest())
            forbidden.add("sha256:" + hashlib.sha256(encoded).hexdigest())
            for child in value:
                visit(child)
        elif isinstance(value, str):
            forbidden.add(value)
            if value.startswith("sha256:"):
                forbidden.add(value.removeprefix("sha256:"))

    visit(input_bindings["external_approval"])
    visit(input_bindings["external_approval_event"])
    return forbidden


def validate_resource_proof_binding(
    proof: dict[str, Any],
    resource_id: str,
    delta_id: str,
    resource_counts: dict[str, int],
) -> None:
    gate = proof["gates"][REQUIRED_PROOF_GATE_NAMES.index("resource_release")]
    expected_sources = sorted(
        [
            {
                "path": path,
                "sha256": file_sha256(WORKSPACE / path),
            }
            for path in (
                RESOURCE_DESCRIPTOR_LOGICAL,
                *RESOURCE_BUNDLE_LOGICALS,
                RELEASE_CENSUS_LOGICAL,
                EMBEDDED_RELEASE_LOGICAL,
                EXPECTED_DELTA_LOGICAL,
                RESOURCE_RELEASE_LOGICAL,
            )
        ],
        key=lambda row: row["path"],
    )
    require(
        gate.get("source_artifacts") == expected_sources,
        "pre-root Resource proof source hashes differ from post-root inputs",
    )
    require(
        gate.get("assertions")
        == {
            "expected_delta_commitment_id": delta_id,
            "release_id": read_json(EMBEDDED_RELEASE)["release_id"],
            "resource_release_commitment_id": resource_id,
        },
        "pre-root Resource proof identity assertions drifted",
    )
    expected_counts = [
        {"name": name, "value": value}
        for name, value in sorted(resource_counts.items())
    ]
    require(
        gate.get("semantic_counts") == expected_counts,
        "pre-root Resource proof exact counts differ from emitted Resource rows",
    )


def validate_stage0_runtime_boundary(proof: dict[str, Any]) -> None:
    gate = proof["gates"][REQUIRED_PROOF_GATE_NAMES.index("migration_rollback")]
    require(
        gate.get("assertions")
        == {
            "runtime_proof_complete": False,
            "stage": "stage0_candidate_only",
            "status": "requirements_complete_runtime_proof_pending",
            "passed_claim": "requirements_frozen_not_runtime_complete",
            "pending_obligation_stage": "Stage11",
            "proof_status": "pending_stage0_execution_and_rehearsal",
            "stage0_execution_complete": False,
            "stage0_rehearsal_complete": False,
        },
        "Stage0 requirement freeze or later runtime-proof boundary drifted",
    )
    counts = {
        row.get("name"): row.get("value")
        for row in gate.get("semantic_counts", [])
        if isinstance(row, dict)
    }
    require(
        set(counts) == {"pending_runtime_proof_count", "requirement_row_count"}
        and all(isinstance(value, int) and value > 0 for value in counts.values()),
        "Stage0 migration requirements do not retain positive later runtime-proof obligations",
    )


def scan_approval_promotion(value: Any, forbidden: set[str]) -> None:
    if isinstance(value, dict):
        require(not (APPROVAL_KEYS & set(value)), "external approval field was promoted")
        for child in value.values():
            scan_approval_promotion(child, forbidden)
    elif isinstance(value, list):
        for child in value:
            scan_approval_promotion(child, forbidden)
    elif isinstance(value, str):
        require(value not in forbidden, "external approval value was promoted")


def validate_no_backward_post_root_reference(documents: dict[str, dict[str, Any]]) -> None:
    for name in (
        "resource",
        "expected_delta",
        "proof",
        "decision",
        "root",
        "finalization",
        "handoff",
    ):
        encoded = json.dumps(documents[name], sort_keys=True, separators=(",", ":"))
        require(
            POST_ROOT_DELTA_NAME not in encoded
            and POST_ROOT_RECEIPT_NAME not in encoded
            and "maestro.vnext.stage0.post-root-" not in encoded,
            "post-root receipt was fed backward into an identity-bearing artifact",
        )


def scan_backward_candidate_identity(value: Any, candidate_ids: set[str]) -> None:
    if isinstance(value, dict):
        for child in value.values():
            scan_backward_candidate_identity(child, candidate_ids)
    elif isinstance(value, list):
        for child in value:
            scan_backward_candidate_identity(child, candidate_ids)
    elif isinstance(value, str):
        require(
            value not in candidate_ids,
            "candidate root, finalization, or Handoff identity was fed backward into Resource",
        )


def validate_sources(documents: dict[str, dict[str, Any]]) -> dict[str, Any]:
    proof_id, proof_gate_count = validate_proof_manifest(documents["proof"])
    proof_sha = file_sha256(PROOF_MANIFEST)
    root_id, finalization_id, handoff_id, binding_count = validate_candidate_closure(
        documents, proof_id, proof_sha, proof_gate_count
    )
    (
        resource_id,
        delta_id,
        release_id,
        successor_bindings,
        compatibility_rows,
        resource_counts,
    ) = (
        validate_resource_closure(documents["resource"], documents["expected_delta"])
    )
    candidate_ids = {root_id, finalization_id, handoff_id}
    scan_backward_candidate_identity(documents["resource"], candidate_ids)
    scan_backward_candidate_identity(documents["expected_delta"], candidate_ids)
    validate_resource_proof_binding(
        documents["proof"], resource_id, delta_id, resource_counts
    )
    validate_stage0_runtime_boundary(documents["proof"])
    forbidden = approval_forbidden_values(documents["input_bindings"])
    for name in (
        "proof",
        "decision",
        "root",
        "finalization",
        "handoff",
        "bindings",
        "resource",
        "expected_delta",
    ):
        scan_approval_promotion(documents[name], forbidden)
    validate_no_backward_post_root_reference(documents)
    return {
        "proof_id": proof_id,
        "proof_sha": proof_sha,
        "proof_gate_count": proof_gate_count,
        "root_id": root_id,
        "finalization_id": finalization_id,
        "handoff_id": handoff_id,
        "binding_count": binding_count,
        "bindings_sha": file_sha256(DECISION_BINDINGS),
        "resource_id": resource_id,
        "delta_id": delta_id,
        "release_id": release_id,
        "successor_bindings": successor_bindings,
        "compatibility_rows": compatibility_rows,
        "resource_counts": resource_counts,
        "forbidden": forbidden,
    }


def validated_reconstruction_statuses(statuses: dict[str, str]) -> dict[str, str]:
    require(
        set(statuses) == set(RECONSTRUCTION_FIELDS),
        "candidate-root reconstruction status field set drifted",
    )
    require(
        all(statuses[field] == "pass" for field in RECONSTRUCTION_FIELDS),
        "candidate-root reconstruction did not pass in Python, Ruby, and Rust",
    )
    return {field: statuses[field] for field in RECONSTRUCTION_FIELDS}


def construct_documents(
    sources: dict[str, Any], statuses: dict[str, str], documents: dict[str, dict[str, Any]]
) -> tuple[dict[str, Any], dict[str, Any]]:
    expected_delta = documents["expected_delta"]
    obligations = expected_delta["downstream_obligations"]
    successor_by_key = {
        ("RootInput", "candidate-root"): sources["root_id"],
        ("RootInput", "candidate-finalization"): sources["finalization_id"],
        ("HandoffInput", "candidate-handoff"): sources["handoff_id"],
    }
    rows = []
    for obligation in obligations:
        key = (obligation["identity_kind"], obligation["logical_key"])
        rows.append(
            {
                **obligation,
                "successor_identity": successor_by_key[key],
                "status": "resolved_post_root_stage0_producer",
            }
        )
    compatibility_rows = sources["compatibility_rows"]
    successor_ids = [row["successor_identity"] for row in compatibility_rows] + [
        row["successor_identity"] for row in rows
    ]
    require(
        len(successor_ids) == len(set(successor_ids)),
        "closed expected-delta union contains duplicate successor identities",
    )

    delta = {
        "schema": "maestro.vnext.stage0.post-root-downstream-delta.v1",
        "artifact_class": "NONCANONICAL",
        "identity_protocol": "none",
        "candidate_only": True,
        "runtime_status": "inactive",
        "runtime_activation": False,
        "source_publication": False,
        "source_resource_release_commitment_id": sources["resource_id"],
        "source_expected_delta_commitment_id": sources["delta_id"],
        "depends_on_release_identity": sources["release_id"],
        "source_compatibility_delta_row_count": len(compatibility_rows),
        "row_count": len(rows),
        "rows": rows,
    }
    receipt = {
        "schema": "maestro.vnext.stage0.post-root-validation-receipt.v1",
        "artifact_class": "NONCANONICAL",
        "identity_protocol": "none",
        "candidate_only": True,
        "runtime_status": "inactive",
        "runtime_activation": False,
        "source_publication": False,
        "post_root_identity_count": 0,
        "backward_identity_reference_count": 0,
        "stage0_proof_manifest": {
            "identity": sources["proof_id"],
            "json_artifact_sha256": sources["proof_sha"],
            "gate_count": sources["proof_gate_count"],
        },
        "candidate_contract_root_id": sources["root_id"],
        "design_finalization_manifest_id": sources["finalization_id"],
        "canonical_build_handoff_id": sources["handoff_id"],
        "decision_root_bindings": {
            "json_artifact_sha256": sources["bindings_sha"],
            "binding_count": sources["binding_count"],
        },
        "resource_release_commitment_id": sources["resource_id"],
        "expected_delta_commitment_id": sources["delta_id"],
        "release_id": sources["release_id"],
        "compatibility_successor_binding_count": len(sources["successor_bindings"]),
        "compatibility_successor_bindings": sources["successor_bindings"],
        "compatibility_successor_delta_rows": compatibility_rows,
        "resource_exact_counts": sources["resource_counts"],
        "through_release_entry_count": len(expected_delta["entries"]),
        "through_release_compatibility_row_count": len(compatibility_rows),
        "resource_downstream_obligation_count": len(obligations),
        "post_root_row_count": len(rows),
        "closed_downstream_row_count": len(compatibility_rows) + len(rows),
        "union_set_equality_status": "pass",
        "stage0_requirement_freeze_status": "pass",
        "later_runtime_proof_status": "pending_stage_11",
        **statuses,
        "external_approval_exclusion_status": "pass",
    }
    return delta, receipt


def validate_union_closure(
    delta: dict[str, Any],
    receipt: dict[str, Any],
    sources: dict[str, Any],
    documents: dict[str, dict[str, Any]],
) -> None:
    actual_compatibility = receipt.get("compatibility_successor_delta_rows")
    actual_post_root = delta.get("rows")
    require(
        isinstance(actual_compatibility, list) and isinstance(actual_post_root, list),
        "post-root closure row sets are missing",
    )
    require(
        all(isinstance(row, dict) and set(row) == COMPATIBILITY_ROW_FIELDS for row in actual_compatibility),
        "through-Release compatibility row contains missing or extra fields",
    )
    require(
        all(isinstance(row, dict) and set(row) == OBLIGATION_FIELDS for row in actual_post_root),
        "post-root obligation row contains missing or extra fields",
    )
    expected_compatibility = sources["compatibility_rows"]
    obligations = documents["expected_delta"]["downstream_obligations"]
    successor_by_key = {
        ("RootInput", "candidate-root"): sources["root_id"],
        ("RootInput", "candidate-finalization"): sources["finalization_id"],
        ("HandoffInput", "candidate-handoff"): sources["handoff_id"],
    }
    expected_post_root = [
        {
            **obligation,
            "successor_identity": successor_by_key[
                (obligation["identity_kind"], obligation["logical_key"])
            ],
            "status": "resolved_post_root_stage0_producer",
        }
        for obligation in obligations
    ]
    require(
        actual_compatibility == expected_compatibility,
        "through-Release compatibility rows differ from Resource bindings and expected delta",
    )
    require(
        actual_post_root == expected_post_root,
        "post-root rows differ from the exact Resource downstream obligations",
    )

    def signature(row_class: str, row: dict[str, Any]) -> tuple[Any, ...]:
        return (
            row_class,
            row.get("slot_name"),
            row["identity_kind"],
            row["logical_key"],
            row["predecessor_identity"],
            row["successor_identity"],
            row["disposition"],
            row.get("depends_on_release_identity"),
        )

    expected_union = {
        signature("compatibility", row) for row in expected_compatibility
    } | {signature("post_root", row) for row in expected_post_root}
    actual_union = {
        signature("compatibility", row) for row in actual_compatibility
    } | {signature("post_root", row) for row in actual_post_root}
    expected_count = len(expected_compatibility) + len(expected_post_root)
    require(
        actual_union == expected_union
        and len(actual_union) == expected_count
        and receipt.get("closed_downstream_row_count") == expected_count,
        "through-Release compatibility plus post-root union set inequality",
    )
    require(
        receipt.get("union_set_equality_status") == "pass",
        "post-root union equality status is not pass",
    )


def build(
    reconstruction_statuses: dict[str, str] | None = None,
) -> tuple[dict[str, Any], dict[str, Any]]:
    statuses = validated_reconstruction_statuses(
        run_reconstruction_checks()
        if reconstruction_statuses is None
        else reconstruction_statuses
    )
    documents = load_inputs()
    sources = validate_sources(documents)
    delta, receipt = construct_documents(sources, statuses, documents)
    validate(delta, receipt, documents=documents)
    return delta, receipt


def validate(
    delta: dict[str, Any],
    receipt: dict[str, Any],
    *,
    documents: dict[str, dict[str, Any]] | None = None,
) -> None:
    loaded = load_inputs() if documents is None else documents
    for document, label in (
        (delta, "post-root downstream delta"),
        (receipt, "post-root validation receipt"),
    ):
        require_exact_fields(
            document,
            {
                "artifact_class": "NONCANONICAL",
                "identity_protocol": "none",
                "candidate_only": True,
                "runtime_status": "inactive",
                "runtime_activation": False,
                "source_publication": False,
            },
            label,
        )
    sources = validate_sources(loaded)
    statuses = validated_reconstruction_statuses(
        {field: receipt.get(field) for field in RECONSTRUCTION_FIELDS}
    )
    scan_approval_promotion(delta, sources["forbidden"])
    scan_approval_promotion(receipt, sources["forbidden"])
    validate_union_closure(delta, receipt, sources, loaded)
    expected_delta, expected_receipt = construct_documents(sources, statuses, loaded)
    require(delta == expected_delta, "post-root downstream delta semantic drift")
    require(receipt == expected_receipt, "post-root validation receipt semantic drift")
    require("identity" not in delta and "identity" not in receipt, "post-root artifact acquired an identity")
    require(
        not ({"canonical_value", "canonical_cbor_hex", "canonical_cbor_sha256"} & set(delta))
        and not ({"canonical_value", "canonical_cbor_hex", "canonical_cbor_sha256"} & set(receipt)),
        "post-root artifact entered an identity-bearing canonicalization cycle",
    )


def mutant_rejections(
    delta: dict[str, Any], receipt: dict[str, Any]
) -> tuple[str, ...]:
    input_bindings = read_json(INPUT_BINDINGS)

    def omitted_row(mutant_delta: dict[str, Any], _: dict[str, Any]) -> None:
        mutant_delta["rows"].pop()
        mutant_delta["row_count"] -= 1

    def reordered_rows(mutant_delta: dict[str, Any], _: dict[str, Any]) -> None:
        mutant_delta["rows"].reverse()

    def substituted_successor(mutant_delta: dict[str, Any], _: dict[str, Any]) -> None:
        mutant_delta["rows"][0]["successor_identity"] = "sha256:" + "00" * 32

    def approval_promotion(_: dict[str, Any], mutant_receipt: dict[str, Any]) -> None:
        mutant_receipt["external_approval"] = copy.deepcopy(input_bindings["external_approval"])

    def extra_field(mutant_delta: dict[str, Any], _: dict[str, Any]) -> None:
        mutant_delta["rows"][0]["unexpected"] = "smuggled"

    mutations = (
        ("omitted_row", omitted_row),
        ("reordered_rows", reordered_rows),
        ("substituted_successor", substituted_successor),
        ("approval_promotion", approval_promotion),
        ("extra_field", extra_field),
    )
    rejected = []
    for name, mutate in mutations:
        mutant_delta = copy.deepcopy(delta)
        mutant_receipt = copy.deepcopy(receipt)
        mutate(mutant_delta, mutant_receipt)
        try:
            validate(mutant_delta, mutant_receipt)
        except (ContractError, KeyError, TypeError):
            rejected.append(name)
            continue
        raise AssertionError(f"semantic mutant accepted: {name}")
    return tuple(rejected)


def parse_last_json(stdout: str, label: str) -> dict[str, Any]:
    for line in reversed(stdout.splitlines()):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        require(isinstance(value, dict), f"{label} did not emit a JSON status object")
        return value
    raise ContractError(f"{label} did not emit a JSON status object")


def run_command(command: list[str], label: str) -> subprocess.CompletedProcess[str]:
    process = subprocess.run(
        command,
        cwd=WORKSPACE,
        capture_output=True,
        text=True,
        check=False,
    )
    if process.returncode != 0:
        detail = process.stderr.strip() or process.stdout.strip() or f"exit {process.returncode}"
        raise ContractError(f"{label} failed: {detail}")
    return process


def run_reconstruction_checks() -> dict[str, str]:
    python_ruby = run_command(
        [sys.executable, str(CANDIDATE_VALIDATOR)],
        "Python/Ruby candidate-root reconstruction",
    )
    status = parse_last_json(python_ruby.stdout, "Python/Ruby candidate-root reconstruction")
    require(
        status.get("status") == "validated" and status.get("ruby_encoder") == "pass",
        "Python/Ruby candidate-root reconstruction status drifted",
    )
    run_command(
        [
            "cargo",
            "test",
            "--quiet",
            "--test",
            "vnext_candidate_root",
            "emitted_candidate_root_reconstructs_with_rust_contract_types",
            "--",
            "--exact",
        ],
        "Rust candidate-root reconstruction",
    )
    return {
        "python_reconstruction_status": "pass",
        "ruby_reconstruction_status": "pass",
        "rust_reconstruction_status": "pass",
    }


def write_atomic(path: Path, document: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_bytes(json_bytes(document))
    temporary.replace(path)


def execute(check: bool, mutants: bool) -> dict[str, Any]:
    delta, receipt = build()
    expected = {
        OUTPUT / POST_ROOT_DELTA_NAME: delta,
        OUTPUT / POST_ROOT_RECEIPT_NAME: receipt,
    }
    if check:
        for path, document in expected.items():
            try:
                actual = path.read_bytes()
            except OSError as error:
                raise ContractError(f"post-root artifact drift: {path.name}: {error}") from error
            require(actual == json_bytes(document), f"post-root artifact drift: {path.name}")
    else:
        for path, document in expected.items():
            write_atomic(path, document)
    for path in expected:
        require(read_json(path) == expected[path], f"post-root artifact readback drift: {path.name}")
    validate(delta, receipt)
    rejected = mutant_rejections(delta, receipt) if mutants else ()
    return {
        "status": "checked" if check else "built",
        "artifact_class": "NONCANONICAL",
        "artifacts": [POST_ROOT_DELTA_NAME, POST_ROOT_RECEIPT_NAME],
        "post_root_rows": len(delta["rows"]),
        "semantic_mutants": len(rejected),
        **{field: receipt[field] for field in RECONSTRUCTION_FIELDS},
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Build or check the NONCANONICAL Stage-0 post-root closure receipts."
    )
    parser.add_argument("--check", action="store_true", help="compare existing artifacts without writing")
    parser.add_argument("--mutants", action="store_true", help="require all semantic mutants to be rejected")
    arguments = parser.parse_args()
    try:
        result = execute(arguments.check, arguments.mutants)
    except ContractError as error:
        print(json.dumps({"status": "failed", "error": str(error)}, sort_keys=True), file=sys.stderr)
        raise SystemExit(1) from error
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
