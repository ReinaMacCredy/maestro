#!/usr/bin/env python3
"""Build the exact Stage-0 Resource -> Bundle -> Census -> Release closure.

Only the four frozen C868 identity domains are emitted as ManifestIdentityV1.
Stage-0 inventories, audits, the expected delta, and the aggregate handoff are
explicit two-slot commitments and cannot masquerade as frozen manifests.
"""

from __future__ import annotations

import argparse
import ast
import copy
import hashlib
import json
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


def proof_environment() -> dict[str, str]:
    return {
        "HOME": tempfile.gettempdir(),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "PYTHONDONTWRITEBYTECODE": "1",
        "PYTHONNOUSERSITE": "1",
        "RUBYOPT": "",
        "RUBYLIB": "",
    }

REPO_ROOT = Path(__file__).resolve().parents[4]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tools.vnext_contracts.stage0.resource_release.c868_contract import (
    BUNDLE_KIND_TAGS,
    BUNDLE_TOPOLOGY,
    DIRECT_CONSUMER_KIND_TAGS,
    DISPOSITION_TAGS,
    FROZEN_RUNTIME_EDGE_MANIFEST_ID,
    FROZEN_SCHEMA_IDS,
    FROZEN_SOURCE_SHA256,
    FROZEN_SUITE_MANIFEST_ID,
    BundleManifest,
    DirectConsumer,
    EmbeddedReleaseBundle,
    ReleaseResourceCensus,
    ResourceDescriptor,
    bytes32,
    construct_bundle_manifest,
    construct_embedded_release_bundle,
    construct_release_resource_census,
    construct_resource_descriptor,
    encode_cbor,
    identity_digest,
    profile_commitment_value,
    validate_release_closure,
    verify_frozen_inputs,
)
from tools.vnext_contracts.stage0.resource_release.current_inventory import (
    BundleKind,
    CurrentInventory,
    DirectReaderEvidence,
    InventoryValidation,
    ReaderRole,
    ResourceCandidate,
    ResourceDisposition,
    SourceKind,
    build_current_inventory,
    canonical_inventory_payload,
    inventory_hash,
    validate_inventory,
)


ROOT = REPO_ROOT
OUT = ROOT / "contracts/vnext/stage0/resource-release"
FROZEN = Path("/Users/reinamaccredy/Code/maestro/.maestro/workbench")

C65_SOURCE_SHA256 = "f9a2ecbff7b8b1912b78ed7c6b028eb0d9c3bdba92e0d9ac8f0377214e8150d9"
C65_MANIFEST_ID = "60e9a3a77104b74f044527232802841e230e192279881286c0a2a9d3618be2c6"
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

PREIDENTITY_OUTPUTS = (
    "c868-successor.v1.cbor",
    "c868-successor.v1.json",
    "capability-evaluator.v1.json",
    "capability-relations.v1.json",
    "vendor-reference-pack.v1.json",
    "writer-compatibility-successor.v1.cbor",
    "writer-compatibility-successor.v1.json",
)
EFFECT_OUTPUT_PATHS = (
    "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.cbor",
    "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.json",
    "contracts/vnext/stage0/resource-release/resource-release.v1.json",
)
IDENTITY_KIND_TAGS = {
    "Schema": 1,
    "Manifest": 2,
    "Resource": 3,
    "Bundle": 4,
    "Census": 5,
    "Release": 6,
    "RootInput": 7,
    "HandoffInput": 8,
}
DELTA_DISPOSITION_TAGS = {"Introduce": 1, "Preserve": 2, "Rotate": 3, "Retire": 4}
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


class BuildError(RuntimeError):
    """An exact source, identity, or coverage claim cannot be reproduced."""


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode() + b"\n"


def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def exact_hash(value: str) -> str:
    raw = value.removeprefix("sha256:")
    if len(raw) != 64 or any(character not in "0123456789abcdef" for character in raw):
        raise BuildError(f"not an exact SHA-256 value: {value!r}")
    return raw


def file_sha(locator: str) -> str:
    path = ROOT / locator
    if not path.is_file():
        raise BuildError(f"missing exact source: {locator}")
    return sha256(path.read_bytes())


def stage0_commitment(schema: str, value: Any, **fields: Any) -> tuple[dict[str, Any], bytes]:
    envelope = [schema, value]
    raw = encode_cbor(envelope)
    identity = sha256(raw)
    document = {
        "schema": schema,
        "identity_protocol": "Stage0CanonicalCommitmentV1",
        "identity_scope": "canonical_commitment_envelope_only",
        "identity": f"sha256:{identity}",
        "canonical_commitment_envelope": envelope,
        "canonical_value": value,
        "canonical_cbor_sha256": identity,
        "canonical_cbor_byte_length": len(raw),
        "canonical_cbor_hex": raw.hex(),
        "candidate_only": True,
        "runtime_activation": False,
        **fields,
    }
    return document, raw


def proof_stable_inventory_validation(validation: InventoryValidation) -> dict[str, Any]:
    """Project only inventory facts invariant under downstream output presence."""

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


def manifest_document(
    *,
    schema: str,
    identity_name: str,
    identity: str,
    envelope: list[Any],
    raw: bytes,
    value: list[Any],
    **fields: Any,
) -> dict[str, Any]:
    return {
        "schema": schema,
        "identity_protocol": "ManifestIdentityV1",
        identity_name: identity,
        "identity": f"sha256:{identity}",
        "manifest_identity_envelope": envelope,
        "canonical_value": value,
        "canonical_cbor_sha256": identity,
        "canonical_cbor_byte_length": len(raw),
        "canonical_cbor_hex": raw.hex(),
        "candidate_only": True,
        "runtime_activation": False,
        **fields,
    }


def replace_bytes32(value: Any, replacements: Mapping[str, str]) -> Any:
    if isinstance(value, list):
        return [replace_bytes32(item, replacements) for item in value]
    if isinstance(value, dict):
        if set(value) == {"bytes"} and value["bytes"] in replacements:
            return bytes32(replacements[value["bytes"]])
        return {key: replace_bytes32(item, replacements) for key, item in value.items()}
    return value


def frozen_c868() -> dict[str, Any]:
    names = {
        "suite_bytes": "vnext-resource-contract-suite-v1.json",
        "builder_bytes": "vnext_resource_contract_suite_build.py",
        "validator_bytes": "vnext_resource_contract_suite_validate.py",
        "suite_envelope_bytes": "vnext-resource-contract-suite-v1-envelope.json",
        "runtime_edge_envelope_bytes": "vnext-distribution-runtime-edge-contract-v1-envelope.json",
    }
    missing = [name for name in names.values() if not (FROZEN / name).is_file()]
    if missing:
        raise BuildError(f"missing frozen C868 sources: {missing}")
    return verify_frozen_inputs(**{key: (FROZEN / name).read_bytes() for key, name in names.items()})


def build_c868_successor(
    predecessor: Mapping[str, Any], grammar_id: str, catalog_ids: Sequence[str]
) -> tuple[dict[str, Any], bytes, bytes]:
    if predecessor["manifest_id"] != FROZEN_SUITE_MANIFEST_ID or len(catalog_ids) != 9:
        raise BuildError("frozen C868 predecessor or nine-catalog closure changed")
    replacements = {OLD_GRAMMAR_ID: grammar_id, **dict(zip(OLD_CATALOG_IDS, catalog_ids, strict=True))}
    envelope = replace_bytes32(copy.deepcopy(predecessor["manifest_identity_envelope"]), replacements)
    descriptor_domain = predecessor["descriptor_domains"][3]
    descriptor_schema_id = envelope[2]["bytes"]
    for row in envelope[4]:
        row[1] = bytes32(sha256(encode_cbor([descriptor_domain, bytes32(descriptor_schema_id), row[2]])))
    successor_id, successor_cbor = identity_digest(envelope)
    artifact = {
        "schema": "maestro.vnext.c868-resource-contract-successor.v1",
        "identity_protocol": "ManifestIdentityV1",
        "candidate_only": True,
        "runtime_activation": False,
        "predecessor": {
            "manifest_id": FROZEN_SUITE_MANIFEST_ID,
            "artifact_sha256": FROZEN_SOURCE_SHA256["suite"],
        },
        "manifest_id": successor_id,
        "canonical_cbor_sha256": successor_id,
        "canonical_cbor_byte_length": len(successor_cbor),
        "manifest_identity_envelope": envelope,
        "catalog_profile_grammar_id": grammar_id,
        "catalog_manifest_ids": list(catalog_ids),
        "runtime_edge_manifest_id": FROZEN_RUNTIME_EDGE_MANIFEST_ID,
        "exact_counts": {"schemas": 38, "suite_components": 62, "runtime_edges": 61},
        "historical_evidence": {
            "e204": {"count": 204, "classification": "non_promoting_historical_evidence"},
            "c325": {"count": 325, "classification": "non_promoting_historical_evidence"},
            "physical_nodes": {"count": 28102, "classification": "non_promoting_historical_evidence"},
            "current_equality_claimed": False,
            "global_absence_claimed": False,
        },
    }
    return artifact, json_bytes(artifact), successor_cbor


