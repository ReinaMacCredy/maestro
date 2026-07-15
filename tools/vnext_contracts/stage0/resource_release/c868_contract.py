"""Frozen C868 Resource/Bundle/Census/Release identity contracts.

This module is deliberately pure: callers supply bytes and typed values.  It
does not discover paths, read ambient files, or write validation scratch data.
"""

from __future__ import annotations

import copy
import hashlib
import json
import unittest
from dataclasses import dataclass, replace
from typing import Any, Iterable, Mapping, Sequence


class ContractError(ValueError):
    """A value violates the frozen C868 or ManifestIdentityV1 contract."""


FROZEN_SOURCE_SHA256 = {
    "suite": "d55e34610d888fca3ec6995820e50fe744332748fe28b766be4c64bbd2672622",
    "builder": "8b19ef31c3dfa83114576665884ad3b5808057660d4b0863097219ea063edbd9",
    "validator": "f32773e1f3b3bc65c86873e5b7841cac56d40f243d136bb45ce20c436ee50f99",
    "suite_envelope": "efe2238107a0f870f9fa967eaef5ef18c68109e7326557e6d259bf4630e344eb",
    "runtime_edge_envelope": "ad7c6136b85aaf7fcccb924fe0046088adcac14044a301090f52550f18ca6974",
}
FROZEN_SUITE_MANIFEST_ID = "5057a0a8e088a0dd394106b928f9b1c4c7a356040b23363455de1177e33e013f"
FROZEN_RUNTIME_EDGE_MANIFEST_ID = "917376f49f5ed01ab53a7a71f1527fc0b3fc03d2632b47b68333cf2ba7899fe2"
FROZEN_SUITE_CBOR_LENGTH = 8181
FROZEN_SCHEMA_COUNT = 38
FROZEN_COMPONENT_COUNT = 62
FROZEN_RUNTIME_EDGE_COUNT = 61

FROZEN_SCHEMA_IDS = {
    "ManifestHeaderCoreV1": "c44af35107292d936b693fcc1375e4ee082b657feea14c2fd4cb9f718ef93b8d",
    "OwnerRefV1": "bcd5818874d4827383819cd4673e7fcce8e25b635dae3f39168ce71cef66b6a7",
    "DistributionDomainRefV1": "87ee40e772df0915f86568d2c85e29eb06882d2e2c9f68ec3fbbdb3d46a9e792",
    "DistributionScopedObjectRefV1": "01534e1c233047cb4ce25ddf0317cccacd230d244876d6ff7b8c9d6f98062c0a",
    "ResourceDescriptorV1": "78cc56e71ae16fa2539429601fb08e37970d32569d0fddfd12c2129b6344bcc9",
    "BundleManifestHeaderV1": "ab811246b97d67ed2414723b046cfa29734cd7e7060114592ec6bd74d6cf8f63",
    "BundleManifestV1": "f2d7bb5d5b5ba81fed67b3d1e25c89285c893aa57491f59212d34c7fb51c5dd2",
    "ReleaseBundleMembershipV1": "d99e73adb9ffb8db858424033eb9afabc9ceb3f17870ee3be14d8176daf56e77",
    "EmbeddedReleaseHeaderV1": "2eb8a479550b066cd39e472f6db303d43df256b7ca621e397c435bc2f5ac24a5",
    "EmbeddedReleaseBundleV1": "0f64450315ba74ce5206d378e71a8d6c63041631d34a59b905af0e2f55a5721f",
    "ReleaseCensusResourceV1": "fef2d185bda28eed49c8dc3288d720a7ec0bb43f38049501f46fc2284a1d8237",
    "ReleaseCensusDirectConsumerV1": "24fb2f2ccf433b837a299da954ccac9c5567463466009845f3dc22fd5e08d3a7",
    "ReleaseResourceCensusEntryV1": "82c01e900d537186647b5258745c45777842d5952fcfa62a90723473189c878a",
    "ReleaseResourceCensusHeaderV1": "7f27f443f927fee5ace98650d80a7c0566a03a236cf49b206c971c7917a6a08a",
    "ReleaseResourceCensusV1": "6b43ddca6f7c18f9693de17d8915eee8a5a51df42b341ac862aac86a7dd108de",
    "InstallationCensusEntryV1": "4aad753afc48503ae88447fbdf781ba1de9680867b772290da689237de201309",
    "InstallationCensusHeaderV1": "dba95bb94ea6e2772845245d5a1412c15f38ea8cb3976fd5e92e9a86a4d079a7",
    "InstallationCensusV1": "37f954d258ef478d442b1358f3ffce380270f8a2dc2a42072b964f304dbb0ad5",
    "InstalledResourceClaimV1": "d85f58640e2ca307400fe2994939536e7496a8db158a0a32e710c033cbec6103",
    "InstalledResourceClaimSetHeaderV1": "396d4deb76bb44274bedce24a21ad4db5dfb3a4f984d92ddbab181489d99226f",
    "InstalledResourceClaimSetV1": "f5cf0c77d42e954ba6f1239f9ec0110ef9f5b099f7df485769f3e7728ad99de6",
    "DistributionSnapshotTargetV1": "dcdd1a1ee7b1f8079c30a92309273833ff9b60af069c33dcb69079b2927e702d",
    "DistributionSnapshotHeaderV1": "243ef9f0348d294ad292d175677a272ec2acf3e0639df965feaa5944d85d1176",
    "DistributionSnapshotV1": "c2bdb12fa79377d4b1c3dfff65f1e38909907e052ee01be0bf21cad6b57cfa9c",
    "OrdinarySnapshotCatalogEntryV1": "225c7a06d560c746f7f8c1f02cdffc03e370eaf301ccb3ccdbc45eb27b23ba95",
    "OrdinarySnapshotCatalogHeaderV1": "c4dbbbbbd13c52b30219ebfc6189617dad2c109f0faae4e4f5775f6148224855",
    "OrdinarySnapshotCatalogV1": "3d9e05ded75660352700c1f8b9940fa1ee170c2d768caccb33c087023d43c366",
    "DistributionReceiptV1": "c493d2cf85aa68dd8b23c3555afe71f74566fe8aace9fc4d1e6a25169700be2b",
    "DistributionCommitRecordV1": "7f5b3401095de19458598b7bf778fdc19334cdc1c68834d05525430bb6eb7caf",
    "HostActivationEntryV1": "f66e7e341f2a5cebe91468d9323254400dbbd2388109e4250ab899c4ceda4f86",
    "UserAgentInstallationClosureV1": "65e911797ccc2589890ee80b244cf0b52ee927071645b6593cca0ef46deb7831",
    "RepositoryInstallationClosureV1": "dbf2db3b16cfc2891846e559fb8d6e2238f9d1945594b18bcbabc0909995fe0c",
    "DistributionRuntimeEdgeContractV1": "dac62da48ce686b55a126406da5fb5320e15bf741bcbc11c138ffb082566c8b9",
    "DistributionRuntimeEdgeContractHeaderV1": "f995a4bf85df49aff234426fb1973e112fbae03c0fb5358c6dcdc2acbf60f0ca",
    "DistributionRuntimeEdgeContractManifestV1": "bb549d43124b7b69b4b440cbd2b8fd9b29a361c5de04dc8538919a7d1b1028f9",
    "ResourceContractSuiteEntryV1": "69f92daa65fdc73a288843a8d3fc120bbe85067bd4f855b1875fe10c0736a48b",
    "ResourceContractSuiteHeaderV1": "9a12a3aaeae4f97cbb42eedcb6ab00609789d9c1342bc6169c1cd3609b83ebc5",
    "ResourceContractSuiteV1": "fb77864a502a58913937da6733443c40b518b63bd729e8787243707cd50c52bb",
}

DESCRIPTOR_DOMAINS = (
    "maestro.vnext.resource.descriptor.v1",
    "maestro.vnext.release-bundle-membership.descriptor.v1",
    "maestro.vnext.release-resource-census-row.descriptor.v1",
    "maestro.vnext.resource-contract-schema.descriptor.v1",
    "maestro.vnext.distribution-runtime-edge-contract.descriptor.v1",
)
MANIFEST_DOMAINS = (
    "maestro.vnext.bundle.manifest.v1",
    "maestro.vnext.embedded-release-bundle.manifest.v1",
    "maestro.vnext.release-resource-census.manifest.v1",
    "maestro.vnext.resource-contract-schema-suite.manifest.v1",
    "maestro.vnext.distribution-runtime-edge-contract.manifest.v1",
)
RESOURCE_DESCRIPTOR_DOMAIN = DESCRIPTOR_DOMAINS[0]
RELEASE_MEMBERSHIP_DESCRIPTOR_DOMAIN = DESCRIPTOR_DOMAINS[1]
RELEASE_CENSUS_ROW_DESCRIPTOR_DOMAIN = DESCRIPTOR_DOMAINS[2]
BUNDLE_MANIFEST_DOMAIN = MANIFEST_DOMAINS[0]
EMBEDDED_RELEASE_MANIFEST_DOMAIN = MANIFEST_DOMAINS[1]
RELEASE_CENSUS_MANIFEST_DOMAIN = MANIFEST_DOMAINS[2]

MANIFEST_IDENTITY_PROTOCOL_SHA256 = "807c478cdd7b84fa44c7bb27827f972dfe05e25b0d2339285dfe311b81cfc077"
OWNER_PROTOCOL_ID = "a21d3d2c1eb16604331c1d206df86ae2fa3263b012dd0de12cf0bb83d19074ca"
DEFAULT_MIGRATION_PROFILE_ID = "12bbaf6404b4943b1f8d3ef85ed12c3e2bf2b97b037fd0ad3f71876634f4909a"
DEFAULT_PARITY_PROFILE_ID = "6cf1432e99a82e54e4698789bb2aa58a79f7a3a0a28abe5b849064ec1a6e1545"
DEFAULT_REMOVAL_PROFILE_ID = "069552018c8211f81eedb347a9427c5b0ade70cb86da25de7d491742e673c043"
DEFAULT_PROOF_PROFILE_ID = "00c33d207de36dcf7a65a3ab60956a55b19cf7cc556fcb2bedfec07fcc6aaa24"

BUNDLE_KIND_TAGS = {
    "Release": 1,
    "AgentBootstrap": 2,
    "Capability": 3,
    "Orchestration": 4,
    "SharedContract": 5,
    "Adapter": 6,
    "ExternalPattern": 7,
    "Migration": 8,
}
BUNDLE_TOPOLOGY = (
    "Migration",
    "ExternalPattern",
    "SharedContract",
    "Orchestration",
    "Capability",
    "Adapter",
    "AgentBootstrap",
)
BUNDLE_TOPOLOGY_TAG = {name: index for index, name in enumerate(BUNDLE_TOPOLOGY, 1)}
CONTENT_ENCODING_TAGS = {"OpaqueBytes": 1, "Utf8Text": 2}
RESOURCE_KIND_TAGS = {
    "Executable": 1,
    "Signature": 2,
    "BillOfMaterials": 3,
    "AgentInstruction": 4,
    "OrchestrationDefinition": 5,
    "PublicContract": 6,
    "AdapterArtifact": 7,
    "ExternalPattern": 8,
    "MigrationArtifact": 9,
    "License": 10,
    "ProvenanceManifest": 11,
}
PROVENANCE_KIND_TAGS = {"FirstParty": 1, "ThirdParty": 2}
DISPOSITION_TAGS = {"Retain": 1, "Rewrite": 2, "Replace": 3, "MigrationOnly": 4, "Remove": 5}
TARGET_CLASS_TAGS = {
    "NoMaterialization": 1,
    "WholeTarget": 2,
    "ManagedBlock": 3,
    "ActivationLink": 4,
    "HostRegistration": 5,
    "ExternalManagerTarget": 6,
}
DIRECT_CONSUMER_KIND_TAGS = {
    "Build": 1,
    "Runtime": 2,
    "Install": 3,
    "Migration": 4,
    "Proof": 5,
    "Documentation": 6,
    "RemovalReader": 7,
}

