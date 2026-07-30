#!/usr/bin/env python3
"""Independent semantic validation for the Stage-0 Resource/Release closure."""

from __future__ import annotations

import argparse
import ast
import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence

REPO_ROOT = Path(__file__).resolve().parents[4]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools.vnext_contracts.stage0.resource_release.c868_contract import (
    BUNDLE_TOPOLOGY,
    DISPOSITION_TAGS,
    FROZEN_SCHEMA_IDS,
    FROZEN_SOURCE_SHA256,
    BundleManifest,
    ContractError,
    EmbeddedReleaseBundle,
    ReleaseResourceCensus,
    ResourceDescriptor,
    bytes32,
    encode_cbor,
    identity_digest,
    profile_commitment_value,
    validate_release_closure,
    verify_frozen_inputs,
)
from tools.vnext_contracts.stage0.resource_release.current_inventory import (
    BundleKind,
    ReaderRole,
    ResourceDisposition,
    SourceKind,
    build_current_inventory,
    inventory_hash,
    validate_inventory,
)


ROOT = REPO_ROOT
OUT = ROOT / "contracts/vnext/stage0/resource-release"
FROZEN = Path("/Users/reinamaccredy/Code/maestro/.maestro/workbench")
BUNDLE_NAMES = (
    "bundle-001-migration.v1",
    "bundle-002-external-pattern-neutral.v1",
    "bundle-003-external-pattern-vendor.v1",
    "bundle-004-shared-contract.v1",
    "bundle-005-orchestration.v1",
    "bundle-006-capability.v1",
    "bundle-007-adapter.v1",
    "bundle-008-agent-bootstrap.v1",
)
AUDIT_NAMES = (
    "current-surface-manifest.v1.json",
    "current-consumer-census.v1.json",
    "current-persistence-manifest.v1.json",
    "current-archive-manifest.v1.json",
    "golden-fixture-manifest.v1.json",
    "migration-rollback-requirements.v1.json",
)
GATES = (
    "current_surface_consumer_census",
    "persistence_archive_fixtures",
    "migration_rollback_removal",
    "resource_release_and_delta",
)
IDENTITY_KINDS = ("Schema", "Manifest", "Resource", "Bundle", "Census", "Release")
EFFECT_OUTPUT_PATHS = (
    "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.cbor",
    "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.json",
    "contracts/vnext/stage0/resource-release/resource-release.v1.json",
)
OLD_GRAMMAR_ID = "2b428f8444253794cd0abb41b32da482cc0805359c2a37bf0cba90a70e3186e9"
OLD_CATALOG_IDS = (
    "f39ae4f0e747cf19ba7bd9cc16b4eaf29bdddb7cfc4cab49625bfdd0e09edee8",
    "b6fd6c299f8978d6f97db042ed01e4984c0273fab838866a910412e8569ebaf5",
    "484e5e174d5408d31561fa5ee34538a995db94e476375f6c94cc5f7f71010575",
    "855d17dfce8d206631261ca9e059b55049f98e5cb9f90a1831088fd4d85d5cf4",
    "de753de440749276563740c5e1b7a6b89e7d8164c58403285be0bed1a5276486",
    "96e984904c2348162156f7f4f77a50974f76f8b7d06f01b82bf03613582b1327",
    "1f002c4704963ba00a58379b210abf086bf82ee69aec96125f23546c8cf675e0",
    "9ed2cc7202730d1cdf2549017b1a7aa39b868fe32c35c49d695ca6ddbdc51e77",
    "fd5732582862f1a59fc4c42aa1d7dbf57ddc4961f82ff9c3ed149a1a799a7eff",
)
READER_OWNER_TAGS = {
    "Distribution": 1,
    "AgentBootstrap": 2,
    "Capability": 3,
    "Orchestration": 4,
    "SharedContract": 5,
    "Adapter": 6,
    "Design": 7,
    "Migration": 8,
    "ContractClosure": 9,
    "Submission": 10,
    "Execution": 11,
    "Integration": 12,
}