def build_writer_successor(
    predecessor: Mapping[str, Any], replacements: dict[str, str]
) -> tuple[dict[str, Any], bytes, bytes]:
    if predecessor.get("manifest_id") != C65_MANIFEST_ID:
        raise BuildError("frozen 65b3 writer-compatibility identity changed")
    if [len(predecessor[name]) for name in ("schemas", "invariants", "predecessors", "descriptors")] != [12, 23, 10, 50]:
        raise BuildError("frozen 65b3 closure counts changed")
    replacements = dict(replacements)
    read_write = replace_bytes32(copy.deepcopy(predecessor["schema_read_write_set"]["identity_envelope"]), replacements)
    read_write_id = sha256(encode_cbor(read_write))
    replacements[predecessor["schema_read_write_set_descriptor_id"]] = read_write_id
    writer = replace_bytes32(copy.deepcopy(predecessor["writer_protocol_epoch"]["identity_envelope"]), replacements)
    writer_id = sha256(encode_cbor(writer))
    replacements[predecessor["writer_protocol_epoch_id"]] = writer_id
    migration = replace_bytes32(copy.deepcopy(predecessor["migration_epoch"]["identity_envelope"]), replacements)
    migration_id = sha256(encode_cbor(migration))
    replacements[predecessor["migration_epoch_id"]] = migration_id
    envelope = replace_bytes32(copy.deepcopy(predecessor["manifest_identity_envelope"]), replacements)
    descriptor_domain = predecessor["descriptor_domain"]
    descriptor_schema_id = envelope[2]["bytes"]
    for row in envelope[4]:
        row[1] = bytes32(sha256(encode_cbor([descriptor_domain, bytes32(descriptor_schema_id), row[2]])))
    successor_id, successor_cbor = identity_digest(envelope)
    artifact = {
        "schema": "maestro.vnext.migration-cutover-writer-compatibility-successor.v1",
        "identity_protocol": "ManifestIdentityV1",
        "candidate_only": True,
        "runtime_activation": False,
        "predecessor": {"manifest_id": C65_MANIFEST_ID, "artifact_sha256": C65_SOURCE_SHA256},
        "manifest_id": successor_id,
        "canonical_cbor_sha256": successor_id,
        "canonical_cbor_byte_length": len(successor_cbor),
        "manifest_identity_envelope": envelope,
        "schema_read_write_set_descriptor_id": read_write_id,
        "writer_protocol_epoch_id": writer_id,
        "migration_epoch_id": migration_id,
        "finality_edge_manifest_id": predecessor["finality_edge_contract"]["manifest_id"],
        "predecessor_components": {
            key: predecessor[key]
            for key in (
                "schema_read_write_set_descriptor_id",
                "writer_protocol_epoch_id",
                "migration_epoch_id",
            )
        }
        | {"finality_edge_manifest_id": predecessor["finality_edge_contract"]["manifest_id"]},
        "exact_counts": {
            "schemas": 12,
            "invariants": 23,
            "predecessors": 10,
            "components": 50,
            "finality_edges": 11,
            "read_write_cohorts": 4,
            "rows_per_cohort": 46,
        },
    }
    return artifact, json_bytes(artifact), successor_cbor


def build_capability_inputs() -> tuple[dict[str, Any], dict[str, Any]]:
    public = read_json(ROOT / "contracts/vnext/public/public_contracts.v1.json")
    source_name = "capability_method_contracts.v1.json"
    if source_name not in public["semantic_artifacts"]:
        raise BuildError("public closure does not admit the capability-method contract")
    capability = read_json(ROOT / f"contracts/vnext/public/{source_name}")
    tree = read_json(ROOT / "embedded/vnext/capability/instruction-tree.v1.json")
    relation = capability["job_method"]
    rows = {
        job: [row["method"] for row in relation["rows"] if row["job"] == job and row["admitted"]]
        for job in capability["jobs"]
    }
    if capability["instruction_resource_count"] != len(tree["logical_paths"]):
        raise BuildError("capability instruction count changed")
    if sum(map(len, rows.values())) != relation["positive"] or len(relation["rows"]) - relation["positive"] != relation["negative"]:
        raise BuildError("capability relation totality changed")
    return (
        {
            "schema": "maestro.vnext.capability-instruction-relations.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "instruction_resource_count": len(tree["logical_paths"]),
            "job_method_rows": rows,
            "positive_job_method_edges": relation["positive"],
            "negative_job_method_edges": relation["negative"],
        },
        {
            "schema": "maestro.vnext.capability-instruction-evaluator.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "closed_jobs": capability["jobs"],
            "closed_methods": capability["direct_methods"],
            "instruction_resource_count": len(tree["logical_paths"]),
            "selection_outcomes": ["Selected", "Ambiguous", "Blocked"],
            "authority": "none",
        },
    )


def build_vendor_pack() -> dict[str, Any]:
    root = ROOT / "embedded/design/vendor/awesome-design-md"
    rows = [
        {"path": path.relative_to(ROOT).as_posix(), "sha256": sha256(path.read_bytes())}
        for path in sorted(root.rglob("DESIGN.md"))
    ]
    digest = sha256(b"".join(f"{row['path']}\0{row['sha256']}\n".encode() for row in rows))
    if len(rows) != 74:
        raise BuildError(f"vendor DESIGN.md closure changed: {len(rows)}/74")
    return {
        "schema": "maestro.vnext.optional-vendor-reference-pack.v1",
        "candidate_only": True,
        "runtime_activation": False,
        "optional": True,
        "tree_sha256": digest,
        "license_path": "embedded/design/vendor/awesome-design-md/LICENSE",
        "provenance_manifest_path": "embedded/design/vendor/awesome-design-md/manifest.yml",
        "files": rows,
    }


def build_preidentity_outputs() -> tuple[dict[str, bytes], dict[str, Any], dict[str, Any]]:
    suite = frozen_c868()
    catalog_inventory = read_json(ROOT / "contracts/vnext/catalogs/generated/inventory.json")
    grammar_id = exact_hash(catalog_inventory["grammar_id"])
    catalog_ids = [exact_hash(row["identity"]) for row in catalog_inventory["artifacts"] if row["kind"] != "grammar"]
    c868, c868_json, c868_cbor = build_c868_successor(suite, grammar_id, catalog_ids)
    c65_path = FROZEN / "vnext-migration-cutover-contract-v1.json"
    if not c65_path.is_file() or sha256(c65_path.read_bytes()) != C65_SOURCE_SHA256:
        raise BuildError("frozen 65b3 source SHA-256 changed")
    c65_predecessor = read_json(c65_path)
    replacements = {
        OLD_GRAMMAR_ID: grammar_id,
        FROZEN_SUITE_MANIFEST_ID: c868["manifest_id"],
        FROZEN_SOURCE_SHA256["suite"]: sha256(c868_json),
        **dict(zip(OLD_CATALOG_IDS, catalog_ids, strict=True)),
    }
    writer, writer_json, writer_cbor = build_writer_successor(c65_predecessor, replacements)
    relations, evaluator = build_capability_inputs()
    vendor = build_vendor_pack()
    generated = {
        "c868-successor.v1.json": c868_json,
        "c868-successor.v1.cbor": c868_cbor,
        "writer-compatibility-successor.v1.json": writer_json,
        "writer-compatibility-successor.v1.cbor": writer_cbor,
        "capability-relations.v1.json": json_bytes(relations),
        "capability-evaluator.v1.json": json_bytes(evaluator),
        "vendor-reference-pack.v1.json": json_bytes(vendor),
    }
    if set(generated) != set(PREIDENTITY_OUTPUTS):
        raise AssertionError("invariant: exact seven pre-identity outputs")
    return generated, c868, writer


def write_or_check(path: Path, expected: bytes, check: bool, mismatches: list[str]) -> None:
    if check:
        if not path.is_file() or path.read_bytes() != expected:
            mismatches.append(path.relative_to(ROOT).as_posix())
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(expected)


def freeze_preidentity_outputs(generated: Mapping[str, bytes], check: bool) -> list[str]:
    mismatches: list[str] = []
    for name in PREIDENTITY_OUTPUTS:
        write_or_check(OUT / name, generated[name], check, mismatches)
    return mismatches


def reader_rows_by_resource(inventory: CurrentInventory) -> dict[str, list[DirectReaderEvidence]]:
    rows: dict[str, list[DirectReaderEvidence]] = defaultdict(list)
    for reader in inventory.direct_readers:
        rows[reader.resource_stable_key].append(reader)
    if set(rows) != {resource.stable_key for resource in inventory.resources}:
        raise BuildError("direct-reader registry is not exact-set equal to the Resource closure")
    return rows


def canonical_reader_coordinate(reader: DirectReaderEvidence) -> list[Any]:
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


