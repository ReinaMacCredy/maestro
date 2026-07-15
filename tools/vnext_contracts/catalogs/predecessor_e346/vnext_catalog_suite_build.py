#!/usr/bin/env python3
"""Build the nine design-only ManifestIdentityV1 catalog candidates."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent
NOMINAL_PATH = ROOT / "vnext-catalog-nominal-source-v1.json"
GRAMMAR_PATH = ROOT / "vnext-catalog-profile-grammar-v1-sha256-2b428f8444253794cd0abb41b32da482cc0805359c2a37bf0cba90a70e3186e9.json"
PY_ENCODER = ROOT / "vnext_manifest_encode_py.py"
RB_ENCODER = ROOT / "vnext_manifest_encode_rb.rb"
NOMINAL = json.loads(NOMINAL_PATH.read_text(encoding="ascii"))
GRAMMAR = json.loads(GRAMMAR_PATH.read_text(encoding="ascii"))
GRAMMAR_ID = GRAMMAR["catalog_profile_grammar"]["catalog_profile_grammar_id"]


def head(major: int, value: int) -> bytes:
    if not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("unsigned value outside u64")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def encode(value) -> bytes:
    if value is False:
        return b"\xf4"
    if value is True:
        return b"\xf5"
    if isinstance(value, int) and not isinstance(value, bool):
        return head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return head(3, len(raw)) + raw
    if isinstance(value, list):
        return head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        raw = bytes.fromhex(value["bytes"])
        return head(2, len(raw)) + raw
    raise ValueError(f"unsupported value: {value!r}")


def digest(value) -> tuple[str, str, int]:
    raw = encode(value)
    return hashlib.sha256(raw).hexdigest(), raw.hex(), len(raw)


def b32(value: str) -> dict[str, str]:
    if len(value) != 64:
        raise ValueError(value)
    return {"bytes": value}


def sha_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


U64 = [1]
BOOL = [2]
ASCII = [3]
BYTES32 = [4, 32]
NO_EXTRA = [[1]]


def ordered_list(value):
    return [7, value]


def tuple_of(*values):
    return [8, list(values)]


def enum(enum_id: str, rows: list[list]):
    return [9, enum_id, rows]


def length(min_count: int, max_count: int):
    return [[2, min_count, max_count]]


def uint_range(minimum: int, maximum: int):
    return [[4, minimum, maximum]]


def field(position: int, name: str, type_expr, constraints=NO_EXTRA):
    return [position, name, type_expr, constraints]


def schema_value(name: str, fields: list[list]):
    return [name, 1, fields, []]


def schema_record(value) -> dict:
    envelope = ["maestro.vnext.schema.v1", value]
    schema_id, cbor_hex, byte_length = digest(envelope)
    return {
        "value": value,
        "schema_id": schema_id,
        "identity_envelope": envelope,
        "cbor_hex": cbor_hex,
        "byte_length": byte_length,
    }


OWNER_BY_NAME = {
    row["name"]: (row["tag"], row["profile_id"]) for row in GRAMMAR["owner_profiles"]
}
OWNER_NAME_BY_TAG = {tag: name for name, (tag, _profile_id) in OWNER_BY_NAME.items()}
PROTOCOL_BY_NAME = {row["name"]: row["profile_id"] for row in GRAMMAR["protocol_profiles"]}
POLICY_BY_NAME = {row["name"]: row["profile_id"] for row in GRAMMAR["policy_profiles"]}
PROFILE_SET_IDS = {
    name: row["profile_set_id"] for name, row in GRAMMAR["profile_sets"].items()
}


def owner_ref(name: str) -> list:
    tag, profile_id = OWNER_BY_NAME[name]
    return [tag, b32(profile_id)]


ACTION_SYMBOLS = sorted(GRAMMAR["action_leaf_symbols"], key=lambda row: row["global_tag"])
ACTION_BY_NAME = {row["name"]: row for row in ACTION_SYMBOLS}
ACTION_BY_TAG = {row["global_tag"]: row for row in ACTION_SYMBOLS}
CEREMONY_SYMBOLS = sorted(GRAMMAR["ceremony_symbols"], key=lambda row: row["tag"])
CEREMONY_BY_NAME = {row["name"]: row for row in CEREMONY_SYMBOLS}
ROUTES = sorted(GRAMMAR["effect_origin_routes"], key=lambda row: row["origin_tag"])


CATALOGS = [
    (1, "observation", "ObservationKindV1", "maestro.vnext.observation-kind.descriptor.v1", "maestro.vnext.observation-kind.manifest.v1", []),
    (2, "effect", "EffectOriginV1", "maestro.vnext.effect-origin.descriptor.v1", "maestro.vnext.effect-origin.manifest.v1", [1]),
    (3, "repository-capacity", "RepositoryGovernedCapacitySlotKindV1", "maestro.vnext.governed-capacity.repository.descriptor.v1", "maestro.vnext.governed-capacity.repository.manifest.v1", []),
    (4, "installation-capacity", "InstallationGovernedCapacitySlotKindV1", "maestro.vnext.governed-capacity.installation.descriptor.v1", "maestro.vnext.governed-capacity.installation.manifest.v1", []),
    (5, "ceremony", "CeremonySpecV1", "maestro.vnext.ceremony-spec.descriptor.v1", "maestro.vnext.ceremony-spec.manifest.v1", [1, 2, 3, 4]),
    (6, "action-leaf", "ActionLeafCensusV1", "maestro.vnext.action-leaf.descriptor.v1", "maestro.vnext.action-leaf-census.manifest.v1", [1, 2, 3, 4, 5]),
    (7, "repository-continuity", "RepositoryAuthorityContinuityClassV1", "maestro.vnext.authority-continuity.repository.descriptor.v1", "maestro.vnext.authority-continuity.repository.manifest.v1", [1, 2, 3, 5, 6]),
    (8, "installation-continuity", "InstallationAuthorityContinuityClassV1", "maestro.vnext.authority-continuity.installation.descriptor.v1", "maestro.vnext.authority-continuity.installation.manifest.v1", [1, 2, 4, 5, 6]),
    (9, "action-spec", "ActionSpecV1", "maestro.vnext.action-spec.descriptor.v1", "maestro.vnext.action-spec.catalog.v1", [1, 2, 3, 4, 5, 6, 7, 8]),
]


COMMON_PROTOCOLS = [
    "CatalogProfileGrammarV1",
    "CatalogOwnerClosureV1",
    "CatalogDependencyDagV1",
    "CatalogDenseTagClosureV1",
    "GeneratedClosedSumEncodingV1",
]
COMMON_POLICIES = [
    "ByteTotalNonPromotingMigrationV1",
    "ExactAdapterParityV1",
    "ConsumerTotalRemovalV1",
    "IndependentCatalogProofV1",
    "CommonFiniteBoundsV1",
    "FailClosedCatalogReopenV1",
]


FORBIDDEN_TAGS = {
    "authority_donation": 1,
    "runtime_registration": 2,
    "adapter_semantics": 3,
    "unknown_fallback": 4,
    "hidden_retry": 5,
    "latest_selector": 6,
    "cross_store_mutation": 7,
    "lease_donation": 8,
    "candidate_self_authorization": 9,
    "migration_promotion": 10,
}


def clauses(domain: str) -> list[list]:
    return [
        [1, "closed_core", f"{domain} is a closed compile-time semantic value; unknown values and versions fail closed."],
        [2, "non_authorizing", f"{domain} identity, ownership, provenance and profile membership grant no runtime authority, applicability or currentness."],
        [3, "migration", f"Migration preserves source bytes and provenance but does not activate {domain} semantics without exact admitted reconstruction."],
    ]


CMA_EDGES = [
    [1, 29, 1],
    [1, 30, 2],
    [2, 34, 3],
    [2, 33, 4],
    [3, 35, 5],
    [3, 33, 6],
    [4, 31, 7],
    [5, 32, 8],
    [5, 36, 9],
    [5, 37, 10],
]
CMA_OBSERVATION_TAGS = {edge[1] for edge in CMA_EDGES}


def observation_values() -> tuple[list, dict]:
    values = []
    general = ACTION_BY_NAME["PublishObservation"]["global_tag"]
    bootstrap_presentation = ACTION_BY_NAME["PublishBootstrapMandatePresentationObservation"]["global_tag"]
    bootstrap_response = ACTION_BY_NAME["PublishBootstrapMandateResponseObservation"]["global_tag"]
    cma = ACTION_BY_NAME["PublishContinuityMaintenanceObservation"]["global_tag"]
    for tag, name in enumerate(NOMINAL["observations"], 1):
        if tag <= 3:
            acquisition_class = tag
        elif tag <= 6:
            acquisition_class = 4
        elif tag <= 14:
            acquisition_class = 5
        elif tag <= 16:
            acquisition_class = 6
        elif tag <= 20:
            acquisition_class = 7
        elif tag <= 37:
            acquisition_class = 8
        elif tag <= 40:
            acquisition_class = 9
        else:
            acquisition_class = 10
        if tag == 17:
            producer = [bootstrap_presentation]
        elif tag == 18:
            producer = [bootstrap_response]
        elif tag in CMA_OBSERVATION_TAGS:
            producer = [cma]
        else:
            producer = [general]
        source_routes = [edge[2] for edge in CMA_EDGES if edge[1] == tag]
        values.append([
            tag,
            name,
            owner_ref("Evidence"),
            1,
            tag,
            tag,
            acquisition_class,
            producer,
            source_routes,
            1,
            1,
            1,
            1,
            sorted(FORBIDDEN_TAGS.values()),
            clauses(name),
        ])
    proof = {
        "kind_count": len(values),
        "cma_positive_edges": CMA_EDGES,
        "cma_positive_count": len(CMA_EDGES),
        "cma_negative_count": 5 * 43 - len(CMA_EDGES),
    }
    return values, proof


def effect_values() -> tuple[list, dict]:
    values = []
    for route in ROUTES:
        source_owner_tag = OWNER_BY_NAME[route["origin_source_owner"]][0]
        values.append([
            route["origin_tag"],
            route["origin_name"],
            owner_ref("Execution"),
            1,
            source_owner_tag,
            route["origin_tag"],
            route["origin_tag"],
            route["value"][3],
            1,
            1,
            sorted(FORBIDDEN_TAGS.values()),
            clauses(route["origin_name"]),
        ])
    return values, {
        "origin_count": len(values),
        "route_count": sum(len(value[7]) for value in values),
        "source_owner_count": len({value[4] for value in values}),
    }


def capacity_values(domain: str) -> tuple[list, dict]:
    names = NOMINAL["capacity_profiles"][domain]
    values = []
    domain_tag = 1 if domain == "Repository" else 2
    for tag, name in enumerate(names, 1):
        values.append([
            tag,
            name,
            owner_ref("Authority"),
            1,
            domain_tag,
            tag,
            True,
            True,
            True,
            tag,
            clauses(name),
        ])
    return values, {"domain": domain, "descriptor_count": len(values), "finite_nonzero_required": True, "refill_forbidden": True}


def invert_routes(symbol_kind: int, symbol_tag: int) -> list[list]:
    result = []
    for route in ROUTES:
        for entry in route["value"][3]:
            if entry[2] == symbol_kind and entry[5] == symbol_tag:
                result.append([route["origin_tag"], entry[0], entry[1], entry[3], entry[4]])
    return sorted(result)


def ceremony_values() -> tuple[list, dict]:
    values = []
    participant_tags = [OWNER_BY_NAME[name][0] for name in ["Authority", "Execution", "Persistence", "Distribution", "Installation"]]
    for symbol in CEREMONY_SYMBOLS:
        tag = symbol["tag"]
        name = symbol["name"]
        routes = invert_routes(2, tag)
        context_tag = 2 if tag == 1 else 3
        values.append([
            tag,
            name,
            owner_ref(symbol["owner"]),
            1,
            context_tag,
            4,
            tag,
            [1, 2],
            [row[0] for row in routes],
            participant_tags,
            sorted(FORBIDDEN_TAGS.values()),
            clauses(name),
        ])
    return values, {"ceremony_count": len(values), "mode_count": 2, "routed_origin_count": len({tag for value in values for tag in value[8]})}


def action_basis(name: str) -> int:
    if "BootstrapMandate" in name:
        return 2
    if "ContinuityMaintenance" in name:
        return 3
    return 1


def action_effect_class(name: str, owner: str) -> int:
    if name.startswith("Reserve") or name.startswith("Originate"):
        return 2
    if "Outcome" in name or "Occurrence" in name or name == "RecordDispatchOutcome":
        return 3
    if name.startswith("Reconcile") or name.startswith("Recover") or name.startswith("Rollback"):
        return 4
    if owner == "Distribution" or name in {"ExecuteGcSweep", "StageRestoreCandidate", "VerifyRestoreCandidate"}:
        return 5
    return 1


def observation_tags_for_action(name: str) -> list[int]:
    if name == "PublishObservation":
        return [tag for tag in range(1, 44) if tag not in CMA_OBSERVATION_TAGS and tag not in {17, 18}]
    if name == "PublishBootstrapMandatePresentationObservation":
        return [17]
    if name == "PublishBootstrapMandateResponseObservation":
        return [18]
    if name == "PublishContinuityMaintenanceObservation":
        return sorted(CMA_OBSERVATION_TAGS)
    return []


def produced_record_tags(name: str, owner: str, effect_class: int) -> list[int]:
    tags = [1, 2, 3, 4]
    if effect_class in {2, 3, 4}:
        tags.extend([5, 6, 11])
    if name.startswith("Publish") and owner == "Evidence":
        tags.append(7 if "Observation" in name else 8)
    if owner == "Persistence":
        tags.append(9)
    if "Tombstone" in name or "Erase" in name or "Purge" in name:
        tags.append(10)
    return sorted(set(tags))


def action_leaf_values() -> tuple[list, dict]:
    values = []
    for symbol in ACTION_SYMBOLS:
        name = symbol["name"]
        owner = symbol["owner"]
        effect_class = action_effect_class(name, owner)
        routes = invert_routes(1, symbol["global_tag"])
        values.append([
            symbol["global_tag"],
            name,
            owner_ref(owner),
            1,
            symbol["family_tag"],
            symbol["family_local_tag"],
            action_basis(name),
            effect_class,
            2 if effect_class in {2, 3, 4, 5} else 1,
            2 if effect_class in {2, 3, 4, 5} else 1,
            observation_tags_for_action(name),
            [row[0] for row in routes],
            produced_record_tags(name, owner, effect_class),
            sorted(FORBIDDEN_TAGS.values()),
            clauses(name),
        ])
    family_counts = {}
    for row in values:
        family_counts[OWNER_NAME_BY_TAG[row[2][0]]] = family_counts.get(OWNER_NAME_BY_TAG[row[2][0]], 0) + 1
    return values, {"leaf_count": len(values), "family_counts": family_counts, "origin_route_refs": sum(len(value[11]) for value in values)}


REPOSITORY_CLASSES = [
    ("RepositoryOrdinaryMutationCapacityState", "Authority", 1, True, True, True, False, False),
    ("RepositoryAuthorityAdministrationCapacityState", "Authority", 1, True, True, True, True, False),
    ("RepositoryEvidenceAcquisitionCapacityState", "Authority", 1, True, True, True, False, False),
    ("RepositoryPlanningPublicationCapacityState", "Authority", 1, True, True, True, False, False),
    ("RepositoryExternalEffectCapacityState", "Authority", 1, True, True, True, False, True),
    ("RepositoryPersistenceMaintenanceCapacityState", "Authority", 1, True, True, True, False, False),
    ("RepositoryStoreGenerationCurrentness", "Authority", 1, True, True, True, False, False),
    ("RepositoryGovernanceHead", "Authority", 1, True, True, True, True, False),
    ("RepositoryAuthorityEpochState", "Authority", 1, True, True, True, True, False),
    ("RepositoryTrustRootState", "Authority", 1, True, True, True, True, False),
    ("RepositoryPrincipalBindingState", "Authority", 1, True, True, True, False, False),
    ("RepositorySessionState", "Authority", 1, True, True, True, False, False),
    ("RepositoryGrantState", "Authority", 1, True, True, True, True, False),
    ("RepositoryDelegationState", "Authority", 1, True, True, True, True, False),
    ("RepositoryMandateState", "Authority", 1, True, True, True, True, False),
    ("RepositoryRevocationState", "Authority", 1, True, True, True, True, False),
    ("RepositoryAuthorizationReceiptState", "Authority", 1, True, True, True, True, False),
    ("RepositoryConsumptionCellState", "Authority", 1, True, True, True, True, False),
    ("RepositoryContinuityState", "Authority", 1, True, True, True, True, True),
    ("RepositoryTrustedTimeState", "Authority", 1, True, True, True, True, False),
    ("RepositoryRecoveryCommitmentState", "Authority", 1, True, True, True, True, False),
    ("RepositoryRecoveryAdmissionState", "Authority", 1, True, True, True, True, True),
    ("RepositoryStepExecutionState", "Execution", 1, True, True, True, False, False),
    ("RepositoryEffectIntentState", "Execution", 1, True, True, True, True, True),
    ("RepositoryEvidenceState", "Evidence", 1, True, True, True, False, False),
    ("RepositoryGateSnapshot", "GatePolicy", 2, False, True, False, False, False),
    ("RepositoryPlanningState", "Planning", 1, True, True, True, False, False),
    ("RepositoryCoordinationState", "Coordination", 1, True, True, True, False, False),
    ("RepositoryDesignDecisionState", "Design", 1, True, True, True, False, False),
    ("RepositoryContractState", "Contract", 1, True, True, True, False, False),
    ("RepositoryWorkState", "Work", 1, True, True, True, False, False),
    ("RepositoryPersistenceRetentionState", "Persistence", 1, True, True, True, False, False),
    ("RepositoryMemoryState", "Memory", 1, True, True, True, False, False),
    ("RepositoryIntakeState", "Intake", 1, True, True, True, False, False),
    ("RepositoryResearchState", "Research", 1, True, True, True, False, False),
]


INSTALLATION_CLASSES = [
    ("InstallationAuthorityAdministrationCapacityState", "Authority", 1, True, True, True, True, False),
    ("InstallationDistributionMutationCapacityState", "Authority", 1, True, True, True, False, True),
    ("InstallationGovernedReviewPublicationCapacityState", "Authority", 1, True, True, True, True, True),
    ("InstallationExternalEffectCapacityState", "Authority", 1, True, True, True, False, True),
    ("InstallationWriterAdministrationCapacityState", "Authority", 1, True, True, True, True, False),
    ("InstallationPersistenceMaintenanceCapacityState", "Authority", 1, True, True, True, False, False),
    ("InstallationLocatorCurrentness", "Installation", 1, True, True, True, True, True),
    ("InstallationStoreGenerationCurrentness", "Installation", 1, True, True, True, False, False),
    ("InstallationGovernanceHead", "Authority", 1, True, True, True, True, False),
    ("InstallationAuthorityEpochState", "Authority", 1, True, True, True, True, False),
    ("InstallationTrustRootState", "Authority", 1, True, True, True, True, False),
    ("InstallationPrincipalBindingState", "Authority", 1, True, True, True, False, False),
    ("InstallationGrantState", "Authority", 1, True, True, True, True, False),
    ("InstallationMandateState", "Authority", 1, True, True, True, True, False),
    ("InstallationRevocationState", "Authority", 1, True, True, True, True, False),
    ("InstallationAuthorizationReceiptState", "Authority", 1, True, True, True, True, False),
    ("InstallationConsumptionCellState", "Authority", 1, True, True, True, True, False),
    ("InstallationContinuityState", "Authority", 1, True, True, True, True, True),
    ("InstallationRecoveryCommitmentState", "Authority", 1, True, True, True, True, False),
    ("InstallationRecoveryAdmissionState", "Installation", 1, True, True, True, True, True),
    ("InstallationWriterCohortState", "Installation", 1, True, True, True, True, False),
    ("InstallationClientCompatibilityState", "Installation", 1, True, True, True, False, False),
    ("InstallationDistributionTargetState", "Distribution", 1, True, True, True, False, True),
    ("InstallationDistributionTransactionState", "Distribution", 1, True, True, True, False, True),
    ("InstallationBinarySlotState", "Distribution", 1, True, True, True, False, True),
    ("InstallationResourceManifestState", "Distribution", 1, True, True, True, False, False),
    ("InstallationGovernedReviewPublicationState", "Authority", 1, True, True, True, True, True),
    ("InstallationEffectIntentState", "Execution", 1, True, True, True, True, True),
    ("InstallationEvidenceState", "Evidence", 1, True, True, True, False, False),
    ("InstallationPersistenceRetentionState", "Persistence", 1, True, True, True, False, False),
]


def continuity_values(domain: str) -> tuple[list, dict]:
    class_defs = REPOSITORY_CLASSES if domain == "Repository" else INSTALLATION_CLASSES
    capacity_catalog_tag = 3 if domain == "Repository" else 4
    source_catalogs = [1, 2, capacity_catalog_tag, 5, 6]
    source_counts = {1: 43, 2: 23, capacity_catalog_tag: 6, 5: 11, 6: 136}
    obligations = []
    raw = []
    for source_kind_tag in source_catalogs:
        for source_tag in range(1, source_counts[source_kind_tag] + 1):
            if source_kind_tag == capacity_catalog_tag:
                disposition = 1
                targets = [source_tag]
                invariant = 0
            else:
                disposition = 2
                targets = []
                invariant = 1
            raw.append((8, source_kind_tag, source_tag, 1, 1, disposition, targets, invariant))
    for class_tag, definition in enumerate(class_defs, 1):
        owner_tag = OWNER_BY_NAME[definition[1]][0]
        raw.append((owner_tag, 100 + class_tag, class_tag, 1, 2, 1, [class_tag], 0))
    raw.sort(key=lambda row: (row[0], row[1], row[2], row[3], row[4], row[5], row[6]))
    for obligation_tag, row in enumerate(raw, 1):
        owner_tag, source_protocol_tag, source_identity_tag, selector_tag, duty_tag, disposition_tag, targets, invariant_tag = row
        obligations.append([
            obligation_tag,
            owner_tag,
            source_protocol_tag,
            source_identity_tag,
            selector_tag,
            duty_tag,
            disposition_tag,
            targets,
            invariant_tag,
        ])
    values = []
    for tag, definition in enumerate(class_defs, 1):
        name, owner, class_disposition, canonical, graph, replay, historical, unresolved = definition
        obligation_tags = [row[0] for row in obligations if row[6] == 1 and tag in row[7]]
        facet_modes = [1 if value else 3 for value in [canonical, graph, replay, historical, unresolved]]
        values.append([
            tag,
            name,
            owner_ref(owner),
            1,
            OWNER_BY_NAME[owner][0],
            class_disposition,
            100 + tag,
            obligation_tags,
            facet_modes,
            1,
            1,
            sorted(FORBIDDEN_TAGS.values()),
            clauses(name),
        ])
    included = sum(1 for row in obligations if row[6] == 1)
    excluded = sum(1 for row in obligations if row[6] == 2)
    proof = {
        "domain": domain,
        "atomic_obligations": obligations,
        "atomic_obligation_count": len(obligations),
        "included_count": included,
        "explicitly_non_continuity_count": excluded,
        "class_count": len(values),
        "source_catalog_counts": [[tag, source_counts[tag]] for tag in source_catalogs],
        "facet_count_per_class": 5,
    }
    return values, proof


def action_spec_values(action_values: list, observation_values_rows: list) -> tuple[list, dict]:
    values = []
    for leaf in action_values:
        global_tag, name, owner, _version, family_tag, leaf_tag, basis, effect_class, idempotency, commit_profile, observation_tags, origin_tags, produced, forbidden, _clauses = leaf
        owner_name = OWNER_NAME_BY_TAG[owner[0]]
        guard_mask = 0b111111 if owner_name == "Authority" else 0b011111
        dependency_mask = 0b11111111
        receipt_mask = 0b1111 if effect_class in {2, 3, 4, 5} else 0b0111
        retry_tag = 2 if effect_class in {2, 3, 4, 5} else 1
        auto_safe = owner_name in {"SearchMaintenance"} and effect_class == 1
        scheduling = owner_name not in {"Authority", "Planning", "SearchMaintenance"}
        wave_tag = 2 if scheduling and effect_class == 1 else 1
        values.append([
            global_tag,
            name,
            owner,
            1,
            family_tag,
            leaf_tag,
            global_tag,
            global_tag,
            basis,
            guard_mask,
            dependency_mask,
            receipt_mask,
            idempotency,
            retry_tag,
            effect_class,
            auto_safe,
            scheduling,
            wave_tag,
            commit_profile,
            produced,
            observation_tags,
            origin_tags,
            forbidden,
            clauses(name),
        ])
    return values, {
        "spec_count": len(values),
        "leaf_equality_count": len(action_values),
        "scheduling_participant_count": sum(1 for value in values if value[16]),
        "auto_safe_count": sum(1 for value in values if value[15]),
    }


def owner_relation(domain_tag: int, descriptor_domain: str, values: list) -> dict:
    rows = [[value[0], value[2][0], value[2][1]] for value in values]
    envelope = [f"maestro.vnext.catalog.primary-owner-relation.{domain_tag}.v1", rows]
    relation_id, cbor_hex, byte_length = digest(envelope)
    return {
        "rows": rows,
        "identity_envelope": envelope,
        "relation_id": relation_id,
        "cbor_hex": cbor_hex,
        "byte_length": byte_length,
        "descriptor_domain": descriptor_domain,
    }


def common_schema_records(slug: str, sum_name: str, values: list, descriptor_fields: list[list], dependency_max: int) -> dict:
    prefix = f"maestro.vnext.catalog.{slug}"
    sum_schema = schema_record(schema_value(f"{prefix}.generated-sum.v1", [
        field(1, "member_tag", enum(f"{sum_name}TagV1", [[value[0], value[1]] for value in values])),
    ]))
    descriptor_schema = schema_record(schema_value(f"{prefix}.descriptor-value.v1", descriptor_fields))
    owner_row_schema = schema_record(schema_value(f"{prefix}.primary-owner-row.v1", [
        field(1, "member_tag", U64, uint_range(1, max(value[0] for value in values))),
        field(2, "owner_tag", U64, uint_range(1, 20)),
        field(3, "owner_profile_id", BYTES32),
    ]))
    header_schema = schema_record(schema_value(f"{prefix}.manifest-header.v1", [
        field(1, "catalog_tag", U64, uint_range(1, 9)),
        field(2, "core_version", U64, uint_range(1, 1)),
        field(3, "canonicalization_version", U64, uint_range(1, 1)),
        field(4, "grammar_id", BYTES32),
        field(5, "generated_sum_schema_id", BYTES32),
        field(6, "descriptor_schema_id", BYTES32),
        field(7, "header_schema_id", BYTES32),
        field(8, "manifest_schema_id", BYTES32),
        field(9, "dependency_refs", ordered_list(tuple_of(U64, BYTES32)), length(dependency_max, dependency_max)),
        field(10, "protocol_profile_ids", ordered_list(BYTES32), length(5, 6)),
        field(11, "primary_owner_relation_id", BYTES32),
        field(12, "owner_profile_set_id", BYTES32),
        field(13, "policy_profile_set_id", BYTES32),
        field(14, "protocol_profile_set_id", BYTES32),
        field(15, "row_count", U64, uint_range(len(values), len(values))),
        field(16, "minimum_tag", U64, uint_range(1, 1)),
        field(17, "maximum_tag", U64, uint_range(max(value[0] for value in values), max(value[0] for value in values))),
        field(18, "migration_profile_id", BYTES32),
        field(19, "parity_profile_id", BYTES32),
        field(20, "removal_profile_id", BYTES32),
        field(21, "proof_profile_id", BYTES32),
        field(22, "finite_bounds_profile_id", BYTES32),
        field(23, "reopen_profile_id", BYTES32),
    ]))
    manifest_row_schema = schema_record(schema_value(f"{prefix}.manifest-row.v1", [
        field(1, "member_tag", U64, uint_range(1, max(value[0] for value in values))),
        field(2, "descriptor_id", BYTES32),
        field(3, "descriptor_value", [
            5,
            f"{prefix}.descriptor-value.v1",
            1,
            b32(descriptor_schema["schema_id"]),
        ]),
    ]))
    manifest_schema = schema_record(schema_value(f"{prefix}.manifest-value.v1", [
        field(1, "header_value", [
            5,
            f"{prefix}.manifest-header.v1",
            1,
            b32(header_schema["schema_id"]),
        ]),
        field(2, "rows", ordered_list([
            5,
            f"{prefix}.manifest-row.v1",
            1,
            b32(manifest_row_schema["schema_id"]),
        ]), length(len(values), len(values))),
    ]))
    return {
        "generated_sum": sum_schema,
        "descriptor": descriptor_schema,
        "primary_owner_row": owner_row_schema,
        "header": header_schema,
        "manifest_row": manifest_row_schema,
        "manifest": manifest_schema,
    }


def generic_descriptor_fields(kind: str, value_length: int) -> list[list]:
    fields = [
        field(1, "member_tag", U64, uint_range(1, 65535)),
        field(2, "member_name", ASCII, length(1, 192)),
        field(3, "primary_owner_ref", tuple_of(U64, BYTES32)),
        field(4, "semantic_version", U64, uint_range(1, 1)),
    ]
    for position in range(5, value_length):
        fields.append(field(position, f"{kind}_semantic_field_{position}", ordered_list(U64), length(0, 65535)))
    fields.append(field(value_length, "normative_clauses", ordered_list(tuple_of(U64, ASCII, ASCII)), length(3, 32)))
    return fields


def type_for_value(value):
    if isinstance(value, bool):
        return BOOL
    if isinstance(value, int):
        return U64
    if isinstance(value, str):
        return ASCII
    if isinstance(value, dict):
        return BYTES32
    if isinstance(value, list):
        if not value:
            return ordered_list(U64)
        first = value[0]
        if isinstance(first, int):
            return ordered_list(U64)
        if isinstance(first, bool):
            return ordered_list(BOOL)
        if isinstance(first, str):
            return ordered_list(ASCII)
        if isinstance(first, list):
            return ordered_list(tuple_of(*(type_for_value(item) for item in first)))
    raise ValueError(f"unsupported schema inference {value!r}")


def descriptor_fields_for(values: list, kind: str) -> list[list]:
    first = values[0]
    names = ["member_tag", "member_name", "primary_owner_ref", "semantic_version"]
    semantic_names = {
        "observation": ["subject_contract_tag", "payload_contract_tag", "acquisition_class_tag", "producer_action_tags", "source_route_tags", "cardinality_tag", "freshness_policy_tag", "retention_policy_tag", "export_policy_tag", "forbidden_semantics"],
        "effect": ["source_owner_tag", "subject_contract_tag", "uniqueness_domain_tag", "route_entries", "credential_class_tag", "retention_policy_tag", "forbidden_semantics"],
        "capacity": ["context_domain_tag", "unit_contract_tag", "finite_nonzero_required", "genesis_bound", "refill_forbidden", "attachment_class_tag"],
        "ceremony": ["context_tag", "authority_basis_tag", "carrier_tag", "operation_mode_tags", "effect_origin_tags", "participant_owner_tags", "forbidden_semantics"],
        "action-leaf": ["family_tag", "family_local_tag", "authority_basis_tag", "effect_class_tag", "idempotency_class_tag", "commit_profile_tag", "observation_kind_tags", "effect_origin_tags", "produced_record_tags", "forbidden_semantics"],
        "continuity": ["source_owner_tag", "class_disposition_tag", "source_protocol_tag", "source_obligation_tags", "facet_mode_tags", "predecessor_mode_tag", "currentness_carrier_tag", "forbidden_semantics"],
        "action-spec": ["family_tag", "family_local_tag", "subject_contract_tag", "payload_contract_tag", "authority_basis_tag", "guard_mask", "material_dependency_mask", "receipt_slot_mask", "idempotency_class_tag", "retry_class_tag", "effect_class_tag", "auto_safe", "scheduling_participant", "wave_class_tag", "commit_profile_tag", "produced_record_tags", "observation_kind_tags", "effect_origin_tags", "forbidden_semantics"],
    }[kind]
    names.extend(semantic_names)
    names.append("normative_clauses")
    if len(names) != len(first):
        raise ValueError((kind, len(names), len(first)))
    fields = []
    for position, (name, sample) in enumerate(zip(names, first), 1):
        if position == 1:
            fields.append(field(position, name, U64, uint_range(1, 65535)))
        elif position == 2:
            fields.append(field(position, name, ASCII, length(1, 192)))
        elif position == 3:
            fields.append(field(position, name, tuple_of(U64, BYTES32)))
        elif position == 4:
            fields.append(field(position, name, U64, uint_range(1, 1)))
        elif name == "normative_clauses":
            fields.append(field(position, name, ordered_list(tuple_of(U64, ASCII, ASCII)), length(3, 32)))
        else:
            inferred = type_for_value(sample)
            constraints = length(0, 65535) if inferred[0] == 7 else NO_EXTRA
            fields.append(field(position, name, inferred, constraints))
    return fields


def encoder_receipt(path: Path) -> dict:
    py = subprocess.run(["python3", str(PY_ENCODER), str(path)], check=True, capture_output=True, text=True).stdout.splitlines()
    rb = subprocess.run(["ruby", str(RB_ENCODER), str(path)], check=True, capture_output=True, text=True).stdout.splitlines()
    if py != rb or len(py) != 3:
        raise RuntimeError(f"encoder mismatch for {path}")
    return {"cbor_hex": py[0], "byte_length": int(py[1]), "sha256": py[2]}


def build_catalog(meta, values: list, proof: dict, prior: dict[int, dict]) -> dict:
    tag, slug, sum_name, descriptor_domain, manifest_domain, dependency_tags = meta
    kind = "capacity" if "capacity" in slug else "continuity" if "continuity" in slug else slug
    fields = descriptor_fields_for(values, kind)
    schemas = common_schema_records(slug, sum_name, values, fields, len(dependency_tags))
    relation = owner_relation(tag, descriptor_domain, values)
    dependency_refs = [[dep_tag, b32(prior[dep_tag]["manifest_id"])] for dep_tag in dependency_tags]
    protocols = COMMON_PROTOCOLS + (["EffectOriginRouteClosureV1"] if tag in {2, 5, 6, 7, 8, 9} else [])
    header = [
        tag,
        1,
        1,
        b32(GRAMMAR_ID),
        b32(schemas["generated_sum"]["schema_id"]),
        b32(schemas["descriptor"]["schema_id"]),
        b32(schemas["header"]["schema_id"]),
        b32(schemas["manifest"]["schema_id"]),
        dependency_refs,
        [b32(PROTOCOL_BY_NAME[name]) for name in protocols],
        b32(relation["relation_id"]),
        b32(PROFILE_SET_IDS["OwnerProfileSetV1"]),
        b32(PROFILE_SET_IDS["PolicyProfileSetV1"]),
        b32(PROFILE_SET_IDS["ProtocolProfileSetV1"]),
        len(values),
        1,
        max(value[0] for value in values),
        b32(POLICY_BY_NAME[COMMON_POLICIES[0]]),
        b32(POLICY_BY_NAME[COMMON_POLICIES[1]]),
        b32(POLICY_BY_NAME[COMMON_POLICIES[2]]),
        b32(POLICY_BY_NAME[COMMON_POLICIES[3]]),
        b32(POLICY_BY_NAME[COMMON_POLICIES[4]]),
        b32(POLICY_BY_NAME[COMMON_POLICIES[5]]),
    ]
    rows = []
    descriptor_records = []
    for value in values:
        envelope = [descriptor_domain, b32(schemas["descriptor"]["schema_id"]), value]
        descriptor_id, cbor_hex, byte_length = digest(envelope)
        rows.append([value[0], b32(descriptor_id), value])
        descriptor_records.append({
            "tag": value[0],
            "name": value[1],
            "descriptor_id": descriptor_id,
            "value": value,
            "identity_envelope": envelope,
            "cbor_hex": cbor_hex,
            "byte_length": byte_length,
        })
    manifest_value = [header, rows]
    manifest_envelope = [
        manifest_domain,
        b32(schemas["manifest"]["schema_id"]),
        b32(schemas["descriptor"]["schema_id"]),
        header,
        rows,
    ]
    manifest_id, manifest_hex, manifest_length = digest(manifest_envelope)
    identity_envelopes = [row["identity_envelope"] for row in schemas.values()]
    identity_envelopes.append(relation["identity_envelope"])
    identity_envelopes.extend(row["identity_envelope"] for row in descriptor_records)
    identity_envelopes.append(manifest_envelope)
    encoder_path = ROOT / f"vnext-{slug}-v1-encoder-input.json"
    encoder_path.write_text(json.dumps(identity_envelopes, indent=2, sort_keys=True) + "\n", encoding="ascii")
    receipt = encoder_receipt(encoder_path)
    artifact = {
        "schema_version": "maestro.vnext.catalog.literal-artifact.v1",
        "status": "design-only-candidate",
        "catalog_tag": tag,
        "catalog_slug": slug,
        "generated_sum_name": sum_name,
        "descriptor_domain": descriptor_domain,
        "manifest_domain": manifest_domain,
        "grammar_id": GRAMMAR_ID,
        "grammar_artifact_sha256": sha_file(GRAMMAR_PATH),
        "nominal_source_sha256": sha_file(NOMINAL_PATH),
        "schemas": schemas,
        "primary_owner_relation": relation,
        "manifest_header": header,
        "descriptors": descriptor_records,
        "manifest_value": manifest_value,
        "manifest_identity_envelope": manifest_envelope,
        "manifest_id": manifest_id,
        "cbor_hex": manifest_hex,
        "byte_length": manifest_length,
        "encoder_input_path": encoder_path.name,
        "encoder_input_sha256": sha_file(encoder_path),
        "encoder_receipts": {
            "python_encoder_sha256": sha_file(PY_ENCODER),
            "ruby_encoder_sha256": sha_file(RB_ENCODER),
            "aggregate": receipt,
            "equal": True,
        },
        "semantic_proof": proof,
    }
    artifact_path = ROOT / f"vnext-{slug}-v1-sha256-{manifest_id}.json"
    artifact_path.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n", encoding="ascii")
    artifact["artifact_path"] = artifact_path.name
    artifact["artifact_sha256"] = sha_file(artifact_path)
    return artifact


def main() -> None:
    observation, observation_proof = observation_values()
    effect, effect_proof = effect_values()
    repo_capacity, repo_capacity_proof = capacity_values("Repository")
    install_capacity, install_capacity_proof = capacity_values("Installation")
    ceremony, ceremony_proof = ceremony_values()
    action_leaf, action_leaf_proof = action_leaf_values()
    repo_continuity, repo_continuity_proof = continuity_values("Repository")
    install_continuity, install_continuity_proof = continuity_values("Installation")
    action_spec, action_spec_proof = action_spec_values(action_leaf, observation)
    prepared = {
        1: (observation, observation_proof),
        2: (effect, effect_proof),
        3: (repo_capacity, repo_capacity_proof),
        4: (install_capacity, install_capacity_proof),
        5: (ceremony, ceremony_proof),
        6: (action_leaf, action_leaf_proof),
        7: (repo_continuity, repo_continuity_proof),
        8: (install_continuity, install_continuity_proof),
        9: (action_spec, action_spec_proof),
    }
    built: dict[int, dict] = {}
    for meta in CATALOGS:
        tag = meta[0]
        values, proof = prepared[tag]
        built[tag] = build_catalog(meta, values, proof, built)
    index = {
        "schema_version": "maestro.vnext.catalog.literal-suite.v1",
        "status": "design-only-candidate",
        "grammar_id": GRAMMAR_ID,
        "grammar_artifact_sha256": sha_file(GRAMMAR_PATH),
        "nominal_source_sha256": sha_file(NOMINAL_PATH),
        "builder_sha256": sha_file(Path(__file__)),
        "python_encoder_sha256": sha_file(PY_ENCODER),
        "ruby_encoder_sha256": sha_file(RB_ENCODER),
        "catalogs": [
            {
                "catalog_tag": tag,
                "catalog_slug": built[tag]["catalog_slug"],
                "manifest_id": built[tag]["manifest_id"],
                "artifact_path": built[tag]["artifact_path"],
                "artifact_sha256": built[tag]["artifact_sha256"],
                "row_count": len(built[tag]["descriptors"]),
                "byte_length": built[tag]["byte_length"],
                "encoder_input_sha256": built[tag]["encoder_input_sha256"],
                "encoder_aggregate_sha256": built[tag]["encoder_receipts"]["aggregate"]["sha256"],
            }
            for tag in range(1, 10)
        ],
        "dependency_dag": [[meta[0], meta[5]] for meta in CATALOGS],
        "aggregate_counts": {
            "catalogs": 9,
            "rows": sum(len(built[tag]["descriptors"]) for tag in built),
            "schemas": sum(len(built[tag]["schemas"]) for tag in built),
            "effect_routes": effect_proof["route_count"],
            "cma_positive": observation_proof["cma_positive_count"],
            "cma_negative": observation_proof["cma_negative_count"],
        },
    }
    index_path = ROOT / "vnext-catalog-literal-suite-v1-index.json"
    index_path.write_text(json.dumps(index, indent=2, sort_keys=True) + "\n", encoding="ascii")
    print(json.dumps({"index": index_path.name, "index_sha256": sha_file(index_path), "catalogs": index["catalogs"]}, indent=2))


if __name__ == "__main__":
    main()