class ValidationError(RuntimeError):
    """An artifact does not reproduce the exact live source closure."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode() + b"\n"


def replace_bytes32(value: Any, replacements: Mapping[str, str]) -> Any:
    if isinstance(value, list):
        return [replace_bytes32(item, replacements) for item in value]
    if isinstance(value, dict):
        if set(value) == {"bytes"} and value["bytes"] in replacements:
            return bytes32(replacements[value["bytes"]])
        return {key: replace_bytes32(item, replacements) for key, item in value.items()}
    return value


def exact_hash(value: Any, name: str = "identity") -> str:
    require(isinstance(value, str), f"{name} must be a SHA-256 string")
    raw = value.removeprefix("sha256:")
    require(len(raw) == 64 and all(character in "0123456789abcdef" for character in raw), f"{name} is not canonical SHA-256")
    return raw


def file_sha(locator: str) -> str:
    path = ROOT / locator
    require(path.is_file(), f"missing exact source: {locator}")
    return sha256(path.read_bytes())


def load_json(name: str) -> dict[str, Any]:
    path = OUT / name
    require(path.is_file(), f"missing Resource/Release artifact: {name}")
    value = json.loads(path.read_text())
    require(isinstance(value, dict), f"{name} is not a JSON object")
    return value


def load_documents() -> dict[str, Any]:
    documents = {
        "resource-descriptors": load_json("resource-descriptors.v1.json"),
        "census": load_json("release-resource-census.v1.json"),
        "release": load_json("embedded-release-bundle.v1.json"),
        "delta": load_json("expected-delta-successor.v1.json"),
        "closure": load_json("resource-release.v1.json"),
        **{name: load_json(name) for name in AUDIT_NAMES},
    }
    documents["bundles"] = [load_json(f"{name}.json") for name in BUNDLE_NAMES]
    return documents


def contains_null(value: Any) -> bool:
    if value is None:
        return True
    if isinstance(value, list):
        return any(contains_null(item) for item in value)
    if isinstance(value, dict):
        return any(contains_null(item) for item in value.values())
    return False


def validate_stage0_commitment(document: Mapping[str, Any], schema: str) -> bytes:
    require(document.get("schema") == schema, f"{schema} schema drifted")
    require(document.get("identity_protocol") == "Stage0CanonicalCommitmentV1", f"{schema} falsely claims a frozen ManifestIdentity")
    require(document.get("identity_scope") == "canonical_commitment_envelope_only", f"{schema} identity scope drifted")
    require("manifest_identity_envelope" not in document, f"{schema} also claims a false ManifestIdentity envelope")
    envelope = document.get("canonical_commitment_envelope")
    require(isinstance(envelope, list) and len(envelope) == 2 and envelope == [schema, document.get("canonical_value")], f"{schema} two-slot commitment drifted")
    raw = encode_cbor(envelope)
    identity = sha256(raw)
    require(exact_hash(document.get("identity"), schema) == identity, f"{schema} commitment identity drifted")
    require(document.get("canonical_cbor_sha256") == identity, f"{schema} CBOR SHA-256 drifted")
    require(document.get("canonical_cbor_byte_length") == len(raw), f"{schema} CBOR length drifted")
    require(document.get("canonical_cbor_hex") == raw.hex(), f"{schema} CBOR bytes drifted")
    require(document.get("candidate_only") is True and document.get("runtime_activation") is False, f"{schema} candidate state drifted")
    return raw


def validate_manifest_document(
    document: Mapping[str, Any],
    *,
    schema: str,
    identity_name: str,
    cbor_name: str,
) -> bytes:
    require(document.get("schema") == schema, f"{schema} wrapper schema drifted")
    require(document.get("identity_protocol") == "ManifestIdentityV1", f"{schema} identity protocol drifted")
    require(document.get("candidate_only") is True and document.get("runtime_activation") is False, f"{schema} candidate state drifted")
    require("canonical_commitment_envelope" not in document, f"{schema} also claims a false Stage0 commitment envelope")
    envelope = document.get("manifest_identity_envelope")
    require(isinstance(envelope, list) and len(envelope) == 5, f"{schema} must use exact five-slot ManifestIdentityV1")
    require(not contains_null(envelope), f"{schema} ManifestIdentity contains null")
    identity, raw = identity_digest(envelope)
    require(exact_hash(document.get(identity_name), identity_name) == identity, f"{schema} identity field drifted")
    require(exact_hash(document.get("identity"), "identity") == identity, f"{schema} rendered identity drifted")
    require(document.get("canonical_value") == envelope[3:5], f"{schema} canonical value differs from manifest header/rows")
    require(document.get("canonical_cbor_sha256") == identity, f"{schema} CBOR SHA-256 drifted")
    require(document.get("canonical_cbor_byte_length") == len(raw), f"{schema} CBOR length drifted")
    require(document.get("canonical_cbor_hex") == raw.hex(), f"{schema} CBOR hex drifted")
    path = OUT / cbor_name
    require(path.is_file() and path.read_bytes() == raw, f"{schema} sibling CBOR payload drifted")
    return raw


def resource_from_record(record: Mapping[str, Any]) -> ResourceDescriptor:
    value = record.get("value")
    envelope = record.get("identity_envelope")
    require(isinstance(value, list) and isinstance(envelope, list), "Resource record lacks exact value/envelope")
    raw = bytes.fromhex(str(record.get("cbor_hex")))
    owner = value[7] if len(value) > 7 else []
    require(isinstance(owner, list) and len(owner) == 2 and isinstance(owner[1], dict), "Resource owner shape drifted")
    return ResourceDescriptor(
        int(record["inventory_ordinal"]),
        str(record["stable_resource_key"]),
        str(record["required_bundle_kind"]),
        str(record["disposition"]),
        (int(owner[0]), str(owner[1]["bytes"])),
        value,
        envelope,
        exact_hash(record.get("resource_id"), "ResourceId"),
        raw,
    )


def bundle_from_document(document: Mapping[str, Any]) -> BundleManifest:
    return BundleManifest(
        int(document["bundle_tag"]),
        str(document["bundle_kind"]),
        tuple(str(value) for value in document["resource_ids"]),
        tuple(str(value) for value in document["dependency_bundle_ids"]),
        list(document["canonical_value"]),
        list(document["manifest_identity_envelope"]),
        exact_hash(document["bundle_id"], "BundleId"),
        bytes.fromhex(str(document["canonical_cbor_hex"])),
    )


def census_from_document(document: Mapping[str, Any]) -> ReleaseResourceCensus:
    return ReleaseResourceCensus(
        tuple(str(value) for value in document["resource_ids"]),
        tuple(str(value) for value in document["bundle_ids"]),
        tuple(sorted((str(row[0]), str(row[1])) for row in document["consumer_edges"])),
        list(document["canonical_value"]),
        list(document["manifest_identity_envelope"]),
        exact_hash(document["census_id"], "CensusId"),
        bytes.fromhex(str(document["canonical_cbor_hex"])),
    )


def release_from_document(document: Mapping[str, Any]) -> EmbeddedReleaseBundle:
    return EmbeddedReleaseBundle(
        tuple(str(value) for value in document["bundle_ids"]),
        str(document["census_id"]),
        list(document["canonical_value"]),
        list(document["manifest_identity_envelope"]),
        exact_hash(document["release_id"], "ReleaseId"),
        bytes.fromhex(str(document["canonical_cbor_hex"])),
    )


def validate_frozen_inputs_live() -> dict[str, Any]:
    names = {
        "suite_bytes": "vnext-resource-contract-suite-v1.json",
        "builder_bytes": "vnext_resource_contract_suite_build.py",
        "validator_bytes": "vnext_resource_contract_suite_validate.py",
        "suite_envelope_bytes": "vnext-resource-contract-suite-v1-envelope.json",
        "runtime_edge_envelope_bytes": "vnext-distribution-runtime-edge-contract-v1-envelope.json",
    }
    for name in names.values():
        require((FROZEN / name).is_file(), f"missing frozen C868 source: {name}")
    return verify_frozen_inputs(**{key: (FROZEN / name).read_bytes() for key, name in names.items()})


def canonical_reader_coordinate(reader: Any) -> list[Any]:
    return [
        reader.reader_locator,
        bytes32(reader.reader_content_sha256),
        reader.semantic_owner.value,
        reader.kind.value,
        reader.evidence_kind.value,
        reader.role.value,
        bytes32(sha256(reader.evidence.encode())),
        reader.explicit_dual_role_contract,
    ]


def expected_profiles(candidate: Any, readers: Sequence[Any], compatibility_profile_id: str) -> dict[str, str | None]:
    base = [
        1,
        candidate.stable_key,
        candidate.stable_locator,
        bytes32(candidate.content_sha256),
        len(candidate.content_bytes),
        candidate.source_kind.tag,
        candidate.semantic_owner.frozen_tag,
        candidate.target_bundle_kind.tag,
        candidate.target_bundle_group,
        candidate.disposition.tag,
        [canonical_reader_coordinate(row) for row in readers],
    ]

    def commitment(label: str, detail: Any) -> str:
        return profile_commitment_value([1, label, base, detail])

    historical = [
        [row.locator, bytes32(row.recorded_sha256), row.family, row.current_bytes_equal]
        for row in candidate.provenance.historical_evidence
    ]
    profiles: dict[str, str | None] = {
        "owner": profile_commitment_value([1, "semantic-owner", candidate.semantic_owner.value]),
        "provenance": commitment(
            "provenance",
            [
                candidate.provenance.kind.tag,
                candidate.provenance.registry_locator or "none",
                candidate.provenance.license_locator or "none",
                bytes32(sha256(candidate.provenance.applicability.encode())),
                historical,
            ],
        ),
        "license": None,
        "compatibility": compatibility_profile_id,
        "generator": None,
        "target": commitment(
            "target-policy",
            [candidate.target_bundle_kind.tag, candidate.target_bundle_group, "candidate_not_materialized"],
        ),
        "custody": commitment(
            "custody-policy",
            [candidate.provenance.kind.tag, "source_bytes_read_only", "no_installed_state_identity"],
        ),
        "migration": commitment(
            "migration-requirement",
            ["pending_stage0_execution", candidate.disposition.tag, "exact_content_and_reader_coordinates"],
        ),
        "rollback": commitment(
            "rollback-requirement",
            ["pending_stage0_rehearsal", candidate.disposition.tag, "exact_preimage_or_refuse"],
        ),
        "uninstall": commitment(
            "uninstall-policy",
            ["pending_stage0_execution", candidate.disposition.tag, "no_ambient_target_inference"],
        ),
        "retention": commitment(
            "retention-policy",
            ["pending_stage0_execution", candidate.disposition.tag, "content_bound"],
        ),
        "removal": commitment(
            "removal-requirement",
            [
                "pending_stage0_execution",
                candidate.disposition.tag,
                sum(row.role == ReaderRole.REMOVAL_PROOF for row in readers),
                sum(row.role == ReaderRole.SEALED_READER for row in readers),
            ],
        ),
        "proof": commitment(
            "proof-requirement",
            ["pending_stage0_execution", False, "stage0_contract_proof_only"],
        ),
    }
    admitted = {
        "c868-successor.v1.cbor",
        "c868-successor.v1.json",
        "capability-evaluator.v1.json",
        "capability-relations.v1.json",
        "vendor-reference-pack.v1.json",
        "writer-compatibility-successor.v1.cbor",
        "writer-compatibility-successor.v1.json",
    }
    if candidate.stable_locator.rsplit("/", 1)[-1] in admitted and candidate.stable_locator.startswith(
        "contracts/vnext/stage0/resource-release/"
    ):
        profiles["generator"] = commitment(
            "generator",
            [
                bytes32(file_sha("tools/vnext_contracts/stage0/resource_release/build.py")),
                bytes32(FROZEN_SOURCE_SHA256["suite"]),
                bytes32("f9a2ecbff7b8b1912b78ed7c6b028eb0d9c3bdba92e0d9ac8f0377214e8150d9"),
                bytes32(candidate.content_sha256),
            ],
        )
    if candidate.provenance.license_locator:
        profiles["license"] = profile_commitment_value(
            [
                1,
                "third-party-license",
                candidate.provenance.license_locator,
                bytes32(file_sha(candidate.provenance.license_locator)),
            ]
        )
    return profiles


def validate_resources(
    documents: Mapping[str, Any], inventory: Any
) -> tuple[list[ResourceDescriptor], list[dict[str, Any]]]:
    descriptor_set = documents["resource-descriptors"]
    raw = validate_stage0_commitment(descriptor_set, "maestro.vnext.stage0.resource-descriptor-set.v1")
    require((OUT / "resource-descriptors.v1.cbor").read_bytes() == raw, "Resource descriptor-set sibling CBOR drifted")
    require(descriptor_set.get("descriptor_domain") == "maestro.vnext.resource.descriptor.v1", "Resource descriptor domain drifted")
    require(descriptor_set.get("descriptor_schema_id") == FROZEN_SCHEMA_IDS["ResourceDescriptorV1"], "Resource descriptor SchemaId drifted")
    require(descriptor_set.get("resource_count") == 377, "Resource descriptor-set count drifted")
    records = descriptor_set.get("resources")
    require(isinstance(records, list) and len(records) == 377, "Resource descriptor set is not exact 377")
    candidates = list(inventory.resources)
    readers_by_key: dict[str, list[Any]] = {candidate.stable_key: [] for candidate in candidates}
    for reader in inventory.direct_readers:
        readers_by_key[reader.resource_stable_key].append(reader)
    c868 = load_json("c868-successor.v1.json")
    writer = load_json("writer-compatibility-successor.v1.json")
    compatibility = profile_commitment_value(
        [1, "resource-compatibility", bytes32(c868["manifest_id"]), bytes32(writer["manifest_id"])]
    )
    resources: list[ResourceDescriptor] = []
    for index, (record, candidate) in enumerate(zip(records, candidates, strict=True), 1):
        require(record.get("inventory_ordinal") == index, "Resource tag/order differs from frozen inventory")
        require(record.get("stable_resource_key") == candidate.stable_key, "Resource stable key differs from inventory")
        require(record.get("stable_locator") == candidate.stable_locator, "Resource locator differs from inventory")
        require(record.get("inventory_candidate_id") == candidate.physical_candidate_id, "Resource candidate identity differs from inventory")
        require(record.get("content_sha256") == candidate.content_sha256, "Resource current content SHA differs from inventory")
        require(record.get("content_byte_length") == len(candidate.content_bytes), "Resource current content length differs from inventory")
        require(record.get("required_bundle_kind") == candidate.target_bundle_kind.value, "Resource Bundle kind differs from inventory")
        require(record.get("target_bundle_group") == candidate.target_bundle_group, "Resource concrete Bundle group differs from inventory")
        require(record.get("disposition") == candidate.disposition.value, "Resource disposition differs from inventory")
        require(record.get("family") == candidate.family, "Resource family differs from inventory")
        require(record.get("source_kind") == candidate.source_kind.value, "Resource source kind differs from inventory")
        require(record.get("semantic_owner") == candidate.semantic_owner.value, "Resource semantic owner differs from inventory")
        require(record.get("content_encoding") == candidate.c868_content_encoding.value, "Resource content encoding differs from inventory")
        require(record.get("media_type") == candidate.media_type, "Resource media type differs from inventory")
        require(record.get("resource_kind") == candidate.frozen_resource_kind.value, "Resource kind differs from inventory")
        require(record.get("provenance_kind") == candidate.provenance.kind.value, "Resource provenance kind differs from inventory")
        require(record.get("license_locator") == candidate.provenance.license_locator, "Resource license locator differs from inventory")
        expected_evidence = [
            {
                "reader_locator": row.reader_locator,
                "reader_content_sha256": row.reader_content_sha256,
                "semantic_owner": row.semantic_owner.value,
                "consumer_kind": row.kind.value,
                "evidence_kind": row.evidence_kind.value,
                "role": row.role.value,
                "evidence_sha256": sha256(row.evidence.encode()),
            }
            for row in readers_by_key[candidate.stable_key]
        ]
        require(record.get("reader_evidence") == expected_evidence, "Resource typed reader evidence differs from inventory")
        path = ROOT / candidate.stable_locator
        require(path.is_file() and sha256(path.read_bytes()) == candidate.content_sha256, "Resource is not bound to exact live bytes")
        resource = resource_from_record(record)
        require(not contains_null(resource.envelope), "Resource Descriptor identity contains null")
        identity, canonical = identity_digest(resource.envelope)
        require(identity == resource.resource_id and canonical == resource.canonical_cbor, "Resource DescriptorId/canonical bytes drifted")
        require(record.get("byte_length") == len(canonical), "Resource Descriptor byte length drifted")
        profiles = expected_profiles(candidate, readers_by_key[candidate.stable_key], compatibility)
        require(record.get("profiles") == profiles, "Resource content-bound profile commitments drifted")
        value = resource.value
        require(
            [
                value[7][1]["bytes"],
                value[10]["bytes"],
                value[11][1]["bytes"] if value[11][0] == 1 else None,
                value[13]["bytes"],
                value[14][1]["bytes"] if value[14][0] == 1 else None,
                *[value[position]["bytes"] for position in range(15, 22)],
                value[23]["bytes"],
            ]
            == [
                profiles["owner"],
                profiles["provenance"],
                profiles["license"],
                profiles["compatibility"],
                profiles["generator"],
                profiles["target"],
                profiles["custody"],
                profiles["migration"],
                profiles["rollback"],
                profiles["uninstall"],
                profiles["retention"],
                profiles["removal"],
                profiles["proof"],
            ],
            "Resource Descriptor profile coordinates differ from exact profiles",
        )
        resources.append(resource)
    require(
        descriptor_set["canonical_value"]
        == [1, [[row.resource_tag, bytes32(row.resource_id)] for row in resources]],
        "Resource descriptor-set commitment is not the exact Resource closure",
    )
    return resources, records


def strings_in(value: Any) -> set[str]:
    if isinstance(value, str):
        return {value}
    if isinstance(value, list):
        result: set[str] = set()
        for item in value:
            result.update(strings_in(item))
        return result
    if isinstance(value, dict):
        result = set(value)
        for item in value.values():
            result.update(strings_in(item))
        return result
    return set()


def expected_proof_stable_inventory_validation(validation: Any) -> dict[str, Any]:
    return {
        "family_count": validation.family_count,
        "authoritative_source_count": validation.authoritative_source_count,
        "generated_reference_producer_count": validation.generated_reference_producer_count,
        "resource_count": validation.resource_count,
        "direct_reader_edge_count": validation.direct_reader_edge_count,
        "historical_e204_count": validation.historical_e204_count,
        "external_pattern_bundle_group_count": validation.external_pattern_bundle_group_count,
        "legacy_tui_source_count": validation.legacy_tui_source_count,
        "legacy_tui_runtime_reachable_count": validation.legacy_tui_runtime_reachable_count,
        "legacy_tui_typescript_project_only_count": validation.legacy_tui_typescript_project_only_count,
        "legacy_tui_migration_census_only_count": validation.legacy_tui_migration_census_only_count,
        "unclassified_paths": list(validation.unclassified_paths),
        "inventory_sha256": validation.inventory_sha256,
    }


def validate_current_surface(
    documents: Mapping[str, Any], inventory: Any, resources: Sequence[ResourceDescriptor]
) -> None:
    surface = documents["current-surface-manifest.v1.json"]
    consumers = documents["current-consumer-census.v1.json"]
    validate_stage0_commitment(surface, "maestro.vnext.current-surface-manifest.v1")
    validate_stage0_commitment(consumers, "maestro.vnext.current-consumer-census.v1")
    require(surface.get("inventory_sha256") == inventory_hash(inventory), "current surface inventory hash drifted")
    validation = validate_inventory(inventory)
    require(
        surface.get("inventory_validation") == expected_proof_stable_inventory_validation(validation),
        "current surface proof-stable inventory validation projection drifted",
    )
    require(surface.get("resource_count") == 377 and surface.get("direct_reader_edge_count") == 377, "current surface totality count drifted")
    require(surface.get("generated_reference_producer_count") == 59, "producer-only src/interfaces count drifted")
    require(surface.get("unclassified_paths") == [], "current surface has unclassified paths")
    rows = surface.get("resources")
    require(isinstance(rows, list) and len(rows) == 377, "current surface Resource rows drifted")
    expected_resource_rows = [
        {
            "resource_tag": resource.resource_tag,
            "resource_id": resource.resource_id,
            "stable_resource_key": candidate.stable_key,
            "stable_locator": candidate.stable_locator,
            "content_sha256": candidate.content_sha256,
            "content_byte_length": len(candidate.content_bytes),
            "source_kind": candidate.source_kind.value,
            "family": candidate.family,
            "semantic_owner": candidate.semantic_owner.value,
            "required_bundle_kind": candidate.target_bundle_kind.value,
            "target_bundle_group": candidate.target_bundle_group,
            "disposition": candidate.disposition.value,
            "resource_kind": candidate.frozen_resource_kind.value,
            "provenance_kind": candidate.provenance.kind.value,
        }
        for candidate, resource in zip(inventory.resources, resources, strict=True)
    ]
    require(rows == expected_resource_rows, "current surface Resource records differ from live inventory")
    reader_rows = surface.get("direct_readers")
    require(isinstance(reader_rows, list) and len(reader_rows) == 377, "current surface exact reader rows drifted")
    by_key = {candidate.stable_key: resources[candidate.inventory_ordinal - 1] for candidate in inventory.resources}
    expected_reader_rows = [
        {
            "reader_locator": row.reader_locator,
            "reader_content_sha256": row.reader_content_sha256,
            "semantic_owner": row.semantic_owner.value,
            "consumer_kind": row.kind.value,
            "evidence_kind": row.evidence_kind.value,
            "role": row.role.value,
            "resource_stable_key": row.resource_stable_key,
            "resource_candidate_id": row.resource_candidate_id,
            "resource_locator": row.resource_locator,
            "resource_id": by_key[row.resource_stable_key].resource_id,
            "disposition": row.disposition.value,
            "evidence": row.evidence,
            "evidence_sha256": sha256(row.evidence.encode()),
            "explicit_dual_role_contract": row.explicit_dual_role_contract,
        }
        for row in inventory.direct_readers
    ]
    require(reader_rows == expected_reader_rows, "current surface reader evidence differs from live typed registry")
    producer_rows = surface.get("generated_reference_producers")
    expected_producers = [
        {
            "locator": row.stable_locator,
            "content_sha256": row.content_sha256,
            "content_byte_length": len(row.content_bytes),
            "classification": "generated_reference_producer_only_not_resource",
        }
        for row in inventory.authoritative_sources
        if row.source_kind == SourceKind.GENERATED_REFERENCE_PRODUCER
    ]
    require(producer_rows == expected_producers, "current surface exact 59 producer-only records drifted")
    forbidden = {
        row.stable_locator
        for row in inventory.vnext_sources
        if row.source_kind in {SourceKind.GENERATED_PROOF_OUTPUT, SourceKind.DOCUMENTATION_NOT_RESOURCE}
    }
    require(not (strings_in(surface["canonical_commitment_envelope"]) & forbidden), "noncanonical output/documentation leaked into current surface identity")
    require(consumers.get("resource_count") == 377 and consumers.get("direct_reader_edge_count") == 377, "current consumer census count drifted")
    require(consumers.get("exact_one_reader_evidence_per_resource") is True, "current consumer census is not exact one-evidence-per-Resource")
    require(consumers.get("historical_c325_promoted") is False, "historical C325 rows were promoted")
    require(consumers.get("consumer_inventory_digest") == consumer_inventory_digest(inventory), "current consumer inventory digest drifted")
    require(consumers.get("readers") == expected_reader_rows, "current consumer evidence rows differ from current surface")
    require(
        surface.get("historical_completeness_evidence")
        == {
            "e204": {
                "count": 204,
                "digest": "c8fc4c6cd53d81272d19c3b402e99a0ca3f69ebd18cf9464539db1d1ecf85388",
                "classification": "non_promoting_historical_evidence",
            },
            "c325": {
                "count": 325,
                "digest": "9aee8ea371f770e8694131079d4bfb4845f849d59d0b545005a2f0371a42976a",
                "classification": "non_promoting_historical_evidence",
            },
            "physical": {
                "node_count": 28102,
                "locator_digest": "0490f6c1960b840181e119d9a5d493a6906686bc3240dfe55f049e5c09d791be",
                "identity_digest": "29bfc337d3b4187c04f9e61c3a9f0bc012bdaef9fb93cc5af9a6ff58b8505d8c",
                "classification": "non_promoting_historical_filesystem_evidence",
                "current_equality_claimed": False,
                "global_absence_claimed": False,
            },
        },
        "historical completeness evidence drifted",
    )
    historical = surface["historical_completeness_evidence"]
    require(
        surface.get("canonical_value")
        == [
            1,
            bytes32(inventory_hash(inventory)),
            [[resource.resource_tag, bytes32(resource.resource_id)] for resource in resources],
            [
                [
                    row["reader_locator"],
                    bytes32(row["reader_content_sha256"]),
                    bytes32(row["resource_id"]),
                    row["role"],
                    row["evidence_kind"],
                ]
                for row in expected_reader_rows
            ],
            [[row["locator"], bytes32(row["content_sha256"])] for row in expected_producers],
            [
                [historical["e204"]["count"], bytes32(historical["e204"]["digest"])],
                [historical["c325"]["count"], bytes32(historical["c325"]["digest"])],
                [
                    historical["physical"]["node_count"],
                    bytes32(historical["physical"]["locator_digest"]),
                    bytes32(historical["physical"]["identity_digest"]),
                ],
            ],
            [],
        ],
        "current surface canonical inventory closure drifted",
    )
    require(
        consumers.get("canonical_value")
        == [
            1,
            bytes32(consumer_inventory_digest(inventory)),
            [
                [
                    bytes32(row["resource_id"]),
                    row["reader_locator"],
                    bytes32(row["reader_content_sha256"]),
                    row["consumer_kind"],
                    row["evidence_kind"],
                    row["role"],
                ]
                for row in expected_reader_rows
            ],
        ],
        "current consumer canonical closure drifted",
    )
    require(
        surface.get("generated_output_policy")
        == {
            "classification": "post_release_noncanonical",
            "path_byte_and_presence_identity_participation": False,
            "root_worker_post_root_delta_owner": True,
        },
        "generated-output identity-isolation policy drifted",
    )


def consumer_inventory_digest(inventory: Any) -> str:
    rows = [
        [
            row.reader_locator,
            bytes32(row.reader_content_sha256),
            row.semantic_owner.value,
            row.kind.value,
            row.evidence_kind.value,
            row.role.value,
            row.resource_stable_key,
            bytes32(row.resource_candidate_id),
            row.disposition.value,
            bytes32(sha256(row.evidence.encode())),
        ]
        for row in inventory.direct_readers
    ]
    return profile_commitment_value([1, "exact-current-consumer-inventory", rows])


def validate_schema_and_migration_audits(
    documents: Mapping[str, Any], inventory: Any, resources: Sequence[ResourceDescriptor]
) -> None:
    by_locator = {
        candidate.stable_locator: resources[candidate.inventory_ordinal - 1]
        for candidate in inventory.resources
    }
    schemas = (
        (
            "current-persistence-manifest.v1.json",
            "maestro.vnext.current-persistence-manifest.v1",
            sorted(
                [path.relative_to(ROOT).as_posix() for path in (ROOT / "embedded/schemas").glob("*/current.yaml")]
                + [path.relative_to(ROOT).as_posix() for path in (ROOT / "embedded/schemas").glob("*/supported.yaml")]
            ),
            2,
            "current_and_supported_persistence_contract",
        ),
        (
            "current-archive-manifest.v1.json",
            "maestro.vnext.current-archive-manifest.v1",
            sorted(path.relative_to(ROOT).as_posix() for path in (ROOT / "embedded/schemas").glob("*/retired.yaml")),
            2,
            "retired_name_archive_contract",
        ),
        (
            "golden-fixture-manifest.v1.json",
            "maestro.vnext.golden-fixture-manifest.v1",
            sorted(path.relative_to(ROOT).as_posix() for path in (ROOT / "embedded/schemas").glob("*/fixtures/*") if path.is_file()),
            3,
            "golden_fixture_runtime_reader_contract",
        ),
    )
    expected_counts = [22, 11, 22]
    for (name, schema, paths, reader_count, purpose), expected_count in zip(schemas, expected_counts, strict=True):
        document = documents[name]
        validate_stage0_commitment(document, schema)
        require(len(paths) == expected_count and document.get("exact_count") == expected_count, f"{schema} exact count drifted")
        require(document.get("purpose") == purpose, f"{schema} purpose drifted")
        rows = document.get("rows")
        require(isinstance(rows, list) and [row["path"] for row in rows] == paths, f"{schema} exact path set drifted")
        for row in rows:
            path = row["path"]
            resource = by_locator.get(path)
            require(resource is not None and row["resource_id"] == resource.resource_id, f"{schema} ResourceId binding drifted: {path}")
            require(row["content_sha256"] == file_sha(path), f"{schema} content SHA drifted: {path}")
            readers = row.get("readers")
            require(isinstance(readers, list) and len(readers) == reader_count, f"{schema} exact reader set drifted")
            for reader in readers:
                locator, _, symbol = reader["reader_locator"].partition("#")
                require(file_sha(locator) == reader["reader_content_sha256"], f"{schema} reader SHA drifted")
                require(symbol in (ROOT / locator).read_text(), f"{schema} reader symbol disappeared")
                require(reader.get("evidence_kind") == "typed_runtime_or_fixture_reader", f"{schema} reader evidence kind drifted")
        require(document.get("reader_set") == rows[0]["readers"], f"{schema} declared reader set drifted")
        require(
            document.get("canonical_value")
            == [
                1,
                purpose,
                [
                    [
                        row["resource_tag"],
                        bytes32(row["resource_id"]),
                        row["path"],
                        bytes32(row["content_sha256"]),
                        [
                            [reader["reader_locator"], bytes32(reader["reader_content_sha256"])]
                            for reader in row["readers"]
                        ],
                    ]
                    for row in rows
                ],
            ],
            f"{schema} canonical exact manifest drifted",
        )

    migration = documents["migration-rollback-requirements.v1.json"]
    validate_stage0_commitment(migration, "maestro.vnext.migration-rollback-requirements.v1")
    rows = migration.get("requirements")
    require(isinstance(rows, list) and len(rows) == 377, "migration/rollback exact Resource coverage drifted")
    require(
        migration.get("stage0_execution_complete") is False
        and migration.get("stage0_rehearsal_complete") is False
        and migration.get("runtime_proof_complete") is False,
        "migration/rollback audit falsely claims Stage-0 runtime completion",
    )
    require(
        migration.get("stage") == "stage0_candidate_only"
        and migration.get("status") == "requirements_complete_runtime_proof_pending"
        and migration.get("proof_status") == "pending_stage0_execution_and_rehearsal"
        and migration.get("pending_runtime_proof_count") == 377,
        "migration/rollback pending Stage-0 proof boundary drifted",
    )
    descriptor_records = documents["resource-descriptors"]["resources"]
    for resource, descriptor, row in zip(resources, descriptor_records, rows, strict=True):
        require(row["resource_tag"] == resource.resource_tag and row["resource_id"] == resource.resource_id, "migration row Resource coordinate drifted")
        require(row["stable_resource_key"] == descriptor["stable_resource_key"] and row["stable_locator"] == descriptor["stable_locator"], "migration row stable Resource coordinate drifted")
        require(row["content_sha256"] == resource.value[2]["bytes"], "migration row is not content-bound")
        require(row["disposition"] == descriptor["disposition"], "migration row disposition differs from Resource")
        require(row["migration_profile_id"] == resource.value[17]["bytes"], "migration profile differs from Resource")
        require(row["rollback_profile_id"] == resource.value[18]["bytes"], "rollback profile differs from Resource")
        require(row["removal_profile_id"] == resource.value[21]["bytes"], "removal profile differs from Resource")
        require(row["proof_profile_id"] == resource.value[23]["bytes"], "proof profile differs from Resource")
        require(
            row["migration_execution_status"] == "pending_stage0_execution"
            and row["rollback_rehearsal_status"] == "pending_stage0_rehearsal"
            and row["removal_execution_status"] == "pending_stage0_execution"
            and row["runtime_proof_complete"] is False,
            "migration/rollback row falsely claims execution proof",
        )
        require(row["reader_evidence"] == descriptor["reader_evidence"], "migration/removal reader evidence differs from Resource")
    require(
        migration.get("canonical_value")
        == [
            1,
            [
                [
                    row["resource_tag"],
                    bytes32(row["resource_id"]),
                    bytes32(row["migration_profile_id"]),
                    bytes32(row["rollback_profile_id"]),
                    bytes32(row["removal_profile_id"]),
                    bytes32(row["proof_profile_id"]),
                    DISPOSITION_TAGS[row["disposition"]],
                    row["migration_execution_status"],
                    row["rollback_rehearsal_status"],
                    row["removal_execution_status"],
                    row["runtime_proof_complete"],
                ]
                for row in rows
            ],
            False,
        ],
        "migration/rollback canonical exact Resource requirements drifted",
    )


def expected_direct_consumer_records(
    inventory: Any, resources: Sequence[ResourceDescriptor]
) -> list[dict[str, Any]]:
    resource_by_key = {
        candidate.stable_key: resources[candidate.inventory_ordinal - 1]
        for candidate in inventory.resources
    }
    grouped: dict[tuple[str, str, str, str, str, str], list[tuple[Any, ResourceDescriptor]]] = {}
    for reader in inventory.direct_readers:
        resource = resource_by_key[reader.resource_stable_key]
        if resource.disposition == "Remove":
            continue
        key = (
            reader.reader_locator,
            reader.reader_content_sha256,
            reader.semantic_owner.value,
            reader.kind.value,
            reader.role.value,
            resource.disposition,
        )
        grouped.setdefault(key, []).append((reader, resource))
    records = []
    for key in sorted(grouped):
        locator, reader_sha, owner, kind, role, disposition = key
        pairs = sorted(grouped[key], key=lambda pair: pair[1].resource_tag)
        evidence_rows = [canonical_reader_coordinate(pair[0]) for pair in pairs]
        records.append(
            {
                "locator": locator,
                "reader_content_sha256": reader_sha,
                "owner": owner,
                "owner_tag": READER_OWNER_TAGS[owner],
                "consumer_kind": kind,
                "reader_role": role,
                "disposition": disposition,
                "resource_pairs": [
                    {"resource_tag": resource.resource_tag, "resource_id": resource.resource_id}
                    for _, resource in pairs
                ],
                "provenance_commitment_id": profile_commitment_value(
                    [1, "direct-consumer-provenance", locator, bytes32(reader_sha), evidence_rows]
                ),
                "migration_profile_id": profile_commitment_value(
                    [1, "direct-consumer-migration", locator, role, "pending_stage0_execution"]
                ),
                "proof_profile_id": profile_commitment_value(
                    [1, "direct-consumer-proof", locator, evidence_rows]
                ),
                "removal_profile_id": profile_commitment_value(
                    [1, "direct-consumer-removal", locator, "pending_stage0_execution"]
                ),
            }
        )
    return records


def validate_bundle_census_release(
    documents: Mapping[str, Any], inventory: Any, resources: Sequence[ResourceDescriptor]
) -> tuple[list[BundleManifest], ReleaseResourceCensus, EmbeddedReleaseBundle]:
    bundle_documents = documents["bundles"]
    require(isinstance(bundle_documents, list) and len(bundle_documents) == 8, "exact eight concrete Bundle artifacts are missing")
    bundles: list[BundleManifest] = []
    exact_groups = (
        "Migration:default",
        "ExternalPattern:first-party-neutral-baseline",
        "ExternalPattern:third-party-awesome-design-md",
        "SharedContract:default",
        "Orchestration:default",
        "Capability:default",
        "Adapter:default",
        "AgentBootstrap:default",
    )
    exact_kinds = (
        "Migration",
        "ExternalPattern",
        "ExternalPattern",
        "SharedContract",
        "Orchestration",
        "Capability",
        "Adapter",
        "AgentBootstrap",
    )
    for index, (name, document, group, kind) in enumerate(
        zip(BUNDLE_NAMES, bundle_documents, exact_groups, exact_kinds, strict=True), 1
    ):
        validate_manifest_document(
            document,
            schema="maestro.vnext.bundle.manifest.v1",
            identity_name="bundle_id",
            cbor_name=f"{name}.cbor",
        )
        require(document.get("bundle_tag") == index and document.get("bundle_kind") == kind, "Bundle tag/kind topology drifted")
        require(document.get("stable_bundle_group") == group, "concrete Bundle instance group drifted")
        bundles.append(bundle_from_document(document))
    require(tuple(bundle.bundle_kind for bundle in bundles) == exact_kinds, "Bundle topology/repetition drifted")
    expected_dependencies = ((), (), (), (), (bundles[3].bundle_id,), (bundles[4].bundle_id,), (), ())
    require(
        tuple(bundle.dependency_bundle_ids for bundle in bundles) == expected_dependencies,
        "Bundle dependencies are not the exact locked strict-backward subsets",
    )
    group_by_tag = {
        candidate.inventory_ordinal: candidate.target_bundle_group for candidate in inventory.resources
    }
    for bundle, group in zip(bundles, exact_groups, strict=True):
        member_tags = [row[0] for row in bundle.value[1]]
        require(all(group_by_tag[tag] == group for tag in member_tags), "Bundle owns a Resource from another concrete group")

    census_document = documents["census"]
    validate_manifest_document(
        census_document,
        schema="maestro.vnext.release-resource-census.manifest.v1",
        identity_name="census_id",
        cbor_name="release-resource-census.v1.cbor",
    )
    release_document = documents["release"]
    validate_manifest_document(
        release_document,
        schema="maestro.vnext.embedded-release-bundle.manifest.v1",
        identity_name="release_id",
        cbor_name="embedded-release-bundle.v1.cbor",
    )
    require(release_document.get("sole_release_root") is True, "EmbeddedReleaseBundle is not the sole Release root")
    require(not ({"state", "runtime", "release_state"} & set(release_document)), "Release artifact contains synthetic state")
    census = census_from_document(census_document)
    release = release_from_document(release_document)
    try:
        validate_release_closure(resources=resources, bundles=bundles, census=census, release=release)
    except ContractError as error:
        raise ValidationError(str(error)) from error
    require(census_document.get("source_inventory_digest") == inventory_hash(inventory), "Census source inventory digest drifted")
    require(census_document.get("consumer_inventory_digest") == consumer_inventory_digest(inventory), "Census consumer inventory digest drifted")
    header = census.value[0]
    require(header[8] == bytes32(inventory_hash(inventory)), "Census header source inventory digest drifted")
    require(header[9] == bytes32(consumer_inventory_digest(inventory)), "Census header consumer inventory digest drifted")
    resource_rows = census.value[1][: len(resources)]
    expected_locators = [candidate.stable_locator for candidate in inventory.resources]
    require(
        [row[2][1][1][0] for row in resource_rows] == expected_locators,
        "Census locator is not the exact separate current Resource locator",
    )
    direct_records = census_document.get("direct_consumers")
    require(isinstance(direct_records, list), "Census direct-consumer evidence records are missing")
    require(
        direct_records == expected_direct_consumer_records(inventory, resources),
        "Census direct-consumer records differ from the live typed consumer registry",
    )
    non_remove = {
        resources[candidate.inventory_ordinal - 1].resource_id
        for candidate in inventory.resources
        if candidate.disposition != ResourceDisposition.REMOVE
    }
    recorded = {
        pair["resource_id"]
        for row in direct_records
        for pair in row["resource_pairs"]
    }
    require(recorded == non_remove, "Census direct-consumer records do not cover exact non-Remove Resources")
    for row in direct_records:
        locator = row["locator"].split("#line:", 1)[0].split("#", 1)[0]
        if locator in {"tsconfig.json"}:
            locator = "tsconfig.json"
        require((ROOT / locator).is_file(), f"Census reader locator is not live: {row['locator']}")
        require(file_sha(locator) == row["reader_content_sha256"], f"Census reader hash drifted: {row['locator']}")
    return bundles, census, release


def domain_identity(domain: str, value: Any) -> str:
    return sha256(encode_cbor([domain, value]))


def expected_manifest_identities() -> dict[str, tuple[str | None, str, str]]:
    catalog_inventory = json.loads((ROOT / "contracts/vnext/catalogs/generated/inventory.json").read_text())
    grammar_id = exact_hash(catalog_inventory["grammar_id"], "grammar identity")
    catalog_artifacts = [row for row in catalog_inventory["artifacts"] if row["kind"] != "grammar"]
    catalog_ids = [exact_hash(row["identity"], "catalog identity") for row in catalog_artifacts]
    public_path = "contracts/vnext/stage0/public-identity/public-identity-closure.v1.json"
    public = json.loads((ROOT / public_path).read_text())
    public_manifest = public["manifest"]
    public_manifest_id = exact_hash(
        public_manifest["manifest_id"] if isinstance(public_manifest, dict) else public_manifest,
        "public ManifestId",
    )
    c868 = load_json("c868-successor.v1.json")
    writer = load_json("writer-compatibility-successor.v1.json")
    decision_path = "contracts/vnext/stage0/decision-closure/decision-closure.v1.json"
    decision = json.loads((ROOT / decision_path).read_text())
    d116 = next(row for row in decision["records"] if row["id"] == "dec-canonical-typed-recoverreserved-d116")
    final_path = "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"
    final = json.loads((ROOT / final_path).read_text())
    result: dict[str, tuple[str | None, str, str]] = {
        "manifest:public-identity": (None, public_manifest_id, "Introduce"),
        "manifest:catalog-profile-grammar": (OLD_GRAMMAR_ID, grammar_id, "Rotate"),
        "manifest:c868-resource-contract-suite": (
            "5057a0a8e088a0dd394106b928f9b1c4c7a356040b23363455de1177e33e013f",
            exact_hash(c868["manifest_id"]),
            "Rotate",
        ),
        "manifest:c868-runtime-edge-contract": (
            "917376f49f5ed01ab53a7a71f1527fc0b3fc03d2632b47b68333cf2ba7899fe2",
            "917376f49f5ed01ab53a7a71f1527fc0b3fc03d2632b47b68333cf2ba7899fe2",
            "Preserve",
        ),
        "manifest:writer-compatibility": (
            "60e9a3a77104b74f044527232802841e230e192279881286c0a2a9d3618be2c6",
            exact_hash(writer["manifest_id"]),
            "Rotate",
        ),
    }
    for predecessor, artifact in zip(OLD_CATALOG_IDS, catalog_artifacts, strict=True):
        result[f"manifest:catalog:{artifact['kind']}"] = (
            predecessor,
            exact_hash(artifact["identity"]),
            "Rotate",
        )
    for key in (
        "schema_read_write_set_descriptor_id",
        "writer_protocol_epoch_id",
        "migration_epoch_id",
        "finality_edge_manifest_id",
    ):
        predecessor = exact_hash(writer["predecessor_components"][key])
        successor = exact_hash(writer[key])
        result[f"manifest:writer:{key}"] = (
            predecessor,
            successor,
            "Preserve" if predecessor == successor else "Rotate",
        )
    public_closure_id = exact_hash(public["closure_id"])
    public_successor_id = domain_identity(
        "maestro.vnext.7138-public-contract-successor.v1",
        [1, bytes32(public_closure_id), bytes32(c868["manifest_id"])],
    )
    catalog_closure_id = domain_identity(
        "maestro.vnext.efa0-core-catalog-closure.v1",
        [1, bytes32(grammar_id), [bytes32(identity) for identity in catalog_ids]],
    )
    d116_id = domain_identity(
        "maestro.vnext.d116-bounded-recovery-successor.v1",
        [1, bytes32(d116["raw_body_sha256"]), [bytes32(identity) for identity in catalog_ids]],
    )
    result.update(
        {
            "manifest:public-transport-7138": (None, public_successor_id, "Introduce"),
            "manifest:bounded-recovery-d116": (None, d116_id, "Introduce"),
            "manifest:catalog-owner-efa0": (None, catalog_closure_id, "Introduce"),
            "manifest:effect-control-h2": (None, exact_hash(final["h2_manifest_identity"]), "Introduce"),
            "manifest:effect-withdrawal-h3": (None, exact_hash(final["h3_withdrawal_identity"]), "Introduce"),
            "manifest:effect-finalization": (None, exact_hash(final["identity"]), "Introduce"),
            "manifest:effect-expected-delta": (None, exact_hash(final["expected_delta_manifest_id"]), "Introduce"),
            "manifest:effect-consumer-census": (None, exact_hash(final["semantic_consumer_census_id"]), "Introduce"),
        }
    )
    require(len(result) == 26, "exact 26 through-Release Manifest identities changed")
    return result


def validate_delta(
    documents: Mapping[str, Any],
    resources: Sequence[ResourceDescriptor],
    bundles: Sequence[BundleManifest],
    census: ReleaseResourceCensus,
    release: EmbeddedReleaseBundle,
) -> None:
    delta = documents["delta"]
    raw = validate_stage0_commitment(delta, "maestro.vnext.migration-cutover-expected-delta-successor.v1")
    require((OUT / "expected-delta-successor.v1.cbor").read_bytes() == raw, "through-Release delta sibling CBOR drifted")
    require(delta.get("exact_identity_kind_counts") == {
        "Schema": 117,
        "Manifest": 26,
        "Resource": 377,
        "Bundle": 8,
        "Census": 1,
        "Release": 1,
    }, "through-Release identity-kind counts drifted")
    entries = delta.get("entries")
    require(isinstance(entries, list) and len(entries) == 530, "through-Release exact entry closure drifted")
    keys = [(row.get("identity_kind"), row.get("logical_key")) for row in entries]
    kind_tags = {name: index for index, name in enumerate((*IDENTITY_KINDS, "RootInput", "HandoffInput"), 1)}
    require(keys == sorted(keys, key=lambda row: (kind_tags[row[0]], row[1])), "through-Release entries are not canonical ordered")
    require(len(keys) == len(set(keys)), "through-Release entry keys are not unique")
    for row in entries:
        predecessor = row.get("predecessor_identity")
        successor = exact_hash(row.get("successor_identity"), "delta successor")
        disposition = row.get("disposition")
        if disposition == "Introduce":
            require(predecessor is None, "Introduce delta row has a predecessor")
        elif disposition == "Preserve":
            require(predecessor is not None and exact_hash(predecessor) == successor, "Preserve delta row changes identity")
        elif disposition == "Rotate":
            require(predecessor is not None and exact_hash(predecessor) != successor, "Rotate delta row preserves identity")
        else:
            raise ValidationError(f"unsupported through-Release disposition: {disposition}")
        recorded_source_sha = exact_hash(row.get("source_artifact_sha256"), "delta source artifact SHA")
        source_artifact = row.get("source_artifact")
        require(isinstance(source_artifact, str) and source_artifact, "delta source artifact locator is missing")
        if source_artifact == "frozen:vnext-resource-contract-suite-v1.json":
            require(recorded_source_sha == FROZEN_SOURCE_SHA256["suite"], "frozen C868 delta source SHA drifted")
        else:
            require((ROOT / source_artifact).is_file(), f"delta source artifact is not live: {source_artifact}")
            require(file_sha(source_artifact) == recorded_source_sha, f"delta source artifact SHA drifted: {source_artifact}")

    by_kind: dict[str, list[dict[str, Any]]] = {
        kind: [row for row in entries if row["identity_kind"] == kind] for kind in IDENTITY_KINDS
    }
    public = json.loads((ROOT / "contracts/vnext/stage0/public-identity/public-identity-closure.v1.json").read_text())
    public_schemas = {
        f"schema:public:{row['schema_name']}@{row['schema_version']}": exact_hash(row["schema_id"])
        for row in public["schema_descriptors"]
    }
    c868_schemas = {f"schema:c868:{name}@1": identity for name, identity in FROZEN_SCHEMA_IDS.items()}
    schema_rows = {row["logical_key"]: row for row in by_kind["Schema"]}
    require(set(schema_rows) == set(public_schemas) | set(c868_schemas), "Schema delta exact-set coverage drifted")
    for key, identity in public_schemas.items():
        require(schema_rows[key]["predecessor_identity"] is None and exact_hash(schema_rows[key]["successor_identity"]) == identity and schema_rows[key]["disposition"] == "Introduce", f"public Schema delta drifted: {key}")
    for key, identity in c868_schemas.items():
        require(exact_hash(schema_rows[key]["predecessor_identity"]) == identity and exact_hash(schema_rows[key]["successor_identity"]) == identity and schema_rows[key]["disposition"] == "Preserve", f"C868 Schema Preserve delta drifted: {key}")

    manifest_expected = expected_manifest_identities()
    manifest_rows = {row["logical_key"]: row for row in by_kind["Manifest"]}
    require(set(manifest_rows) == set(manifest_expected), "Manifest delta exact-set coverage drifted")
    for key, (predecessor, successor, disposition) in manifest_expected.items():
        row = manifest_rows[key]
        require((exact_hash(row["predecessor_identity"]) if row["predecessor_identity"] else None) == predecessor, f"Manifest predecessor drifted: {key}")
        require(exact_hash(row["successor_identity"]) == successor and row["disposition"] == disposition, f"Manifest successor/disposition drifted: {key}")

    resource_rows = {row["logical_key"]: row for row in by_kind["Resource"]}
    expected_resource_rows = {
        f"resource:{record['stable_resource_key']}": resource.resource_id
        for record, resource in zip(documents["resource-descriptors"]["resources"], resources, strict=True)
    }
    require(set(resource_rows) == set(expected_resource_rows), "Resource delta exact-set coverage drifted")
    require(all(resource_rows[key]["predecessor_identity"] is None and exact_hash(resource_rows[key]["successor_identity"]) == identity for key, identity in expected_resource_rows.items()), "Resource Introduce delta drifted")
    bundle_rows = {row["logical_key"]: row for row in by_kind["Bundle"]}
    expected_bundle_rows = {
        f"bundle:{document['stable_bundle_group']}": bundle.bundle_id
        for document, bundle in zip(documents["bundles"], bundles, strict=True)
    }
    require(set(bundle_rows) == set(expected_bundle_rows), "Bundle delta exact-set coverage drifted")
    require(all(bundle_rows[key]["predecessor_identity"] is None and exact_hash(bundle_rows[key]["successor_identity"]) == identity for key, identity in expected_bundle_rows.items()), "Bundle Introduce delta drifted")
    require(len(by_kind["Census"]) == 1 and by_kind["Census"][0]["predecessor_identity"] is None and exact_hash(by_kind["Census"][0]["successor_identity"]) == census.census_id, "Census Introduce delta drifted")
    require(len(by_kind["Release"]) == 1 and by_kind["Release"][0]["predecessor_identity"] is None and exact_hash(by_kind["Release"][0]["successor_identity"]) == release.release_id, "Release Introduce delta drifted")

    obligations = delta.get("downstream_obligations")
    expected_obligations = [
        ("RootInput", "candidate-root"),
        ("RootInput", "candidate-finalization"),
        ("HandoffInput", "candidate-handoff"),
    ]
    require(isinstance(obligations, list) and [(row["identity_kind"], row["logical_key"]) for row in obligations] == expected_obligations, "downstream null obligation set/order drifted")
    for row in obligations:
        require(row["predecessor_identity"] is None and row["successor_identity"] is None, "downstream obligation identity is not exact null")
        require(row["depends_on_release_identity"] == f"sha256:{release.release_id}", "downstream obligation does not bind exact ReleaseId")
        require(row["status"] == "pending_downstream_stage0_producer" and row["owner"] == "candidate-root-worker", "downstream obligation owner/status drifted")
    require(delta.get("post_root_delta_identity") is None and delta.get("post_root_union_identity") is None, "root-worker post-root outputs were guessed")
    require(delta.get("post_root_identity_feedback_into_resource_bundle_census_release") is False, "post-root identity feedback was admitted")
    canonical = delta["canonical_value"]
    require(
        canonical[3]
        == [
            [
                kind_tags[row["identity_kind"]],
                row["logical_key"],
                [0],
                [0],
                1,
                bytes32(release.release_id),
                row["status"],
                row["owner"],
            ]
            for row in obligations
        ],
        "downstream obligation canonical owner/status coordinates drifted",
    )
    require(canonical[-2:] == [[0], [0]], "post-root nulls are not canonical optionals")
    require(all(row[2:4] == [[0], [0]] for row in canonical[3]), "downstream obligation nulls are not canonical optionals")


def ast_binding(locator: str, symbol: str, literal: str) -> dict[str, Any]:
    path = ROOT / locator
    tree = ast.parse(path.read_text(), filename=locator)
    matches = []
    for node in tree.body:
        targets = node.targets if isinstance(node, ast.Assign) else [node.target] if isinstance(node, ast.AnnAssign) else []
        if any(isinstance(target, ast.Name) and target.id == symbol for target in targets):
            matches.append(node)
    require(len(matches) == 1, f"generated-output reader symbol missing/duplicate: {locator}#{symbol}")
    strings = {node.value for node in ast.walk(matches[0]) if isinstance(node, ast.Constant) and isinstance(node.value, str)}
    require(literal in strings, f"generated-output reader no longer binds exact path: {locator}#{symbol}")
    return {
        "reader_locator": f"{locator}#{symbol}",
        "reader_content_sha256": sha256(path.read_bytes()),
        "evidence_kind": "python_ast_exact_string_constant",
        "literal": literal,
    }


def validate_preidentity_artifacts() -> None:
    frozen = validate_frozen_inputs_live()
    catalog_inventory = json.loads((ROOT / "contracts/vnext/catalogs/generated/inventory.json").read_text())
    grammar_id = exact_hash(catalog_inventory["grammar_id"], "catalog grammar identity")
    catalog_ids = [
        exact_hash(row["identity"], "catalog identity")
        for row in catalog_inventory["artifacts"]
        if row["kind"] != "grammar"
    ]
    require(len(catalog_ids) == 9, "exact nine-catalog closure drifted")
    c868 = load_json("c868-successor.v1.json")
    require(c868.get("schema") == "maestro.vnext.c868-resource-contract-successor.v1", "C868 successor schema drifted")
    require(c868.get("identity_protocol") == "ManifestIdentityV1", "C868 successor protocol drifted")
    require(c868.get("candidate_only") is True and c868.get("runtime_activation") is False, "C868 successor state drifted")
    require(
        c868.get("predecessor")
        == {
            "manifest_id": "5057a0a8e088a0dd394106b928f9b1c4c7a356040b23363455de1177e33e013f",
            "artifact_sha256": FROZEN_SOURCE_SHA256["suite"],
        },
        "C868 predecessor receipt drifted",
    )
    require(
        c868.get("runtime_edge_manifest_id")
        == "917376f49f5ed01ab53a7a71f1527fc0b3fc03d2632b47b68333cf2ba7899fe2",
        "C868 runtime-edge identity drifted",
    )
    require(c868.get("exact_counts") == {"schemas": 38, "suite_components": 62, "runtime_edges": 61}, "C868 exact counts drifted")
    c868_envelope = c868.get("manifest_identity_envelope")
    require(isinstance(c868_envelope, list) and len(c868_envelope) == 5 and not contains_null(c868_envelope), "C868 successor is not an exact five-slot manifest")
    c868_replacements = {
        OLD_GRAMMAR_ID: grammar_id,
        **dict(zip(OLD_CATALOG_IDS, catalog_ids, strict=True)),
    }
    expected_c868_envelope = replace_bytes32(copy.deepcopy(frozen["manifest_identity_envelope"]), c868_replacements)
    descriptor_domain = frozen["descriptor_domains"][3]
    descriptor_schema_id = expected_c868_envelope[2]["bytes"]
    for row in expected_c868_envelope[4]:
        row[1] = bytes32(sha256(encode_cbor([descriptor_domain, bytes32(descriptor_schema_id), row[2]])))
    require(c868_envelope == expected_c868_envelope, "C868 successor is not the exact catalog-rotated predecessor")
    c868_id, c868_raw = identity_digest(c868_envelope)
    require(exact_hash(c868.get("manifest_id"), "C868 successor ManifestId") == c868_id, "C868 successor identity drifted")
    require(c868.get("canonical_cbor_sha256") == c868_id and c868.get("canonical_cbor_byte_length") == len(c868_raw), "C868 successor CBOR receipt drifted")
    require((OUT / "c868-successor.v1.cbor").read_bytes() == c868_raw, "C868 successor sibling CBOR drifted")
    require(frozen["manifest_id"] == "5057a0a8e088a0dd394106b928f9b1c4c7a356040b23363455de1177e33e013f", "frozen C868 reproduction drifted")
    require(c868.get("catalog_profile_grammar_id") == grammar_id and c868.get("catalog_manifest_ids") == catalog_ids, "C868 catalog successor coordinates drifted")
    require((OUT / "c868-successor.v1.json").read_bytes() == json_bytes(c868), "C868 successor JSON rendering is not canonical")

    writer = load_json("writer-compatibility-successor.v1.json")
    require(writer.get("schema") == "maestro.vnext.migration-cutover-writer-compatibility-successor.v1", "writer successor schema drifted")
    require(writer.get("identity_protocol") == "ManifestIdentityV1", "writer successor protocol drifted")
    require(writer.get("candidate_only") is True and writer.get("runtime_activation") is False, "writer successor state drifted")
    require(
        writer.get("predecessor")
        == {
            "manifest_id": "60e9a3a77104b74f044527232802841e230e192279881286c0a2a9d3618be2c6",
            "artifact_sha256": "f9a2ecbff7b8b1912b78ed7c6b028eb0d9c3bdba92e0d9ac8f0377214e8150d9",
        },
        "writer predecessor receipt drifted",
    )
    writer_envelope = writer.get("manifest_identity_envelope")
    require(isinstance(writer_envelope, list) and len(writer_envelope) == 5 and not contains_null(writer_envelope), "writer successor is not an exact five-slot manifest")
    c65_path = FROZEN / "vnext-migration-cutover-contract-v1.json"
    require(c65_path.is_file() and sha256(c65_path.read_bytes()) == "f9a2ecbff7b8b1912b78ed7c6b028eb0d9c3bdba92e0d9ac8f0377214e8150d9", "frozen 65b3 source drifted")
    c65 = json.loads(c65_path.read_text())
    writer_replacements = {
        OLD_GRAMMAR_ID: grammar_id,
        "5057a0a8e088a0dd394106b928f9b1c4c7a356040b23363455de1177e33e013f": c868_id,
        FROZEN_SOURCE_SHA256["suite"]: sha256(json_bytes(c868)),
        **dict(zip(OLD_CATALOG_IDS, catalog_ids, strict=True)),
    }
    read_write = replace_bytes32(copy.deepcopy(c65["schema_read_write_set"]["identity_envelope"]), writer_replacements)
    read_write_id = sha256(encode_cbor(read_write))
    writer_replacements[c65["schema_read_write_set_descriptor_id"]] = read_write_id
    writer_epoch = replace_bytes32(copy.deepcopy(c65["writer_protocol_epoch"]["identity_envelope"]), writer_replacements)
    writer_epoch_id = sha256(encode_cbor(writer_epoch))
    writer_replacements[c65["writer_protocol_epoch_id"]] = writer_epoch_id
    migration_epoch = replace_bytes32(copy.deepcopy(c65["migration_epoch"]["identity_envelope"]), writer_replacements)
    migration_epoch_id = sha256(encode_cbor(migration_epoch))
    writer_replacements[c65["migration_epoch_id"]] = migration_epoch_id
    expected_writer_envelope = replace_bytes32(copy.deepcopy(c65["manifest_identity_envelope"]), writer_replacements)
    writer_descriptor_schema = expected_writer_envelope[2]["bytes"]
    for row in expected_writer_envelope[4]:
        row[1] = bytes32(
            sha256(encode_cbor([c65["descriptor_domain"], bytes32(writer_descriptor_schema), row[2]]))
        )
    require(writer_envelope == expected_writer_envelope, "writer successor is not the exact catalog/C868-rotated predecessor")
    writer_id, writer_raw = identity_digest(writer_envelope)
    require(exact_hash(writer.get("manifest_id"), "writer successor ManifestId") == writer_id, "writer successor identity drifted")
    require(writer.get("canonical_cbor_sha256") == writer_id and writer.get("canonical_cbor_byte_length") == len(writer_raw), "writer successor CBOR receipt drifted")
    require((OUT / "writer-compatibility-successor.v1.cbor").read_bytes() == writer_raw, "writer successor sibling CBOR drifted")
    require(
        [
            writer.get("schema_read_write_set_descriptor_id"),
            writer.get("writer_protocol_epoch_id"),
            writer.get("migration_epoch_id"),
            writer.get("finality_edge_manifest_id"),
        ]
        == [
            read_write_id,
            writer_epoch_id,
            migration_epoch_id,
            c65["finality_edge_contract"]["manifest_id"],
        ],
        "writer successor component identities drifted",
    )
    require((OUT / "writer-compatibility-successor.v1.json").read_bytes() == json_bytes(writer), "writer successor JSON rendering is not canonical")
    require(
        writer.get("exact_counts")
        == {
            "schemas": 12,
            "invariants": 23,
            "predecessors": 10,
            "components": 50,
            "finality_edges": 11,
            "read_write_cohorts": 4,
            "rows_per_cohort": 46,
        },
        "writer successor exact counts drifted",
    )
    require(
        (OUT / "predecessor-resource-contract-suite-v1.json").read_bytes()
        == (FROZEN / "vnext-resource-contract-suite-v1.json").read_bytes()
        and (OUT / "predecessor-resource-contract-suite-v1.cbor").read_bytes()
        == bytes.fromhex(frozen["cbor_hex"]),
        "frozen C868 predecessor copies drifted",
    )
    require(
        (OUT / "predecessor-migration-cutover-contract-v1.json").read_bytes() == c65_path.read_bytes()
        and (OUT / "predecessor-migration-cutover-contract-v1.cbor").read_bytes()
        == bytes.fromhex(c65["cbor_hex"]),
        "frozen 65b3 predecessor copies drifted",
    )

    public = json.loads((ROOT / "contracts/vnext/public/public_contracts.v1.json").read_text())
    capability_source = "capability_method_contracts.v1.json"
    require(capability_source in public["semantic_artifacts"], "public closure no longer admits capability relations")
    capability = json.loads((ROOT / f"contracts/vnext/public/{capability_source}").read_text())
    tree = json.loads((ROOT / "embedded/vnext/capability/instruction-tree.v1.json").read_text())
    relation = capability["job_method"]
    expected_relations = {
        "schema": "maestro.vnext.capability-instruction-relations.v1",
        "candidate_only": True,
        "runtime_activation": False,
        "instruction_resource_count": len(tree["logical_paths"]),
        "job_method_rows": {
            job: [row["method"] for row in relation["rows"] if row["job"] == job and row["admitted"]]
            for job in capability["jobs"]
        },
        "positive_job_method_edges": relation["positive"],
        "negative_job_method_edges": relation["negative"],
    }
    expected_evaluator = {
        "schema": "maestro.vnext.capability-instruction-evaluator.v1",
        "candidate_only": True,
        "runtime_activation": False,
        "closed_jobs": capability["jobs"],
        "closed_methods": capability["direct_methods"],
        "instruction_resource_count": len(tree["logical_paths"]),
        "selection_outcomes": ["Selected", "Ambiguous", "Blocked"],
        "authority": "none",
    }
    require(load_json("capability-relations.v1.json") == expected_relations, "capability relation Resource drifted")
    require(load_json("capability-evaluator.v1.json") == expected_evaluator, "capability evaluator Resource drifted")
    require((OUT / "capability-relations.v1.json").read_bytes() == json_bytes(expected_relations), "capability relation JSON is not canonical")
    require((OUT / "capability-evaluator.v1.json").read_bytes() == json_bytes(expected_evaluator), "capability evaluator JSON is not canonical")

    vendor_root = ROOT / "embedded/design/vendor/awesome-design-md"
    vendor_rows = [
        {"path": path.relative_to(ROOT).as_posix(), "sha256": sha256(path.read_bytes())}
        for path in sorted(vendor_root.rglob("DESIGN.md"))
    ]
    require(len(vendor_rows) == 74, "optional vendor DESIGN.md closure drifted")
    vendor_digest = sha256(
        b"".join(f"{row['path']}\0{row['sha256']}\n".encode() for row in vendor_rows)
    )
    expected_vendor = {
        "schema": "maestro.vnext.optional-vendor-reference-pack.v1",
        "candidate_only": True,
        "runtime_activation": False,
        "optional": True,
        "tree_sha256": vendor_digest,
        "license_path": "embedded/design/vendor/awesome-design-md/LICENSE",
        "provenance_manifest_path": "embedded/design/vendor/awesome-design-md/manifest.yml",
        "files": vendor_rows,
    }
    require(load_json("vendor-reference-pack.v1.json") == expected_vendor, "vendor reference-pack Resource drifted")
    require((OUT / "vendor-reference-pack.v1.json").read_bytes() == json_bytes(expected_vendor), "vendor reference-pack JSON is not canonical")


def expected_successor_bindings(release: EmbeddedReleaseBundle) -> list[dict[str, str]]:
    manifests = expected_manifest_identities()
    c868 = load_json("c868-successor.v1.json")
    writer = load_json("writer-compatibility-successor.v1.json")
    return [
        {"slot_name": "public_transport_7138", "successor_identity": f"sha256:{manifests['manifest:public-transport-7138'][1]}"},
        {"slot_name": "grammar_catalog_d116", "successor_identity": f"sha256:{manifests['manifest:bounded-recovery-d116'][1]}"},
        {"slot_name": "effect_control_h2", "successor_identity": f"sha256:{manifests['manifest:effect-control-h2'][1]}"},
        {"slot_name": "local_withdrawal_h3", "successor_identity": f"sha256:{manifests['manifest:effect-withdrawal-h3'][1]}"},
        {"slot_name": "catalog_owner_efa0", "successor_identity": f"sha256:{manifests['manifest:catalog-owner-efa0'][1]}"},
        {"slot_name": "resource_bundle_c868", "successor_identity": f"sha256:{exact_hash(c868['manifest_id'])}"},
        {"slot_name": "release_binding", "successor_identity": f"sha256:{release.release_id}"},
        {"slot_name": "writer_compatibility", "successor_identity": f"sha256:{exact_hash(writer['writer_protocol_epoch_id'])}"},
    ]


def contains_bytes32(value: Any, identity: str) -> bool:
    if isinstance(value, dict):
        if set(value) == {"bytes"} and value.get("bytes") == identity:
            return True
        return any(contains_bytes32(item, identity) for item in value.values())
    if isinstance(value, list):
        return any(contains_bytes32(item, identity) for item in value)
    return False


def validate_generated_output_bindings(
    closure: Mapping[str, Any],
    delta: Mapping[str, Any],
    release: EmbeddedReleaseBundle,
) -> list[dict[str, Any]]:
    bindings = closure.get("downstream_generated_output_bindings")
    require(isinstance(bindings, list) and len(bindings) == 3, "exact three generated-output bindings are required")
    require([row.get("logical_path") for row in bindings] == list(EFFECT_OUTPUT_PATHS), "generated-output path/order drifted")
    delta_id = exact_hash(delta.get("identity"), "expected delta commitment")
    delta_json_path = OUT / "expected-delta-successor.v1.json"
    delta_cbor_path = OUT / "expected-delta-successor.v1.cbor"
    require(delta_json_path.is_file() and delta_cbor_path.is_file(), "expected-delta rendered bytes are missing")
    rendered_hashes = {
        EFFECT_OUTPUT_PATHS[0]: sha256(delta_cbor_path.read_bytes()),
        EFFECT_OUTPUT_PATHS[1]: sha256(delta_json_path.read_bytes()),
    }
    for binding in bindings:
        path = binding["logical_path"]
        readers = [
            ast_binding(
                "tools/vnext_contracts/stage0/effect_home/build.py",
                "DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS",
                path,
            ),
            ast_binding(
                "tools/vnext_contracts/stage0/effect_home/validate.py",
                "DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS",
                path,
            ),
        ]
        if path == EFFECT_OUTPUT_PATHS[1]:
            readers.append(
                ast_binding(
                    "tools/vnext_contracts/stage0/candidate_root/build.py",
                    "RESOURCE_SUCCESSOR_DELTA",
                    path,
                )
            )
        elif path == EFFECT_OUTPUT_PATHS[2]:
            readers.append(
                ast_binding(
                    "tools/vnext_contracts/stage0/candidate_root/build.py",
                    "RESOURCE_RELEASE",
                    path,
                )
            )
        producer_id = delta_id if "expected-delta" in path else release.release_id
        exact_content_sha256 = rendered_hashes.get(path)
        encoding = "CanonicalCbor" if path.endswith(".cbor") else "CanonicalJson"
        content_binding = "ExactRenderedBytes" if exact_content_sha256 else "ExternalByteReceiptAfterRender"
        canonical = [
            1,
            path,
            bytes32(producer_id),
            encoding,
            content_binding,
            [1, bytes32(exact_content_sha256)] if exact_content_sha256 else [0],
            [[reader["reader_locator"], bytes32(reader["reader_content_sha256"])] for reader in readers],
        ]
        expected = {
            "binding_id": profile_commitment_value(canonical),
            "logical_path": path,
            "producer_identity": f"sha256:{producer_id}",
            "encoding": encoding,
            "content_binding": content_binding,
            "exact_content_sha256": exact_content_sha256,
            "readers": readers,
            "removal_obligations": [],
            "canonical_value": canonical,
        }
        require(binding == expected, f"generated-output binding drifted: {path}")
    return list(bindings)


def validate_resource_release(
    documents: Mapping[str, Any],
    inventory: Any,
    resources: Sequence[ResourceDescriptor],
    bundles: Sequence[BundleManifest],
    census: ReleaseResourceCensus,
    release: EmbeddedReleaseBundle,
) -> None:
    closure = documents["closure"]
    raw = validate_stage0_commitment(closure, "maestro.vnext.stage0.resource-release.v1")
    require((OUT / "resource-release.v1.cbor").read_bytes() == raw, "Resource/Release sibling CBOR drifted")
    require(
        closure.get("source_publication") is False
        and closure.get("runtime_registration") is False
        and closure.get("installation") is False,
        "Resource/Release closure falsely claims publication, registration, or installation",
    )
    require(not ({"manifest_identity_envelope", "release_state", "runtime", "state"} & set(closure)), "Stage-0 closure contains a false Manifest/runtime/state claim")
    descriptor_set = documents["resource-descriptors"]
    require(closure.get("resource_descriptor_set_identity") == descriptor_set["identity"], "Resource descriptor-set identity drifted in closure")
    require(closure.get("resource_count") == 377 and closure.get("resources") == descriptor_set["resources"], "closure Resource exact-set drifted")
    expected_bundle_records = [
        dict(document) | {"artifact_path": f"contracts/vnext/stage0/resource-release/{name}.json"}
        for name, document in zip(BUNDLE_NAMES, documents["bundles"], strict=True)
    ]
    require(closure.get("bundle_count") == 8 and closure.get("bundles") == expected_bundle_records, "closure Bundle exact-set drifted")
    require(closure.get("release_resource_census") == documents["census"], "closure Census embedding drifted")
    require(closure.get("embedded_release_bundle") == documents["release"], "closure Release embedding drifted")
    require(closure.get("expected_delta") == documents["delta"], "closure through-Release delta embedding drifted")
    require(closure.get("resolved_expected_delta_commitment_id") == documents["delta"]["identity"], "closure delta commitment pointer drifted")
    require(closure.get("downstream_delta_obligations") == documents["delta"]["downstream_obligations"], "closure downstream obligation set drifted")
    for name in AUDIT_NAMES:
        field = name.removesuffix(".v1.json").replace("-", "_")
        require(closure.get(field) == documents[name], f"closure audit embedding drifted: {name}")
    bindings = validate_generated_output_bindings(closure, documents["delta"], release)
    require(
        closure.get("declared_successor_slot_count") == 8
        and closure.get("resolved_successor_slot_count") == 8
        and closure.get("blocked_successor_slot_count") == 0
        and closure.get("null_successor_identity_count") == 0,
        "closure successor slot totality drifted",
    )
    require(closure.get("resolved_successor_bindings") == expected_successor_bindings(release), "exact eight successor bindings drifted")
    c868 = load_json("c868-successor.v1.json")
    writer = load_json("writer-compatibility-successor.v1.json")
    require(
        closure.get("successor_closure")
        == {
            "c868_manifest_id": c868["manifest_id"],
            "c868_runtime_edge_manifest_id": c868["runtime_edge_manifest_id"],
            "migration_cutover_manifest_id": writer["manifest_id"],
            "writer_protocol_epoch_id": writer["writer_protocol_epoch_id"],
            "expected_delta_commitment_id": documents["delta"]["identity"],
            "release_id": release.release_id,
        },
        "closure successor coordinates drifted",
    )
    effect_path = "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"
    effect = json.loads((ROOT / effect_path).read_text())
    require(closure.get("effect_home_finalization_receipt_sha256") == file_sha(effect_path), "Effect finalization source SHA drifted")
    require(closure.get("effect_home_finalization_identity") == effect["identity"], "Effect finalization identity drifted")
    require(closure.get("effect_home_expected_delta_manifest_id") == effect["expected_delta_manifest_id"], "Effect expected-delta ManifestId drifted")
    predecessor = closure.get("predecessor_reproduction")
    require(
        isinstance(predecessor, dict)
        and predecessor.get("c868")
        == {
            "artifact_sha256": FROZEN_SOURCE_SHA256["suite"],
            "manifest_id": "5057a0a8e088a0dd394106b928f9b1c4c7a356040b23363455de1177e33e013f",
            "exact_five_source_verification": True,
        }
        and predecessor.get("migration_cutover_65b3")
        == {
            "artifact_sha256": "f9a2ecbff7b8b1912b78ed7c6b028eb0d9c3bdba92e0d9ac8f0377214e8150d9",
            "manifest_id": "60e9a3a77104b74f044527232802841e230e192279881286c0a2a9d3618be2c6",
            "canonical_bytes_reproduced": True,
        },
        "closure predecessor reproduction receipt drifted",
    )
    require(closure.get("inventory_sha256") == inventory_hash(inventory), "closure inventory hash drifted")
    validation = validate_inventory(inventory)
    require(
        closure.get("inventory_validation") == expected_proof_stable_inventory_validation(validation),
        "closure proof-stable inventory validation projection drifted",
    )
    counts = closure.get("exact_source_counts")
    require(
        counts
        == {
            "resources": 377,
            "direct_reader_edges": 377,
            "bundle_instances": 8,
            "bundle_kinds": 7,
            "current_persistence_descriptors": 22,
            "current_archive_descriptors": 11,
            "current_golden_fixtures": 22,
            "c868_schemas": 38,
            "c868_suite_components": 62,
            "c868_runtime_edges": 61,
        },
        "closure exact source counts drifted",
    )
    require(closure.get("migration_requirement_count") == 377 and closure.get("migration_runtime_proof_complete") is False, "closure migration proof status drifted")
    require(
        closure.get("post_root_delta_identity") is None
        and closure.get("post_root_union_identity") is None
        and closure.get("post_root_status") == "pending_root_worker_noncanonical_delta_and_union"
        and closure.get("post_root_identity_feedback_into_resource_bundle_census_release") is False,
        "root-worker post-root ownership/null obligations drifted",
    )
    audit_ids = [
        [name, bytes32(exact_hash(documents[name]["identity"]))]
        for name in sorted(AUDIT_NAMES)
    ]
    expected_value = [
        1,
        bytes32(exact_hash(descriptor_set["identity"])),
        [[bundle.bundle_tag, bytes32(bundle.bundle_id)] for bundle in bundles],
        bytes32(census.census_id),
        bytes32(release.release_id),
        bytes32(exact_hash(documents["delta"]["identity"])),
        audit_ids,
        [bytes32(row["binding_id"]) for row in bindings],
        [0],
        [0],
        False,
    ]
    require(closure.get("canonical_value") == expected_value, "Resource/Release closure canonical value drifted")
    forbidden_ids = (exact_hash(documents["delta"]["identity"]), exact_hash(closure["identity"]))
    for identity in forbidden_ids:
        for resource in resources:
            require(not contains_bytes32(resource.envelope, identity), "post-Release identity fed back into Resource")
        for bundle in bundles:
            require(not contains_bytes32(bundle.envelope, identity), "post-Release identity fed back into Bundle")
        require(not contains_bytes32(census.envelope, identity), "post-Release identity fed back into Census")
        require(not contains_bytes32(release.envelope, identity), "post-Release identity fed back into Release")


def validate_all(
    documents: Mapping[str, Any] | None = None,
    *,
    inventory: Any | None = None,
    verify_preidentity: bool = True,
) -> dict[str, Any]:
    try:
        if verify_preidentity:
            validate_preidentity_artifacts()
        documents = load_documents() if documents is None else documents
        inventory = build_current_inventory(ROOT) if inventory is None else inventory
        validate_inventory(inventory)
        resources, _ = validate_resources(documents, inventory)
        validate_current_surface(documents, inventory, resources)
        validate_schema_and_migration_audits(documents, inventory, resources)
        bundles, census, release = validate_bundle_census_release(documents, inventory, resources)
        validate_delta(documents, resources, bundles, census, release)
        validate_resource_release(documents, inventory, resources, bundles, census, release)
    except ValidationError:
        raise
    except (ContractError, KeyError, IndexError, TypeError, ValueError) as error:
        raise ValidationError(str(error)) from error
    return {
        "status": "pass",
        "resource_count": len(resources),
        "bundle_count": len(bundles),
        "census_id": census.census_id,
        "release_id": release.release_id,
        "resource_release_identity": exact_hash(documents["closure"]["identity"]),
        "inventory_sha256": inventory_hash(inventory),
        "migration_runtime_proof_complete": False,
        "post_root_identity_feedback": False,
    }


def run_gate(name: str) -> dict[str, Any]:
    require(name in GATES, f"unknown Resource/Release gate: {name}")
    validate_preidentity_artifacts()
    documents = load_documents()
    inventory = build_current_inventory(ROOT)
    validate_inventory(inventory)
    resources, _ = validate_resources(documents, inventory)
    if name == "current_surface_consumer_census":
        validate_current_surface(documents, inventory, resources)
    elif name == "persistence_archive_fixtures":
        validate_schema_and_migration_audits(documents, inventory, resources)
    elif name == "migration_rollback_removal":
        validate_schema_and_migration_audits(documents, inventory, resources)
        validate_bundle_census_release(documents, inventory, resources)
    else:
        validate_all(documents, inventory=inventory, verify_preidentity=False)
    return {"status": "pass", "gate": name, "inventory_sha256": inventory_hash(inventory)}


def mutate_at(document: Any, path: Sequence[Any], value: Any) -> None:
    target = document
    for part in path[:-1]:
        target = target[part]
    target[path[-1]] = value


def delete_at(document: Any, path: Sequence[Any]) -> None:
    target = document
    for part in path[:-1]:
        target = target[part]
    del target[path[-1]]


def mutant_cases() -> list[tuple[str, Callable[[dict[str, Any]], None]]]:
    zero = "0" * 64
    one = "1" * 64
    cases: list[tuple[str, Callable[[dict[str, Any]], None]]] = [
        ("stage0_false_manifest_envelope", lambda d: mutate_at(d, ["resource-descriptors", "manifest_identity_envelope"], [1])),
        ("descriptor_count", lambda d: mutate_at(d, ["resource-descriptors", "resource_count"], 378)),
        ("resource_tag", lambda d: mutate_at(d, ["resource-descriptors", "resources", 0, "inventory_ordinal"], 2)),
        ("resource_id", lambda d: mutate_at(d, ["resource-descriptors", "resources", 0, "resource_id"], zero)),
        ("resource_locator", lambda d: mutate_at(d, ["resource-descriptors", "resources", 0, "stable_locator"], "not/a/live/resource")),
        ("resource_content_hash", lambda d: mutate_at(d, ["resource-descriptors", "resources", 0, "content_sha256"], zero)),
        ("resource_migration_profile", lambda d: mutate_at(d, ["resource-descriptors", "resources", 0, "profiles", "migration"], zero)),
        ("resource_disposition", lambda d: mutate_at(d, ["resource-descriptors", "resources", 0, "disposition"], "Remove")),
        ("resource_bundle_kind", lambda d: mutate_at(d, ["resource-descriptors", "resources", 0, "required_bundle_kind"], "Adapter")),
        ("resource_envelope_domain", lambda d: mutate_at(d, ["resource-descriptors", "resources", 0, "identity_envelope", 0], "wrong.domain")),
        ("resource_envelope_tag", lambda d: mutate_at(d, ["resource-descriptors", "resources", 0, "identity_envelope", 2, 0], 2)),
        ("surface_inventory", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "inventory_sha256"], zero)),
        ("surface_inventory_vnext_count", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "inventory_validation", "vnext_source_count"], 1)),
        ("surface_inventory_exclusion_count", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "inventory_validation", "exclusion_count"], 1)),
        ("surface_inventory_generated_output_count", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "inventory_validation", "generated_output_audit_count"], 1)),
        ("surface_inventory_stable_key_deleted", lambda d: delete_at(d, ["current-surface-manifest.v1.json", "inventory_validation", "inventory_sha256"])),
        ("surface_resource_count", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "resource_count"], 378)),
        ("surface_reader_count", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "direct_reader_edge_count"], 378)),
        ("surface_producer_count", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "generated_reference_producer_count"], 58)),
        ("surface_unclassified", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "unclassified_paths"], ["unknown"])),
        ("surface_reader_hash", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "direct_readers", 0, "reader_content_sha256"], zero)),
        ("surface_producer_hash", lambda d: mutate_at(d, ["current-surface-manifest.v1.json", "generated_reference_producers", 0, "content_sha256"], zero)),
        ("consumer_edge_count", lambda d: mutate_at(d, ["current-consumer-census.v1.json", "direct_reader_edge_count"], 378)),
        ("consumer_exactness", lambda d: mutate_at(d, ["current-consumer-census.v1.json", "exact_one_reader_evidence_per_resource"], False)),
        ("persistence_count", lambda d: mutate_at(d, ["current-persistence-manifest.v1.json", "exact_count"], 21)),
        ("persistence_path", lambda d: mutate_at(d, ["current-persistence-manifest.v1.json", "rows", 0, "path"], "wrong.yaml")),
        ("persistence_content", lambda d: mutate_at(d, ["current-persistence-manifest.v1.json", "rows", 0, "content_sha256"], zero)),
        ("persistence_reader", lambda d: mutate_at(d, ["current-persistence-manifest.v1.json", "rows", 0, "readers", 0, "reader_content_sha256"], zero)),
        ("archive_count", lambda d: mutate_at(d, ["current-archive-manifest.v1.json", "exact_count"], 10)),
        ("archive_resource", lambda d: mutate_at(d, ["current-archive-manifest.v1.json", "rows", 0, "resource_id"], zero)),
        ("fixture_count", lambda d: mutate_at(d, ["golden-fixture-manifest.v1.json", "exact_count"], 21)),
        ("fixture_reader", lambda d: delete_at(d, ["golden-fixture-manifest.v1.json", "rows", 0, "readers", 2])),
        ("migration_complete", lambda d: mutate_at(d, ["migration-rollback-requirements.v1.json", "stage0_execution_complete"], True)),
        ("migration_resource", lambda d: mutate_at(d, ["migration-rollback-requirements.v1.json", "requirements", 0, "resource_id"], zero)),
        ("migration_profile", lambda d: mutate_at(d, ["migration-rollback-requirements.v1.json", "requirements", 0, "migration_profile_id"], zero)),
        ("migration_status", lambda d: mutate_at(d, ["migration-rollback-requirements.v1.json", "requirements", 0, "migration_execution_status"], "complete")),
        ("bundle_tag", lambda d: mutate_at(d, ["bundles", 0, "bundle_tag"], 2)),
        ("bundle_group", lambda d: mutate_at(d, ["bundles", 0, "stable_bundle_group"], "Migration:wrong")),
        ("bundle_dependency", lambda d: mutate_at(d, ["bundles", 0, "dependency_bundle_ids"], [one])),
        ("bundle_resource", lambda d: mutate_at(d, ["bundles", 0, "resource_ids"], [])),
        ("manifest_false_stage0_envelope", lambda d: mutate_at(d, ["bundles", 0, "canonical_commitment_envelope"], [1])),
        ("census_identity", lambda d: mutate_at(d, ["census", "census_id"], zero)),
        ("census_source_digest", lambda d: mutate_at(d, ["census", "source_inventory_digest"], zero)),
        ("census_consumers", lambda d: mutate_at(d, ["census", "direct_consumers"], [])),
        ("census_reader_hash", lambda d: mutate_at(d, ["census", "direct_consumers", 0, "reader_content_sha256"], zero)),
        ("release_identity", lambda d: mutate_at(d, ["release", "release_id"], zero)),
        ("release_root", lambda d: mutate_at(d, ["release", "sole_release_root"], False)),
        ("release_census", lambda d: mutate_at(d, ["release", "census_id"], zero)),
        ("delta_count", lambda d: mutate_at(d, ["delta", "exact_identity_kind_counts", "Manifest"], 25)),
        ("delta_predecessor", lambda d: mutate_at(d, ["delta", "entries", 0, "predecessor_identity"], f"sha256:{one}")),
        ("delta_obligation", lambda d: mutate_at(d, ["delta", "downstream_obligations", 0, "successor_identity"], f"sha256:{one}")),
        ("binding_removal", lambda d: mutate_at(d, ["closure", "downstream_generated_output_bindings", 0, "removal_obligations"], ["invented"])),
        ("effect_receipt_sha", lambda d: mutate_at(d, ["closure", "effect_home_finalization_receipt_sha256"], zero)),
        ("effect_finalization_identity", lambda d: mutate_at(d, ["closure", "effect_home_finalization_identity"], f"sha256:{zero}")),
        ("effect_expected_delta_identity", lambda d: mutate_at(d, ["closure", "effect_home_expected_delta_manifest_id"], f"sha256:{zero}")),
        ("closure_inventory_vnext_count", lambda d: mutate_at(d, ["closure", "inventory_validation", "vnext_source_count"], 1)),
        ("closure_inventory_exclusion_count", lambda d: mutate_at(d, ["closure", "inventory_validation", "exclusion_count"], 1)),
        ("closure_inventory_generated_output_count", lambda d: mutate_at(d, ["closure", "inventory_validation", "generated_output_audit_count"], 1)),
        ("closure_inventory_stable_key_deleted", lambda d: delete_at(d, ["closure", "inventory_validation", "inventory_sha256"])),
        ("post_root_feedback", lambda d: mutate_at(d, ["closure", "post_root_identity_feedback_into_resource_bundle_census_release"], True)),
    ]
    require(len(cases) == 60 and len({name for name, _ in cases}) == 60, "semantic mutant matrix must remain exact 60")
    return cases


def run_mutants() -> dict[str, Any]:
    documents = load_documents()
    inventory = build_current_inventory(ROOT)
    validate_all(documents, inventory=inventory)
    survivors = []
    for name, mutate in mutant_cases():
        candidate = copy.deepcopy(documents)
        mutate(candidate)
        try:
            validate_all(candidate, inventory=inventory, verify_preidentity=False)
        except ValidationError:
            continue
        survivors.append(name)
    require(not survivors, f"semantic mutants survived: {survivors}")
    return {"status": "all_rejected", "case_count": 60, "survivors": []}


def parity_receipt_row(document: Mapping[str, Any], raw: bytes) -> dict[str, Any]:
    return {
        "identity_protocol": document["identity_protocol"],
        "identity": document["identity"],
        "canonical_cbor_sha256": sha256(raw),
        "canonical_cbor_byte_length": len(raw),
    }


def independent_ruby_receipt(documents: Mapping[str, Any]) -> dict[str, Any]:
    process = subprocess.run(
        ["/usr/bin/ruby", "tools/vnext_contracts/stage0/resource_release/verify.rb"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    value = json.loads(process.stdout)
    require(isinstance(value, dict) and value.get("status") == "pass", "independent Ruby verifier did not pass")
    python_artifacts = {
        "resource-descriptors.v1": parity_receipt_row(
            documents["resource-descriptors"], (OUT / "resource-descriptors.v1.cbor").read_bytes()
        ),
        **{
            f"bundle-{index:03d}": parity_receipt_row(
                document, (OUT / f"{name}.cbor").read_bytes()
            )
            for index, (name, document) in enumerate(
                zip(BUNDLE_NAMES, documents["bundles"], strict=True), 1
            )
        },
        "release-resource-census.v1": parity_receipt_row(
            documents["census"], (OUT / "release-resource-census.v1.cbor").read_bytes()
        ),
        "embedded-release-bundle.v1": parity_receipt_row(
            documents["release"], (OUT / "embedded-release-bundle.v1.cbor").read_bytes()
        ),
        "expected-delta-successor.v1": parity_receipt_row(
            documents["delta"], (OUT / "expected-delta-successor.v1.cbor").read_bytes()
        ),
        "resource-release.v1": parity_receipt_row(
            documents["closure"], (OUT / "resource-release.v1.cbor").read_bytes()
        ),
    }
    generated_receipts = {
        row["logical_path"]: {
            "binding_id": row["binding_id"],
            "sha256": sha256((OUT / Path(row["logical_path"]).name).read_bytes()),
            "byte_length": len((OUT / Path(row["logical_path"]).name).read_bytes()),
        }
        for row in documents["closure"]["downstream_generated_output_bindings"]
    }
    require(value.get("artifacts") == python_artifacts, "Python/Ruby Resource/Release artifact receipts differ")
    require(value.get("generated_output_byte_receipts") == generated_receipts, "Python/Ruby generated-output receipts differ")
    encoder = load_json("encoder-receipt.v1.json")
    require(
        encoder
        == {
            "schema": "maestro.vnext.resource-release-independent-encoder-receipt.v1",
            "status": "pass",
            "artifact_set_equal": True,
            "equality": "exact_protocol_identity_cbor_hash_and_byte_length",
            "python": {
                "encoder": "python-primary",
                "encoder_source_sha256": file_sha(
                    "tools/vnext_contracts/stage0/resource_release/build.py"
                ),
                "artifacts": python_artifacts,
            },
            "generated_output_byte_receipts": generated_receipts,
            "ruby": value,
        },
        "independent encoder parity receipt drifted",
    )
    return value


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--gate", choices=GATES)
    parser.add_argument("--mutants", action="store_true")
    args = parser.parse_args()
    if args.mutants:
        result = run_mutants()
    elif args.gate:
        result = run_gate(args.gate)
    else:
        documents = load_documents()
        result = validate_all(documents)
        result["independent_encoder"] = independent_ruby_receipt(documents)
        receipt = {
            "schema": "maestro.vnext.resource-release-validation-receipt.v1",
            **result,
            "gates": list(GATES),
            "semantic_mutant_case_count": 60,
        }
        (OUT / "validation-receipt.v1.json").write_text(
            json.dumps(receipt, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n"
        )
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValidationError, ValueError, subprocess.CalledProcessError) as error:
        detail = error.stderr.strip() if isinstance(error, subprocess.CalledProcessError) else str(error)
        print(json.dumps({"status": "fail", "error": detail}, sort_keys=True), file=sys.stderr)
        raise SystemExit(1)