def resource_profiles(
    candidate: ResourceCandidate,
    readers: Sequence[DirectReaderEvidence],
    *,
    compatibility_profile_id: str,
    build_source_sha256: str,
) -> dict[str, str | None]:
    reader_coordinates = [canonical_reader_coordinate(row) for row in readers]
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
        reader_coordinates,
    ]

    def commitment(label: str, detail: Any) -> str:
        return profile_commitment_value([1, label, base, detail])

    historical = [
        [row.locator, bytes32(row.recorded_sha256), row.family, row.current_bytes_equal]
        for row in candidate.provenance.historical_evidence
    ]
    provenance = commitment(
        "provenance",
        [
            candidate.provenance.kind.tag,
            candidate.provenance.registry_locator or "none",
            candidate.provenance.license_locator or "none",
            bytes32(sha256(candidate.provenance.applicability.encode())),
            historical,
        ],
    )
    generator: str | None = None
    if candidate.stable_locator in {
        f"contracts/vnext/stage0/resource-release/{name}" for name in PREIDENTITY_OUTPUTS
    }:
        generator = commitment(
            "generator",
            [
                bytes32(build_source_sha256),
                bytes32(FROZEN_SOURCE_SHA256["suite"]),
                bytes32(C65_SOURCE_SHA256),
                bytes32(candidate.content_sha256),
            ],
        )
    license_commitment: str | None = None
    if candidate.provenance.license_locator:
        license_commitment = profile_commitment_value(
            [
                1,
                "third-party-license",
                candidate.provenance.license_locator,
                bytes32(file_sha(candidate.provenance.license_locator)),
            ]
        )
    pending = "pending_stage0_execution"
    rehearsal = "pending_stage0_rehearsal"
    return {
        "owner": profile_commitment_value([1, "semantic-owner", candidate.semantic_owner.value]),
        "provenance": provenance,
        "license": license_commitment,
        "compatibility": compatibility_profile_id,
        "generator": generator,
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
            [pending, candidate.disposition.tag, "exact_content_and_reader_coordinates"],
        ),
        "rollback": commitment(
            "rollback-requirement",
            [rehearsal, candidate.disposition.tag, "exact_preimage_or_refuse"],
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


def build_resources(
    inventory: CurrentInventory,
    *,
    c868_manifest_id: str,
    writer_manifest_id: str,
) -> tuple[list[ResourceDescriptor], list[dict[str, Any]], list[dict[str, Any]]]:
    readers_by_key = reader_rows_by_resource(inventory)
    compatibility_profile_id = profile_commitment_value(
        [1, "resource-compatibility", bytes32(c868_manifest_id), bytes32(writer_manifest_id)]
    )
    build_source_sha256 = sha256(Path(__file__).read_bytes())
    resources: list[ResourceDescriptor] = []
    records: list[dict[str, Any]] = []
    requirement_rows: list[dict[str, Any]] = []
    for candidate in inventory.resources:
        readers = readers_by_key[candidate.stable_key]
        profiles = resource_profiles(
            candidate,
            readers,
            compatibility_profile_id=compatibility_profile_id,
            build_source_sha256=build_source_sha256,
        )
        resource = construct_resource_descriptor(
            resource_tag=candidate.inventory_ordinal,
            stable_resource_key=candidate.stable_key,
            content=candidate.content_bytes,
            content_encoding=candidate.c868_content_encoding.value,
            media_type=candidate.media_type,
            resource_kind=candidate.frozen_resource_kind.value,
            owner_tag=candidate.semantic_owner.frozen_tag,
            owner_profile_id=str(profiles["owner"]),
            required_bundle_kind=candidate.target_bundle_kind.value,
            provenance_kind=candidate.provenance.kind.value,
            provenance_commitment_id=str(profiles["provenance"]),
            license_commitment_id=profiles["license"],
            backward_dependencies=(),
            compatibility_profile_id=str(profiles["compatibility"]),
            generator_commitment_id=profiles["generator"],
            target_policy_profile_id=str(profiles["target"]),
            custody_policy_profile_id=str(profiles["custody"]),
            migration_profile_id=str(profiles["migration"]),
            rollback_profile_id=str(profiles["rollback"]),
            uninstall_profile_id=str(profiles["uninstall"]),
            retention_profile_id=str(profiles["retention"]),
            removal_profile_id=str(profiles["removal"]),
            disposition=candidate.disposition.value,
            proof_profile_id=str(profiles["proof"]),
        )
        resources.append(resource)
        records.append(
            resource.as_record()
            | {
                "stable_resource_key": candidate.stable_key,
                "stable_locator": candidate.stable_locator,
                "inventory_ordinal": candidate.inventory_ordinal,
                "inventory_candidate_id": candidate.physical_candidate_id,
                "family": candidate.family,
                "source_kind": candidate.source_kind.value,
                "semantic_owner": candidate.semantic_owner.value,
                "required_bundle_kind": candidate.target_bundle_kind.value,
                "target_bundle_group": candidate.target_bundle_group,
                "disposition": candidate.disposition.value,
                "content_sha256": candidate.content_sha256,
                "content_byte_length": len(candidate.content_bytes),
                "content_encoding": candidate.c868_content_encoding.value,
                "media_type": candidate.media_type,
                "resource_kind": candidate.frozen_resource_kind.value,
                "provenance_kind": candidate.provenance.kind.value,
                "license_locator": candidate.provenance.license_locator,
                "profiles": profiles,
                "reader_evidence": [
                    {
                        "reader_locator": row.reader_locator,
                        "reader_content_sha256": row.reader_content_sha256,
                        "semantic_owner": row.semantic_owner.value,
                        "consumer_kind": row.kind.value,
                        "evidence_kind": row.evidence_kind.value,
                        "role": row.role.value,
                        "evidence_sha256": sha256(row.evidence.encode()),
                    }
                    for row in readers
                ],
            }
        )
        requirement_rows.append(
            {
                "resource_tag": resource.resource_tag,
                "resource_id": resource.resource_id,
                "stable_resource_key": candidate.stable_key,
                "stable_locator": candidate.stable_locator,
                "content_sha256": candidate.content_sha256,
                "disposition": candidate.disposition.value,
                "migration_profile_id": profiles["migration"],
                "rollback_profile_id": profiles["rollback"],
                "removal_profile_id": profiles["removal"],
                "proof_profile_id": profiles["proof"],
                "migration_execution_status": "pending_stage0_execution",
                "rollback_rehearsal_status": "pending_stage0_rehearsal",
                "removal_execution_status": "pending_stage0_execution",
                "runtime_proof_complete": False,
                "reader_evidence": records[-1]["reader_evidence"],
            }
        )
    if len(resources) != 412 or [row.resource_tag for row in resources] != list(range(1, 413)):
        raise BuildError("exact 412-Resource tag closure changed")
    return resources, records, requirement_rows


BUNDLE_INSTANCES = (
    (1, "Migration", "Migration:default", "migration"),
    (2, "ExternalPattern", "ExternalPattern:first-party-neutral-baseline", "external-pattern-neutral"),
    (3, "ExternalPattern", "ExternalPattern:third-party-awesome-design-md", "external-pattern-vendor"),
    (4, "SharedContract", "SharedContract:default", "shared-contract"),
    (5, "Orchestration", "Orchestration:default", "orchestration"),
    (6, "Capability", "Capability:default", "capability"),
    (7, "Adapter", "Adapter:default", "adapter"),
    (8, "AgentBootstrap", "AgentBootstrap:default", "agent-bootstrap"),
)


def build_bundles(
    resources: Sequence[ResourceDescriptor],
    inventory: CurrentInventory,
) -> tuple[list[BundleManifest], list[dict[str, Any]], dict[str, bytes]]:
    resource_by_tag = {row.resource_tag: row for row in resources}
    candidates_by_group: dict[str, list[ResourceCandidate]] = defaultdict(list)
    for candidate in inventory.resources:
        candidates_by_group[candidate.target_bundle_group].append(candidate)
    bundles: list[BundleManifest] = []
    records: list[dict[str, Any]] = []
    outputs: dict[str, bytes] = {}
    by_group: dict[str, BundleManifest] = {}
    for bundle_tag, kind, group, slug in BUNDLE_INSTANCES:
        members = [resource_by_tag[row.inventory_ordinal] for row in candidates_by_group.get(group, ())]
        if not members:
            raise BuildError(f"empty exact Bundle instance: {group}")
        dependency_groups = {
            "Orchestration:default": ("SharedContract:default",),
            "Capability:default": ("Orchestration:default",),
        }.get(group, ())
        dependencies = [by_group[name] for name in dependency_groups]
        provenance = profile_commitment_value(
            [1, "bundle-provenance", group, [[row.resource_tag, bytes32(row.resource_id)] for row in members]]
        )
        bundle = construct_bundle_manifest(
            bundle_kind=kind,
            stable_bundle_key=f"maestro.vnext.bundle.{slug}",
            semantic_version="1",
            compatibility_profile_id=profile_commitment_value([1, "bundle-compatibility", group]),
            resources=members,
            dependency_bundles=dependencies,
            provenance_commitment_id=provenance,
            license_commitment_id=(
                profile_commitment_value(
                    [
                        1,
                        "third-party-license",
                        "embedded/design/vendor/awesome-design-md/LICENSE",
                        bytes32(file_sha("embedded/design/vendor/awesome-design-md/LICENSE")),
                    ]
                )
                if slug == "external-pattern-vendor"
                else None
            ),
            package_policy_profile_id=profile_commitment_value([1, "package-policy", group, "candidate_only"]),
            supported_target_classes=("WholeTarget",) if kind in {"Adapter", "AgentBootstrap"} else ("NoMaterialization",),
            rollback_profile_id=profile_commitment_value([1, "bundle-rollback", group, "pending_stage0_rehearsal"]),
            uninstall_profile_id=profile_commitment_value([1, "bundle-uninstall", group, "pending_stage0_execution"]),
            retention_profile_id=profile_commitment_value([1, "bundle-retention", group, "pending_stage0_execution"]),
            bundle_tag=bundle_tag,
        )
        bundles.append(bundle)
        by_group[group] = bundle
        name = f"bundle-{bundle_tag:03d}-{slug}.v1"
        document = manifest_document(
            schema="maestro.vnext.bundle.manifest.v1",
            identity_name="bundle_id",
            identity=bundle.bundle_id,
            envelope=bundle.envelope,
            raw=bundle.canonical_cbor,
            value=bundle.value,
            bundle_tag=bundle.bundle_tag,
            bundle_kind=bundle.bundle_kind,
            stable_bundle_group=group,
            resource_ids=list(bundle.resource_ids),
            dependency_bundle_ids=list(bundle.dependency_bundle_ids),
        )
        records.append(document | {"artifact_path": f"contracts/vnext/stage0/resource-release/{name}.json"})
        outputs[f"{name}.json"] = json_bytes(document)
        outputs[f"{name}.cbor"] = bundle.canonical_cbor
    if set(row.bundle_kind for row in bundles) != set(BUNDLE_TOPOLOGY) or len(bundles) != 8:
        raise BuildError("exact eight-instance/seven-kind Bundle closure changed")
    return bundles, records, outputs


def build_direct_consumers(
    inventory: CurrentInventory,
    resources: Sequence[ResourceDescriptor],
) -> tuple[list[DirectConsumer], list[dict[str, Any]]]:
    resource_by_key = {
        candidate.stable_key: resources[candidate.inventory_ordinal - 1]
        for candidate in inventory.resources
    }
    grouped: dict[tuple[str, str, str, str, str, str], list[tuple[DirectReaderEvidence, ResourceDescriptor]]] = defaultdict(list)
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
        grouped[key].append((reader, resource))
    consumers: list[DirectConsumer] = []
    records: list[dict[str, Any]] = []
    for key in sorted(grouped):
        locator, reader_sha, owner, kind, role, disposition = key
        pairs = sorted(grouped[key], key=lambda pair: pair[1].resource_tag)
        bound_resources = tuple(pair[1] for pair in pairs)
        evidence_rows = [canonical_reader_coordinate(pair[0]) for pair in pairs]
        owner_profile_id = profile_commitment_value([1, "consumer-owner", owner])
        provenance = profile_commitment_value(
            [1, "direct-consumer-provenance", locator, bytes32(reader_sha), evidence_rows]
        )
        migration = profile_commitment_value([1, "direct-consumer-migration", locator, role, "pending_stage0_execution"])
        proof = profile_commitment_value([1, "direct-consumer-proof", locator, evidence_rows])
        removal = profile_commitment_value([1, "direct-consumer-removal", locator, "pending_stage0_execution"])
        consumer = DirectConsumer(
            locator=locator,
            owner_tag=READER_OWNER_TAGS[owner],
            owner_profile_id=owner_profile_id,
            consumer_kind=kind,
            resources=bound_resources,
            provenance_commitment_id=provenance,
            disposition=disposition,
            migration_profile_id=migration,
            proof_profile_id=proof,
            removal_profile_id=removal,
        )
        consumers.append(consumer)
        records.append(
            {
                "locator": locator,
                "reader_content_sha256": reader_sha,
                "owner": owner,
                "owner_tag": consumer.owner_tag,
                "consumer_kind": kind,
                "reader_role": role,
                "disposition": disposition,
                "resource_pairs": [
                    {"resource_tag": row.resource_tag, "resource_id": row.resource_id}
                    for row in bound_resources
                ],
                "provenance_commitment_id": provenance,
                "migration_profile_id": migration,
                "proof_profile_id": proof,
                "removal_profile_id": removal,
            }
        )
    return consumers, records


def consumer_inventory_digest(inventory: CurrentInventory) -> str:
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


def build_census_and_release(
    *,
    inventory: CurrentInventory,
    resources: Sequence[ResourceDescriptor],
    bundles: Sequence[BundleManifest],
    direct_consumers: Sequence[DirectConsumer],
    direct_consumer_records: Sequence[dict[str, Any]],
    c868_manifest_id: str,
    writer_manifest_id: str,
    public_catalog_id: str,
) -> tuple[ReleaseResourceCensus, EmbeddedReleaseBundle, dict[str, Any], dict[str, Any]]:
    graph_digest = profile_commitment_value(
        [
            1,
            "resource-bundle-consumer-graph",
            [[row.resource_tag, bytes32(row.resource_id)] for row in resources],
            [
                [
                    row.bundle_tag,
                    bytes32(row.bundle_id),
                    [bytes32(identity) for identity in row.resource_ids],
                    [bytes32(identity) for identity in row.dependency_bundle_ids],
                ]
                for row in bundles
            ],
            [
                [consumer.locator, [[resource.resource_tag, bytes32(resource.resource_id)] for resource in consumer.resources]]
                for consumer in direct_consumers
            ],
        ]
    )
    locators = {
        resources[candidate.inventory_ordinal - 1].resource_id: candidate.stable_locator
        for candidate in inventory.resources
    }
    census = construct_release_resource_census(
        release_key="maestro-vnext-candidate",
        release_version="1",
        platform_qualifier="macos-arm64",
        resources=resources,
        bundles=bundles,
        direct_consumers=direct_consumers,
        source_inventory_digest=inventory_hash(inventory),
        consumer_inventory_digest=consumer_inventory_digest(inventory),
        build_graph_digest=graph_digest,
        resource_locators=locators,
    )
    release = construct_embedded_release_bundle(
        release_key="maestro-vnext-candidate",
        release_version="1",
        platform_qualifier="macos-arm64",
        resources=resources,
        bundles=bundles,
        census=census,
        core_contract_root_id=c868_manifest_id,
        binary_compatibility_id=writer_manifest_id,
        public_catalog_id=public_catalog_id,
        compatibility_profile_id=profile_commitment_value([1, "release-compatibility", bytes32(c868_manifest_id)]),
        rollback_profile_id=profile_commitment_value([1, "release-rollback", "pending_stage0_rehearsal"]),
        uninstall_profile_id=profile_commitment_value([1, "release-uninstall", "pending_stage0_execution"]),
        retention_profile_id=profile_commitment_value([1, "release-retention", "pending_stage0_execution"]),
    )
    validate_release_closure(resources=resources, bundles=bundles, census=census, release=release)
    census_document = manifest_document(
        schema="maestro.vnext.release-resource-census.manifest.v1",
        identity_name="census_id",
        identity=census.census_id,
        envelope=census.envelope,
        raw=census.canonical_cbor,
        value=census.value,
        resource_ids=list(census.resource_ids),
        bundle_ids=list(census.bundle_ids),
        consumer_edges=[list(row) for row in census.consumer_edges],
        direct_consumers=list(direct_consumer_records),
        source_inventory_digest=inventory_hash(inventory),
        consumer_inventory_digest=consumer_inventory_digest(inventory),
        build_graph_digest=graph_digest,
    )
    release_document = manifest_document(
        schema="maestro.vnext.embedded-release-bundle.manifest.v1",
        identity_name="release_id",
        identity=release.release_id,
        envelope=release.envelope,
        raw=release.canonical_cbor,
        value=release.value,
        bundle_ids=list(release.bundle_ids),
        census_id=release.census_id,
        sole_release_root=True,
    )
    return census, release, census_document, release_document


def domain_identity(domain: str, value: Any) -> str:
    return sha256(encode_cbor([domain, value]))


def inventory_reader_document(reader: DirectReaderEvidence, resource_id: str) -> dict[str, Any]:
    return {
        "reader_locator": reader.reader_locator,
        "reader_content_sha256": reader.reader_content_sha256,
        "semantic_owner": reader.semantic_owner.value,
        "consumer_kind": reader.kind.value,
        "evidence_kind": reader.evidence_kind.value,
        "role": reader.role.value,
        "resource_stable_key": reader.resource_stable_key,
        "resource_candidate_id": reader.resource_candidate_id,
        "resource_locator": reader.resource_locator,
        "resource_id": resource_id,
        "disposition": reader.disposition.value,
        "evidence": reader.evidence,
        "evidence_sha256": sha256(reader.evidence.encode()),
        "explicit_dual_role_contract": reader.explicit_dual_role_contract,
    }


def build_current_surface_audits(
    *,
    inventory: CurrentInventory,
    validation: Any,
    resources: Sequence[ResourceDescriptor],
    resource_records: Sequence[dict[str, Any]],
    requirement_rows: Sequence[dict[str, Any]],
) -> tuple[dict[str, dict[str, Any]], dict[str, bytes]]:
    by_key = {row.stable_resource_key: row for row in resources}
    by_locator = {row["stable_locator"]: row for row in resource_records}
    reader_documents = [
        inventory_reader_document(row, by_key[row.resource_stable_key].resource_id)
        for row in inventory.direct_readers
    ]
    resource_documents = [
        {
            "resource_tag": row.resource_tag,
            "resource_id": row.resource_id,
            "stable_resource_key": record["stable_resource_key"],
            "stable_locator": record["stable_locator"],
            "content_sha256": record["content_sha256"],
            "content_byte_length": record["content_byte_length"],
            "source_kind": record["source_kind"],
            "family": record["family"],
            "semantic_owner": record["semantic_owner"],
            "required_bundle_kind": record["required_bundle_kind"],
            "target_bundle_group": record["target_bundle_group"],
            "disposition": record["disposition"],
            "resource_kind": record["resource_kind"],
            "provenance_kind": record["provenance_kind"],
        }
        for row, record in zip(resources, resource_records, strict=True)
    ]
    producer_only = [
        {
            "locator": row.stable_locator,
            "content_sha256": row.content_sha256,
            "content_byte_length": len(row.content_bytes),
            "classification": "generated_reference_producer_only_not_resource",
        }
        for row in inventory.authoritative_sources
        if row.source_kind == SourceKind.GENERATED_REFERENCE_PRODUCER
    ]
    historical = {
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
    }
    surface_value = [
        1,
        bytes32(inventory_hash(inventory)),
        [[row.resource_tag, bytes32(row.resource_id)] for row in resources],
        [
            [
                document["reader_locator"],
                bytes32(document["reader_content_sha256"]),
                bytes32(document["resource_id"]),
                document["role"],
                document["evidence_kind"],
            ]
            for document in reader_documents
        ],
        [[row["locator"], bytes32(row["content_sha256"])] for row in producer_only],
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
    ]
    surface, _ = stage0_commitment(
        "maestro.vnext.current-surface-manifest.v1",
        surface_value,
        inventory_sha256=inventory_hash(inventory),
        inventory_validation=proof_stable_inventory_validation(validation),
        resource_count=len(resource_documents),
        direct_reader_edge_count=len(reader_documents),
        generated_reference_producer_count=len(producer_only),
        resources=resource_documents,
        direct_readers=reader_documents,
        generated_reference_producers=producer_only,
        historical_completeness_evidence=historical,
        unclassified_paths=[],
        generated_output_policy={
            "classification": "post_release_noncanonical",
            "path_byte_and_presence_identity_participation": False,
            "root_worker_post_root_delta_owner": True,
        },
    )
    consumer_value = [
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
            for row in reader_documents
        ],
    ]
    consumers, _ = stage0_commitment(
        "maestro.vnext.current-consumer-census.v1",
        consumer_value,
        consumer_inventory_digest=consumer_inventory_digest(inventory),
        resource_count=len(resources),
        direct_reader_edge_count=len(reader_documents),
        exact_one_reader_evidence_per_resource=True,
        readers=reader_documents,
        historical_c325_promoted=False,
    )

    schema_root = ROOT / "embedded/schemas"
    persistence_paths = sorted(
        [path.relative_to(ROOT).as_posix() for path in schema_root.glob("*/current.yaml")]
        + [path.relative_to(ROOT).as_posix() for path in schema_root.glob("*/supported.yaml")]
    )
    archive_paths = sorted(path.relative_to(ROOT).as_posix() for path in schema_root.glob("*/retired.yaml"))
    fixture_paths = sorted(path.relative_to(ROOT).as_posix() for path in schema_root.glob("*/fixtures/*") if path.is_file())
    if [len(persistence_paths), len(archive_paths), len(fixture_paths)] != [22, 11, 22]:
        raise BuildError("exact persistence/archive/golden-fixture counts changed")

    reader_specs = {
        "persistence": (
            "src/domain/schema_contracts/catalog.rs#packs",
            "src/domain/schema_contracts/validate.rs#violations",
        ),
        "archive": (
            "src/domain/schema_contracts/catalog.rs#packs",
            "src/domain/schema_contracts/validate.rs#violations",
        ),
        "fixtures": (
            "src/domain/schema_contracts/catalog.rs#packs",
            "src/domain/schema_contracts/validate.rs#violations",
            "tests/schema_fixture_harness.rs#fixture",
        ),
    }

    def schema_manifest(
        *, name: str, schema: str, paths: Sequence[str], purpose: str
    ) -> dict[str, Any]:
        reader_rows = []
        for locator in reader_specs[name]:
            file_locator, _, symbol = locator.partition("#")
            text = (ROOT / file_locator).read_text()
            if symbol not in text:
                raise BuildError(f"exact schema reader symbol is missing: {locator}")
            reader_rows.append(
                {
                    "reader_locator": locator,
                    "reader_content_sha256": file_sha(file_locator),
                    "evidence_kind": "typed_runtime_or_fixture_reader",
                }
            )
        rows = []
        for path in paths:
            record = by_locator.get(path)
            if record is None or file_sha(path) != record["content_sha256"]:
                raise BuildError(f"schema surface lacks exact Resource content binding: {path}")
            rows.append(
                {
                    "path": path,
                    "content_sha256": record["content_sha256"],
                    "content_byte_length": record["content_byte_length"],
                    "resource_id": record["resource_id"],
                    "resource_tag": record["inventory_ordinal"],
                    "readers": reader_rows,
                }
            )
        value = [
            1,
            purpose,
            [
                [
                    row["resource_tag"],
                    bytes32(row["resource_id"]),
                    row["path"],
                    bytes32(row["content_sha256"]),
                    [[reader["reader_locator"], bytes32(reader["reader_content_sha256"])] for reader in reader_rows],
                ]
                for row in rows
            ],
        ]
        document, _ = stage0_commitment(
            schema,
            value,
            purpose=purpose,
            exact_count=len(rows),
            rows=rows,
            reader_set=reader_rows,
        )
        return document

    persistence = schema_manifest(
        name="persistence",
        schema="maestro.vnext.current-persistence-manifest.v1",
        paths=persistence_paths,
        purpose="current_and_supported_persistence_contract",
    )
    archive = schema_manifest(
        name="archive",
        schema="maestro.vnext.current-archive-manifest.v1",
        paths=archive_paths,
        purpose="retired_name_archive_contract",
    )
    fixtures = schema_manifest(
        name="fixtures",
        schema="maestro.vnext.golden-fixture-manifest.v1",
        paths=fixture_paths,
        purpose="golden_fixture_runtime_reader_contract",
    )
    migration_value = [
        1,
        [
            [
                row["resource_tag"],
                bytes32(row["resource_id"]),
                bytes32(str(row["migration_profile_id"])),
                bytes32(str(row["rollback_profile_id"])),
                bytes32(str(row["removal_profile_id"])),
                bytes32(str(row["proof_profile_id"])),
                DISPOSITION_TAGS[row["disposition"]],
                row["migration_execution_status"],
                row["rollback_rehearsal_status"],
                row["removal_execution_status"],
                row["runtime_proof_complete"],
            ]
            for row in requirement_rows
        ],
        False,
    ]
    migration, _ = stage0_commitment(
        "maestro.vnext.migration-rollback-requirements.v1",
        migration_value,
        resource_count=len(requirement_rows),
        requirements=list(requirement_rows),
        stage="stage0_candidate_only",
        status="requirements_complete_runtime_proof_pending",
        stage0_execution_complete=False,
        stage0_rehearsal_complete=False,
        runtime_proof_complete=False,
        pending_runtime_proof_count=len(requirement_rows),
        proof_status="pending_stage0_execution_and_rehearsal",
    )
    documents = {
        "current-surface-manifest.v1.json": surface,
        "current-consumer-census.v1.json": consumers,
        "current-persistence-manifest.v1.json": persistence,
        "current-archive-manifest.v1.json": archive,
        "golden-fixture-manifest.v1.json": fixtures,
        "migration-rollback-requirements.v1.json": migration,
    }
    return documents, {name: json_bytes(document) for name, document in documents.items()}


