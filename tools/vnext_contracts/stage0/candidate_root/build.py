#!/usr/bin/env python3
"""Build the inactive, fully pinned Stage-0 candidate Contract closure.

This builder has no pending or partial output mode.  Until both the finalized
effect-home and resource-release receipts are present, it exits before writing
any root, finalization, or handoff artifact.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


WORKSPACE = Path(__file__).resolve().parents[4]
OUTPUT = WORKSPACE / "contracts/vnext/stage0/candidate-root"
DECISION = WORKSPACE / "contracts/vnext/stage0/decision-closure/decision-closure.v1.json"
PUBLIC_RECEIPT = WORKSPACE / "contracts/vnext/stage0/public-identity/encoder-receipt.v1.json"
PUBLIC_SCHEMAS = WORKSPACE / "contracts/vnext/stage0/public-identity/schema-descriptors.v1.json"
PUBLIC_CLOSURE = WORKSPACE / "contracts/vnext/stage0/public-identity/public-identity-closure.v1.json"
EFFECT_HOME = WORKSPACE / "contracts/vnext/stage0/effect-home/expected-delta-manifest.json"
EFFECT_RECEIPT = WORKSPACE / "contracts/vnext/stage0/effect-home/encoder-receipt.json"
EFFECT_FINALIZATION = WORKSPACE / "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"
SUBMISSION_CLAIM = WORKSPACE / "contracts/vnext/stage0/submission-claim/submission-claim-set.v1.json"
SUBMISSION_RECEIPT = WORKSPACE / "contracts/vnext/stage0/submission-claim/encoder-receipt.v1.json"
RESOURCE_RELEASE = WORKSPACE / "contracts/vnext/stage0/resource-release/resource-release.v1.json"
EMBEDDED_RELEASE = WORKSPACE / "contracts/vnext/stage0/resource-release/embedded-release-bundle.v1.json"
RESOURCE_SUCCESSOR_DELTA = WORKSPACE / "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.json"
INPUT_BINDINGS = WORKSPACE / "contracts/vnext/stage0/input-bindings.json"
PROOF_MANIFEST = WORKSPACE / "contracts/vnext/stage0/proof-matrix/stage0-proof-manifest.v1.json"
PROOF_MANIFEST_CBOR = PROOF_MANIFEST.with_suffix(".cbor")

DESIGN_REVISION_DOMAIN = "maestro.vnext.design-revision.v1"
DECISION_RESOLUTION_DOMAIN = "maestro.vnext.decision-resolution.v1"
SCHEMA_DOMAIN = "maestro.vnext.schema.v1"
SOURCE_BINDING_DOMAIN = "maestro.vnext.design-source-binding.v1"
COMPONENT_DOMAIN = "maestro.vnext.contract-component.v1"
ROOT_DOMAIN = "maestro.vnext.candidate-contract-root.v1"
FINALIZATION_INPUT_DOMAIN = "maestro.vnext.design-finalization-input.v1"
FINALIZATION_DOMAIN = "maestro.vnext.design-finalization-manifest.v1"
HANDOFF_DOMAIN = "maestro.vnext.build-handoff-projection.v1"
CANDIDATE_SCHEMA_CLOSURE_DOMAIN = "maestro.vnext.candidate-root-schema-closure.v1"
STAGE0_PROOF_MANIFEST_DOMAIN = "maestro.vnext.stage0-proof-manifest.v1"
SUCCESSOR_DECISION_STORE_MANIFEST_SHA256 = (
    "18f14bce862e15be09c9d88155d62627582df50c7754e2e8e1d6f6bee8f7d522"
)
SUCCESSOR_PACKET_SHA256 = (
    "fb33b048b59c66df9858558a2c80e59a478d101465761f902366c9a00751cbc5"
)

NORMATIVE_INPUTS_KIND = 12
REQUIRED_PROOF_GATES = (
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
    "Resource": 377,
    "Bundle": 8,
    "Census": 1,
    "Release": 1,
}

FINALIZATION_SCHEMA_NAMES = {
    1: "ClosureRequirementFinalizationInputV1",
    2: "DeterministicSynthesisFinalizationInputV1",
    3: "ScopeAndExclusionsFinalizationInputV1",
    4: "CapabilityCensusAndJourneysFinalizationInputV1",
    5: "MigrationRollbackRemovalFinalizationInputV1",
    6: "StageProofMatrixFinalizationInputV1",
    7: "ReviewEvidenceFinalizationInputV1",
    8: "EdgeSweepEvidenceFinalizationInputV1",
    9: "RiskRecoveryFinalizationInputV1",
    10: "FreshnessReferencesFinalizationInputV1",
    11: "CanonicalizationPolicyFinalizationInputV1",
}

FINALIZATION_FACETS = {
    1: (1, 2, 3, 4, 5),
    2: (6, 7, 8, 9),
    3: (3, 4, 5),
    4: (16, 18),
    5: (23,),
    6: (24,),
    7: (9, 24),
    8: (14, 15),
    9: (13, 14, 15, 23),
    10: (17, 18, 19, 20, 21, 22),
    11: (17, 18, 24),
}

FACET_FIELDS = {
    1: (("design_sha256", "bytes", "design_hash"),),
    2: (("acceptance_card_sha256", "bytes", "card_hash"),),
    3: (("design_sha256", "bytes", "design_hash"), ("acceptance_card_sha256", "bytes", "card_hash")),
    4: (("design_sha256", "bytes", "design_hash"),),
    5: (("design_sha256", "bytes", "design_hash"),),
    6: (("decision_closure_id", "bytes", "decision_id"),),
    7: (("decision_closure_id", "bytes", "decision_id"),),
    8: (("decision_closure_id", "bytes", "decision_id"),),
    9: (("decision_closure_id", "bytes", "decision_id"),),
    10: (("decision_closure_id", "bytes", "decision_id"),),
    11: (("decision_closure_id", "bytes", "decision_id"),),
    13: (("acceptance_card_sha256", "bytes", "card_hash"),),
    14: (("public_identity_closure_id", "bytes", "public_closure_id"), ("resource_release_id", "bytes", "resource_release_id")),
    15: (("design_sha256", "bytes", "design_hash"), ("acceptance_card_sha256", "bytes", "card_hash")),
    16: (("public_identity_closure_id", "bytes", "public_closure_id"), ("effect_home_id", "bytes", "effect_id")),
    17: (("public_schema_descriptor_ids", "bytes_list", "descriptor_ids"), ("submission_claim_set_schema_id", "bytes", "submission_schema_id"), ("submission_claim_set_artifact_sha256", "bytes", "submission_artifact_hash")),
    18: (("public_manifest_id", "bytes", "public_manifest_id"), ("public_identity_closure_id", "bytes", "public_closure_id"), ("effect_home_id", "bytes", "effect_id")),
    19: (("public_resource_input_id", "bytes", "public_resource_input_id"), ("resource_release_id", "bytes", "resource_release_id")),
    20: (("resource_release_id", "bytes", "resource_release_id"),),
    21: (("resource_release_id", "bytes", "resource_release_id"),),
    22: (("resource_release_id", "bytes", "resource_release_id"),),
    23: (("decision_closure_id", "bytes", "decision_id"), ("effect_home_id", "bytes", "effect_id")),
    24: (("stage0_proof_manifest_id", "bytes", "proof_manifest_id"), ("stage0_proof_manifest_artifact_sha256", "bytes", "proof_manifest_artifact_hash"), ("stage0_proof_gate_count", "unsigned", "proof_gate_count")),
}

COMPONENT_KINDS = tuple(sorted({NORMATIVE_INPUTS_KIND, *FACET_FIELDS}))
FINALIZATION_KINDS = tuple(sorted(FINALIZATION_SCHEMA_NAMES))
if set(FINALIZATION_KINDS) != set(FINALIZATION_FACETS):
    raise RuntimeError("finalization schema and facet ownership kinds diverged")


@dataclass(frozen=True)
class Bytes:
    value: bytes


class Blocked(RuntimeError):
    pass


def load(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as file:
        value = json.load(file)
    if not isinstance(value, dict):
        raise ValueError(f"{path} must contain an object")
    return value


def json_value(value: Any) -> Any:
    if isinstance(value, Bytes):
        return {"bytes": value.value.hex()}
    if isinstance(value, list):
        return [json_value(item) for item in value]
    if isinstance(value, dict):
        return {key: json_value(item) for key, item in value.items()}
    return value


def cbor(value: Any) -> bytes:
    if isinstance(value, Bytes):
        return cbor_head(2, len(value.value)) + value.value
    if isinstance(value, str):
        raw = value.encode("ascii")
        return cbor_head(3, len(raw)) + raw
    if isinstance(value, bool):
        return b"\xf5" if value else b"\xf4"
    if isinstance(value, int) and value >= 0:
        return cbor_head(0, value)
    if isinstance(value, list):
        return cbor_head(4, len(value)) + b"".join(cbor(item) for item in value)
    raise TypeError(f"unsupported deterministic CBOR value: {type(value)!r}")


def cbor_head(major: int, value: int) -> bytes:
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    if value <= 0xFFFFFFFFFFFFFFFF:
        return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")
    raise ValueError("CBOR value exceeds u64")


def digest(domain: str, value: Any) -> bytes:
    return hashlib.sha256(cbor([domain, value])).digest()


def rendered(digest_value: bytes) -> str:
    return f"sha256:{digest_value.hex()}"


def exact_id(value: str) -> bytes:
    if not isinstance(value, str) or not value.startswith("sha256:"):
        raise ValueError("identity must be sha256:<64 lowercase hex>")
    raw = bytes.fromhex(value.removeprefix("sha256:"))
    if len(raw) != 32 or rendered(raw) != value:
        raise ValueError("identity must be a canonical sha256 value")
    return raw


def exact_hash(value: str) -> bytes:
    if not isinstance(value, str):
        raise ValueError("hash must be lowercase hexadecimal")
    raw = bytes.fromhex(value)
    if len(raw) != 32 or raw.hex() != value:
        raise ValueError("hash must be a canonical SHA-256 hexadecimal value")
    return raw


def artifact_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def immutable_artifact(
    path: Path,
    document: dict[str, Any],
    *,
    identity_key: str = "identity",
    require_candidate: bool = True,
) -> bytes:
    identity = exact_id(document[identity_key])
    if require_candidate and document.get("candidate_only") is not True and document.get("publication_state") != "candidate_only_runtime_inactive":
        raise ValueError(f"{path.name} is not candidate-only")
    if require_candidate and document.get("runtime_activation") not in (None, False) and document.get("runtime") != "inactive":
        raise ValueError(f"{path.name} is not runtime-inactive")
    cbor_hex = document.get("canonical_cbor_hex")
    cbor_hash = document.get("canonical_cbor_sha256")
    if cbor_hex is not None or cbor_hash is not None:
        if not isinstance(cbor_hash, str):
            raise ValueError(f"{path.name} has an incomplete canonical CBOR receipt")
        if isinstance(cbor_hex, str):
            received_hash = hashlib.sha256(bytes.fromhex(cbor_hex)).hexdigest()
        else:
            cbor_path = path.with_suffix(".cbor")
            if not cbor_path.is_file():
                raise ValueError(f"{path.name} has no canonical CBOR payload")
            received_hash = artifact_hash(cbor_path)
        if received_hash != cbor_hash:
            raise ValueError(f"{path.name} canonical CBOR receipt drifted")
    return identity


def stage0_commitment_artifact(
    path: Path,
    document: dict[str, Any],
    *,
    expected_schema: str,
) -> bytes:
    if document.get("schema") != expected_schema:
        raise Blocked(f"{path.name} does not declare {expected_schema}")
    if document.get("identity_protocol") != "Stage0CanonicalCommitmentV1":
        raise Blocked(f"{path.name} does not use Stage0CanonicalCommitmentV1")
    envelope = document.get("canonical_commitment_envelope")
    if (
        not isinstance(envelope, list)
        or len(envelope) != 2
        or envelope[0] != expected_schema
        or envelope[1] != document.get("canonical_value")
    ):
        raise Blocked(f"{path.name} has an invalid canonical commitment envelope")
    cbor_path = path.with_suffix(".cbor")
    if not cbor_path.is_file():
        raise Blocked(f"{path.name} omits its canonical CBOR payload")
    canonical_bytes = cbor_path.read_bytes()
    if cbor(value_from_json(envelope)) != canonical_bytes:
        raise ValueError(f"{path.name} commitment envelope does not reproduce its canonical bytes")
    canonical_hash = hashlib.sha256(canonical_bytes).hexdigest()
    if (
        document.get("canonical_cbor_sha256") != canonical_hash
        or document.get("identity") != rendered(bytes.fromhex(canonical_hash))
        or document.get("canonical_cbor_byte_length") != cbor_path.stat().st_size
    ):
        raise ValueError(f"{path.name} canonical commitment receipt drifted")
    canonical_hex = document.get("canonical_cbor_hex")
    if not isinstance(canonical_hex, str) or bytes.fromhex(canonical_hex) != canonical_bytes:
        raise ValueError(f"{path.name} canonical commitment bytes drifted")
    return bytes.fromhex(canonical_hash)


def exact_embedded_release(document: dict[str, Any]) -> bytes:
    if document.get("schema") != "maestro.vnext.embedded-release-bundle.manifest.v1":
        raise Blocked("embedded Release schema is not final")
    if document.get("identity_protocol") != "ManifestIdentityV1":
        raise Blocked("embedded Release does not use ManifestIdentityV1")
    envelope = document.get("manifest_identity_envelope")
    if (
        not isinstance(envelope, list)
        or len(envelope) != 5
        or document.get("canonical_value") != envelope[3:5]
        or document.get("sole_release_root") is not True
    ):
        raise Blocked("embedded Release has an invalid five-slot manifest envelope")
    cbor_path = EMBEDDED_RELEASE.with_suffix(".cbor")
    if not cbor_path.is_file():
        raise Blocked("embedded Release omits its canonical ManifestIdentityV1 bytes")
    release_id = exact_hash(document["release_id"])
    canonical_bytes = cbor_path.read_bytes()
    if cbor(value_from_json(envelope)) != canonical_bytes:
        raise ValueError("embedded Release envelope does not reproduce its canonical bytes")
    canonical_hash = hashlib.sha256(canonical_bytes).hexdigest()
    if (
        release_id.hex() != canonical_hash
        or document.get("identity") != rendered(release_id)
        or document.get("canonical_cbor_sha256") != canonical_hash
        or document.get("canonical_cbor_byte_length") != cbor_path.stat().st_size
    ):
        raise ValueError("embedded Release ManifestIdentityV1 receipt drifted")
    canonical_hex = document.get("canonical_cbor_hex")
    if not isinstance(canonical_hex, str) or bytes.fromhex(canonical_hex) != canonical_bytes:
        raise ValueError("embedded Release canonical bytes drifted")
    return release_id


def final_resource_release(
    effect_id: bytes, effect_finalization: dict[str, Any]
) -> tuple[dict[str, Any], bytes]:
    if not RESOURCE_RELEASE.is_file():
        raise Blocked(f"missing required final resource-release artifact: {RESOURCE_RELEASE.relative_to(WORKSPACE)}")
    document = load(RESOURCE_RELEASE)
    if document.get("schema") != "maestro.vnext.stage0.resource-release.v1":
        raise Blocked("resource-release artifact does not declare the required Stage-0 final schema")
    required_state = {
        "identity_protocol": "Stage0CanonicalCommitmentV1",
        "candidate_only": True,
        "source_publication": False,
        "runtime_activation": False,
        "runtime_registration": False,
    }
    if any(document.get(key) != value for key, value in required_state.items()):
        raise Blocked("resource-release artifact is not an unpublished, runtime-inactive Stage0 commitment")
    if "release_state" in document or "runtime" in document:
        raise Blocked("resource-release artifact contains legacy synthetic state fields")
    wrapper_commitment_id = stage0_commitment_artifact(
        RESOURCE_RELEASE,
        document,
        expected_schema="maestro.vnext.stage0.resource-release.v1",
    )
    if not EMBEDDED_RELEASE.is_file():
        raise Blocked("missing exact embedded Release artifact")
    release_document = load(EMBEDDED_RELEASE)
    release_id = exact_embedded_release(release_document)
    if document.get("embedded_release_bundle") != release_document:
        raise Blocked("resource-release wrapper does not embed the exact Release artifact")

    successor_delta, successor_delta_id = final_resource_successor_delta()
    if document.get("expected_delta") != successor_delta:
        raise Blocked("resource-release does not embed the exact through-Release expected-delta artifact")
    obligations = successor_delta["downstream_obligations"]
    if document.get("downstream_delta_obligations") != obligations:
        raise Blocked("resource-release does not preserve its exact downstream obligation rows")
    if any(
        item["depends_on_release_identity"] != rendered(release_id)
        for item in obligations
    ):
        raise Blocked("resource-release downstream obligations do not depend on its exact Release")

    bindings = document.get("resolved_successor_bindings")
    if not isinstance(bindings, list) or not bindings:
        raise Blocked("resource-release must bind every compatibility successor slot")
    slot_names = [item.get("slot_name") for item in bindings]
    successor_ids = [item.get("successor_identity") for item in bindings]
    if (
        slot_names != list(RESOURCE_SUCCESSOR_SLOTS)
        or any(set(item) != {"slot_name", "successor_identity"} for item in bindings)
        or len({exact_id(identifier) for identifier in successor_ids}) != len(bindings)
    ):
        raise Blocked("resource-release compatibility successor bindings are incomplete, reordered, malformed, or duplicate")
    counts = {
        "declared_successor_slot_count": len(bindings),
        "resolved_successor_slot_count": len(bindings),
        "blocked_successor_slot_count": 0,
        "null_successor_identity_count": 0,
    }
    if any(document.get(key) != value for key, value in counts.items()):
        raise Blocked("resource-release successor slot closure is not fully resolved")
    if document.get("resolved_expected_delta_commitment_id") != rendered(successor_delta_id):
        raise Blocked("resource-release does not pin its independently recomputed successor delta")
    effect_pins = {
        "effect_home_finalization_receipt_sha256": artifact_hash(EFFECT_FINALIZATION),
        "effect_home_finalization_identity": effect_finalization["identity"],
        "effect_home_expected_delta_manifest_id": rendered(effect_id),
    }
    if any(document.get(key) != value for key, value in effect_pins.items()):
        raise Blocked("resource-release does not pin the final effect-home receipt and expected delta")
    by_slot = {item["slot_name"]: item["successor_identity"] for item in bindings}
    delta_successors = [item["successor_identity"] for item in successor_delta["entries"]]
    if any(delta_successors.count(identifier) != 1 for identifier in by_slot.values()):
        raise Blocked("resource-release compatibility successor is not represented exactly once in its delta")
    if by_slot.get("effect_control_h2") != effect_finalization["h2_manifest_identity"]:
        raise Blocked("resource-release h2 successor does not match the final effect-home receipt")
    if by_slot.get("local_withdrawal_h3") != effect_finalization["h3_withdrawal_identity"]:
        raise Blocked("resource-release h3 successor does not match the final effect-home receipt")
    if by_slot.get("release_binding") != rendered(release_id):
        raise Blocked("resource-release release-binding slot does not equal the exact ReleaseId")
    if (
        document.get("resource_count") != 377
        or len(document.get("resources", ())) != 377
        or document.get("bundle_count") != 8
        or len(document.get("bundles", ())) != 8
        or len(release_document.get("bundle_ids", ())) != 8
    ):
        raise Blocked("resource-release exact Resource/Bundle counts changed")
    if wrapper_commitment_id == release_id:
        raise Blocked("Stage0 wrapper commitment must remain distinct from the canonical ReleaseId")
    return document, release_id


def final_resource_successor_delta() -> tuple[dict[str, Any], bytes]:
    if not RESOURCE_SUCCESSOR_DELTA.is_file():
        raise Blocked(
            "missing required Resource/Release successor delta artifact: "
            f"{RESOURCE_SUCCESSOR_DELTA.relative_to(WORKSPACE)}"
        )
    document = load(RESOURCE_SUCCESSOR_DELTA)
    if document.get("schema") != "maestro.vnext.migration-cutover-expected-delta-successor.v1":
        raise Blocked("Resource/Release successor delta schema is not final")
    cbor_path = RESOURCE_SUCCESSOR_DELTA.with_suffix(".cbor")
    if not cbor_path.is_file():
        raise Blocked("Resource/Release successor delta omits its canonical CBOR payload")
    canonical_hash = artifact_hash(cbor_path)
    if document.get("canonical_cbor_sha256") != canonical_hash:
        raise ValueError("Resource/Release successor delta canonical CBOR hash drifted")
    identity = stage0_commitment_artifact(
        RESOURCE_SUCCESSOR_DELTA,
        document,
        expected_schema="maestro.vnext.migration-cutover-expected-delta-successor.v1",
    )
    required = {
        "identity_protocol": "Stage0CanonicalCommitmentV1",
        "candidate_only": True,
        "publication_status": "resolved_through_release_downstream_obligations_pending",
        "runtime_activation": False,
    }
    if any(document.get(key) != value for key, value in required.items()):
        raise Blocked("Resource/Release successor delta is not a closed inactive Stage0 commitment")
    entries = document.get("entries")
    if (
        not isinstance(entries, list)
        or len(entries) != sum(THROUGH_RELEASE_IDENTITY_COUNTS.values())
        or document.get("exact_identity_kind_counts") != THROUGH_RELEASE_IDENTITY_COUNTS
    ):
        raise Blocked("Resource/Release successor delta does not contain the exact through-Release identity census")
    entry_keys = [(item.get("identity_kind"), item.get("logical_key")) for item in entries]
    successor_ids = [item.get("successor_identity") for item in entries]
    if (
        len(set(entry_keys)) != len(entries)
        or len({exact_id(identifier) for identifier in successor_ids}) != len(entries)
        or document.get("resolved_entry_count") != len(entries)
    ):
        raise Blocked("Resource/Release successor delta entries are incomplete or duplicate")
    actual_counts = {
        kind: sum(item.get("identity_kind") == kind for item in entries)
        for kind in THROUGH_RELEASE_IDENTITY_COUNTS
    }
    if actual_counts != THROUGH_RELEASE_IDENTITY_COUNTS:
        raise Blocked("Resource/Release successor delta entry kinds do not match the exact identity census")
    for item in entries:
        predecessor = item.get("predecessor_identity")
        successor = item["successor_identity"]
        disposition = item.get("disposition")
        if predecessor is not None:
            exact_id(predecessor)
        if (
            (disposition == "Introduce" and predecessor is not None)
            or (disposition == "Rotate" and (predecessor is None or predecessor == successor))
            or (disposition == "Preserve" and predecessor != successor)
            or disposition not in {"Introduce", "Rotate", "Preserve"}
        ):
            raise Blocked("Resource/Release successor delta disposition is inconsistent")

    obligations = document.get("downstream_obligations")
    required_obligation_keys = {
        ("RootInput", "candidate-root"),
        ("RootInput", "candidate-finalization"),
        ("HandoffInput", "candidate-handoff"),
    }
    if not isinstance(obligations, list) or {
        (item.get("identity_kind"), item.get("logical_key")) for item in obligations
    } != required_obligation_keys:
        raise Blocked("Resource/Release successor delta downstream obligation set is incomplete")
    if any(
        item.get("predecessor_identity") is not None
        or item.get("successor_identity") is not None
        or item.get("disposition") != "Introduce"
        or item.get("status") != "pending_downstream_stage0_producer"
        for item in obligations
    ):
        raise Blocked("Resource/Release successor delta invents a downstream identity")
    if (
        document.get("blocked_dependency_count") != len(obligations)
        or document.get("unresolved_obligation_count") != len(obligations)
    ):
        raise Blocked("Resource/Release successor delta obligation counts are inconsistent")
    if identity != bytes.fromhex(canonical_hash):
        raise ValueError("Resource/Release successor delta identity is not its canonical commitment hash")
    return document, identity


def final_effect_home() -> tuple[dict[str, Any], bytes, dict[str, Any]]:
    if not EFFECT_FINALIZATION.is_file():
        raise Blocked(f"missing required final effect-home receipt: {EFFECT_FINALIZATION.relative_to(WORKSPACE)}")
    effect = load(EFFECT_HOME)
    effect_id = immutable_artifact(EFFECT_HOME, effect)
    receipt = load(EFFECT_RECEIPT)
    if receipt.get("equality") != "exact_bytes_length_and_sha256":
        raise ValueError("effect-home independent encoder equality is incomplete")
    finalization = load(EFFECT_FINALIZATION)
    required = {
        "schema_version": "maestro.vnext.stage0.effect-home-finalization-receipt.v1",
        "finalization_state": "final",
        "candidate_only": True,
        "runtime": "inactive",
        "expected_delta_manifest_id": rendered(effect_id),
        "encoder_receipt_sha256": artifact_hash(EFFECT_RECEIPT),
        "unresolved_actual_semantic_consumers": 0,
        "runtime_activation": False,
    }
    if any(finalization.get(key) != value for key, value in required.items()):
        raise Blocked("effect-home final receipt does not pin the exact inactive expected-delta artifact")
    for key in ("identity", "h2_manifest_identity", "h3_withdrawal_identity"):
        exact_id(finalization[key])
    finalization_body = {key: value for key, value in finalization.items() if key != "identity"}
    expected_identity = rendered(
        hashlib.sha256(
            json.dumps(finalization_body, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")
        ).digest()
    )
    if finalization["identity"] != expected_identity:
        raise ValueError("effect-home finalization receipt identity drifted")
    return effect, effect_id, finalization


def forbidden_promotion_values(*values: Any) -> set[str]:
    forbidden: set[str] = set()

    def visit(value: Any) -> None:
        if isinstance(value, dict):
            canonical = json.dumps(
                value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
            ).encode("ascii")
            add_hash(hashlib.sha256(canonical).hexdigest())
            for child in value.values():
                visit(child)
            return
        if isinstance(value, list):
            canonical = json.dumps(
                value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
            ).encode("ascii")
            add_hash(hashlib.sha256(canonical).hexdigest())
            for child in value:
                visit(child)
            return
        if isinstance(value, (str, int)) and not isinstance(value, bool):
            text = str(value)
            forbidden.add(text)
            if text.startswith("sha256:"):
                add_hash(text.removeprefix("sha256:"))
            elif len(text) == 64:
                try:
                    exact_hash(text)
                except (ValueError, TypeError):
                    pass
                else:
                    add_hash(text)
            add_hash(hashlib.sha256(text.encode("ascii")).hexdigest())

    def add_hash(hexadecimal: str) -> None:
        forbidden.add(hexadecimal)
        forbidden.add(f"sha256:{hexadecimal}")

    for value in values:
        visit(value)
    return forbidden


def stage0_proof_manifest(forbidden: set[str]) -> tuple[dict[str, Any], bytes, bytes, int]:
    if not PROOF_MANIFEST.is_file() or not PROOF_MANIFEST_CBOR.is_file():
        raise Blocked("missing required pre-root Stage0ProofManifest")
    document = load(PROOF_MANIFEST)
    required = {
        "schema": "maestro.vnext.stage0-proof-manifest.v1",
        "candidate_only": True,
        "runtime_activation": False,
    }
    if any(document.get(key) != value for key, value in required.items()):
        raise Blocked("Stage0ProofManifest is not an inactive candidate proof artifact")
    gates = document.get("gates")
    if not isinstance(gates, list):
        raise Blocked("Stage0ProofManifest omits its exact gate set")
    if [gate.get("tag") for gate in gates] != list(range(1, len(REQUIRED_PROOF_GATES) + 1)):
        raise Blocked("Stage0ProofManifest gate tags are missing, duplicate, or reordered")
    if [gate.get("name") for gate in gates] != list(REQUIRED_PROOF_GATES):
        raise Blocked("Stage0ProofManifest does not contain the exact required gate set")
    if document.get("gate_count") != len(gates) or len(gates) != len(REQUIRED_PROOF_GATES):
        raise Blocked("Stage0ProofManifest gate count is incomplete")
    if any(gate.get("result") != "passed" for gate in gates):
        raise Blocked("Stage0ProofManifest contains a missing or failed gate")
    if gates[0].get("result_class") != "verified_non_promoting":
        raise Blocked("external input authorization proof is not non-promoting")
    encoded = PROOF_MANIFEST_CBOR.read_bytes()
    if document.get("canonical_cbor_sha256") != hashlib.sha256(encoded).hexdigest():
        raise ValueError("Stage0ProofManifest canonical CBOR hash drifted")
    canonical = value_from_json(document.get("canonical_value"))
    if cbor(canonical) != encoded:
        raise ValueError("Stage0ProofManifest canonical value does not reproduce its bytes")
    if (
        not isinstance(canonical, list)
        or len(canonical) != 2
        or canonical[0] != 1
        or not isinstance(canonical[1], list)
        or len(canonical[1]) != len(gates)
    ):
        raise ValueError("Stage0ProofManifest canonical envelope is malformed")
    for gate, gate_value in zip(gates, canonical[1], strict=True):
        if (
            not isinstance(gate_value, list)
            or len(gate_value) != 9
            or gate_value[0] != gate["tag"]
            or gate_value[1] != gate["name"]
            or gate_value[5] != 1
            or gate_value[6] != gate.get("result_class")
            or not isinstance(gate_value[7], Bytes)
            or len(gate_value[7].value) != 32
            or not isinstance(gate_value[3], list)
            or not gate_value[3]
        ):
            raise ValueError("Stage0ProofManifest canonical gate binding drifted")
    external_gate = canonical[1][0]
    external_result_hash = hashlib.sha256(b"verified_non_promoting").digest()
    if (
        external_gate[2]
        or external_gate[4]
        or external_gate[8]
        or external_gate[7].value != external_result_hash
    ):
        raise Blocked(
            "external input authorization gate must bind only validator source identity"
        )
    proof_id = digest(STAGE0_PROOF_MANIFEST_DOMAIN, canonical)
    if document.get("identity") != rendered(proof_id):
        raise ValueError("Stage0ProofManifest identity drifted")
    scan_forbidden(document, forbidden)
    return document, proof_id, exact_hash(artifact_hash(PROOF_MANIFEST)), len(gates)


def validate_successor_decision_manifest(decision: dict[str, Any]) -> None:
    provenance = decision.get("source_provenance_excluded_from_identity", {})
    if provenance.get("decisions_sha256") != SUCCESSOR_DECISION_STORE_MANIFEST_SHA256:
        raise ValueError("candidate root is not bound to the successor Decision store")
    records = decision.get("records")
    if not isinstance(records, list) or len(records) != 213:
        raise ValueError("candidate root lacks the exact successor Decision closure")
    manifest = "".join(
        f"{item['id']}\t{item['terminal_status']}\t"
        f"{item['raw_record_sha256']}\t{item['raw_body_sha256']}\n"
        for item in records
    ).encode("ascii")
    if hashlib.sha256(manifest).hexdigest() != SUCCESSOR_DECISION_STORE_MANIFEST_SHA256:
        raise ValueError("candidate root successor Decision manifest reconstruction mismatch")


def input_sources() -> dict[str, Any]:
    bindings = load(INPUT_BINDINGS)
    forbidden = forbidden_promotion_values(
        bindings["external_approval"], bindings["external_approval_event"]
    )
    bindings_hash = artifact_hash(INPUT_BINDINGS)
    forbidden.update((bindings_hash, f"sha256:{bindings_hash}"))
    effect, effect_id, effect_finalization = final_effect_home()
    resource_release, resource_release_id = final_resource_release(effect_id, effect_finalization)
    decision = load(DECISION)
    if bindings["external_approval"]["packet_sha256"] == SUCCESSOR_PACKET_SHA256:
        validate_successor_decision_manifest(decision)
    decision_id = immutable_artifact(DECISION, decision, require_candidate=False)
    materials = decision.get("materializations")
    base = {
        "kind": "initial_external_design_closure",
        "decision_closure_id": rendered(decision_id),
    }
    if decision.get("root_assembly", {}).get("materialization_base") != base:
        raise ValueError("decision closure does not encode the typed initial materialization base")
    if not isinstance(materials, list) or not materials:
        raise ValueError("decision closure must expose a non-empty materialization set")
    material_ids = [item.get("id") for item in materials]
    if len(set(material_ids)) != len(material_ids):
        raise ValueError("decision materialization slots are not unique")
    materials = sorted(materials, key=lambda item: item["id"])
    for item in materials:
        if item.get("component_kind_tag") != NORMATIVE_INPUTS_KIND or item.get("materialization_base") != base:
            raise ValueError("decision materialization slot lacks typed NormativeInputs provenance")
        exact_hash(item["id"])

    public = load(PUBLIC_RECEIPT)
    if public.get("encoder_equality") != "pass" or public.get("python_semantic_validator") != "pass" or public.get("ruby_encoder") != "pass":
        raise ValueError("public identity independent verification is incomplete")
    public_ids = {
        "closure": exact_id(public["closure_id"]),
        "manifest": exact_id(public["manifest_id"]),
        "resource_input": exact_id(public["resource_input_id"]),
    }
    schemas = load(PUBLIC_SCHEMAS)
    descriptor_ids = sorted(exact_id(item["schema_id"]) for item in schemas.get("descriptors", []))
    public_closure = load(PUBLIC_CLOSURE)
    closure_descriptor_ids = sorted(
        exact_id(item["schema_id"]) for item in public_closure.get("schema_descriptors", [])
    )
    if not descriptor_ids or len(set(descriptor_ids)) != len(descriptor_ids):
        raise ValueError("public identity schema descriptor set is empty or duplicate")
    if descriptor_ids != closure_descriptor_ids:
        raise ValueError("public identity descriptor set differs from its canonical closure")

    submission = load(SUBMISSION_CLAIM)
    submission_receipt = load(SUBMISSION_RECEIPT)
    if submission.get("candidate_only") is not True or submission.get("runtime_activation") is not False:
        raise ValueError("submission-claim set is not inactive candidate input")
    if submission["schema_id"] != submission_receipt.get("schema_id"):
        raise ValueError("submission-claim schema receipt mismatch")
    if submission_receipt.get("artifact_sha256") != artifact_hash(SUBMISSION_CLAIM):
        raise ValueError("submission-claim artifact hash receipt mismatch")
    if submission_receipt.get("semantic_mutant_count") != 10:
        raise ValueError("submission-claim semantic mutant proof is incomplete")

    proof_manifest, proof_manifest_id, proof_manifest_artifact_hash, proof_gate_count = (
        stage0_proof_manifest(forbidden)
    )
    for source in (effect, resource_release, load(EFFECT_FINALIZATION), proof_manifest):
        scan_forbidden(source, forbidden)
    verify_closed_sources()
    source_inputs = bindings["canonical_source_inputs"]
    current_source_inputs = bindings["current_source_inputs"]
    if bindings["external_approval"]["packet_sha256"] == SUCCESSOR_PACKET_SHA256:
        if source_inputs != current_source_inputs:
            raise ValueError("successor canonical and current source inputs diverged")
        tracked_design = (
            WORKSPACE
            / ".maestro/cards/maestro-whole-flow-architecture-refoundation/design.md"
        )
        if artifact_hash(tracked_design) != current_source_inputs["design_sha256"]:
            raise ValueError("successor tracked design drifted from its packet-bound commitment")
    else:
        source_card = (
            Path(bindings["source_repository_realpath"])
            / ".maestro/cards"
            / bindings["feature_id"]
        )
        for filename, expected_key in (
            ("design.md", "design_sha256"),
            ("card.yaml", "card_sha256"),
        ):
            if artifact_hash(source_card / filename) != current_source_inputs[expected_key]:
                raise ValueError(
                    f"current {filename} content drifted from its attested source commitment"
                )
    return {
        "decision": decision,
        "decision_id": decision_id,
        "materials": materials,
        "base": base,
        "public_ids": public_ids,
        "public_closure_id": public_ids["closure"],
        "public_manifest_id": public_ids["manifest"],
        "public_resource_input_id": public_ids["resource_input"],
        "descriptor_ids": descriptor_ids,
        "effect_id": effect_id,
        "resource_release": resource_release,
        "resource_release_id": resource_release_id,
        "submission_schema_id": exact_id(submission["schema_id"]),
        "submission_artifact_hash": exact_hash(submission_receipt["artifact_sha256"]),
        "design_hash": exact_hash(source_inputs["design_sha256"]),
        "card_hash": exact_hash(source_inputs["card_sha256"]),
        "proof_manifest_id": proof_manifest_id,
        "proof_manifest_artifact_hash": proof_manifest_artifact_hash,
        "proof_gate_count": proof_gate_count,
        "forbidden": forbidden,
    }


def verify_closed_sources() -> None:
    env = {
        **os.environ,
        "MAESTRO_AUTHORITATIVE_SOURCE": load(INPUT_BINDINGS)[
            "source_repository_realpath"
        ],
    }
    commands = (
        [sys.executable, str(WORKSPACE / "tools/vnext_contracts/stage0/verify_input_bindings.py")],
        [sys.executable, str(WORKSPACE / "tools/vnext_contracts/stage0/decision_closure/validate.py")],
        [sys.executable, str(WORKSPACE / "tools/vnext_contracts/stage0/public_identity/verify.py")],
        [sys.executable, str(WORKSPACE / "tools/vnext_contracts/stage0/effect_home/validate.py")],
        ["/usr/bin/ruby", str(WORKSPACE / "tools/vnext_contracts/stage0/submission_claim/verify.rb")],
        [sys.executable, str(WORKSPACE / "tools/vnext_contracts/stage0/proof_matrix/validate.py")],
    )
    for command in commands:
        try:
            subprocess.run(
                command,
                cwd=WORKSPACE,
                check=True,
                capture_output=True,
                text=True,
                env=env,
            )
        except subprocess.CalledProcessError as error:
            raise ValueError(
                f"validated Stage-0 dependency failed: {Path(command[-1]).name}: {error.stderr.strip()}"
            ) from error


def descriptor_value(name: str, fields: tuple[tuple[str, str, str], ...]) -> Any:
    type_values = {
        "unsigned": [1],
        "bytes": [4, 32],
        "bytes_list": [7, [4, 32]],
        "source_rows": [7, [8, [[3], [4, 32]]]],
    }
    return [
        name,
        1,
        [[position, field_name, type_values[field_type], [[1]]] for position, (field_name, field_type, _) in enumerate(fields, start=1)],
        [],
    ]


def candidate_schema_descriptors() -> dict[str, dict[str, Any]]:
    definitions: dict[str, tuple[str, tuple[tuple[str, str, str], ...]]] = {
        "normative": (
            "NormativeInputsDecisionMaterializationV1",
            (("version", "unsigned", ""), ("materialization_id", "bytes", ""), ("decision_sources", "source_rows", "")),
        ),
    }
    for kind, fields in FACET_FIELDS.items():
        definitions[f"facet-{kind}"] = (f"CandidateRootFacet{kind}V1", (("version", "unsigned", ""), *fields))
    finalization_fields = (
        ("version", "unsigned", ""),
        ("input_kind", "unsigned", ""),
        ("design_revision_id", "bytes", ""),
        ("decision_closure_id", "bytes", ""),
        ("candidate_contract_root_id", "bytes", ""),
        ("owner_facet_component_ids", "bytes_list", ""),
    )
    for kind, name in FINALIZATION_SCHEMA_NAMES.items():
        fields = finalization_fields
        if kind == 6:
            fields = (
                *fields,
                ("stage0_proof_manifest_id", "bytes", ""),
                ("stage0_proof_manifest_artifact_sha256", "bytes", ""),
                ("stage0_proof_gate_count", "unsigned", ""),
            )
        definitions[f"finalization-{kind}"] = (name, fields)
    descriptors = {}
    for key, (name, fields) in definitions.items():
        canonical = descriptor_value(name, fields)
        descriptors[key] = {
            "schema_name": name,
            "schema_id": digest(SCHEMA_DOMAIN, canonical),
            "canonical_value": canonical,
        }
    return descriptors


def design_revision(sources: dict[str, Any], schemas: dict[str, dict[str, Any]]) -> tuple[bytes, Any]:
    public_ids = sources["public_ids"]
    value = [
        1,
        Bytes(sources["decision_id"]),
        Bytes(public_ids["closure"]),
        Bytes(public_ids["manifest"]),
        Bytes(public_ids["resource_input"]),
        [Bytes(item) for item in sources["descriptor_ids"]],
        [Bytes(item["schema_id"]) for _, item in sorted(schemas.items())],
        Bytes(sources["effect_id"]),
        Bytes(sources["resource_release_id"]),
        Bytes(sources["submission_schema_id"]),
        Bytes(sources["submission_artifact_hash"]),
        Bytes(sources["design_hash"]),
        Bytes(sources["card_hash"]),
    ]
    return digest(DESIGN_REVISION_DOMAIN, value), value


def resolution_id(decision_id: bytes, materialization_id: bytes) -> tuple[bytes, Any]:
    base = [1, 1, Bytes(decision_id)]
    value = [1, base, Bytes(decision_id), Bytes(materialization_id)]
    return digest(DECISION_RESOLUTION_DOMAIN, value), value


def component_id(kind: int, schema: bytes, value: Any, dependencies: list[bytes], provenance: Any) -> tuple[bytes, Any]:
    canonical = [1, kind, Bytes(schema), value, [Bytes(item) for item in dependencies], provenance]
    return digest(COMPONENT_DOMAIN, canonical), canonical


def candidate_components(
    sources: dict[str, Any], design_id: bytes, schemas: dict[str, dict[str, Any]]
) -> tuple[list[dict[str, Any]], dict[str, bytes]]:
    components: list[dict[str, Any]] = []
    normative_ids: dict[str, bytes] = {}
    for material in sources["materials"]:
        materialization_id = exact_hash(material["id"])
        resolution, resolution_value = resolution_id(sources["decision_id"], materialization_id)
        source_rows = [
            [source["id"], Bytes(exact_hash(source["body_sha256"]))]
            for source in material["decision_sources"]
        ]
        value = [1, Bytes(materialization_id), source_rows]
        schema = schemas["normative"]["schema_id"]
        provenance = [3, Bytes(resolution), Bytes(materialization_id)]
        component, canonical = component_id(NORMATIVE_INPUTS_KIND, schema, value, [], provenance)
        normative_ids[material["id"]] = component
        components.append({
            "kind_tag": NORMATIVE_INPUTS_KIND,
            "component_id": rendered(component),
            "schema_id": rendered(schema),
            "provenance": {"kind": "decision_materialization", "resolution_id": rendered(resolution), "materialization_id": rendered(materialization_id), "canonical_value": json_value(resolution_value)},
            "canonical_value": json_value(canonical),
        })

    dependencies = sorted(normative_ids.values())
    aggregate_ids: dict[str, bytes] = {}
    for kind in COMPONENT_KINDS:
        if kind == NORMATIVE_INPUTS_KIND:
            continue
        fields = FACET_FIELDS[kind]
        owned_values = []
        owned_commitments = {}
        for field_name, field_type, source_key in fields:
            raw = sources[source_key]
            if field_type == "bytes_list":
                value = [Bytes(item) for item in raw]
            elif field_type == "unsigned":
                value = raw
            else:
                value = Bytes(raw)
            owned_values.append(value)
            if field_type == "bytes_list":
                owned_commitments[field_name] = [rendered(item) for item in raw]
            elif field_type == "unsigned":
                owned_commitments[field_name] = raw
            elif field_name.endswith("sha256"):
                owned_commitments[field_name] = raw.hex()
            else:
                owned_commitments[field_name] = rendered(raw)
        value = [1, *owned_values]
        source_binding_value = [1, Bytes(design_id), kind, value]
        source_binding = digest(SOURCE_BINDING_DOMAIN, source_binding_value)
        schema = schemas[f"facet-{kind}"]["schema_id"]
        provenance = [1, Bytes(design_id), kind, Bytes(source_binding)]
        component, canonical = component_id(kind, schema, value, dependencies, provenance)
        aggregate_ids[str(kind)] = component
        record: dict[str, Any] = {
            "kind_tag": kind,
            "component_id": rendered(component),
            "schema_id": rendered(schema),
            "provenance": {"kind": "design_slot", "design_revision_id": rendered(design_id), "slot_tag": kind, "source_binding_id": rendered(source_binding)},
            "canonical_value": json_value(canonical),
        }
        record["owned_commitments"] = owned_commitments
        components.append(record)
    return components, normative_ids | aggregate_ids


def root(components: list[dict[str, Any]]) -> tuple[bytes, Any, list[dict[str, Any]]]:
    by_id = {item["component_id"]: item for item in components}
    component_values = {item["component_id"]: value_from_json(item["canonical_value"]) for item in components}
    dependencies = {
        item["component_id"]: [rendered(value.value) for value in component_values[item["component_id"]][4]]
        for item in components
    }
    ordered: list[str] = []
    ready = sorted(
        (item["kind_tag"], item["component_id"])
        for item in components
        if not dependencies[item["component_id"]]
    )
    while ready:
        _, current = ready.pop(0)
        ordered.append(current)
        for candidate in components:
            candidate_id = candidate["component_id"]
            if current in dependencies[candidate_id]:
                dependencies[candidate_id].remove(current)
                if not dependencies[candidate_id]:
                    ready.append((candidate["kind_tag"], candidate_id))
                    ready.sort()
    if len(ordered) != len(components):
        raise ValueError("candidate component graph is cyclic")
    ordered_components = [by_id[item] for item in ordered]
    value = [
        1,
        len(ordered_components),
        [[Bytes(exact_id(item["component_id"])), component_values[item["component_id"]]] for item in ordered_components],
    ]
    return digest(ROOT_DOMAIN, value), value, ordered_components


def finalization_inputs(
    sources: dict[str, Any],
    design_id: bytes,
    root_id: bytes,
    schemas: dict[str, dict[str, Any]],
    component_ids: dict[str, bytes],
) -> list[dict[str, Any]]:
    records = []
    for kind in FINALIZATION_KINDS:
        schema = schemas[f"finalization-{kind}"]["schema_id"]
        owner_facet_ids = [component_ids[str(facet_kind)] for facet_kind in FINALIZATION_FACETS[kind]]
        value = [
            1,
            kind,
            Bytes(design_id),
            Bytes(sources["decision_id"]),
            Bytes(root_id),
            [Bytes(identifier) for identifier in owner_facet_ids],
        ]
        if kind == 6:
            value.extend(
                [
                    Bytes(sources["proof_manifest_id"]),
                    Bytes(sources["proof_manifest_artifact_hash"]),
                    sources["proof_gate_count"],
                ]
            )
        input_id = digest(FINALIZATION_INPUT_DOMAIN, [kind, Bytes(schema), value])
        records.append({
            "kind_tag": kind,
            "schema_id": rendered(schema),
            "input_id": rendered(input_id),
            "owner_facet_component_ids": [rendered(identifier) for identifier in owner_facet_ids],
            "canonical_value": json_value(value),
        })
    return records


def finalization(design_id: bytes, decision_id: bytes, root_id: bytes, inputs: list[dict[str, Any]]) -> tuple[bytes, Any]:
    value = [
        1,
        [1, Bytes(design_id)],
        Bytes(decision_id),
        Bytes(root_id),
        [[item["kind_tag"], Bytes(exact_id(item["schema_id"])), Bytes(exact_id(item["input_id"]))] for item in inputs],
    ]
    return digest(FINALIZATION_DOMAIN, value), value


def handoff(finalization_id: bytes, root_id: bytes, components: list[dict[str, Any]], inputs: list[dict[str, Any]]) -> tuple[bytes, Any]:
    value = [
        1,
        Bytes(finalization_id),
        Bytes(root_id),
        [[item["kind_tag"], Bytes(exact_id(item["component_id"]))] for item in components],
        [[item["kind_tag"], Bytes(exact_id(item["schema_id"])), Bytes(exact_id(item["input_id"]))] for item in inputs],
    ]
    return digest(HANDOFF_DOMAIN, value), value


def value_from_json(value: Any) -> Any:
    if isinstance(value, dict) and set(value) == {"bytes"}:
        return Bytes(exact_hash(value["bytes"]))
    if isinstance(value, list):
        return [value_from_json(item) for item in value]
    if isinstance(value, dict):
        return {key: value_from_json(item) for key, item in value.items()}
    return value


def write_artifact(path: Path, document: dict[str, Any], canonical: Any) -> None:
    encoded = cbor(canonical)
    document["canonical_value"] = json_value(canonical)
    document["canonical_cbor_hex"] = encoded.hex()
    document["canonical_cbor_sha256"] = hashlib.sha256(encoded).hexdigest()
    path.write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")


def build() -> dict[str, Any]:
    sources = input_sources()
    schemas = candidate_schema_descriptors()
    design_id, design_value = design_revision(sources, schemas)
    components, component_ids = candidate_components(sources, design_id, schemas)
    root_id, root_value, ordered_components = root(components)
    inputs = finalization_inputs(sources, design_id, root_id, schemas, component_ids)
    manifest_id, finalization_value = finalization(design_id, sources["decision_id"], root_id, inputs)
    handoff_id, handoff_value = handoff(manifest_id, root_id, ordered_components, inputs)
    bindings = [
        {
            "materialization_id": rendered(exact_hash(material["id"])),
            "component_id": rendered(component_ids[material["id"]]),
            "materialization_base": sources["base"],
            "after_root_id": rendered(root_id),
            "finalization_manifest_id": rendered(manifest_id),
        }
        for material in sources["materials"]
    ]
    generated = {
        "candidate-root-schema-descriptors.v1.json": ({
            "schema": "maestro.vnext.candidate-root-schema-closure.v1",
            "candidate_only": True,
            "runtime": "inactive",
            "identity": rendered(digest(CANDIDATE_SCHEMA_CLOSURE_DOMAIN, [1, [[item["schema_name"], Bytes(item["schema_id"]), item["canonical_value"]] for _, item in sorted(schemas.items())]])),
            "descriptors": [
                {"schema_name": item["schema_name"], "schema_id": rendered(item["schema_id"]), "canonical_value": json_value(item["canonical_value"])}
                for _, item in sorted(schemas.items())
            ],
        }, [1, [[item["schema_name"], Bytes(item["schema_id"]), item["canonical_value"]] for _, item in sorted(schemas.items())]]),
        "design-revision.v1.json": ({
            "schema": "maestro.vnext.design-revision.v1",
            "candidate_only": True,
            "runtime": "inactive",
            "identity": rendered(design_id),
            "source_ids": {
                "decision_closure": rendered(sources["decision_id"]),
                "public_identity_closure": rendered(sources["public_ids"]["closure"]),
                "effect_home": rendered(sources["effect_id"]),
                "resource_release": rendered(sources["resource_release_id"]),
                "approved_design_sha256": sources["design_hash"].hex(),
                "acceptance_card_sha256": sources["card_hash"].hex(),
            },
            "submission_claim_set": {"schema_id": rendered(sources["submission_schema_id"]), "artifact_sha256": sources["submission_artifact_hash"].hex()},
        }, design_value),
        "candidate-contract-root.v1.json": ({
            "schema": "maestro.vnext.candidate-contract-root.v1",
            "candidate_only": True,
            "runtime": "inactive",
            "identity": rendered(root_id),
            "component_count": len(ordered_components),
            "components": ordered_components,
        }, root_value),
          "design-finalization-manifest.v1.json": ({
            "schema": "maestro.vnext.design-finalization-manifest.v1",
            "candidate_only": True,
            "runtime": "inactive",
            "identity": rendered(manifest_id),
            "design_revision_id": rendered(design_id),
            "decision_closure_id": rendered(sources["decision_id"]),
              "candidate_contract_root_id": rendered(root_id),
              "stage0_proof_manifest": {
                  "identity": rendered(sources["proof_manifest_id"]),
                  "artifact_sha256": sources["proof_manifest_artifact_hash"].hex(),
                  "gate_count": sources["proof_gate_count"],
              },
              "pinned_inputs": inputs,
        }, finalization_value),
        "canonical-build-handoff.v1.json": ({
            "schema": "maestro.vnext.canonical-build-handoff.v1",
            "candidate_only": True,
            "runtime": "inactive",
            "identity": rendered(handoff_id),
            "finalization_manifest_id": rendered(manifest_id),
              "candidate_contract_root_id": rendered(root_id),
              "stage0_proof_manifest": {
                  "identity": rendered(sources["proof_manifest_id"]),
                  "artifact_sha256": sources["proof_manifest_artifact_hash"].hex(),
                  "gate_count": sources["proof_gate_count"],
              },
            "component_count": len(ordered_components),
            "pinned_input_count": len(inputs),
        }, handoff_value),
    }
    return {"sources": sources, "schemas": schemas, "generated": generated, "bindings": bindings}


def scan_forbidden(document: Any, forbidden: set[str]) -> None:
    forbidden_keys = {
        "external_approval",
        "external_approval_event",
        "packet_sha256",
        "candidate_input_commitment",
        "build_plan_handoff",
        "external_candidate_input_commitment",
        "external_build_plan_handoff",
        "bindings_sha256",
    }
    if isinstance(document, dict):
        if forbidden_keys & set(document):
            raise ValueError("external approval input leaked into candidate root material")
        for key, value in document.items():
            scan_forbidden(key, forbidden)
            scan_forbidden(value, forbidden)
        return
    if isinstance(document, list):
        for value in document:
            scan_forbidden(value, forbidden)
        return
    if isinstance(document, (str, int)) and not isinstance(document, bool):
        if str(document) in forbidden:
            raise ValueError("external approval input leaked into candidate root material")


def execute(check: bool) -> None:
    try:
        result = build()
    except Blocked as error:
        print(json.dumps({"status": "blocked", "reason": str(error)}))
        raise SystemExit(2) from error
    for document, _ in result["generated"].values():
        scan_forbidden(document, result["sources"]["forbidden"])
    scan_forbidden(result["bindings"], result["sources"]["forbidden"])
    if check:
        expected_paths = set(result["generated"]) | {"decision-root-bindings.v1.json"}
        actual_paths = {path.name for path in OUTPUT.glob("*.json")} if OUTPUT.is_dir() else set()
        if actual_paths != expected_paths:
            raise SystemExit("candidate-root artifact set is missing or contains stale files")
        for name, (document, canonical) in result["generated"].items():
            expected = dict(document)
            encoded = cbor(canonical)
            expected.update({"canonical_value": json_value(canonical), "canonical_cbor_hex": encoded.hex(), "canonical_cbor_sha256": hashlib.sha256(encoded).hexdigest()})
            if load(OUTPUT / name) != expected:
                raise SystemExit(f"candidate-root artifact drift: {name}")
        expected_bindings = {"schema": "maestro.vnext.exact-decision-root-bindings.v1", "candidate_only": True, "runtime": "inactive", "decision_closure_id": rendered(result["sources"]["decision_id"]), "bindings": result["bindings"]}
        if load(OUTPUT / "decision-root-bindings.v1.json") != expected_bindings:
            raise SystemExit("candidate-root artifact drift: decision-root-bindings.v1.json")
    else:
        staging = OUTPUT.with_name(f"{OUTPUT.name}.tmp")
        shutil.rmtree(staging, ignore_errors=True)
        staging.mkdir(parents=True)
        for name, (document, canonical) in result["generated"].items():
            write_artifact(staging / name, document, canonical)
        (staging / "decision-root-bindings.v1.json").write_text(json.dumps({"schema": "maestro.vnext.exact-decision-root-bindings.v1", "candidate_only": True, "runtime": "inactive", "decision_closure_id": rendered(result["sources"]["decision_id"]), "bindings": result["bindings"]}, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
        shutil.rmtree(OUTPUT, ignore_errors=True)
        staging.rename(OUTPUT)
    root_document = next(
        document
        for document, _ in result["generated"].values()
        if document["schema"] == "maestro.vnext.candidate-contract-root.v1"
    )
    finalization_document = next(
        document
        for document, _ in result["generated"].values()
        if document["schema"] == "maestro.vnext.design-finalization-manifest.v1"
    )
    print(json.dumps({"status": "built" if not check else "checked", "candidate_contract_root_id": root_document["identity"], "components": root_document["component_count"], "decision_bindings": len(result["bindings"]), "finalization_inputs": len(finalization_document["pinned_inputs"]), "proof_gates": result["sources"]["proof_gate_count"]}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    execute(parser.parse_args().check)