_ABSENT = object()


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def bytes32(value: str) -> dict[str, str]:
    if not isinstance(value, str):
        raise ContractError("expected a 32-byte lowercase hexadecimal digest")
    value = value.removeprefix("sha256:")
    if len(value) != 64:
        raise ContractError("expected a 32-byte lowercase hexadecimal digest")
    try:
        raw = bytes.fromhex(value)
    except ValueError as exc:
        raise ContractError("expected a 32-byte lowercase hexadecimal digest") from exc
    if raw.hex() != value:
        raise ContractError("digest hexadecimal must be lowercase and canonical")
    return {"bytes": value}


def optional(value: Any = _ABSENT) -> list[Any]:
    return [0] if value is _ABSENT else [1, value]


def validate_optional(value: Any, *, name: str = "optional") -> None:
    if not isinstance(value, list) or not value or value[0] not in (0, 1):
        raise ContractError(f"{name} must be exactly [0] or [1,value]")
    if value[0] == 0 and len(value) != 1:
        raise ContractError(f"absent {name} must be exactly [0]")
    if value[0] == 1 and len(value) != 2:
        raise ContractError(f"present {name} must be exactly [1,value]")


def _head(major: int, value: int) -> bytes:
    if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        raise ContractError("CBOR integer/length must be an unsigned u64")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def encode_cbor(value: Any) -> bytes:
    """Encode only the frozen ManifestIdentityV1 deterministic-CBOR subset."""

    if value is False:
        return b"\xf4"
    if value is True:
        return b"\xf5"
    if isinstance(value, int) and not isinstance(value, bool):
        return _head(0, value)
    if isinstance(value, str):
        try:
            raw = value.encode("ascii")
        except UnicodeEncodeError as exc:
            raise ContractError("CBOR text must be ASCII") from exc
        return _head(3, len(raw)) + raw
    if isinstance(value, list):
        return _head(4, len(value)) + b"".join(encode_cbor(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"] and isinstance(value["bytes"], str):
        raw = bytes.fromhex(bytes32(value["bytes"])["bytes"])
        return _head(2, len(raw)) + raw
    raise ContractError(
        "CBOR value must be a nonnegative integer, boolean, bytes32, ASCII text, or definite array"
    )


def _decode_length(data: bytes, offset: int, additional: int) -> tuple[int, int]:
    if additional < 24:
        return additional, offset
    width_by_additional = {24: 1, 25: 2, 26: 4, 27: 8}
    width = width_by_additional.get(additional)
    if width is None:
        raise ContractError("indefinite or reserved CBOR length is forbidden")
    end = offset + width
    if end > len(data):
        raise ContractError("truncated CBOR length")
    value = int.from_bytes(data[offset:end], "big")
    minimum = {1: 24, 2: 0x100, 4: 0x10000, 8: 0x100000000}[width]
    if value < minimum:
        raise ContractError("CBOR integer/length is not shortest-form")
    return value, end


def _decode_one(data: bytes, offset: int) -> tuple[Any, int]:
    if offset >= len(data):
        raise ContractError("truncated CBOR value")
    initial = data[offset]
    offset += 1
    major, additional = initial >> 5, initial & 0x1F
    if major in (1, 5, 6):
        names = {1: "negative integer", 5: "map", 6: "tag"}
        raise ContractError(f"CBOR {names[major]} is forbidden")
    if major == 7:
        if additional == 20:
            return False, offset
        if additional == 21:
            return True, offset
        raise ContractError("CBOR null, float, or unsupported simple value is forbidden")
    length, offset = _decode_length(data, offset, additional)
    if major == 0:
        return length, offset
    if major not in (2, 3, 4):
        raise ContractError("unsupported CBOR major type")
    if major == 4:
        values = []
        for _ in range(length):
            value, offset = _decode_one(data, offset)
            values.append(value)
        return values, offset
    end = offset + length
    if end > len(data):
        raise ContractError("truncated CBOR string")
    raw = data[offset:end]
    if major == 2:
        if len(raw) != 32:
            raise ContractError("ManifestIdentityV1 byte strings must be exactly 32 bytes")
        return bytes32(raw.hex()), end
    try:
        return raw.decode("ascii"), end
    except UnicodeDecodeError as exc:
        raise ContractError("CBOR text must be ASCII") from exc


def decode_cbor(data: bytes) -> Any:
    value, offset = _decode_one(data, 0)
    if offset != len(data):
        raise ContractError("trailing CBOR bytes")
    if encode_cbor(value) != data:
        raise ContractError("CBOR is not deterministic shortest-form")
    return value


def profile_commitment_bytes(payload: bytes) -> str:
    """Hash exact profile bytes as a commitment, not as a second identity domain."""

    return sha256_hex(payload)


def profile_commitment_value(value: Any) -> str:
    """Hash a strict-CBOR profile value without adding an identity envelope/domain."""

    return sha256_hex(encode_cbor(value))


def descriptor_envelope(domain: str, schema_id: str, value: Any) -> list[Any]:
    if domain not in DESCRIPTOR_DOMAINS:
        raise ContractError("descriptor domain is not frozen by C868")
    return [domain, bytes32(schema_id), value]


def manifest_envelope(
    domain: str,
    manifest_schema_id: str,
    descriptor_schema_id: str,
    header: Any,
    rows: Any,
) -> list[Any]:
    if domain not in MANIFEST_DOMAINS:
        raise ContractError("manifest domain is not frozen by C868")
    envelope = [domain, bytes32(manifest_schema_id), bytes32(descriptor_schema_id), header, rows]
    if len(envelope) != 5:
        raise AssertionError("invariant: ManifestIdentityV1 envelope has five slots")
    return envelope


def identity_digest(envelope: Any) -> tuple[str, bytes]:
    raw = encode_cbor(envelope)
    return sha256_hex(raw), raw


def verify_frozen_inputs(
    *,
    suite_bytes: bytes,
    builder_bytes: bytes,
    validator_bytes: bytes,
    suite_envelope_bytes: bytes,
    runtime_edge_envelope_bytes: bytes,
) -> dict[str, Any]:
    """Verify the exact frozen C868 sources supplied by the caller."""

    supplied = {
        "suite": suite_bytes,
        "builder": builder_bytes,
        "validator": validator_bytes,
        "suite_envelope": suite_envelope_bytes,
        "runtime_edge_envelope": runtime_edge_envelope_bytes,
    }
    for name, raw in supplied.items():
        if sha256_hex(raw) != FROZEN_SOURCE_SHA256[name]:
            raise ContractError(f"frozen C868 {name} SHA-256 mismatch")
    try:
        suite = json.loads(suite_bytes.decode("ascii"))
        suite_envelope_value = json.loads(suite_envelope_bytes.decode("ascii"))
        edge_envelope_value = json.loads(runtime_edge_envelope_bytes.decode("ascii"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ContractError("frozen C868 JSON must be exact ASCII JSON") from exc

    if tuple(suite.get("descriptor_domains", ())) != DESCRIPTOR_DOMAINS:
        raise ContractError("frozen descriptor domains mismatch")
    if tuple(suite.get("manifest_domains", ())) != MANIFEST_DOMAINS:
        raise ContractError("frozen manifest domains mismatch")
    schemas = suite.get("schemas", {})
    actual_schema_ids = {name: record.get("schema_id") for name, record in schemas.items()}
    if actual_schema_ids != FROZEN_SCHEMA_IDS:
        raise ContractError("frozen C868 SchemaId closure mismatch")
    if len(schemas) != FROZEN_SCHEMA_COUNT or len(suite.get("descriptors", ())) != FROZEN_COMPONENT_COUNT:
        raise ContractError("frozen C868 schema/component count mismatch")
    edge = suite.get("runtime_edge_contract", {})
    if len(edge.get("rows", ())) != FROZEN_RUNTIME_EDGE_COUNT:
        raise ContractError("frozen C868 runtime-edge count mismatch")
    if suite.get("manifest_id") != FROZEN_SUITE_MANIFEST_ID:
        raise ContractError("frozen C868 suite ManifestId mismatch")
    if edge.get("manifest_id") != FROZEN_RUNTIME_EDGE_MANIFEST_ID:
        raise ContractError("frozen C868 runtime-edge ManifestId mismatch")
    if suite.get("byte_length") != FROZEN_SUITE_CBOR_LENGTH:
        raise ContractError("frozen C868 suite CBOR length mismatch")
    if suite_envelope_value != suite.get("manifest_identity_envelope"):
        raise ContractError("frozen C868 suite envelope mismatch")
    if edge_envelope_value != edge.get("manifest_identity_envelope"):
        raise ContractError("frozen C868 runtime-edge envelope mismatch")
    suite_cbor = encode_cbor(suite_envelope_value)
    edge_cbor = encode_cbor(edge_envelope_value)
    if suite_cbor.hex() != suite.get("cbor_hex") or sha256_hex(suite_cbor) != FROZEN_SUITE_MANIFEST_ID:
        raise ContractError("frozen C868 suite canonical bytes mismatch")
    if edge_cbor.hex() != edge.get("cbor_hex") or sha256_hex(edge_cbor) != FROZEN_RUNTIME_EDGE_MANIFEST_ID:
        raise ContractError("frozen C868 runtime-edge canonical bytes mismatch")
    if decode_cbor(suite_cbor) != suite_envelope_value or decode_cbor(edge_cbor) != edge_envelope_value:
        raise ContractError("frozen C868 deterministic-CBOR round trip mismatch")
    return suite


def _uint(value: int, minimum: int, maximum: int, name: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or not minimum <= value <= maximum:
        raise ContractError(f"{name} must be in {minimum}..{maximum}")
    return value


def _ascii(value: str, minimum: int, maximum: int, name: str) -> str:
    if not isinstance(value, str):
        raise ContractError(f"{name} must be ASCII text")
    try:
        raw = value.encode("ascii")
    except UnicodeEncodeError as exc:
        raise ContractError(f"{name} must be ASCII text") from exc
    if not minimum <= len(raw) <= maximum:
        raise ContractError(f"{name} length must be in {minimum}..{maximum}")
    return value


def _tag(mapping: dict[str, int], name: str, field: str) -> int:
    try:
        return mapping[name]
    except (KeyError, TypeError) as exc:
        raise ContractError(f"unknown closed {field}: {name}") from exc


def _owner(owner_tag: int, owner_profile_id: str) -> list[Any]:
    return [_uint(owner_tag, 1, 20, "owner_tag"), bytes32(owner_profile_id)]


def _check_strict_tags(values: Iterable[int], name: str) -> None:
    tags = list(values)
    if (
        not tags
        or any(not isinstance(tag, int) or isinstance(tag, bool) or tag <= 0 for tag in tags)
        or tags != sorted(tags)
        or len(tags) != len(set(tags))
    ):
        raise ContractError(f"{name} tags must be nonempty, positive, strictly sorted, and unique")


def _bytes32_value(value: Any, name: str) -> str:
    if not isinstance(value, dict) or list(value) != ["bytes"] or not isinstance(value["bytes"], str):
        raise ContractError(f"{name} must be an exact bytes32 value")
    try:
        return bytes32(value["bytes"])["bytes"]
    except ContractError as exc:
        raise ContractError(f"{name} must be an exact bytes32 value") from exc


def _closed_tag(value: Any, mapping: Mapping[str, int], name: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value not in mapping.values():
        raise ContractError(f"{name} must be a frozen closed-enum tag")
    return value


def _validate_owner(value: Any, name: str) -> tuple[int, str]:
    if not isinstance(value, list) or len(value) != 2:
        raise ContractError(f"{name} must be an exact OwnerRefV1")
    return _uint(value[0], 1, 20, f"{name}.owner_tag"), _bytes32_value(
        value[1], f"{name}.owner_profile_id"
    )


def _validate_optional_bytes32(value: Any, name: str) -> None:
    validate_optional(value, name=name)
    if value[0] == 1:
        _bytes32_value(value[1], name)


def _validate_manifest_core(
    core: Any,
    *,
    generated_sum_schema_id: str,
    descriptor_schema_id: str,
    header_schema_id: str,
    manifest_schema_id: str,
    dependency_manifest_ids: Sequence[tuple[int, str]],
    row_count: int,
    max_row_tag: int,
) -> None:
    if not isinstance(core, list) or len(core) != 16 or core[:3] != [1, 1, 1]:
        raise ContractError("ManifestHeaderCoreV1 shape/version mismatch")
    expected_ids = (
        generated_sum_schema_id,
        descriptor_schema_id,
        header_schema_id,
        manifest_schema_id,
    )
    if tuple(_bytes32_value(core[index], f"ManifestHeaderCoreV1 field {index + 1}") for index in range(3, 7)) != expected_ids:
        raise ContractError("ManifestHeaderCoreV1 SchemaId coordinates mismatch")
    dependency_rows = core[7]
    if not isinstance(dependency_rows, list) or len(dependency_rows) > 4096:
        raise ContractError("ManifestHeaderCoreV1 dependency maximum exceeded")
    dependency_tags: list[int] = []
    actual_dependencies: list[tuple[int, str]] = []
    for row in dependency_rows:
        if not isinstance(row, list) or len(row) != 2:
            raise ContractError("ManifestHeaderCoreV1 dependency row shape mismatch")
        dependency_tags.append(_uint(row[0], 1, 0xFFFFFFFFFFFFFFFF, "dependency Manifest tag"))
        actual_dependencies.append((row[0], _bytes32_value(row[1], "dependency ManifestId")))
    if dependency_tags:
        _check_strict_tags(dependency_tags, "dependency Manifest")
    if actual_dependencies != list(dependency_manifest_ids):
        raise ContractError("ManifestHeaderCoreV1 dependency coordinates mismatch")
    if _bytes32_value(core[8], "manifest_identity_protocol_sha256") != MANIFEST_IDENTITY_PROTOCOL_SHA256:
        raise ContractError("ManifestHeaderCoreV1 identity protocol mismatch")
    if _bytes32_value(core[9], "owner_protocol_id") != OWNER_PROTOCOL_ID:
        raise ContractError("ManifestHeaderCoreV1 owner protocol mismatch")
    if _uint(core[10], 0, 0xFFFFFFFFFFFFFFFF, "row_count") != row_count:
        raise ContractError("ManifestHeaderCoreV1 row_count mismatch")
    if _uint(core[11], 0, 0xFFFFFFFFFFFFFFFF, "max_row_tag") != max_row_tag:
        raise ContractError("ManifestHeaderCoreV1 max_row_tag mismatch")
    expected_profiles = (
        DEFAULT_MIGRATION_PROFILE_ID,
        DEFAULT_PARITY_PROFILE_ID,
        DEFAULT_REMOVAL_PROFILE_ID,
        DEFAULT_PROOF_PROFILE_ID,
    )
    actual_profiles = tuple(
        _bytes32_value(core[index], f"ManifestHeaderCoreV1 field {index + 1}")
        for index in range(12, 16)
    )
    if actual_profiles != expected_profiles:
        raise ContractError("ManifestHeaderCoreV1 frozen profile commitment mismatch")


def _validate_resource_value(value: Any) -> None:
    if not isinstance(value, list) or len(value) != 24:
        raise ContractError("ResourceDescriptorV1 must have exactly 24 fields")
    resource_tag = _uint(value[0], 1, 4096, "resource_tag")
    _ascii(value[1], 1, 512, "stable_resource_key")
    _bytes32_value(value[2], "content_sha256")
    _uint(value[3], 0, 0xFFFFFFFFFFFFFFFF, "content_length")
    _closed_tag(value[4], CONTENT_ENCODING_TAGS, "content_encoding")
    _ascii(value[5], 1, 128, "media_type")
    _closed_tag(value[6], RESOURCE_KIND_TAGS, "resource_kind")
    _validate_owner(value[7], "semantic_owner")
    _closed_tag(value[8], BUNDLE_KIND_TAGS, "required_bundle_kind")
    _closed_tag(value[9], PROVENANCE_KIND_TAGS, "provenance_kind")
    _bytes32_value(value[10], "provenance_commitment_id")
    _validate_optional_bytes32(value[11], "license_commitment_id")
    dependencies = value[12]
    if not isinstance(dependencies, list) or len(dependencies) > 4095:
        raise ContractError("Resource dependency maximum is 4095")
    dependency_tags: list[int] = []
    for row in dependencies:
        if not isinstance(row, list) or len(row) != 2:
            raise ContractError("Resource dependency row shape mismatch")
        dependency_tag = _uint(row[0], 1, 4096, "Resource dependency tag")
        _bytes32_value(row[1], "Resource dependency ResourceId")
        dependency_tags.append(dependency_tag)
    if dependency_tags:
        _check_strict_tags(dependency_tags, "Resource dependency")
    if any(tag >= resource_tag for tag in dependency_tags):
        raise ContractError("Resource dependency is not strictly backward")
    _bytes32_value(value[13], "compatibility_profile_id")
    _validate_optional_bytes32(value[14], "generator_commitment_id")
    for index, name in (
        (15, "target_policy_profile_id"),
        (16, "custody_policy_profile_id"),
        (17, "migration_profile_id"),
        (18, "rollback_profile_id"),
        (19, "uninstall_profile_id"),
        (20, "retention_profile_id"),
        (21, "removal_profile_id"),
    ):
        _bytes32_value(value[index], name)
    _closed_tag(value[22], DISPOSITION_TAGS, "Resource disposition")
    _bytes32_value(value[23], "proof_profile_id")


def make_manifest_core(
    *,
    generated_sum_schema_id: str,
    descriptor_schema_id: str,
    header_schema_id: str,
    manifest_schema_id: str,
    dependency_manifest_ids: Sequence[tuple[int, str]],
    row_count: int,
    max_row_tag: int,
    owner_protocol_id: str = OWNER_PROTOCOL_ID,
    migration_profile_id: str = DEFAULT_MIGRATION_PROFILE_ID,
    parity_profile_id: str = DEFAULT_PARITY_PROFILE_ID,
    removal_profile_id: str = DEFAULT_REMOVAL_PROFILE_ID,
    proof_profile_id: str = DEFAULT_PROOF_PROFILE_ID,
) -> list[Any]:
    dependency_tags = [tag for tag, _ in dependency_manifest_ids]
    if dependency_tags:
        _check_strict_tags(dependency_tags, "dependency Manifest")
    _uint(row_count, 0, 0xFFFFFFFFFFFFFFFF, "row_count")
    _uint(max_row_tag, 0, 0xFFFFFFFFFFFFFFFF, "max_row_tag")
    if row_count and max_row_tag < row_count:
        raise ContractError("max_row_tag cannot be smaller than row_count")
    return [
        1,
        1,
        1,
        bytes32(generated_sum_schema_id),
        bytes32(descriptor_schema_id),
        bytes32(header_schema_id),
        bytes32(manifest_schema_id),
        [[tag, bytes32(identity)] for tag, identity in dependency_manifest_ids],
        bytes32(MANIFEST_IDENTITY_PROTOCOL_SHA256),
        bytes32(owner_protocol_id),
        row_count,
        max_row_tag,
        bytes32(migration_profile_id),
        bytes32(parity_profile_id),
        bytes32(removal_profile_id),
        bytes32(proof_profile_id),
    ]


@dataclass(frozen=True)
class ResourceDescriptor:
    resource_tag: int
    stable_resource_key: str
    required_bundle_kind: str
    disposition: str
    owner: tuple[int, str]
    value: list[Any]
    envelope: list[Any]
    resource_id: str
    canonical_cbor: bytes

    def as_record(self) -> dict[str, Any]:
        return {
            "resource_id": self.resource_id,
            "value": self.value,
            "identity_envelope": self.envelope,
            "cbor_hex": self.canonical_cbor.hex(),
            "byte_length": len(self.canonical_cbor),
        }


@dataclass(frozen=True)
class BundleManifest:
    bundle_tag: int
    bundle_kind: str
    resource_ids: tuple[str, ...]
    dependency_bundle_ids: tuple[str, ...]
    value: list[Any]
    envelope: list[Any]
    bundle_id: str
    canonical_cbor: bytes

    def as_record(self) -> dict[str, Any]:
        return {
            "bundle_id": self.bundle_id,
            "value": self.value,
            "manifest_identity_envelope": self.envelope,
            "cbor_hex": self.canonical_cbor.hex(),
            "byte_length": len(self.canonical_cbor),
        }


@dataclass(frozen=True)
class DirectConsumer:
    locator: str
    owner_tag: int
    owner_profile_id: str
    consumer_kind: str
    resources: tuple[ResourceDescriptor, ...]
    provenance_commitment_id: str
    disposition: str
    migration_profile_id: str
    proof_profile_id: str
    removal_profile_id: str


@dataclass(frozen=True)
class ReleaseResourceCensus:
    resource_ids: tuple[str, ...]
    bundle_ids: tuple[str, ...]
    consumer_edges: tuple[tuple[str, str], ...]
    value: list[Any]
    envelope: list[Any]
    census_id: str
    canonical_cbor: bytes

    def as_record(self) -> dict[str, Any]:
        return {
            "census_id": self.census_id,
            "value": self.value,
            "manifest_identity_envelope": self.envelope,
            "cbor_hex": self.canonical_cbor.hex(),
            "byte_length": len(self.canonical_cbor),
        }


@dataclass(frozen=True)
class EmbeddedReleaseBundle:
    bundle_ids: tuple[str, ...]
    census_id: str
    value: list[Any]
    envelope: list[Any]
    release_id: str
    canonical_cbor: bytes

    def as_record(self) -> dict[str, Any]:
        return {
            "release_id": self.release_id,
            "value": self.value,
            "manifest_identity_envelope": self.envelope,
            "cbor_hex": self.canonical_cbor.hex(),
            "byte_length": len(self.canonical_cbor),
        }


def construct_resource_descriptor(
    *,
    resource_tag: int,
    stable_resource_key: str,
    content: bytes,
    content_encoding: str,
    media_type: str,
    resource_kind: str,
    owner_tag: int,
    owner_profile_id: str,
    required_bundle_kind: str,
    provenance_kind: str,
    provenance_commitment_id: str,
    license_commitment_id: str | None,
    backward_dependencies: Sequence[ResourceDescriptor],
    compatibility_profile_id: str,
    generator_commitment_id: str | None,
    target_policy_profile_id: str,
    custody_policy_profile_id: str,
    migration_profile_id: str,
    rollback_profile_id: str,
    uninstall_profile_id: str,
    retention_profile_id: str,
    removal_profile_id: str,
    disposition: str,
    proof_profile_id: str,
) -> ResourceDescriptor:
    tag = _uint(resource_tag, 1, 4096, "resource_tag")
    key = _ascii(stable_resource_key, 1, 512, "stable_resource_key")
    if not isinstance(content, bytes):
        raise ContractError("content must be exact bytes")
    dependency_tags = [item.resource_tag for item in backward_dependencies]
    if dependency_tags:
        _check_strict_tags(dependency_tags, "Resource dependency")
    if any(item.resource_tag >= tag for item in backward_dependencies):
        raise ContractError("Resource dependencies must be strictly backward")
    if len(backward_dependencies) > 4095:
        raise ContractError("Resource dependency maximum is 4095")
    bundle_kind = required_bundle_kind
    value = [
        tag,
        key,
        bytes32(sha256_hex(content)),
        len(content),
        _tag(CONTENT_ENCODING_TAGS, content_encoding, "content encoding"),
        _ascii(media_type, 1, 128, "media_type"),
        _tag(RESOURCE_KIND_TAGS, resource_kind, "Resource kind"),
        _owner(owner_tag, owner_profile_id),
        _tag(BUNDLE_KIND_TAGS, bundle_kind, "Bundle kind"),
        _tag(PROVENANCE_KIND_TAGS, provenance_kind, "provenance kind"),
        bytes32(provenance_commitment_id),
        optional() if license_commitment_id is None else optional(bytes32(license_commitment_id)),
        [[item.resource_tag, bytes32(item.resource_id)] for item in backward_dependencies],
        bytes32(compatibility_profile_id),
        optional() if generator_commitment_id is None else optional(bytes32(generator_commitment_id)),
        bytes32(target_policy_profile_id),
        bytes32(custody_policy_profile_id),
        bytes32(migration_profile_id),
        bytes32(rollback_profile_id),
        bytes32(uninstall_profile_id),
        bytes32(retention_profile_id),
        bytes32(removal_profile_id),
        _tag(DISPOSITION_TAGS, disposition, "Resource disposition"),
        bytes32(proof_profile_id),
    ]
    if len(value) != 24:
        raise AssertionError("invariant: ResourceDescriptorV1 has 24 fields")
    envelope = descriptor_envelope(
        RESOURCE_DESCRIPTOR_DOMAIN, FROZEN_SCHEMA_IDS["ResourceDescriptorV1"], value
    )
    resource_id, raw = identity_digest(envelope)
    return ResourceDescriptor(
        tag,
        key,
        bundle_kind,
        disposition,
        (owner_tag, bytes32(owner_profile_id)["bytes"]),
        value,
        envelope,
        resource_id,
        raw,
    )


def _validate_resource(resource: ResourceDescriptor) -> None:
    _validate_resource_value(resource.value)
    value = resource.value
    expected_attributes = (
        value[0],
        value[1],
        next((name for name, tag in BUNDLE_KIND_TAGS.items() if tag == value[8]), None),
        next((name for name, tag in DISPOSITION_TAGS.items() if tag == value[22]), None),
        tuple(_validate_owner(value[7], "semantic_owner")),
    )
    if (
        resource.resource_tag,
        resource.stable_resource_key,
        resource.required_bundle_kind,
        resource.disposition,
        resource.owner,
    ) != expected_attributes:
        raise ContractError("ResourceDescriptor dataclass coordinates differ from its value")
    expected_envelope = descriptor_envelope(
        RESOURCE_DESCRIPTOR_DOMAIN, FROZEN_SCHEMA_IDS["ResourceDescriptorV1"], value
    )
    expected_id, expected_raw = identity_digest(expected_envelope)
    if (resource.envelope, resource.resource_id, resource.canonical_cbor) != (
        expected_envelope,
        expected_id,
        expected_raw,
    ):
        raise ContractError("ResourceDescriptorV1 identity mismatch")
    bytes32(resource.resource_id)


def construct_bundle_manifest(
    *,
    bundle_kind: str,
    stable_bundle_key: str,
    semantic_version: str,
    compatibility_profile_id: str,
    resources: Sequence[ResourceDescriptor],
    dependency_bundles: Sequence[BundleManifest],
    provenance_commitment_id: str,
    license_commitment_id: str | None,
    package_policy_profile_id: str,
    supported_target_classes: Sequence[str],
    rollback_profile_id: str,
    uninstall_profile_id: str,
    retention_profile_id: str,
    bundle_tag: int | None = None,
) -> BundleManifest:
    if not isinstance(bundle_kind, str) or bundle_kind not in BUNDLE_TOPOLOGY_TAG:
        raise ContractError("Bundle kind is outside the exact C868 topology")
    tag = _uint(
        BUNDLE_TOPOLOGY_TAG[bundle_kind] if bundle_tag is None else bundle_tag,
        1,
        256,
        "bundle_tag",
    )
    if len(dependency_bundles) > 64:
        raise ContractError("Bundle dependency maximum is 64")
    for dependency in dependency_bundles:
        _validate_bundle(dependency)
    dependency_tags = [item.bundle_tag for item in dependency_bundles]
    if dependency_tags:
        _check_strict_tags(dependency_tags, "Bundle dependency")
    if any(dependency_tag >= tag for dependency_tag in dependency_tags):
        raise ContractError("Bundle dependencies must be strictly backward")
    if len({item.bundle_id for item in dependency_bundles}) != len(dependency_bundles):
        raise ContractError("duplicate Bundle dependency")
    if not 1 <= len(resources) <= 4096:
        raise ContractError("Bundle must contain 1..4096 Resources")
    for resource in resources:
        _validate_resource(resource)
        if resource.required_bundle_kind != bundle_kind:
            raise ContractError("Resource required_bundle_kind differs from owning Bundle")
    resource_tags = [resource.resource_tag for resource in resources]
    _check_strict_tags(resource_tags, "Bundle Resource")
    if len({resource.resource_id for resource in resources}) != len(resources):
        raise ContractError("duplicate Resource membership in Bundle")
    target_tags = [_tag(TARGET_CLASS_TAGS, item, "target class") for item in supported_target_classes]
    _check_strict_tags(target_tags, "supported target class")
    if len(target_tags) > 6:
        raise ContractError("supported target class maximum is 6")
    dependency_rows = [[item.bundle_tag, bytes32(item.bundle_id)] for item in dependency_bundles]
    rows = [[item.resource_tag, bytes32(item.resource_id), item.value] for item in resources]
    core = make_manifest_core(
        generated_sum_schema_id=FROZEN_SCHEMA_IDS["ResourceDescriptorV1"],
        descriptor_schema_id=FROZEN_SCHEMA_IDS["ResourceDescriptorV1"],
        header_schema_id=FROZEN_SCHEMA_IDS["BundleManifestHeaderV1"],
        manifest_schema_id=FROZEN_SCHEMA_IDS["BundleManifestV1"],
        dependency_manifest_ids=[(item.bundle_tag, item.bundle_id) for item in dependency_bundles],
        row_count=len(rows),
        max_row_tag=max(resource_tags),
    )
    header = [
        core,
        tag,
        _ascii(stable_bundle_key, 1, 512, "stable_bundle_key"),
        _tag(BUNDLE_KIND_TAGS, bundle_kind, "Bundle kind"),
        _ascii(semantic_version, 1, 128, "semantic_version"),
        bytes32(compatibility_profile_id),
        dependency_rows,
        bytes32(provenance_commitment_id),
        optional() if license_commitment_id is None else optional(bytes32(license_commitment_id)),
        bytes32(package_policy_profile_id),
        [[tag, tag] for tag in target_tags],
        bytes32(rollback_profile_id),
        bytes32(uninstall_profile_id),
        bytes32(retention_profile_id),
    ]
    value = [header, rows]
    envelope = manifest_envelope(
        BUNDLE_MANIFEST_DOMAIN,
        FROZEN_SCHEMA_IDS["BundleManifestV1"],
        FROZEN_SCHEMA_IDS["ResourceDescriptorV1"],
        header,
        rows,
    )
    bundle_id, raw = identity_digest(envelope)
    if _contains_bytes32(value, bundle_id):
        raise ContractError("Bundle contains a backreference to its own identity")
    return BundleManifest(
        tag,
        bundle_kind,
        tuple(item.resource_id for item in resources),
        tuple(item.bundle_id for item in dependency_bundles),
        value,
        envelope,
        bundle_id,
        raw,
    )


def _validate_bundle(bundle: BundleManifest) -> None:
    if not isinstance(bundle.envelope, list) or len(bundle.envelope) != 5 or bundle.envelope[0] != BUNDLE_MANIFEST_DOMAIN:
        raise ContractError("BundleManifestV1 must use its exact five-slot envelope")
    if bundle.envelope[1:3] != [
        bytes32(FROZEN_SCHEMA_IDS["BundleManifestV1"]),
        bytes32(FROZEN_SCHEMA_IDS["ResourceDescriptorV1"]),
    ]:
        raise ContractError("BundleManifestV1 envelope SchemaIds mismatch")
    if not isinstance(bundle.value, list) or bundle.value != bundle.envelope[3:5] or len(bundle.value) != 2:
        raise ContractError("BundleManifestV1 value/envelope mismatch")
    header, rows = bundle.value
    if not isinstance(header, list) or len(header) != 14 or not isinstance(rows, list):
        raise ContractError("BundleManifestHeaderV1 shape/tag mismatch")
    bundle_tag = _uint(header[1], 1, 256, "bundle_tag")
    if bundle_tag != bundle.bundle_tag:
        raise ContractError("BundleManifestHeaderV1 shape/tag mismatch")
    _ascii(header[2], 1, 512, "stable_bundle_key")
    bundle_kind_tag = _closed_tag(header[3], BUNDLE_KIND_TAGS, "Bundle kind")
    if (
        not isinstance(bundle.bundle_kind, str)
        or bundle.bundle_kind not in BUNDLE_TOPOLOGY_TAG
        or bundle_kind_tag != BUNDLE_KIND_TAGS[bundle.bundle_kind]
    ):
        raise ContractError("BundleManifestHeaderV1 kind mismatch")
    _ascii(header[4], 1, 128, "semantic_version")
    _bytes32_value(header[5], "compatibility_profile_id")
    dependency_rows = header[6]
    if not isinstance(dependency_rows, list) or len(dependency_rows) > 64:
        raise ContractError("Bundle dependency maximum is 64")
    dependency_tags: list[int] = []
    dependency_ids: list[str] = []
    for row in dependency_rows:
        if not isinstance(row, list) or len(row) != 2:
            raise ContractError("Bundle dependency row shape mismatch")
        dependency_tag = _uint(row[0], 1, 256, "Bundle dependency tag")
        if dependency_tag >= bundle_tag:
            raise ContractError("Bundle dependency is not strictly backward")
        dependency_tags.append(dependency_tag)
        dependency_ids.append(_bytes32_value(row[1], "Bundle dependency BundleId"))
    if dependency_tags:
        _check_strict_tags(dependency_tags, "Bundle dependency")
    if tuple(dependency_ids) != bundle.dependency_bundle_ids:
        raise ContractError("Bundle dependency dataclass coordinates mismatch")
    _bytes32_value(header[7], "provenance_commitment_id")
    _validate_optional_bytes32(header[8], "license_commitment_id")
    _bytes32_value(header[9], "package_policy_profile_id")
    target_rows = header[10]
    if not isinstance(target_rows, list) or not 1 <= len(target_rows) <= 6:
        raise ContractError("supported target class must contain 1..6 rows")
    target_tags: list[int] = []
    for row in target_rows:
        if not isinstance(row, list) or len(row) != 2:
            raise ContractError("supported target class row shape mismatch")
        target_tags.append(_uint(row[0], 1, 0xFFFFFFFFFFFFFFFF, "supported target class tag"))
        _closed_tag(row[1], TARGET_CLASS_TAGS, "target class")
    _check_strict_tags(target_tags, "supported target class")
    for index, name in (
        (11, "rollback_profile_id"),
        (12, "uninstall_profile_id"),
        (13, "retention_profile_id"),
    ):
        _bytes32_value(header[index], name)
    if not 1 <= len(rows) <= 4096 or len(rows) != len(bundle.resource_ids):
        raise ContractError("Bundle Resource row count mismatch")
    row_tags: list[int] = []
    row_resource_ids: list[str] = []
    for row in rows:
        if not isinstance(row, list) or len(row) != 3:
            raise ContractError("Bundle Resource row shape/identity mismatch")
        row_tag = _uint(row[0], 1, 4096, "Bundle Resource tag")
        resource_id = _bytes32_value(row[1], "Bundle ResourceId")
        _validate_resource_value(row[2])
        if row[2][0] != row_tag:
            raise ContractError("Bundle Resource row/descriptor tag mismatch")
        row_tags.append(row_tag)
        row_resource_ids.append(resource_id)
    _check_strict_tags(row_tags, "Bundle Resource")
    if tuple(row_resource_ids) != bundle.resource_ids:
        raise ContractError("Bundle Resource dataclass coordinates mismatch")
    for row, resource_id in zip(rows, row_resource_ids, strict=True):
        descriptor = descriptor_envelope(
            RESOURCE_DESCRIPTOR_DOMAIN, FROZEN_SCHEMA_IDS["ResourceDescriptorV1"], row[2]
        )
        descriptor_id, _ = identity_digest(descriptor)
        if descriptor_id != resource_id:
            raise ContractError("Bundle Resource descriptor does not reproduce ResourceId")
    _validate_manifest_core(
        header[0],
        generated_sum_schema_id=FROZEN_SCHEMA_IDS["ResourceDescriptorV1"],
        descriptor_schema_id=FROZEN_SCHEMA_IDS["ResourceDescriptorV1"],
        header_schema_id=FROZEN_SCHEMA_IDS["BundleManifestHeaderV1"],
        manifest_schema_id=FROZEN_SCHEMA_IDS["BundleManifestV1"],
        dependency_manifest_ids=list(zip(dependency_tags, dependency_ids, strict=True)),
        row_count=len(rows),
        max_row_tag=max(row_tags),
    )
    expected_id, expected_raw = identity_digest(bundle.envelope)
    if expected_id != bundle.bundle_id or expected_raw != bundle.canonical_cbor:
        raise ContractError("BundleManifestV1 identity mismatch")
    if _contains_bytes32(bundle.value, bundle.bundle_id):
        raise ContractError("Bundle contains its own identity")


def _validate_census(
    census: ReleaseResourceCensus,
    resources: Sequence[ResourceDescriptor],
    bundles: Sequence[BundleManifest],
) -> None:
    if not isinstance(census.envelope, list) or len(census.envelope) != 5 or census.envelope[0] != RELEASE_CENSUS_MANIFEST_DOMAIN:
        raise ContractError("ReleaseResourceCensusV1 must use its exact five-slot envelope")
    if census.envelope[1:3] != [
        bytes32(FROZEN_SCHEMA_IDS["ReleaseResourceCensusV1"]),
        bytes32(FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"]),
    ]:
        raise ContractError("ReleaseResourceCensusV1 envelope SchemaIds mismatch")
    if not isinstance(census.value, list) or census.value != census.envelope[3:5] or len(census.value) != 2:
        raise ContractError("ReleaseResourceCensusV1 value/envelope mismatch")
    header, rows = census.value
    if not isinstance(header, list) or len(header) != 12 or not isinstance(rows, list):
        raise ContractError("ReleaseResourceCensusHeaderV1 shape/totality guard mismatch")
    _ascii(header[1], 1, 512, "release_key")
    _ascii(header[2], 1, 128, "release_version")
    _ascii(header[3], 1, 256, "platform_qualifier")
    if _closed_tag(header[11], {"RowsPartitionCountsAndEdgesEqual": 1}, "totality_guard") != 1:
        raise ContractError("ReleaseResourceCensusHeaderV1 shape/totality guard mismatch")
    if not 1 <= len(rows) <= 327679:
        raise ContractError("Release census row maximum exceeded")
    row_tags = []
    for row in rows:
        if not isinstance(row, list) or len(row) != 3:
            raise ContractError("Release census descriptor row shape/tag mismatch")
        row_tags.append(_uint(row[0], 1, 327679, "Release census entry_tag"))
    _check_strict_tags(row_tags, "Release census")
    for resource in resources:
        _validate_resource(resource)
    known_resources = {resource.resource_id: resource for resource in resources}
    if len(known_resources) != len(resources):
        raise ContractError("duplicate concrete Resource identity")
    bundle_by_resource: dict[str, BundleManifest] = {}
    for bundle in bundles:
        _validate_bundle(bundle)
        for resource_id in bundle.resource_ids:
            if resource_id in bundle_by_resource:
                raise ContractError("Resource has more than one owning Bundle")
            bundle_by_resource[resource_id] = bundle
    if set(bundle_by_resource) != set(known_resources):
        raise ContractError("Census Resources and Bundle membership must be exact-set equal")
    for resource_id, resource in known_resources.items():
        if bundle_by_resource[resource_id].bundle_kind != resource.required_bundle_kind:
            raise ContractError("Resource has a mismatched owning Bundle")
    expected_bundle_rows = [[item.bundle_tag, bytes32(item.bundle_id)] for item in bundles]
    if header[4] != expected_bundle_rows or census.bundle_ids != tuple(item.bundle_id for item in bundles):
        raise ContractError("census ordered Bundle identities mismatch")
    resource_rows: list[str] = []
    edges: list[tuple[str, str]] = []
    for row in rows:
        if not isinstance(row[2], list) or len(row[2]) != 3 or row[2][0] != row[0]:
            raise ContractError("Release census descriptor row shape/tag mismatch")
        entry = row[2]
        validate_optional(entry[1], name="census Resource branch")
        validate_optional(entry[2], name="census direct-consumer branch")
        if (entry[1][0], entry[2][0]) not in ((1, 0), (0, 1)):
            raise ContractError("census row must contain exactly one branch")
        descriptor = descriptor_envelope(
            RELEASE_CENSUS_ROW_DESCRIPTOR_DOMAIN,
            FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"],
            entry,
        )
        descriptor_id, _ = identity_digest(descriptor)
        if row[1] != bytes32(descriptor_id):
            raise ContractError("Release census DescriptorId mismatch")
        if entry[1][0] == 1:
            resource_value = entry[1][1]
            if not isinstance(resource_value, list) or len(resource_value) != 10:
                raise ContractError("ReleaseCensusResourceV1 must have exactly 10 fields")
            _ascii(resource_value[0], 1, 4096, "census Resource locator")
            owner = _validate_owner(resource_value[1], "census Resource semantic_owner")
            resource_id = _bytes32_value(resource_value[2], "census ResourceId")
            owning_bundle_id = _bytes32_value(resource_value[3], "census owning BundleId")
            resource = known_resources.get(resource_id)
            owning_bundle = bundle_by_resource.get(resource_id)
            if resource is None or owning_bundle is None or owning_bundle_id != owning_bundle.bundle_id:
                raise ContractError("census Resource is outside the same Release closure")
            expected_coordinates = [
                list(owner),
                resource_id,
                owning_bundle.bundle_id,
                resource.value[10]["bytes"],
                resource.value[15]["bytes"],
                resource.value[22],
                resource.value[17]["bytes"],
                resource.value[23]["bytes"],
                resource.value[21]["bytes"],
            ]
            actual_coordinates = [
                list(resource.owner),
                resource_id,
                owning_bundle_id,
                _bytes32_value(resource_value[4], "census provenance_commitment_id"),
                _bytes32_value(resource_value[5], "census target_policy_id"),
                _closed_tag(resource_value[6], DISPOSITION_TAGS, "census Resource disposition"),
                _bytes32_value(resource_value[7], "census migration_profile_id"),
                _bytes32_value(resource_value[8], "census proof_profile_id"),
                _bytes32_value(resource_value[9], "census removal_profile_id"),
            ]
            if actual_coordinates != expected_coordinates:
                raise ContractError("census Resource coordinates differ from the concrete Resource/Bundle")
            resource_rows.append(resource_id)
        else:
            consumer_value = entry[2][1]
            if not isinstance(consumer_value, list) or len(consumer_value) != 9:
                raise ContractError("ReleaseCensusDirectConsumerV1 must have exactly 9 fields")
            _ascii(consumer_value[0], 1, 4096, "direct consumer locator")
            _validate_owner(consumer_value[1], "direct consumer semantic_owner")
            _closed_tag(consumer_value[2], DIRECT_CONSUMER_KIND_TAGS, "direct consumer kind")
            consumer_key = sha256_hex(encode_cbor(consumer_value))
            consumer_resource_rows = consumer_value[3]
            if not isinstance(consumer_resource_rows, list) or not 1 <= len(consumer_resource_rows) <= 65535:
                raise ContractError("direct consumer must reference 1..65535 Resources")
            consumer_resource_tags: list[int] = []
            for edge in consumer_resource_rows:
                if not isinstance(edge, list) or len(edge) != 2:
                    raise ContractError("direct consumer edge shape mismatch")
                consumer_resource_tags.append(_uint(edge[0], 1, 4096, "consumer Resource tag"))
            _check_strict_tags(consumer_resource_tags, "consumer Resource")
            for resource_tag, resource_id_value in consumer_resource_rows:
                resource_id = _bytes32_value(resource_id_value, "consumer ResourceId")
                resource = known_resources.get(resource_id)
                if resource is None or resource.resource_tag != resource_tag:
                    raise ContractError("direct consumer edge targets a different/future Release Resource")
                edges.append((consumer_key, resource_id))
            _bytes32_value(consumer_value[4], "consumer provenance_commitment_id")
            _closed_tag(consumer_value[5], DISPOSITION_TAGS, "consumer disposition")
            _bytes32_value(consumer_value[6], "consumer migration_profile_id")
            _bytes32_value(consumer_value[7], "consumer proof_profile_id")
            _bytes32_value(consumer_value[8], "consumer removal_profile_id")
    resource_count = len(resource_rows)
    consumer_count = len(rows) - resource_count
    if [
        _uint(header[5], 0, 65535, "resource_count"),
        _uint(header[6], 0, 262144, "direct_consumer_count"),
        _uint(header[7], 0, 1048576, "direct_consumer_edge_count"),
    ] != [resource_count, consumer_count, len(edges)]:
        raise ContractError("census rows/counts/direct-consumer edge totality mismatch")
    if resource_rows != list(census.resource_ids) or len(resource_rows) != len(set(resource_rows)):
        raise ContractError("census Resource rows are not the exact Resource closure")
    for index, name in (
        (8, "source_inventory_digest"),
        (9, "consumer_inventory_digest"),
        (10, "build_graph_digest"),
    ):
        _bytes32_value(header[index], name)
    _validate_manifest_core(
        header[0],
        generated_sum_schema_id=FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"],
        descriptor_schema_id=FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"],
        header_schema_id=FROZEN_SCHEMA_IDS["ReleaseResourceCensusHeaderV1"],
        manifest_schema_id=FROZEN_SCHEMA_IDS["ReleaseResourceCensusV1"],
        dependency_manifest_ids=[(item.bundle_tag, item.bundle_id) for item in bundles],
        row_count=len(rows),
        max_row_tag=max(row_tags),
    )
    if tuple(sorted(edges)) != census.consumer_edges:
        raise ContractError("census direct-consumer edge metadata mismatch")
    edge_count = {resource_id: 0 for resource_id in known_resources}
    for _, resource_id in edges:
        edge_count[resource_id] += 1
    for resource in resources:
        if resource.disposition == "Remove" and edge_count[resource.resource_id]:
            raise ContractError("Remove requires an exact consumer-zero Resource")


def _validate_release(
    release: EmbeddedReleaseBundle,
    bundles: Sequence[BundleManifest],
    census: ReleaseResourceCensus,
) -> None:
    if not isinstance(release.envelope, list) or len(release.envelope) != 5 or release.envelope[0] != EMBEDDED_RELEASE_MANIFEST_DOMAIN:
        raise ContractError("EmbeddedReleaseBundleV1 must use its exact five-slot envelope")
    if release.envelope[1:3] != [
        bytes32(FROZEN_SCHEMA_IDS["EmbeddedReleaseBundleV1"]),
        bytes32(FROZEN_SCHEMA_IDS["ReleaseBundleMembershipV1"]),
    ]:
        raise ContractError("EmbeddedReleaseBundleV1 envelope SchemaIds mismatch")
    if not isinstance(release.value, list) or release.value != release.envelope[3:5] or len(release.value) != 2:
        raise ContractError("EmbeddedReleaseBundleV1 value/envelope mismatch")
    header, rows = release.value
    if not isinstance(header, list) or len(header) != 13 or not isinstance(rows, list):
        raise ContractError("EmbeddedReleaseHeaderV1 must be the sole Release Bundle")
    _ascii(header[1], 1, 512, "release_key")
    if _closed_tag(header[2], BUNDLE_KIND_TAGS, "Release Bundle kind") != BUNDLE_KIND_TAGS["Release"]:
        raise ContractError("EmbeddedReleaseHeaderV1 must be the sole Release Bundle")
    _ascii(header[3], 1, 128, "release_version")
    _ascii(header[4], 1, 256, "platform_qualifier")
    for index, name in (
        (5, "core_contract_root_id"),
        (6, "binary_compatibility_id"),
        (7, "public_catalog_id"),
        (8, "release_resource_census_id"),
        (9, "compatibility_profile_id"),
        (10, "rollback_profile_id"),
        (11, "uninstall_profile_id"),
        (12, "retention_profile_id"),
    ):
        _bytes32_value(header[index], name)
    if header[8] != bytes32(census.census_id):
        raise ContractError("EmbeddedReleaseHeaderV1 census identity mismatch")
    if release.census_id != census.census_id or release.bundle_ids != tuple(bundle.bundle_id for bundle in bundles):
        raise ContractError("EmbeddedReleaseBundle dataclass coordinates mismatch")
    if not 1 <= len(rows) <= 256:
        raise ContractError("EmbeddedReleaseBundleV1 must contain 1..256 non-Release Bundles")
    if any(not isinstance(row, list) or len(row) != 3 for row in rows):
        raise ContractError("EmbeddedReleaseBundleV1 membership row shape mismatch")
    row_tags = [_uint(row[0], 1, 256, "Release Bundle membership tag") for row in rows]
    _check_strict_tags(row_tags, "Release Bundle membership")
    if row_tags != [bundle.bundle_tag for bundle in bundles]:
        raise ContractError("EmbeddedReleaseBundleV1 row topology mismatch")
    bundle_by_id = {bundle.bundle_id: bundle for bundle in bundles}
    for row, bundle in zip(rows, bundles, strict=True):
        membership = row[2]
        if not isinstance(membership, list) or len(membership) != 4:
            raise ContractError("ReleaseBundleMembershipV1 shape mismatch")
        dependency_tags = []
        for dependency_id in bundle.dependency_bundle_ids:
            dependency = bundle_by_id.get(dependency_id)
            if dependency is None:
                raise ContractError("Release membership dependency is outside the same Release")
            dependency_tags.append(dependency.bundle_tag)
        if len(dependency_tags) > 64:
            raise ContractError("Release membership dependency maximum is 64")
        if dependency_tags:
            _check_strict_tags(dependency_tags, "Release membership dependency")
        expected_dependencies = [[tag] for tag in dependency_tags]
        if membership != [
            bundle.bundle_tag,
            BUNDLE_KIND_TAGS[bundle.bundle_kind],
            bytes32(bundle.bundle_id),
            expected_dependencies,
        ]:
            raise ContractError("ReleaseBundleMembershipV1 topology mismatch")
        descriptor = descriptor_envelope(
            RELEASE_MEMBERSHIP_DESCRIPTOR_DOMAIN,
            FROZEN_SCHEMA_IDS["ReleaseBundleMembershipV1"],
            membership,
        )
        descriptor_id, _ = identity_digest(descriptor)
        if row[1] != bytes32(descriptor_id):
            raise ContractError("Release Bundle membership DescriptorId mismatch")
    census_dependency_tag = max(row_tags) + 1
    _validate_manifest_core(
        header[0],
        generated_sum_schema_id=FROZEN_SCHEMA_IDS["ReleaseBundleMembershipV1"],
        descriptor_schema_id=FROZEN_SCHEMA_IDS["ReleaseBundleMembershipV1"],
        header_schema_id=FROZEN_SCHEMA_IDS["EmbeddedReleaseHeaderV1"],
        manifest_schema_id=FROZEN_SCHEMA_IDS["EmbeddedReleaseBundleV1"],
        dependency_manifest_ids=[(item.bundle_tag, item.bundle_id) for item in bundles]
        + [(census_dependency_tag, census.census_id)],
        row_count=len(rows),
        max_row_tag=max(row_tags),
    )


def construct_release_resource_census(
    *,
    release_key: str,
    release_version: str,
    platform_qualifier: str,
    resources: Sequence[ResourceDescriptor],
    bundles: Sequence[BundleManifest],
    direct_consumers: Sequence[DirectConsumer],
    source_inventory_digest: str,
    consumer_inventory_digest: str,
    build_graph_digest: str,
    resource_locators: Mapping[str, str] | None = None,
) -> ReleaseResourceCensus:
    _validate_bundle_topology(bundles)
    bundle_by_resource: dict[str, BundleManifest] = {}
    for bundle in bundles:
        for resource_id in bundle.resource_ids:
            if resource_id in bundle_by_resource:
                raise ContractError("Resource has more than one owning Bundle")
            bundle_by_resource[resource_id] = bundle
    resource_ids = [item.resource_id for item in resources]
    if len(resource_ids) != len(set(resource_ids)) or set(resource_ids) != set(bundle_by_resource):
        raise ContractError("Census Resources and Bundle membership must be exact-set equal")
    _check_strict_tags([item.resource_tag for item in resources], "Census Resource")
    if resource_locators is not None and set(resource_locators) != set(resource_ids):
        raise ContractError("explicit census Resource locators must be exact-set keyed by ResourceId")
    for resource in resources:
        _validate_resource(resource)
        bundle = bundle_by_resource[resource.resource_id]
        if bundle.bundle_kind != resource.required_bundle_kind:
            raise ContractError("Resource has a mismatched owning Bundle")

    entries: list[tuple[int, list[Any]]] = []
    for resource in resources:
        bundle = bundle_by_resource[resource.resource_id]
        locator = (
            resource.stable_resource_key
            if resource_locators is None
            else resource_locators[resource.resource_id]
        )
        resource_value = [
            _ascii(locator, 1, 4096, "census Resource locator"),
            _owner(*resource.owner),
            bytes32(resource.resource_id),
            bytes32(bundle.bundle_id),
            resource.value[10],
            resource.value[15],
            resource.value[22],
            resource.value[17],
            resource.value[23],
            resource.value[21],
        ]
        entries.append((len(entries) + 1, [len(entries) + 1, optional(resource_value), optional()]))

    known_resources = {item.resource_id: item for item in resources}
    consumer_edges: list[tuple[str, str]] = []
    for consumer in direct_consumers:
        if not consumer.resources:
            raise ContractError("direct consumer must reference at least one Resource")
        consumer_resource_tags = [item.resource_tag for item in consumer.resources]
        _check_strict_tags(consumer_resource_tags, "consumer Resource")
        for resource in consumer.resources:
            _validate_resource(resource)
            known_resource = known_resources.get(resource.resource_id)
            if known_resource is None or known_resource.resource_tag != resource.resource_tag:
                raise ContractError("direct consumer edge must target the same concrete Release Resource")
        consumer_value = [
            _ascii(consumer.locator, 1, 4096, "consumer locator"),
            _owner(consumer.owner_tag, consumer.owner_profile_id),
            _tag(DIRECT_CONSUMER_KIND_TAGS, consumer.consumer_kind, "direct consumer kind"),
            [[item.resource_tag, bytes32(item.resource_id)] for item in consumer.resources],
            bytes32(consumer.provenance_commitment_id),
            _tag(DISPOSITION_TAGS, consumer.disposition, "consumer disposition"),
            bytes32(consumer.migration_profile_id),
            bytes32(consumer.proof_profile_id),
            bytes32(consumer.removal_profile_id),
        ]
        entry_tag = len(entries) + 1
        entries.append((entry_tag, [entry_tag, optional(), optional(consumer_value)]))
        consumer_key = sha256_hex(encode_cbor(consumer_value))
        consumer_edges.extend((consumer_key, item.resource_id) for item in consumer.resources)

    edge_count_by_resource = {resource_id: 0 for resource_id in resource_ids}
    for _, resource_id in consumer_edges:
        edge_count_by_resource[resource_id] += 1
    for resource in resources:
        if resource.disposition == "Remove" and edge_count_by_resource[resource.resource_id] != 0:
            raise ContractError("Remove requires an exact consumer-zero Resource")
    if len(resources) > 65535 or len(direct_consumers) > 262144 or len(consumer_edges) > 1048576:
        raise ContractError("Release census finite maximum exceeded")

    descriptor_rows = []
    for entry_tag, entry_value in entries:
        validate_optional(entry_value[1], name="census Resource branch")
        validate_optional(entry_value[2], name="census direct-consumer branch")
        if (entry_value[1][0], entry_value[2][0]) not in ((1, 0), (0, 1)):
            raise ContractError("census row must contain exactly one branch")
        descriptor = descriptor_envelope(
            RELEASE_CENSUS_ROW_DESCRIPTOR_DOMAIN,
            FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"],
            entry_value,
        )
        descriptor_id, _ = identity_digest(descriptor)
        descriptor_rows.append([entry_tag, bytes32(descriptor_id), entry_value])
    bundle_dependencies = [(item.bundle_tag, item.bundle_id) for item in bundles]
    core = make_manifest_core(
        generated_sum_schema_id=FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"],
        descriptor_schema_id=FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"],
        header_schema_id=FROZEN_SCHEMA_IDS["ReleaseResourceCensusHeaderV1"],
        manifest_schema_id=FROZEN_SCHEMA_IDS["ReleaseResourceCensusV1"],
        dependency_manifest_ids=bundle_dependencies,
        row_count=len(descriptor_rows),
        max_row_tag=len(descriptor_rows),
    )
    header = [
        core,
        _ascii(release_key, 1, 512, "release_key"),
        _ascii(release_version, 1, 128, "release_version"),
        _ascii(platform_qualifier, 1, 256, "platform_qualifier"),
        [[item.bundle_tag, bytes32(item.bundle_id)] for item in bundles],
        len(resources),
        len(direct_consumers),
        len(consumer_edges),
        bytes32(source_inventory_digest),
        bytes32(consumer_inventory_digest),
        bytes32(build_graph_digest),
        1,
    ]
    value = [header, descriptor_rows]
    envelope = manifest_envelope(
        RELEASE_CENSUS_MANIFEST_DOMAIN,
        FROZEN_SCHEMA_IDS["ReleaseResourceCensusV1"],
        FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"],
        header,
        descriptor_rows,
    )
    census_id, raw = identity_digest(envelope)
    return ReleaseResourceCensus(
        tuple(resource_ids),
        tuple(item.bundle_id for item in bundles),
        tuple(sorted(consumer_edges)),
        value,
        envelope,
        census_id,
        raw,
    )


def construct_embedded_release_bundle(
    *,
    release_key: str,
    release_version: str,
    platform_qualifier: str,
    resources: Sequence[ResourceDescriptor],
    bundles: Sequence[BundleManifest],
    census: ReleaseResourceCensus,
    core_contract_root_id: str,
    binary_compatibility_id: str,
    public_catalog_id: str,
    compatibility_profile_id: str,
    rollback_profile_id: str,
    uninstall_profile_id: str,
    retention_profile_id: str,
) -> EmbeddedReleaseBundle:
    _validate_bundle_topology(bundles)
    _validate_census(census, resources, bundles)
    census_id, census_raw = identity_digest(census.envelope)
    if census_id != census.census_id or census_raw != census.canonical_cbor:
        raise ContractError("ReleaseResourceCensusV1 identity mismatch")
    if tuple(item.bundle_id for item in bundles) != census.bundle_ids:
        raise ContractError("Release and census Bundle order must be exact-set equal")
    membership_rows = []
    for bundle in bundles:
        dependency_tags = [
            item.bundle_tag for item in bundles if item.bundle_id in bundle.dependency_bundle_ids
        ]
        membership_value = [
            bundle.bundle_tag,
            _tag(BUNDLE_KIND_TAGS, bundle.bundle_kind, "Bundle kind"),
            bytes32(bundle.bundle_id),
            [[tag] for tag in dependency_tags],
        ]
        descriptor = descriptor_envelope(
            RELEASE_MEMBERSHIP_DESCRIPTOR_DOMAIN,
            FROZEN_SCHEMA_IDS["ReleaseBundleMembershipV1"],
            membership_value,
        )
        descriptor_id, _ = identity_digest(descriptor)
        membership_rows.append([bundle.bundle_tag, bytes32(descriptor_id), membership_value])
    dependencies = [(item.bundle_tag, item.bundle_id) for item in bundles]
    dependencies.append((max(item.bundle_tag for item in bundles) + 1, census.census_id))
    core = make_manifest_core(
        generated_sum_schema_id=FROZEN_SCHEMA_IDS["ReleaseBundleMembershipV1"],
        descriptor_schema_id=FROZEN_SCHEMA_IDS["ReleaseBundleMembershipV1"],
        header_schema_id=FROZEN_SCHEMA_IDS["EmbeddedReleaseHeaderV1"],
        manifest_schema_id=FROZEN_SCHEMA_IDS["EmbeddedReleaseBundleV1"],
        dependency_manifest_ids=dependencies,
        row_count=len(membership_rows),
        max_row_tag=max(item.bundle_tag for item in bundles),
    )
    header = [
        core,
        _ascii(release_key, 1, 512, "release_key"),
        BUNDLE_KIND_TAGS["Release"],
        _ascii(release_version, 1, 128, "release_version"),
        _ascii(platform_qualifier, 1, 256, "platform_qualifier"),
        bytes32(core_contract_root_id),
        bytes32(binary_compatibility_id),
        bytes32(public_catalog_id),
        bytes32(census.census_id),
        bytes32(compatibility_profile_id),
        bytes32(rollback_profile_id),
        bytes32(uninstall_profile_id),
        bytes32(retention_profile_id),
    ]
    value = [header, membership_rows]
    envelope = manifest_envelope(
        EMBEDDED_RELEASE_MANIFEST_DOMAIN,
        FROZEN_SCHEMA_IDS["EmbeddedReleaseBundleV1"],
        FROZEN_SCHEMA_IDS["ReleaseBundleMembershipV1"],
        header,
        membership_rows,
    )
    release_id, raw = identity_digest(envelope)
    return EmbeddedReleaseBundle(
        tuple(item.bundle_id for item in bundles),
        census.census_id,
        value,
        envelope,
        release_id,
        raw,
    )


def _validate_bundle_topology(bundles: Sequence[BundleManifest]) -> None:
    if not 1 <= len(bundles) <= 256:
        raise ContractError("Release must contain 1..256 non-Release Bundles")
    bundle_tags = [item.bundle_tag for item in bundles]
    _check_strict_tags(bundle_tags, "Bundle")
    if any(tag > 256 for tag in bundle_tags):
        raise ContractError("Bundle tags must be in 1..256")
    bundle_kinds = [item.bundle_kind for item in bundles]
    if any(not isinstance(kind, str) for kind in bundle_kinds) or set(bundle_kinds) != set(BUNDLE_TOPOLOGY):
        raise ContractError("Release must contain the exact seven-kind non-Release closure")
    topology_ranks = [BUNDLE_TOPOLOGY_TAG.get(kind, 0) for kind in bundle_kinds]
    if any(rank == 0 for rank in topology_ranks) or topology_ranks != sorted(topology_ranks):
        raise ContractError("Bundles must follow the exact C868 kind topology order")
    by_id: dict[str, BundleManifest] = {}
    for bundle in bundles:
        _validate_bundle(bundle)
        if bundle.bundle_id in by_id:
            raise ContractError("duplicate Bundle identity")
        by_id[bundle.bundle_id] = bundle
    for bundle in bundles:
        dependencies = []
        for dependency_id in bundle.dependency_bundle_ids:
            dependency = by_id.get(dependency_id)
            if dependency is None or dependency.bundle_tag >= bundle.bundle_tag:
                raise ContractError("Bundle dependency must resolve strictly backward in the same Release")
            dependencies.append(dependency.bundle_tag)
        if len(dependencies) > 64:
            raise ContractError("Bundle dependency maximum is 64")
        if dependencies:
            _check_strict_tags(dependencies, "Bundle dependency")


def _contains_bytes32(value: Any, identity: str) -> bool:
    if isinstance(value, dict):
        return value == {"bytes": identity}
    if isinstance(value, list):
        return any(_contains_bytes32(item, identity) for item in value)
    return False


def reject_identity_backreferences(value: Any, forbidden_ids: Iterable[str], *, context: str) -> None:
    for identity in forbidden_ids:
        canonical = bytes32(identity)["bytes"]
        if _contains_bytes32(value, canonical):
            raise ContractError(f"{context} contains a forbidden upward/future/installed-state identity")


def _validate_resource_identity_placement(
    resource: ResourceDescriptor,
    resources: Sequence[ResourceDescriptor],
) -> None:
    """Permit Resource identities only in the exact dependency field.

    The dependency field is validated separately for exact tag/id pairing and
    strict backward order. Every other occurrence is an untyped upward,
    future, self, or hidden same-Release reference.
    """

    value_without_dependencies = resource.value[:12] + resource.value[13:]
    reject_identity_backreferences(
        value_without_dependencies,
        (item.resource_id for item in resources),
        context=f"Resource {resource.stable_resource_key} outside backward_resource_dependencies",
    )


def validate_release_closure(
    *,
    resources: Sequence[ResourceDescriptor],
    bundles: Sequence[BundleManifest],
    census: ReleaseResourceCensus,
    release: EmbeddedReleaseBundle,
    installed_state_ids: Iterable[str] = (),
) -> None:
    """Validate the exact one-way concrete source identity DAG."""

    _validate_bundle_topology(bundles)
    resource_tags = [item.resource_tag for item in resources]
    _check_strict_tags(resource_tags, "Release Resource")
    resource_ids = [item.resource_id for item in resources]
    if len(resource_ids) != len(set(resource_ids)):
        raise ContractError("duplicate concrete Resource identity")
    membership = [resource_id for bundle in bundles for resource_id in bundle.resource_ids]
    if len(membership) != len(set(membership)) or set(membership) != set(resource_ids):
        raise ContractError("every Resource must have exactly one owning Bundle")
    by_id = {resource.resource_id: resource for resource in resources}
    for resource in resources:
        _validate_resource(resource)
        _validate_resource_identity_placement(resource, resources)
        for dependency_tag, dependency_id in resource.value[12]:
            dependency = by_id.get(dependency_id["bytes"])
            if dependency is None or dependency.resource_tag != dependency_tag:
                raise ContractError("Resource dependency must resolve in the same Release")
    if census.resource_ids != tuple(resource_ids):
        raise ContractError("census Resource order differs from concrete Resource order")
    if census.bundle_ids != tuple(item.bundle_id for item in bundles):
        raise ContractError("census Bundle order differs from exact topology")
    _validate_census(census, resources, bundles)
    census_id, census_raw = identity_digest(census.envelope)
    if len(census.envelope) != 5 or census_id != census.census_id or census_raw != census.canonical_cbor:
        raise ContractError("ReleaseResourceCensusV1 identity mismatch")
    release_id, release_raw = identity_digest(release.envelope)
    if len(release.envelope) != 5 or release_id != release.release_id or release_raw != release.canonical_cbor:
        raise ContractError("EmbeddedReleaseBundleV1 identity mismatch")
    _validate_release(release, bundles, census)
    if release.census_id != census.census_id or release.bundle_ids != census.bundle_ids:
        raise ContractError("Release does not bind the exact census and Bundle closure")

    upward_ids = [item.bundle_id for item in bundles] + [census.census_id, release.release_id]
    installed_ids = list(installed_state_ids)
    for resource in resources:
        reject_identity_backreferences(
            resource.value,
            upward_ids + installed_ids,
            context=f"Resource {resource.stable_resource_key}",
        )
    for bundle in bundles:
        allowed_dependencies = set(bundle.dependency_bundle_ids)
        forbidden_bundles = [item.bundle_id for item in bundles if item.bundle_id not in allowed_dependencies]
        reject_identity_backreferences(
            bundle.value,
            forbidden_bundles + [census.census_id, release.release_id] + installed_ids,
            context=f"Bundle {bundle.bundle_kind}",
        )
    reject_identity_backreferences(
        census.value,
        [release.release_id] + installed_ids,
        context="ReleaseResourceCensusV1",
    )
    reject_identity_backreferences(
        release.value,
        [release.release_id] + installed_ids,
        context="EmbeddedReleaseBundleV1",
    )


__all__ = [
    "BUNDLE_KIND_TAGS",
    "BUNDLE_TOPOLOGY",
    "ContractError",
    "DESCRIPTOR_DOMAINS",
    "DirectConsumer",
    "EmbeddedReleaseBundle",
    "FROZEN_RUNTIME_EDGE_MANIFEST_ID",
    "FROZEN_SCHEMA_IDS",
    "FROZEN_SOURCE_SHA256",
    "FROZEN_SUITE_MANIFEST_ID",
    "MANIFEST_DOMAINS",
    "ReleaseResourceCensus",
    "ResourceDescriptor",
    "BundleManifest",
    "bytes32",
    "construct_bundle_manifest",
    "construct_embedded_release_bundle",
    "construct_release_resource_census",
    "construct_resource_descriptor",
    "decode_cbor",
    "descriptor_envelope",
    "encode_cbor",
    "identity_digest",
    "make_manifest_core",
    "manifest_envelope",
    "optional",
    "profile_commitment_bytes",
    "profile_commitment_value",
    "reject_identity_backreferences",
    "validate_optional",
    "validate_release_closure",
    "verify_frozen_inputs",
]


class C868ContractTests(unittest.TestCase):
    _DIGEST = "11" * 32

    def _closure(
        self,
        bundle_kinds: Sequence[str] = BUNDLE_TOPOLOGY,
    ) -> tuple[
        list[ResourceDescriptor],
        list[BundleManifest],
        ReleaseResourceCensus,
        EmbeddedReleaseBundle,
    ]:
        resources = [
            construct_resource_descriptor(
                resource_tag=index,
                stable_resource_key=f"resource-{index}",
                content=f"resource-{index}".encode(),
                content_encoding="Utf8Text",
                media_type="text/plain",
                resource_kind="PublicContract",
                owner_tag=1,
                owner_profile_id=self._DIGEST,
                required_bundle_kind=kind,
                provenance_kind="FirstParty",
                provenance_commitment_id=self._DIGEST,
                license_commitment_id=None,
                backward_dependencies=(),
                compatibility_profile_id=self._DIGEST,
                generator_commitment_id=None,
                target_policy_profile_id=self._DIGEST,
                custody_policy_profile_id=self._DIGEST,
                migration_profile_id=self._DIGEST,
                rollback_profile_id=self._DIGEST,
                uninstall_profile_id=self._DIGEST,
                retention_profile_id=self._DIGEST,
                removal_profile_id=self._DIGEST,
                disposition="Retain",
                proof_profile_id=self._DIGEST,
            )
            for index, kind in enumerate(bundle_kinds, 1)
        ]
        bundles: list[BundleManifest] = []
        for index, (kind, resource) in enumerate(zip(bundle_kinds, resources, strict=True), 1):
            bundles.append(
                construct_bundle_manifest(
                    bundle_kind=kind,
                    stable_bundle_key=f"bundle-{index}",
                    semantic_version="1",
                    compatibility_profile_id=self._DIGEST,
                    resources=[resource],
                    dependency_bundles=() if not bundles else [bundles[-1]],
                    provenance_commitment_id=self._DIGEST,
                    license_commitment_id=None,
                    package_policy_profile_id=self._DIGEST,
                    supported_target_classes=["WholeTarget"],
                    rollback_profile_id=self._DIGEST,
                    uninstall_profile_id=self._DIGEST,
                    retention_profile_id=self._DIGEST,
                    bundle_tag=index,
                )
            )
        consumer = DirectConsumer(
            locator="consumer",
            owner_tag=1,
            owner_profile_id=self._DIGEST,
            consumer_kind="Runtime",
            resources=(replace(resources[0]),),
            provenance_commitment_id=self._DIGEST,
            disposition="Retain",
            migration_profile_id=self._DIGEST,
            proof_profile_id=self._DIGEST,
            removal_profile_id=self._DIGEST,
        )
        census = construct_release_resource_census(
            release_key="release",
            release_version="1",
            platform_qualifier="test",
            resources=resources,
            bundles=bundles,
            direct_consumers=[consumer],
            source_inventory_digest=self._DIGEST,
            consumer_inventory_digest=self._DIGEST,
            build_graph_digest=self._DIGEST,
            resource_locators={item.resource_id: f"locator/{item.resource_tag}" for item in resources},
        )
        release = construct_embedded_release_bundle(
            release_key="release",
            release_version="1",
            platform_qualifier="test",
            resources=resources,
            bundles=bundles,
            census=census,
            core_contract_root_id=self._DIGEST,
            binary_compatibility_id=self._DIGEST,
            public_catalog_id=self._DIGEST,
            compatibility_profile_id=self._DIGEST,
            rollback_profile_id=self._DIGEST,
            uninstall_profile_id=self._DIGEST,
            retention_profile_id=self._DIGEST,
        )
        return resources, bundles, census, release

    def _rehash_census(self, census: ReleaseResourceCensus, value: list[Any]) -> ReleaseResourceCensus:
        envelope = manifest_envelope(
            RELEASE_CENSUS_MANIFEST_DOMAIN,
            FROZEN_SCHEMA_IDS["ReleaseResourceCensusV1"],
            FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"],
            value[0],
            value[1],
        )
        census_id, raw = identity_digest(envelope)
        return replace(census, value=value, envelope=envelope, census_id=census_id, canonical_cbor=raw)

    def _rehash_census_row(self, row: list[Any]) -> None:
        descriptor = descriptor_envelope(
            RELEASE_CENSUS_ROW_DESCRIPTOR_DOMAIN,
            FROZEN_SCHEMA_IDS["ReleaseResourceCensusEntryV1"],
            row[2],
        )
        descriptor_id, _ = identity_digest(descriptor)
        row[1] = bytes32(descriptor_id)

    def test_strict_cbor_round_trip_and_rejections(self) -> None:
        value = [1, True, False, "ascii", bytes32("00" * 32), [2, 3]]
        self.assertEqual(decode_cbor(encode_cbor(value)), value)
        for invalid in (None, -1, 1.5, {"not": "bytes32"}, "caf\N{LATIN SMALL LETTER E WITH ACUTE}"):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ContractError):
                    encode_cbor(invalid)
        for invalid in (b"\xf6", b"\xa0", b"\xc0\x00", b"\xf9\x00\x00", b"\x18\x01"):
            with self.subTest(invalid=invalid.hex()):
                with self.assertRaises(ContractError):
                    decode_cbor(invalid)

    def test_typed_optionals_are_exact(self) -> None:
        validate_optional(optional())
        validate_optional(optional(0))
        for invalid in ([], [0, 1], [1], [1, 2, 3], [2]):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ContractError):
                    validate_optional(invalid)

    def test_manifest_envelope_has_exact_five_slots(self) -> None:
        envelope = manifest_envelope(
            BUNDLE_MANIFEST_DOMAIN,
            FROZEN_SCHEMA_IDS["BundleManifestV1"],
            FROZEN_SCHEMA_IDS["ResourceDescriptorV1"],
            [1],
            [[1]],
        )
        self.assertEqual(len(envelope), 5)
        self.assertEqual(envelope[0], BUNDLE_MANIFEST_DOMAIN)

    def test_release_closure_accepts_backward_dependency_subsets_and_distinct_locators(self) -> None:
        resources, bundles, census, release = self._closure()
        self.assertEqual(BUNDLE_TOPOLOGY[2:5], ("SharedContract", "Orchestration", "Capability"))
        self.assertEqual(len(bundles[-1].dependency_bundle_ids), 1)
        self.assertNotEqual(census.value[1][0][2][1][1][0], resources[0].stable_resource_key)
        validate_release_closure(resources=resources, bundles=bundles, census=census, release=release)

    def test_release_closure_accepts_multiple_bundles_of_one_kind(self) -> None:
        bundle_kinds = (
            "Migration",
            "ExternalPattern",
            "SharedContract",
            "SharedContract",
            "Orchestration",
            "Capability",
            "Adapter",
            "AgentBootstrap",
        )
        resources, bundles, census, release = self._closure(bundle_kinds)
        self.assertEqual(len(bundles), 8)
        validate_release_closure(resources=resources, bundles=bundles, census=census, release=release)

    def test_deserialized_style_closed_tag_mutant_is_rejected(self) -> None:
        resources, _, _, _ = self._closure()
        value = copy.deepcopy(resources[0].value)
        value[4] = 99
        envelope = descriptor_envelope(
            RESOURCE_DESCRIPTOR_DOMAIN, FROZEN_SCHEMA_IDS["ResourceDescriptorV1"], value
        )
        resource_id, raw = identity_digest(envelope)
        mutant = replace(
            resources[0],
            value=value,
            envelope=envelope,
            resource_id=resource_id,
            canonical_cbor=raw,
        )
        with self.assertRaises(ContractError):
            _validate_resource(mutant)

    def test_census_wrong_bundle_and_cross_resource_edges_are_rejected(self) -> None:
        resources, bundles, census, _ = self._closure()

        wrong_bundle_value = copy.deepcopy(census.value)
        wrong_bundle_row = wrong_bundle_value[1][0]
        wrong_bundle_row[2][1][1][3] = bytes32(bundles[1].bundle_id)
        self._rehash_census_row(wrong_bundle_row)
        with self.assertRaises(ContractError):
            _validate_census(self._rehash_census(census, wrong_bundle_value), resources, bundles)

        cross_resource_value = copy.deepcopy(census.value)
        consumer_row = cross_resource_value[1][len(resources)]
        consumer_row[2][2][1][3][0][0] = resources[1].resource_tag
        self._rehash_census_row(consumer_row)
        with self.assertRaises(ContractError):
            _validate_census(self._rehash_census(census, cross_resource_value), resources, bundles)

    def test_manifest_core_frozen_commitments_are_rejected_when_mutated(self) -> None:
        _, bundles, _, _ = self._closure()
        for field_index in (9, 12, 13, 14, 15):
            with self.subTest(field_index=field_index):
                value = copy.deepcopy(bundles[0].value)
                value[0][0][field_index] = bytes32("22" * 32)
                envelope = manifest_envelope(
                    BUNDLE_MANIFEST_DOMAIN,
                    FROZEN_SCHEMA_IDS["BundleManifestV1"],
                    FROZEN_SCHEMA_IDS["ResourceDescriptorV1"],
                    value[0],
                    value[1],
                )
                bundle_id, raw = identity_digest(envelope)
                mutant = replace(
                    bundles[0],
                    value=value,
                    envelope=envelope,
                    bundle_id=bundle_id,
                    canonical_cbor=raw,
                )
                with self.assertRaises(ContractError):
                    _validate_bundle(mutant)

    def test_resource_identity_is_forbidden_outside_declared_dependencies(self) -> None:
        resources, _, _, _ = self._closure()
        value = copy.deepcopy(resources[0].value)
        value[13] = bytes32(resources[1].resource_id)
        envelope = descriptor_envelope(
            RESOURCE_DESCRIPTOR_DOMAIN,
            FROZEN_SCHEMA_IDS["ResourceDescriptorV1"],
            value,
        )
        resource_id, raw = identity_digest(envelope)
        mutant = replace(
            resources[0],
            value=value,
            envelope=envelope,
            resource_id=resource_id,
            canonical_cbor=raw,
        )
        with self.assertRaises(ContractError):
            _validate_resource_identity_placement(mutant, [mutant, *resources[1:]])

    def test_release_validation_rejects_duplicate_global_resource_tags(self) -> None:
        resources, bundles, census, release = self._closure()
        duplicate = replace(resources[1], resource_tag=resources[0].resource_tag)
        with self.assertRaises(ContractError):
            validate_release_closure(
                resources=[resources[0], duplicate, *resources[2:]],
                bundles=bundles,
                census=census,
                release=release,
            )

    def test_release_constructor_rejects_unvalidated_census_identity(self) -> None:
        resources, bundles, census, _ = self._closure()
        mutant = replace(census, census_id="22" * 32)
        with self.assertRaises(ContractError):
            construct_embedded_release_bundle(
                release_key="release",
                release_version="1",
                platform_qualifier="test",
                resources=resources,
                bundles=bundles,
                census=mutant,
                core_contract_root_id=self._DIGEST,
                binary_compatibility_id=self._DIGEST,
                public_catalog_id=self._DIGEST,
                compatibility_profile_id=self._DIGEST,
                rollback_profile_id=self._DIGEST,
                uninstall_profile_id=self._DIGEST,
                retention_profile_id=self._DIGEST,
            )

        value = copy.deepcopy(census.value)
        value[0][5] += 1
        semantic_mutant = self._rehash_census(census, value)
        with self.assertRaises(ContractError):
            construct_embedded_release_bundle(
                release_key="release",
                release_version="1",
                platform_qualifier="test",
                resources=resources,
                bundles=bundles,
                census=semantic_mutant,
                core_contract_root_id=self._DIGEST,
                binary_compatibility_id=self._DIGEST,
                public_catalog_id=self._DIGEST,
                compatibility_profile_id=self._DIGEST,
                rollback_profile_id=self._DIGEST,
                uninstall_profile_id=self._DIGEST,
                retention_profile_id=self._DIGEST,
            )

    def test_resource_constructor_normalizes_rendered_owner_digest(self) -> None:
        resource = construct_resource_descriptor(
            resource_tag=1,
            stable_resource_key="normalized-owner",
            content=b"normalized-owner",
            content_encoding="Utf8Text",
            media_type="text/plain",
            resource_kind="PublicContract",
            owner_tag=1,
            owner_profile_id=f"sha256:{self._DIGEST}",
            required_bundle_kind="SharedContract",
            provenance_kind="FirstParty",
            provenance_commitment_id=self._DIGEST,
            license_commitment_id=None,
            backward_dependencies=(),
            compatibility_profile_id=self._DIGEST,
            generator_commitment_id=None,
            target_policy_profile_id=self._DIGEST,
            custody_policy_profile_id=self._DIGEST,
            migration_profile_id=self._DIGEST,
            rollback_profile_id=self._DIGEST,
            uninstall_profile_id=self._DIGEST,
            retention_profile_id=self._DIGEST,
            removal_profile_id=self._DIGEST,
            disposition="Retain",
            proof_profile_id=self._DIGEST,
        )
        self.assertEqual(resource.owner, (1, self._DIGEST))
        _validate_resource(resource)


if __name__ == "__main__":
    unittest.main()