def ast_string_binding(locator: str, symbol: str, literal: str) -> dict[str, Any]:
    path = ROOT / locator
    source = path.read_text()
    tree = ast.parse(source, filename=locator)
    matches: list[ast.AST] = []
    for node in tree.body:
        targets: list[ast.expr] = []
        if isinstance(node, ast.Assign):
            targets = node.targets
        elif isinstance(node, ast.AnnAssign):
            targets = [node.target]
        if any(isinstance(target, ast.Name) and target.id == symbol for target in targets):
            matches.append(node)
    if len(matches) != 1 or literal not in {
        node.value for node in ast.walk(matches[0]) if isinstance(node, ast.Constant) and isinstance(node.value, str)
    }:
        raise BuildError(f"AST binding {locator}#{symbol} does not contain exact literal {literal}")
    return {
        "reader_locator": f"{locator}#{symbol}",
        "reader_content_sha256": sha256(path.read_bytes()),
        "evidence_kind": "python_ast_exact_string_constant",
        "literal": literal,
    }


def build_generated_output_bindings(
    *, delta_id: str, delta_json: bytes, delta_cbor: bytes, release_id: str
) -> list[dict[str, Any]]:
    bindings = []
    for path in EFFECT_OUTPUT_PATHS:
        readers = [
            ast_string_binding(
                "tools/vnext_contracts/stage0/effect_home/build.py",
                "DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS",
                path,
            ),
            ast_string_binding(
                "tools/vnext_contracts/stage0/effect_home/validate.py",
                "DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS",
                path,
            ),
        ]
        if path.endswith("expected-delta-successor.v1.json"):
            readers.append(
                ast_string_binding(
                    "tools/vnext_contracts/stage0/candidate_root/build.py",
                    "RESOURCE_SUCCESSOR_DELTA",
                    "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.json",
                )
            )
        elif path.endswith("resource-release.v1.json"):
            readers.append(
                ast_string_binding(
                    "tools/vnext_contracts/stage0/candidate_root/build.py",
                    "RESOURCE_RELEASE",
                    "contracts/vnext/stage0/resource-release/resource-release.v1.json",
                )
            )
        encoding = "CanonicalCbor" if path.endswith(".cbor") else "CanonicalJson"
        exact_content_sha256 = (
            sha256(delta_cbor)
            if path.endswith(".cbor")
            else sha256(delta_json) if "expected-delta" in path else None
        )
        content_binding = "ExactRenderedBytes" if exact_content_sha256 else "ExternalByteReceiptAfterRender"
        producer_identity = delta_id if "expected-delta" in path else release_id
        canonical = [
            1,
            path,
            bytes32(producer_identity),
            encoding,
            content_binding,
            [1, bytes32(exact_content_sha256)] if exact_content_sha256 else [0],
            [[row["reader_locator"], bytes32(row["reader_content_sha256"])] for row in readers],
        ]
        bindings.append(
            {
                "binding_id": profile_commitment_value(canonical),
                "logical_path": path,
                "producer_identity": f"sha256:{producer_identity}",
                "encoding": encoding,
                "content_binding": content_binding,
                "exact_content_sha256": exact_content_sha256,
                "readers": readers,
                "removal_obligations": [],
                "canonical_value": canonical,
            }
        )
    if [row["logical_path"] for row in bindings] != list(EFFECT_OUTPUT_PATHS):
        raise BuildError("exact three generated-output bindings changed")
    return bindings


def build_delta(
    *,
    generated: Mapping[str, bytes],
    c868: Mapping[str, Any],
    writer: Mapping[str, Any],
    resources: Sequence[ResourceDescriptor],
    resource_records: Sequence[dict[str, Any]],
    bundles: Sequence[BundleManifest],
    bundle_records: Sequence[dict[str, Any]],
    census: ReleaseResourceCensus,
    census_document: Mapping[str, Any],
    release: EmbeddedReleaseBundle,
    release_document: Mapping[str, Any],
) -> tuple[dict[str, Any], bytes, list[dict[str, str]], dict[str, str]]:
    catalog_inventory = read_json(ROOT / "contracts/vnext/catalogs/generated/inventory.json")
    grammar_id = exact_hash(catalog_inventory["grammar_id"])
    catalog_artifacts = [row for row in catalog_inventory["artifacts"] if row["kind"] != "grammar"]
    catalog_ids = [exact_hash(row["identity"]) for row in catalog_artifacts]
    public_identity_path = "contracts/vnext/stage0/public-identity/public-identity-closure.v1.json"
    public_identity = read_json(ROOT / public_identity_path)
    public_closure_id = exact_hash(public_identity["closure_id"])
    public_manifest = public_identity["manifest"]
    public_manifest_id = exact_hash(
        public_manifest["manifest_id"] if isinstance(public_manifest, dict) else public_manifest
    )
    public_successor_id = domain_identity(
        "maestro.vnext.7138-public-contract-successor.v1",
        [1, bytes32(public_closure_id), bytes32(c868["manifest_id"])],
    )
    catalog_closure_id = domain_identity(
        "maestro.vnext.efa0-core-catalog-closure.v1",
        [1, bytes32(grammar_id), [bytes32(identity) for identity in catalog_ids]],
    )
    decision_path = "contracts/vnext/stage0/decision-closure/decision-closure.v1.json"
    decision = read_json(ROOT / decision_path)
    d116 = next(row for row in decision["records"] if row["id"] == "dec-canonical-typed-recoverreserved-d116")
    d116_id = domain_identity(
        "maestro.vnext.d116-bounded-recovery-successor.v1",
        [1, bytes32(d116["raw_body_sha256"]), [bytes32(identity) for identity in catalog_ids]],
    )
    final_effect_path = "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"
    final_effect = read_json(ROOT / final_effect_path)
    effect_ids = {
        "effect_control_h2": exact_hash(final_effect["h2_manifest_identity"]),
        "local_withdrawal_h3": exact_hash(final_effect["h3_withdrawal_identity"]),
        "effect_finalization": exact_hash(final_effect["identity"]),
        "effect_expected_delta": exact_hash(final_effect["expected_delta_manifest_id"]),
        "effect_consumer_census": exact_hash(final_effect["semantic_consumer_census_id"]),
    }

    entries: list[dict[str, Any]] = []

    def add(
        identity_kind: str,
        logical_key: str,
        successor: str,
        disposition: str,
        source_artifact: str,
        source_artifact_sha256: str,
        predecessor: str | None = None,
    ) -> None:
        successor = exact_hash(successor)
        predecessor = exact_hash(predecessor) if predecessor else None
        if disposition == "Introduce" and predecessor is not None:
            raise BuildError(f"Introduce row has predecessor: {logical_key}")
        if disposition in {"Preserve", "Rotate", "Retire"} and predecessor is None:
            raise BuildError(f"{disposition} row lacks predecessor: {logical_key}")
        if disposition == "Preserve" and predecessor != successor:
            raise BuildError(f"Preserve row rotates: {logical_key}")
        if disposition == "Rotate" and predecessor == successor:
            raise BuildError(f"Rotate row preserves: {logical_key}")
        entries.append(
            {
                "identity_kind": identity_kind,
                "logical_key": logical_key,
                "predecessor_identity": f"sha256:{predecessor}" if predecessor else None,
                "successor_identity": f"sha256:{successor}",
                "disposition": disposition,
                "source_artifact": source_artifact,
                "source_artifact_sha256": exact_hash(source_artifact_sha256),
            }
        )

    public_sha = file_sha(public_identity_path)
    for descriptor in public_identity["schema_descriptors"]:
        add(
            "Schema",
            f"schema:public:{descriptor['schema_name']}@{descriptor['schema_version']}",
            descriptor["schema_id"],
            "Introduce",
            public_identity_path,
            public_sha,
        )
    for name, schema_id in sorted(FROZEN_SCHEMA_IDS.items()):
        add(
            "Schema",
            f"schema:c868:{name}@1",
            schema_id,
            "Preserve",
            "frozen:vnext-resource-contract-suite-v1.json",
            FROZEN_SOURCE_SHA256["suite"],
            schema_id,
        )
    add("Manifest", "manifest:public-identity", public_manifest_id, "Introduce", public_identity_path, public_sha)
    add(
        "Manifest",
        "manifest:catalog-profile-grammar",
        grammar_id,
        "Rotate",
        "contracts/vnext/catalogs/generated/catalog-profile-grammar-v1.json",
        file_sha("contracts/vnext/catalogs/generated/catalog-profile-grammar-v1.json"),
        OLD_GRAMMAR_ID,
    )
    for predecessor, artifact in zip(OLD_CATALOG_IDS, catalog_artifacts, strict=True):
        source = f"contracts/vnext/catalogs/generated/{artifact['path']}"
        add(
            "Manifest",
            f"manifest:catalog:{artifact['kind']}",
            artifact["identity"],
            "Rotate",
            source,
            file_sha(source),
            predecessor,
        )
    add(
        "Manifest",
        "manifest:c868-resource-contract-suite",
        c868["manifest_id"],
        "Rotate",
        "contracts/vnext/stage0/resource-release/c868-successor.v1.json",
        sha256(generated["c868-successor.v1.json"]),
        FROZEN_SUITE_MANIFEST_ID,
    )
    add(
        "Manifest",
        "manifest:c868-runtime-edge-contract",
        FROZEN_RUNTIME_EDGE_MANIFEST_ID,
        "Preserve",
        "contracts/vnext/stage0/resource-release/c868-successor.v1.json",
        sha256(generated["c868-successor.v1.json"]),
        FROZEN_RUNTIME_EDGE_MANIFEST_ID,
    )
    add(
        "Manifest",
        "manifest:writer-compatibility",
        writer["manifest_id"],
        "Rotate",
        "contracts/vnext/stage0/resource-release/writer-compatibility-successor.v1.json",
        sha256(generated["writer-compatibility-successor.v1.json"]),
        C65_MANIFEST_ID,
    )
    for key in (
        "schema_read_write_set_descriptor_id",
        "writer_protocol_epoch_id",
        "migration_epoch_id",
        "finality_edge_manifest_id",
    ):
        predecessor = writer["predecessor_components"][key]
        successor = writer[key]
        add(
            "Manifest",
            f"manifest:writer:{key}",
            successor,
            "Preserve" if exact_hash(predecessor) == exact_hash(successor) else "Rotate",
            "contracts/vnext/stage0/resource-release/writer-compatibility-successor.v1.json",
            sha256(generated["writer-compatibility-successor.v1.json"]),
            predecessor,
        )
    for logical_key, successor, source in (
        ("manifest:public-transport-7138", public_successor_id, public_identity_path),
        ("manifest:bounded-recovery-d116", d116_id, decision_path),
        ("manifest:catalog-owner-efa0", catalog_closure_id, "contracts/vnext/catalogs/generated/inventory.json"),
        ("manifest:effect-control-h2", effect_ids["effect_control_h2"], final_effect_path),
        ("manifest:effect-withdrawal-h3", effect_ids["local_withdrawal_h3"], final_effect_path),
        ("manifest:effect-finalization", effect_ids["effect_finalization"], final_effect_path),
        ("manifest:effect-expected-delta", effect_ids["effect_expected_delta"], final_effect_path),
        ("manifest:effect-consumer-census", effect_ids["effect_consumer_census"], final_effect_path),
    ):
        add("Manifest", logical_key, successor, "Introduce", source, file_sha(source))
    for resource, record in zip(resources, resource_records, strict=True):
        add(
            "Resource",
            f"resource:{record['stable_resource_key']}",
            resource.resource_id,
            "Introduce",
            record["stable_locator"],
            record["content_sha256"],
        )
    for bundle, record in zip(bundles, bundle_records, strict=True):
        artifact_path = record["artifact_path"]
        artifact_name = Path(artifact_path).name
        artifact_document = {key: value for key, value in record.items() if key != "artifact_path"}
        add(
            "Bundle",
            f"bundle:{record['stable_bundle_group']}",
            bundle.bundle_id,
            "Introduce",
            artifact_path,
            sha256(json_bytes(artifact_document)),
        )
    add(
        "Census",
        "census:release-resources",
        census.census_id,
        "Introduce",
        "contracts/vnext/stage0/resource-release/release-resource-census.v1.json",
        sha256(json_bytes(census_document)),
    )
    add(
        "Release",
        "release:embedded-candidate",
        release.release_id,
        "Introduce",
        "contracts/vnext/stage0/resource-release/embedded-release-bundle.v1.json",
        sha256(json_bytes(release_document)),
    )
    entries.sort(key=lambda row: (IDENTITY_KIND_TAGS[row["identity_kind"]], row["logical_key"]))
    keys = [(row["identity_kind"], row["logical_key"]) for row in entries]
    if len(keys) != len(set(keys)):
        raise BuildError("expected delta contains duplicate identity-kind/logical-key rows")
    canonical_entries = [
        [
            index,
            IDENTITY_KIND_TAGS[row["identity_kind"]],
            row["logical_key"],
            [1, bytes32(row["predecessor_identity"])] if row["predecessor_identity"] else [0],
            bytes32(row["successor_identity"]),
            DELTA_DISPOSITION_TAGS[row["disposition"]],
            row["source_artifact"],
            bytes32(row["source_artifact_sha256"]),
        ]
        for index, row in enumerate(entries, 1)
    ]
    obligations = [
        {
            "identity_kind": kind,
            "logical_key": key,
            "predecessor_identity": None,
            "successor_identity": None,
            "disposition": "Introduce",
            "depends_on_release_identity": f"sha256:{release.release_id}",
            "status": "pending_downstream_stage0_producer",
            "owner": "candidate-root-worker",
        }
        for kind, key in (
            ("RootInput", "candidate-root"),
            ("RootInput", "candidate-finalization"),
            ("HandoffInput", "candidate-handoff"),
        )
    ]
    canonical_obligations = [
        [
            IDENTITY_KIND_TAGS[row["identity_kind"]],
            row["logical_key"],
            [0],
            [0],
            DELTA_DISPOSITION_TAGS[row["disposition"]],
            bytes32(release.release_id),
            row["status"],
            row["owner"],
        ]
        for row in obligations
    ]
    exact_counts = {
        kind: sum(row["identity_kind"] == kind for row in entries)
        for kind in ("Schema", "Manifest", "Resource", "Bundle", "Census", "Release")
    }
    if exact_counts != {
        "Schema": len(public_identity["schema_descriptors"]) + 38,
        "Manifest": 26,
        "Resource": 412,
        "Bundle": 8,
        "Census": 1,
        "Release": 1,
    }:
        raise BuildError(f"through-Release exact identity-kind coverage changed: {exact_counts}")
    value = [
        1,
        "maestro.vnext.exact-identity-delta.v4",
        canonical_entries,
        canonical_obligations,
        [0],
        [0],
    ]
    delta, delta_cbor = stage0_commitment(
        "maestro.vnext.migration-cutover-expected-delta-successor.v1",
        value,
        publication_status="resolved_through_release_downstream_obligations_pending",
        resolved_entry_count=len(entries),
        blocked_dependency_count=3,
        unresolved_obligation_count=3,
        entries=entries,
        downstream_obligations=obligations,
        exact_identity_kind_counts=exact_counts,
        post_root_delta_identity=None,
        post_root_union_identity=None,
        post_root_status="pending_root_worker_noncanonical_delta_and_union",
        post_root_identity_feedback_into_resource_bundle_census_release=False,
    )
    successor_bindings = [
        {"slot_name": name, "successor_identity": f"sha256:{identity}"}
        for name, identity in (
            ("public_transport_7138", public_successor_id),
            ("grammar_catalog_d116", d116_id),
            ("effect_control_h2", effect_ids["effect_control_h2"]),
            ("local_withdrawal_h3", effect_ids["local_withdrawal_h3"]),
            ("catalog_owner_efa0", catalog_closure_id),
            ("resource_bundle_c868", exact_hash(c868["manifest_id"])),
            ("release_binding", release.release_id),
            ("writer_compatibility", exact_hash(writer["writer_protocol_epoch_id"])),
        )
    ]
    supporting_ids = {
        "public_successor_id": public_successor_id,
        "d116_id": d116_id,
        "catalog_closure_id": catalog_closure_id,
        **effect_ids,
    }
    return delta, delta_cbor, successor_bindings, supporting_ids


def build_resource_release(
    *,
    inventory: CurrentInventory,
    validation: Any,
    c868: Mapping[str, Any],
    writer: Mapping[str, Any],
    resources: Sequence[ResourceDescriptor],
    resource_records: Sequence[dict[str, Any]],
    bundles: Sequence[BundleManifest],
    bundle_records: Sequence[dict[str, Any]],
    census: ReleaseResourceCensus,
    census_document: Mapping[str, Any],
    release: EmbeddedReleaseBundle,
    release_document: Mapping[str, Any],
    delta: dict[str, Any],
    delta_cbor: bytes,
    successor_bindings: Sequence[dict[str, str]],
    audits: Mapping[str, dict[str, Any]],
    requirement_rows: Sequence[dict[str, Any]],
) -> tuple[dict[str, Any], bytes, dict[str, Any], bytes, list[dict[str, Any]]]:
    descriptor_value = [
        1,
        [[row.resource_tag, bytes32(row.resource_id)] for row in resources],
    ]
    descriptor_set, descriptor_cbor = stage0_commitment(
        "maestro.vnext.stage0.resource-descriptor-set.v1",
        descriptor_value,
        descriptor_domain="maestro.vnext.resource.descriptor.v1",
        descriptor_schema_id=FROZEN_SCHEMA_IDS["ResourceDescriptorV1"],
        resource_count=len(resources),
        resources=list(resource_records),
    )
    delta_json = json_bytes(delta)
    delta_id = exact_hash(delta["identity"])
    generated_bindings = build_generated_output_bindings(
        delta_id=delta_id,
        delta_json=delta_json,
        delta_cbor=delta_cbor,
        release_id=release.release_id,
    )
    audit_ids = [[name, bytes32(exact_hash(document["identity"]))] for name, document in sorted(audits.items())]
    value = [
        1,
        bytes32(exact_hash(descriptor_set["identity"])),
        [[row.bundle_tag, bytes32(row.bundle_id)] for row in bundles],
        bytes32(census.census_id),
        bytes32(release.release_id),
        bytes32(delta_id),
        audit_ids,
        [bytes32(row["binding_id"]) for row in generated_bindings],
        [0],
        [0],
        False,
    ]
    final_effect_path = "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"
    final_effect = read_json(ROOT / final_effect_path)
    closure, closure_cbor = stage0_commitment(
        "maestro.vnext.stage0.resource-release.v1",
        value,
        source_publication=False,
        runtime_registration=False,
        installation=False,
        resource_descriptor_set_identity=descriptor_set["identity"],
        resource_count=len(resources),
        resources=list(resource_records),
        bundle_count=len(bundles),
        bundles=list(bundle_records),
        release_resource_census=dict(census_document),
        embedded_release_bundle=dict(release_document),
        expected_delta=delta,
        resolved_expected_delta_commitment_id=delta["identity"],
        downstream_delta_obligations=delta["downstream_obligations"],
        downstream_generated_output_bindings=generated_bindings,
        declared_successor_slot_count=8,
        resolved_successor_slot_count=8,
        blocked_successor_slot_count=0,
        null_successor_identity_count=0,
        resolved_successor_bindings=list(successor_bindings),
        predecessor_reproduction={
            "c868": {
                "artifact_sha256": FROZEN_SOURCE_SHA256["suite"],
                "manifest_id": FROZEN_SUITE_MANIFEST_ID,
                "exact_five_source_verification": True,
            },
            "migration_cutover_65b3": {
                "artifact_sha256": C65_SOURCE_SHA256,
                "manifest_id": C65_MANIFEST_ID,
                "canonical_bytes_reproduced": True,
            },
        },
        successor_closure={
            "c868_manifest_id": c868["manifest_id"],
            "c868_runtime_edge_manifest_id": c868["runtime_edge_manifest_id"],
            "migration_cutover_manifest_id": writer["manifest_id"],
            "writer_protocol_epoch_id": writer["writer_protocol_epoch_id"],
            "expected_delta_commitment_id": delta["identity"],
            "release_id": release.release_id,
        },
        effect_home_finalization_receipt_sha256=file_sha(final_effect_path),
        effect_home_finalization_identity=final_effect["identity"],
        effect_home_expected_delta_manifest_id=final_effect["expected_delta_manifest_id"],
        current_surface_manifest=audits["current-surface-manifest.v1.json"],
        current_consumer_census=audits["current-consumer-census.v1.json"],
        current_persistence_manifest=audits["current-persistence-manifest.v1.json"],
        current_archive_manifest=audits["current-archive-manifest.v1.json"],
        golden_fixture_manifest=audits["golden-fixture-manifest.v1.json"],
        migration_rollback_requirements=audits["migration-rollback-requirements.v1.json"],
        exact_source_counts={
            "resources": len(resources),
            "direct_reader_edges": len(inventory.direct_readers),
            "bundle_instances": len(bundles),
            "bundle_kinds": len(set(row.bundle_kind for row in bundles)),
            "current_persistence_descriptors": 22,
            "current_archive_descriptors": 11,
            "current_golden_fixtures": 22,
            "c868_schemas": 38,
            "c868_suite_components": 62,
            "c868_runtime_edges": 61,
        },
        inventory_sha256=inventory_hash(inventory),
        inventory_validation=proof_stable_inventory_validation(validation),
        migration_requirement_count=len(requirement_rows),
        migration_runtime_proof_complete=False,
        post_root_delta_identity=None,
        post_root_union_identity=None,
        post_root_status="pending_root_worker_noncanonical_delta_and_union",
        post_root_identity_feedback_into_resource_bundle_census_release=False,
    )
    return closure, closure_cbor, descriptor_set, descriptor_cbor, generated_bindings


def receipt_row(document: Mapping[str, Any], raw: bytes) -> dict[str, Any]:
    return {
        "identity_protocol": document["identity_protocol"],
        "identity": document["identity"],
        "canonical_cbor_sha256": sha256(raw),
        "canonical_cbor_byte_length": len(raw),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    required = (
        "contracts/vnext/catalogs/generated/inventory.json",
        "contracts/vnext/stage0/public-identity/public-identity-closure.v1.json",
        "contracts/vnext/stage0/decision-closure/decision-closure.v1.json",
        "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json",
        "embedded/vnext/capability/instruction-tree.v1.json",
        "embedded/vnext/orchestration/recipe-catalog.v1.json",
        "embedded/vnext/adapter/mcp-tools.v1.json",
        "embedded/vnext/bootstrap/MAESTRO.md",
        "embedded/vnext/release/release-policy.v1.json",
    )
    missing = [locator for locator in required if not (ROOT / locator).is_file()]
    if missing:
        print(json.dumps({"status": "blocked_missing_identities", "missing_dependencies": missing}, sort_keys=True))
        return 2

    generated, c868, writer = build_preidentity_outputs()
    preidentity_mismatches = freeze_preidentity_outputs(generated, args.check)
    if preidentity_mismatches:
        print(
            json.dumps(
                {
                    "status": "mismatch",
                    "mode": "check",
                    "phase": "preidentity_inputs",
                    "mismatches": preidentity_mismatches,
                },
                sort_keys=True,
            )
        )
        return 1

    inventory = build_current_inventory(ROOT)
    inventory_validation = validate_inventory(inventory)
    resources, resource_records, requirement_rows = build_resources(
        inventory,
        c868_manifest_id=c868["manifest_id"],
        writer_manifest_id=writer["manifest_id"],
    )
    bundles, bundle_records, bundle_outputs = build_bundles(resources, inventory)
    direct_consumers, direct_consumer_records = build_direct_consumers(inventory, resources)
    catalog_inventory = read_json(ROOT / "contracts/vnext/catalogs/generated/inventory.json")
    grammar_id = exact_hash(catalog_inventory["grammar_id"])
    catalog_ids = [
        exact_hash(row["identity"])
        for row in catalog_inventory["artifacts"]
        if row["kind"] != "grammar"
    ]
    public_catalog_id = domain_identity(
        "maestro.vnext.efa0-core-catalog-closure.v1",
        [1, bytes32(grammar_id), [bytes32(identity) for identity in catalog_ids]],
    )
    census, release, census_document, release_document = build_census_and_release(
        inventory=inventory,
        resources=resources,
        bundles=bundles,
        direct_consumers=direct_consumers,
        direct_consumer_records=direct_consumer_records,
        c868_manifest_id=c868["manifest_id"],
        writer_manifest_id=writer["manifest_id"],
        public_catalog_id=public_catalog_id,
    )
    audits, audit_outputs = build_current_surface_audits(
        inventory=inventory,
        validation=inventory_validation,
        resources=resources,
        resource_records=resource_records,
        requirement_rows=requirement_rows,
    )
    delta, delta_cbor, successor_bindings, _supporting = build_delta(
        generated=generated,
        c868=c868,
        writer=writer,
        resources=resources,
        resource_records=resource_records,
        bundles=bundles,
        bundle_records=bundle_records,
        census=census,
        census_document=census_document,
        release=release,
        release_document=release_document,
    )
    closure, closure_cbor, descriptor_set, descriptor_cbor, generated_bindings = build_resource_release(
        inventory=inventory,
        validation=inventory_validation,
        c868=c868,
        writer=writer,
        resources=resources,
        resource_records=resource_records,
        bundles=bundles,
        bundle_records=bundle_records,
        census=census,
        census_document=census_document,
        release=release,
        release_document=release_document,
        delta=delta,
        delta_cbor=delta_cbor,
        successor_bindings=successor_bindings,
        audits=audits,
        requirement_rows=requirement_rows,
    )
    suite = frozen_c868()
    c65_source = read_json(FROZEN / "vnext-migration-cutover-contract-v1.json")
    outputs = {
        **generated,
        **bundle_outputs,
        **audit_outputs,
        "resource-descriptors.v1.json": json_bytes(descriptor_set),
        "resource-descriptors.v1.cbor": descriptor_cbor,
        "release-resource-census.v1.json": json_bytes(census_document),
        "release-resource-census.v1.cbor": census.canonical_cbor,
        "embedded-release-bundle.v1.json": json_bytes(release_document),
        "embedded-release-bundle.v1.cbor": release.canonical_cbor,
        "expected-delta-successor.v1.json": json_bytes(delta),
        "expected-delta-successor.v1.cbor": delta_cbor,
        "resource-release.v1.json": json_bytes(closure),
        "resource-release.v1.cbor": closure_cbor,
        "predecessor-resource-contract-suite-v1.json": (FROZEN / "vnext-resource-contract-suite-v1.json").read_bytes(),
        "predecessor-resource-contract-suite-v1.cbor": bytes.fromhex(suite["cbor_hex"]),
        "predecessor-migration-cutover-contract-v1.json": (FROZEN / "vnext-migration-cutover-contract-v1.json").read_bytes(),
        "predecessor-migration-cutover-contract-v1.cbor": bytes.fromhex(c65_source["cbor_hex"]),
    }
    mismatches: list[str] = []
    for name, raw in sorted(outputs.items()):
        write_or_check(OUT / name, raw, args.check, mismatches)

    ruby_receipt: dict[str, Any] | None = None
    if not mismatches:
        try:
            process = subprocess.run(
                ["/usr/bin/ruby", "tools/vnext_contracts/stage0/resource_release/verify.rb"],
                cwd=ROOT,
                check=True,
                capture_output=True,
                text=True,
                env=proof_environment(),
            )
        except subprocess.CalledProcessError as error:
            raise BuildError(f"independent Ruby verifier failed: {error.stderr.strip()}") from error
        ruby_receipt = json.loads(process.stdout)
        python_artifacts = {
            "resource-descriptors.v1": receipt_row(descriptor_set, descriptor_cbor),
            **{
                f"bundle-{bundle.bundle_tag:03d}": receipt_row(record, bundle.canonical_cbor)
                for bundle, record in zip(bundles, bundle_records, strict=True)
            },
            "release-resource-census.v1": receipt_row(census_document, census.canonical_cbor),
            "embedded-release-bundle.v1": receipt_row(release_document, release.canonical_cbor),
            "expected-delta-successor.v1": receipt_row(delta, delta_cbor),
            "resource-release.v1": receipt_row(closure, closure_cbor),
        }
        if ruby_receipt.get("artifacts") != python_artifacts:
            raise BuildError("Python and Ruby disagree on the exact Resource/Release artifact receipts")
        external_receipts = {
            row["logical_path"]: {
                "binding_id": row["binding_id"],
                "sha256": sha256(outputs[Path(row["logical_path"]).name]),
                "byte_length": len(outputs[Path(row["logical_path"]).name]),
            }
            for row in generated_bindings
        }
        if ruby_receipt.get("generated_output_byte_receipts") != external_receipts:
            raise BuildError("Python and Ruby disagree on generated-output byte receipts")
        encoder_receipt = {
            "schema": "maestro.vnext.resource-release-independent-encoder-receipt.v1",
            "status": "pass",
            "artifact_set_equal": True,
            "equality": "exact_protocol_identity_cbor_hash_and_byte_length",
            "python": {
                "encoder": "python-primary",
                "encoder_source_sha256": sha256(Path(__file__).read_bytes()),
                "artifacts": python_artifacts,
            },
            "generated_output_byte_receipts": external_receipts,
            "ruby": ruby_receipt,
        }
        write_or_check(OUT / "encoder-receipt.v1.json", json_bytes(encoder_receipt), args.check, mismatches)
    print(
        json.dumps(
            {
                "status": "pass" if not mismatches else "mismatch",
                "mode": "check" if args.check else "write",
                "identity": closure["identity"],
                "release_id": release.release_id,
                "census_id": census.census_id,
                "resource_count": len(resources),
                "bundle_count": len(bundles),
                "direct_consumer_count": len(direct_consumers),
                "blocked_dependency_count": 3,
                "inventory_sha256": inventory_hash(inventory),
                "independent_encoder": ruby_receipt,
                "mismatches": mismatches,
            },
            sort_keys=True,
        )
    )
    return 1 if mismatches else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (BuildError, KeyError, ValueError) as error:
        print(json.dumps({"status": "fail", "error": str(error)}, sort_keys=True), file=sys.stderr)
        raise SystemExit(1)
