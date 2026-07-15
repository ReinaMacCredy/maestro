#!/usr/bin/env python3
"""Materialize the inactive efa0 Stage-0 grammar and nine core catalogs."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

from cbor_py import encode


REPO = Path(__file__).resolve().parents[3]
CONTRACT_ROOT = REPO / "contracts/vnext/catalogs"
EVIDENCE_ROOT = CONTRACT_ROOT / "evidence"
DEFAULT_OUTPUT = CONTRACT_ROOT / "generated"
PY_ENCODER = Path(__file__).with_name("cbor_py.py")
RB_ENCODER = Path(__file__).with_name("cbor_rb.rb")
VALIDATOR = Path(__file__).with_name("validate.py")
U64_MAX = 0xFFFFFFFFFFFFFFFF

STATUS = "stage0_candidate_not_published"
PUBLICATION_STATE = "inactive_candidate"
CURRENT_DECISION = "dec-close-lifecycle-to-action-totality-and-efa0"
CURRENT_DECISION_BODY_SHA256 = "811474c083177cb9b625e001e7431782e0827eec95572562c8a227054c15e147"
PREDECESSOR_GRAMMAR_ID = "2b428f8444253794cd0abb41b32da482cc0805359c2a37bf0cba90a70e3186e9"

U64 = [1]
BOOL = [2]
ASCII = [3]
BYTES32 = [4, 32]
NO_EXTRA = [[1]]

OWNER_NAMES = [
    "Work",
    "Step",
    "Contract",
    "Design",
    "Decision",
    "Execution",
    "Evidence",
    "GatePolicy",
    "Authority",
    "Coordination",
    "Orchestration",
    "Planning",
    "Projection",
    "Persistence",
    "SearchMaintenance",
    "Memory",
    "Intake",
    "Research",
    "Integration",
    "Distribution",
    "Installation",
]
ZERO_MEMBER_PROFILES = {"GatePolicy", "Orchestration", "Projection", "Integration"}
ACTION_FAMILY_COUNTS = {
    "Work": 7,
    "Step": 4,
    "Contract": 2,
    "Design": 4,
    "Decision": 5,
    "Execution": 16,
    "Evidence": 7,
    "Authority": 48,
    "Coordination": 9,
    "Planning": 4,
    "Persistence": 10,
    "Distribution": 13,
    "SearchMaintenance": 2,
    "Memory": 7,
    "Intake": 3,
    "Research": 4,
}
COMBINED_PROFILE_COUNTS = {
    "Work": 7,
    "Step": 4,
    "Contract": 2,
    "Design": 4,
    "Decision": 5,
    "Execution": 16,
    "Evidence": 7,
    "GatePolicy": 0,
    "Authority": 52,
    "Coordination": 9,
    "Orchestration": 0,
    "Planning": 4,
    "Projection": 0,
    "Persistence": 10,
    "SearchMaintenance": 2,
    "Memory": 7,
    "Intake": 3,
    "Research": 4,
    "Integration": 0,
    "Distribution": 14,
    "Installation": 6,
}

ROUTE_ROLES = [
    [1, "ActionReserve"],
    [2, "ActionRecoverReserved"],
    [3, "ActionOutcome"],
    [4, "ActionReconcile"],
    [5, "ActionWithdraw"],
    [6, "CeremonyInitiate"],
    [7, "CeremonyRecoverReserved"],
    [8, "CeremonyResolveResult"],
    [9, "CeremonyWithdraw"],
]
DISPATCH_RESERVATION_MODES = [[1, "InitiateNew"], [2, "RecoverReserved"]]
CEREMONY_REQUEST_MODES = [
    [1, "Initiate"],
    [2, "RecoverReserved"],
    [3, "ResolveResult"],
    [4, "Withdraw"],
]
ACTION_RESULT_OUTCOMES = [
    [1, "committed"],
    [2, "no_op"],
    [3, "rejected"],
    [4, "stale"],
    [5, "conflict"],
    [6, "unavailable"],
    [7, "in_doubt"],
]

DAG_EDGES = [
    [1, 2, 1],
    [2, 5, 1],
    [3, 5, 2],
    [4, 5, 3],
    [5, 5, 4],
    [6, 6, 1],
    [7, 6, 2],
    [8, 6, 3],
    [9, 6, 4],
    [10, 6, 5],
    [11, 7, 1],
    [12, 7, 2],
    [13, 7, 3],
    [14, 7, 5],
    [15, 7, 6],
    [16, 8, 1],
    [17, 8, 2],
    [18, 8, 4],
    [19, 8, 5],
    [20, 8, 6],
    [21, 9, 1],
    [22, 9, 2],
    [23, 9, 3],
    [24, 9, 4],
    [25, 9, 5],
    [26, 9, 6],
    [27, 9, 7],
    [28, 9, 8],
]

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
CMA_OBSERVATION_TAGS = {row[1] for row in CMA_EDGES}

ACTION_PHASES = {
    "OrdinaryGeneric": {
        "reserve": "OriginateEffectIntent",
        "outcome": "RecordDispatchOutcome",
        "reconcile": "ReconcileEffectIntent",
        "withdraw": "WithdrawEffectIntent",
    },
    "CoordinationDelivery": {
        "reserve": "OriginateCoordinationDelivery",
        "outcome": "RecordDispatchOutcome",
        "reconcile": "ReconcileEffectIntent",
        "withdraw": "WithdrawEffectIntent",
    },
    "BootstrapInteraction": {
        "reserve": "ReserveBootstrapMandateInteractionEffect",
        "outcome": "PublishBootstrapMandateInteractionOutcome",
        "reconcile": "ReconcileBootstrapMandateInteractionEffect",
        "withdraw": "WithdrawBootstrapMandateInteractionEffect",
    },
    "ContinuityMaintenance": {
        "reserve": "ReserveContinuityMaintenanceEffect",
        "outcome": "PublishContinuityMaintenanceEffectOutcome",
        "reconcile": "ReconcileContinuityMaintenanceEffect",
        "withdraw": "WithdrawContinuityMaintenanceEffect",
    },
}
ROUTE_CONTEXT = {"ActiveStore": 1, "NoStore": 2, "PreStore": 3}
ROUTE_BASIS = {"Ordinary": 1, "BootstrapG0": 2, "ContinuityMaintenance": 3, "CeremonyExternal": 4}

LIFECYCLE = {
    "CreateDraftWork": [1, ["absent"], "draft"],
    "PublishInitialContract": [1, ["draft"], "ready"],
    "AcquireStepExecution": [1, ["ready"], "active"],
    "SubmitWorkCompletion": [1, ["active"], "awaiting_acceptance"],
    "CompleteWork": [1, ["awaiting_acceptance"], "completed"],
    "RejectWorkCompletion": [1, ["awaiting_acceptance"], "active"],
    "ReturnWorkForRepair": [1, ["awaiting_acceptance"], "active"],
    "CancelWork": [1, ["draft", "ready", "active", "awaiting_acceptance"], "cancelled"],
    "AbsorbWork": [1, ["draft", "ready", "active", "awaiting_acceptance"], "superseded"],
    "SubmitStep": [2, ["open"], "submitted"],
    "SatisfyStep": [2, ["submitted"], "satisfied"],
    "RejectStepSubmission": [2, ["submitted"], "open"],
    "RecoverStepSubmission": [2, ["submitted"], "open"],
}
RESERVE_ACTIONS = {
    "OriginateEffectIntent",
    "OriginateCoordinationDelivery",
    "ReserveBootstrapMandateInteractionEffect",
    "ReserveContinuityMaintenanceEffect",
}
WITHDRAW_ACTIONS = {
    "WithdrawEffectIntent": 1,
    "WithdrawBootstrapMandateInteractionEffect": 2,
    "WithdrawContinuityMaintenanceEffect": 3,
}


def b32(value: str) -> dict[str, str]:
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ValueError(f"not a lowercase SHA-256 digest: {value}")
    return {"bytes": value}


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def identity(value: object) -> tuple[str, str, int]:
    raw = encode(value)
    return sha256_bytes(raw), raw.hex(), len(raw)


def optional(value: object | None) -> list[object]:
    return [0] if value is None else [1, value]


def ordered(value: object) -> list[object]:
    return [7, value]


def tuple_of(*values: object) -> list[object]:
    return [8, list(values)]


def enum(enum_id: str, rows: list[list[object]]) -> list[object]:
    return [9, enum_id, rows]


def length(minimum: int, maximum: int) -> list[list[int]]:
    return [[2, minimum, maximum]]


def canonical_set(minimum: int, maximum: int, tuple_index: int = 0) -> list[list[object]]:
    return [[3, [[2, tuple_index]], minimum, maximum]]


def schema_record(name: str, fields: list[tuple[str, object, list[list[object]]]]) -> dict[str, object]:
    value = [
        name,
        1,
        [[position, field_name, type_expression, constraints] for position, (field_name, type_expression, constraints) in enumerate(fields, 1)],
        [],
    ]
    envelope = ["maestro.vnext.schema.v1", value]
    schema_id, cbor_hex, byte_length = identity(envelope)
    return {
        "value": value,
        "schema_id": schema_id,
        "identity_envelope": envelope,
        "cbor_hex": cbor_hex,
        "byte_length": byte_length,
    }


def descriptor(domain: str, schema_id: str, value: list[object]) -> dict[str, object]:
    envelope = [domain, b32(schema_id), value]
    descriptor_id, cbor_hex, byte_length = identity(envelope)
    return {
        "value": value,
        "descriptor_id": descriptor_id,
        "identity_envelope": envelope,
        "cbor_hex": cbor_hex,
        "byte_length": byte_length,
    }


def current_action_families(nominal: dict[str, object]) -> list[dict[str, object]]:
    result = []
    for predecessor in copy.deepcopy(nominal["action_families"]):
        owner = predecessor["owner"]
        if owner == "Work":
            predecessor["leaves"].extend(["SubmitWorkCompletion", "RejectWorkCompletion", "ReturnWorkForRepair"])
            result.append(predecessor)
            result.append(
                {
                    "owner": "Step",
                    "leaves": ["SubmitStep", "SatisfyStep", "RejectStepSubmission", "RecoverStepSubmission"],
                }
            )
            continue
        if owner == "Execution":
            predecessor["leaves"] = [leaf for leaf in predecessor["leaves"] if leaf != "SubmitStep"]
            predecessor["leaves"].extend(
                ["WithdrawEffectIntent", "WithdrawBootstrapMandateInteractionEffect", "WithdrawContinuityMaintenanceEffect"]
            )
        result.append(predecessor)
    if [row["owner"] for row in result] != list(ACTION_FAMILY_COUNTS):
        raise ValueError("the effective efa0 Action-family order drifted")
    actual = {row["owner"]: len(row["leaves"]) for row in result}
    if actual != ACTION_FAMILY_COUNTS:
        raise ValueError(f"the effective efa0 Action-family counts drifted: {actual}")
    return result


def owner_ref(owner: str, owner_profiles_by_name: dict[str, dict[str, object]]) -> list[object]:
    profile = owner_profiles_by_name[owner]
    return [profile["tag"], b32(profile["descriptor_id"])]


def action_basis(name: str) -> int:
    if name in WITHDRAW_ACTIONS:
        return WITHDRAW_ACTIONS[name]
    if "BootstrapMandate" in name:
        return 2
    if "ContinuityMaintenance" in name:
        return 3
    return 1


def action_participants(name: str, owner: str) -> list[str]:
    special = {
        "PublishInitialContract": ["Work", "Contract"],
        "AcquireStepExecution": ["Work", "Step", "Execution"],
        "SubmitWorkCompletion": ["Work", "Evidence"],
        "SubmitStep": ["Step", "Execution", "Evidence"],
    }
    return special.get(name, [owner])


def produced_records(name: str) -> list[str]:
    special = {
        "SubmitWorkCompletion": ["WorkSubmissionV1", "SubmissionClaimSetV1", "ClaimV1", "ActionResultV1"],
        "SubmitStep": ["StepSubmissionV1", "SubmissionClaimSetV1", "ClaimV1", "ActionResultV1"],
        "CompleteWork": ["WorkLifecycleRevisionV1", "ActionResultV1"],
        "RejectWorkCompletion": ["WorkCompletionRejectionReceiptV1", "ActionResultV1"],
        "ReturnWorkForRepair": ["WorkRepairDirectiveReceiptV1", "ActionResultV1"],
        "SatisfyStep": ["StepSatisfactionReceiptV1", "ActionResultV1"],
        "RejectStepSubmission": ["StepSubmissionRejectionReceiptV1", "ActionResultV1"],
        "RecoverStepSubmission": ["StepSubmissionRecoveryReceiptV1", "ActionResultV1"],
        "WithdrawEffectIntent": ["EffectIntentControlRevisionV1", "AuthorizationReceiptV1", "ActionResultV1"],
        "WithdrawBootstrapMandateInteractionEffect": ["EffectIntentControlRevisionV1", "AuthorizationReceiptV1", "ActionResultV1"],
        "WithdrawContinuityMaintenanceEffect": ["EffectIntentControlRevisionV1", "AuthorizationReceiptV1", "ActionResultV1"],
    }
    return special.get(name, ["ActionResultV1"])


def json_bytes(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("ascii")


def write_json(path: Path, value: object) -> str:
    payload = json_bytes(value)
    path.write_bytes(payload)
    return sha256_bytes(payload)


def encoder_receipt(command: list[str], input_path: Path) -> dict[str, object]:
    result = subprocess.run([*command, str(input_path)], check=True, capture_output=True, text=True)
    lines = result.stdout.strip().splitlines()
    if len(lines) != 3:
        raise RuntimeError(f"encoder returned an invalid receipt: {result.stdout}")
    return {"cbor_hex": lines[0], "byte_length": int(lines[1]), "sha256": lines[2]}


def build(output: Path) -> dict[str, object]:
    output.mkdir(parents=True, exist_ok=True)
    nominal_path = EVIDENCE_ROOT / "e346-nominal-source.json"
    baseline_path = EVIDENCE_ROOT / "e346-semantic-baseline.json"
    predecessors_path = EVIDENCE_ROOT / "predecessors.json"
    nominal = json.loads(nominal_path.read_text(encoding="ascii"))
    baseline = json.loads(baseline_path.read_text(encoding="ascii"))
    predecessor_evidence = json.loads(predecessors_path.read_text(encoding="ascii"))
    if sha256_file(nominal_path) != "3142ff4334ddeb9b77c49786d29ff75de2ef6f023bb7942c827e1c54a84b69c2":
        raise ValueError("the frozen e346 nominal source is not byte-exact")
    if predecessor_evidence["artifact_count"] != 32 or predecessor_evidence["missing_count"] != 0:
        raise ValueError("the exact predecessor evidence set is incomplete")
    if baseline["predecessor_grammar_id"] != PREDECESSOR_GRAMMAR_ID:
        raise ValueError("the predecessor baseline does not reproduce e346")

    action_families = current_action_families(nominal)
    ceremony_source = nominal["ceremonies"]
    action_members_by_owner: dict[str, list[list[int]]] = {name: [] for name in OWNER_NAMES}
    action_symbol_sources = []
    global_tag = 0
    for family_tag, family in enumerate(action_families, 1):
        for family_local_tag, name in enumerate(family["leaves"], 1):
            global_tag += 1
            action_symbol_sources.append(
                {
                    "global_tag": global_tag,
                    "family_tag": family_tag,
                    "family_local_tag": family_local_tag,
                    "name": name,
                    "owner": family["owner"],
                }
            )
            action_members_by_owner[family["owner"]].append([global_tag, 1, global_tag])
    if global_tag != 145:
        raise ValueError(f"expected 145 current Action symbols, got {global_tag}")

    ceremony_members_by_owner: dict[str, list[list[int]]] = {name: [] for name in OWNER_NAMES}
    for tag, row in enumerate(ceremony_source, 1):
        ceremony_members_by_owner[row["owner"]].append([145 + tag, 2, tag])

    owner_profile_schema = schema_record(
        "maestro.vnext.catalog.owner-profile.v1",
        [
            ("tag", U64, NO_EXTRA),
            ("name", ASCII, NO_EXTRA),
            ("membership_mode", enum("OwnerProfileMembershipModeV1", [[1, "ExactGrammarMembers"], [2, "NoGrammarSymbolMembership"]]), NO_EXTRA),
            ("allowed_grammar_members", ordered(tuple_of(U64, U64, U64)), canonical_set(0, 156)),
        ],
    )
    owner_profiles = []
    owner_profiles_by_name = {}
    for tag, name in enumerate(OWNER_NAMES, 1):
        members = sorted(action_members_by_owner[name] + ceremony_members_by_owner[name])
        mode = 2 if name in ZERO_MEMBER_PROFILES else 1
        value = [tag, name, mode, members]
        row = descriptor(
            "maestro.vnext.catalog.owner-profile.descriptor.v1",
            owner_profile_schema["schema_id"],
            value,
        )
        row.update({"tag": tag, "name": name, "member_count": len(members), "membership_mode": mode})
        owner_profiles.append(row)
        owner_profiles_by_name[name] = row
    actual_combined = {row["name"]: row["member_count"] for row in owner_profiles}
    if actual_combined != COMBINED_PROFILE_COUNTS:
        raise ValueError(f"OwnerProfile partition drifted: {actual_combined}")
    owner_profile_rows = [[row["tag"], b32(row["descriptor_id"]), row["value"]] for row in owner_profiles]
    owner_profile_set_envelope = ["maestro.vnext.catalog.owner-profile-set.v1", owner_profile_rows]
    owner_profile_set_id, owner_profile_set_hex, owner_profile_set_length = identity(owner_profile_set_envelope)

    action_symbol_schema = schema_record(
        "maestro.vnext.catalog.action-leaf-symbol.v1",
        [
            ("global_tag", U64, NO_EXTRA),
            ("owner_ref", tuple_of(U64, BYTES32), NO_EXTRA),
            ("family_tag", U64, NO_EXTRA),
            ("family_local_tag", U64, NO_EXTRA),
            ("name", ASCII, NO_EXTRA),
            ("semantic_version", U64, NO_EXTRA),
        ],
    )
    action_symbols = []
    action_by_name = {}
    for source in action_symbol_sources:
        value = [
            source["global_tag"],
            owner_ref(source["owner"], owner_profiles_by_name),
            source["family_tag"],
            source["family_local_tag"],
            source["name"],
            1,
        ]
        row = descriptor(
            "maestro.vnext.catalog.action-leaf-symbol.descriptor.v1",
            action_symbol_schema["schema_id"],
            value,
        )
        row.update(source)
        action_symbols.append(row)
        action_by_name[source["name"]] = row

    ceremony_symbol_schema = schema_record(
        "maestro.vnext.catalog.ceremony-symbol.v1",
        [
            ("tag", U64, NO_EXTRA),
            ("owner_ref", tuple_of(U64, BYTES32), NO_EXTRA),
            ("name", ASCII, NO_EXTRA),
            ("semantic_version", U64, NO_EXTRA),
        ],
    )
    ceremony_symbols = []
    ceremony_by_name = {}
    for tag, source in enumerate(ceremony_source, 1):
        value = [tag, owner_ref(source["owner"], owner_profiles_by_name), source["name"], 1]
        row = descriptor(
            "maestro.vnext.catalog.ceremony-symbol.descriptor.v1",
            ceremony_symbol_schema["schema_id"],
            value,
        )
        row.update({"tag": tag, "name": source["name"], "owner": source["owner"]})
        ceremony_symbols.append(row)
        ceremony_by_name[source["name"]] = row

    route_entry_type = tuple_of(U64, U64, U64, U64, U64, U64, BYTES32)
    route_schema = schema_record(
        "maestro.vnext.catalog.effect-origin-route.v1",
        [
            ("origin_tag", U64, NO_EXTRA),
            ("origin_name", ASCII, NO_EXTRA),
            ("origin_source_owner", U64, NO_EXTRA),
            ("entries", ordered(route_entry_type), canonical_set(1, 28)),
        ],
    )
    effect_routes = []
    action_branch_count = 0
    ceremony_branch_count = 0
    route_count = 0
    action_basis_partition = {1: 0, 2: 0, 3: 0}
    for origin_tag, source in enumerate(nominal["effect_origin_routes"], 1):
        entries = []
        local_tag = 0
        if source["action_phase_set"] is not None:
            action_branch_count += 1
            basis = ROUTE_BASIS[source["action_basis"]]
            action_basis_partition[basis] += 1
            phase = ACTION_PHASES[source["action_phase_set"]]
            routed_actions = [
                (1, phase["reserve"]),
                (2, phase["reserve"]),
                (3, phase["outcome"]),
                (4, phase["reconcile"]),
                (5, phase["withdraw"]),
            ]
            for role_tag, action_name in routed_actions:
                local_tag += 1
                symbol = action_by_name[action_name]
                entries.append(
                    [local_tag, role_tag, ROUTE_CONTEXT["ActiveStore"], basis, 1, symbol["global_tag"], b32(symbol["descriptor_id"])]
                )
        for ceremony_name in source["ceremony_symbols"]:
            ceremony_branch_count += 1
            symbol = ceremony_by_name[ceremony_name]
            context = ROUTE_CONTEXT["NoStore"] if symbol["tag"] == 1 else ROUTE_CONTEXT["PreStore"]
            for role_tag in (6, 7, 8, 9):
                local_tag += 1
                entries.append(
                    [local_tag, role_tag, context, ROUTE_BASIS["CeremonyExternal"], 2, symbol["tag"], b32(symbol["descriptor_id"])]
                )
        value = [origin_tag, source["origin"], OWNER_NAMES.index(source["origin_source_owner"]) + 1, entries]
        row = descriptor(
            "maestro.vnext.catalog.effect-origin-route.descriptor.v1",
            route_schema["schema_id"],
            value,
        )
        row.update(
            {
                "origin_tag": origin_tag,
                "origin_name": source["origin"],
                "origin_source_owner": source["origin_source_owner"],
                "route_count": len(entries),
            }
        )
        effect_routes.append(row)
        route_count += len(entries)
    if (action_branch_count, ceremony_branch_count, route_count) != (19, 11, 139):
        raise ValueError("the effective route closure is not 19x5 plus 11x4")
    if action_basis_partition != {1: 12, 2: 2, 3: 5}:
        raise ValueError(f"the Action route basis partition drifted: {action_basis_partition}")

    grammar_schema = schema_record(
        "maestro.vnext.catalog.profile-grammar-value.v1",
        [
            ("grammar_version", U64, NO_EXTRA),
            ("route_roles", ordered(tuple_of(U64, ASCII)), canonical_set(9, 9)),
            ("dispatch_reservation_modes", ordered(tuple_of(U64, ASCII)), canonical_set(2, 2)),
            ("ceremony_request_modes", ordered(tuple_of(U64, ASCII)), canonical_set(4, 4)),
            ("owner_profiles", ordered(tuple_of(U64, BYTES32, [5, owner_profile_schema["value"][0], 1, b32(owner_profile_schema["schema_id"])])), canonical_set(21, 21)),
            ("action_symbols", ordered(tuple_of(U64, BYTES32, [5, action_symbol_schema["value"][0], 1, b32(action_symbol_schema["schema_id"])])), canonical_set(145, 145)),
            ("ceremony_symbols", ordered(tuple_of(U64, BYTES32, [5, ceremony_symbol_schema["value"][0], 1, b32(ceremony_symbol_schema["schema_id"])])), canonical_set(11, 11)),
            ("effect_origin_routes", ordered(tuple_of(U64, BYTES32, [5, route_schema["value"][0], 1, b32(route_schema["schema_id"])])), canonical_set(23, 23)),
            ("dependency_dag_edges", ordered(tuple_of(U64, U64, U64)), canonical_set(28, 28)),
            ("semantic_counts", ordered(tuple_of(ASCII, U64)), length(15, 15)),
            ("owner_profile_set_id", BYTES32, NO_EXTRA),
        ],
    )
    grammar_counts = [
        ["action_count", 145],
        ["action_family_count", 16],
        ["ceremony_count", 11],
        ["effect_origin_count", 23],
        ["effect_route_count", 139],
        ["observation_count", 43],
        ["owner_profile_count", 21],
        ["owner_relation_count", 444],
        ["repository_capacity_count", 6],
        ["installation_capacity_count", 6],
        ["repository_continuity_count", 35],
        ["installation_continuity_count", 30],
        ["execution_attempt_owner_count", 3],
        ["route_role_count", 9],
        ["grammar_symbol_count", 156],
    ]
    grammar_value = [
        1,
        ROUTE_ROLES,
        DISPATCH_RESERVATION_MODES,
        CEREMONY_REQUEST_MODES,
        owner_profile_rows,
        [[row["global_tag"], b32(row["descriptor_id"]), row["value"]] for row in action_symbols],
        [[row["tag"], b32(row["descriptor_id"]), row["value"]] for row in ceremony_symbols],
        [[row["origin_tag"], b32(row["descriptor_id"]), row["value"]] for row in effect_routes],
        DAG_EDGES,
        grammar_counts,
        b32(owner_profile_set_id),
    ]
    grammar_envelope = ["maestro.vnext.catalog-profile-grammar.v1", b32(grammar_schema["schema_id"]), grammar_value]
    grammar_id, grammar_hex, grammar_length = identity(grammar_envelope)
    if grammar_id == PREDECESSOR_GRAMMAR_ID:
        raise ValueError("the current efa0 GrammarId equals its predecessor")

    grammar_artifact = {
        "schema_version": "maestro.vnext.catalog-profile-grammar.artifact.v1",
        "status": STATUS,
        "publication_state": PUBLICATION_STATE,
        "effective_decision": [CURRENT_DECISION, CURRENT_DECISION_BODY_SHA256],
        "predecessor_disposition": "immutable_non_current_evidence_only",
        "predecessor_grammar_id": PREDECESSOR_GRAMMAR_ID,
        "schemas": {
            "owner_profile": owner_profile_schema,
            "action_symbol": action_symbol_schema,
            "ceremony_symbol": ceremony_symbol_schema,
            "effect_origin_route": route_schema,
            "grammar": grammar_schema,
        },
        "owner_profiles": owner_profiles,
        "owner_profile_set": {
            "owner_profile_set_id": owner_profile_set_id,
            "identity_envelope": owner_profile_set_envelope,
            "cbor_hex": owner_profile_set_hex,
            "byte_length": owner_profile_set_length,
        },
        "action_leaf_symbols": action_symbols,
        "ceremony_symbols": ceremony_symbols,
        "effect_origin_routes": effect_routes,
        "dependency_dag_edges": DAG_EDGES,
        "catalog_profile_grammar": {
            "value": grammar_value,
            "catalog_profile_grammar_id": grammar_id,
            "identity_envelope": grammar_envelope,
            "cbor_hex": grammar_hex,
            "byte_length": grammar_length,
        },
    }

    action_route_refs: dict[str, set[int]] = {name: set() for name in action_by_name}
    ceremony_route_refs: dict[str, set[int]] = {name: set() for name in ceremony_by_name}
    for route in effect_routes:
        for entry in route["value"][3]:
            if entry[4] == 1:
                name = action_symbols[entry[5] - 1]["name"]
                action_route_refs[name].add(route["origin_tag"])
            else:
                name = ceremony_symbols[entry[5] - 1]["name"]
                ceremony_route_refs[name].add(route["origin_tag"])

    observation_values = []
    for tag, name in enumerate(nominal["observations"], 1):
        if tag == 17:
            producers = [action_by_name["PublishBootstrapMandatePresentationObservation"]["global_tag"]]
        elif tag == 18:
            producers = [action_by_name["PublishBootstrapMandateResponseObservation"]["global_tag"]]
        elif tag in CMA_OBSERVATION_TAGS:
            producers = [action_by_name["PublishContinuityMaintenanceObservation"]["global_tag"]]
        else:
            producers = [action_by_name["PublishObservation"]["global_tag"]]
        source_routes = [edge[2] for edge in CMA_EDGES if edge[1] == tag]
        compatibility = [[edge[0], edge[2]] for edge in CMA_EDGES if edge[1] == tag]
        observation_values.append([tag, name, owner_ref("Evidence", owner_profiles_by_name), producers, source_routes, compatibility])

    effect_values = [
        [
            row["origin_tag"],
            row["origin_name"],
            owner_ref("Execution", owner_profiles_by_name),
            OWNER_NAMES.index(row["origin_source_owner"]) + 1,
            row["value"][3],
        ]
        for row in effect_routes
    ]

    repository_capacity_values = [
        [tag, name, owner_ref("Authority", owner_profiles_by_name), 1, [1, 1, U64_MAX], True]
        for tag, name in enumerate(nominal["capacity_profiles"]["Repository"], 1)
    ]
    installation_capacity_values = [
        [tag, name, owner_ref("Authority", owner_profiles_by_name), 2, [1, 1, U64_MAX], True]
        for tag, name in enumerate(nominal["capacity_profiles"]["Installation"], 1)
    ]

    ceremony_values = []
    for symbol in ceremony_symbols:
        context = 2 if symbol["tag"] == 1 else 3
        ceremony_values.append(
            [
                symbol["tag"],
                symbol["name"],
                owner_ref(symbol["owner"], owner_profiles_by_name),
                [1, 2, 3, 4],
                sorted(ceremony_route_refs[symbol["name"]]),
                context,
            ]
        )

    action_leaf_values = []
    action_spec_values = []
    for symbol in action_symbols:
        name = symbol["name"]
        owner = symbol["owner"]
        request_modes = optional([1, 2] if name in RESERVE_ACTIONS else None)
        lifecycle = optional(LIFECYCLE.get(name))
        leaf = [
            symbol["global_tag"],
            name,
            owner_ref(owner, owner_profiles_by_name),
            symbol["family_tag"],
            symbol["family_local_tag"],
            request_modes,
            action_basis(name),
            sorted(action_route_refs[name]),
            lifecycle,
        ]
        action_leaf_values.append(leaf)
        participants = sorted(OWNER_NAMES.index(participant) + 1 for participant in action_participants(name, owner))
        guards = ["expected_owner_revision", "fresh_nominal_authority", "same_key_same_meaning_replay"]
        if name in LIFECYCLE:
            guards.extend(["exact_source_state", "terminal_non_reopen", "loser_zero_write_zero_spend"])
        if name in WITHDRAW_ACTIONS:
            guards.extend(["live_dispatch_none", "prepared_or_confirmed_not_applied", "no_live_attempt_run_or_io"])
        action_spec_values.append(
            [*leaf, participants, produced_records(name), ACTION_RESULT_OUTCOMES, guards]
        )

    continuity_closure_value = [
        [
            action_by_name[name]["global_tag"]
            for name in [
                "FirstHumanBindingEnrollment",
                "ReserveBootstrapMandateInteractionEffect",
                "WithdrawBootstrapMandateInteractionEffect",
                "PublishBootstrapMandateInteractionOutcome",
                "PublishBootstrapMandatePresentationObservation",
                "PublishBootstrapMandateResponseObservation",
                "ReconcileBootstrapMandateInteractionEffect",
                "IssueBootstrapMandate",
            ]
        ],
        [
            [purpose, action_by_name["WithdrawContinuityMaintenanceEffect"]["global_tag"]]
            for purpose in range(1, 6)
        ],
        ["no_refill", "no_refund", "no_sixth_purpose", "no_new_capacity_kind"],
    ]
    continuity_closure_id, _, _ = identity(["maestro.vnext.catalog.h3-continuity-closure.v1", continuity_closure_value])
    repository_continuity_values = [
        [row["tag"], row["name"], owner_ref(row["owner"], owner_profiles_by_name), 1, b32(continuity_closure_id)]
        for row in baseline["repository_continuity"]
    ]
    installation_continuity_values = [
        [row["tag"], row["name"], owner_ref(row["owner"], owner_profiles_by_name), 2, b32(continuity_closure_id)]
        for row in baseline["installation_continuity"]
    ]

    catalog_values = {
        1: observation_values,
        2: effect_values,
        3: repository_capacity_values,
        4: installation_capacity_values,
        5: ceremony_values,
        6: action_leaf_values,
        7: repository_continuity_values,
        8: installation_continuity_values,
        9: action_spec_values,
    }

    owner_relation_total = 0
    manifests_by_tag: dict[int, dict[str, object]] = {}
    generated_artifacts = []
    identity_inputs = [
        owner_profile_schema["identity_envelope"],
        action_symbol_schema["identity_envelope"],
        ceremony_symbol_schema["identity_envelope"],
        route_schema["identity_envelope"],
        grammar_schema["identity_envelope"],
        *[row["identity_envelope"] for row in owner_profiles],
        owner_profile_set_envelope,
        *[row["identity_envelope"] for row in action_symbols],
        *[row["identity_envelope"] for row in ceremony_symbols],
        *[row["identity_envelope"] for row in effect_routes],
        grammar_envelope,
    ]

    for catalog_tag, slug, type_name, descriptor_domain, manifest_domain, dependency_tags in CATALOGS:
        values = catalog_values[catalog_tag]
        if [row[0] for row in values] != list(range(1, len(values) + 1)):
            raise ValueError(f"{slug} tags are not dense")
        relation_rows = [[row[0], row[2][0], row[2][1]] for row in values]
        relation_envelope = [f"maestro.vnext.catalog.primary-owner-relation.{catalog_tag}.v1", relation_rows]
        relation_id, relation_hex, relation_length = identity(relation_envelope)
        owner_relation_total += len(relation_rows)

        if catalog_tag == 1:
            descriptor_fields = [
                ("tag", U64, NO_EXTRA), ("name", ASCII, NO_EXTRA), ("primary_owner", tuple_of(U64, BYTES32), NO_EXTRA),
                ("producer_actions", ordered(U64), length(1, 4)), ("source_routes", ordered(U64), length(0, 2)),
                ("cma_compatibility", ordered(tuple_of(U64, U64)), length(0, 2)),
            ]
        elif catalog_tag == 2:
            descriptor_fields = [
                ("tag", U64, NO_EXTRA), ("name", ASCII, NO_EXTRA), ("primary_owner", tuple_of(U64, BYTES32), NO_EXTRA),
                ("origin_source_owner", U64, NO_EXTRA), ("routes", ordered(route_entry_type), canonical_set(4, 20)),
            ]
        elif catalog_tag in {3, 4}:
            descriptor_fields = [
                ("tag", U64, NO_EXTRA), ("name", ASCII, NO_EXTRA), ("primary_owner", tuple_of(U64, BYTES32), NO_EXTRA),
                ("domain", U64, NO_EXTRA), ("incoming_capacity_maximum", tuple_of(U64, U64, U64), NO_EXTRA),
                ("refill_forbidden", BOOL, NO_EXTRA),
            ]
        elif catalog_tag == 5:
            descriptor_fields = [
                ("tag", U64, NO_EXTRA), ("name", ASCII, NO_EXTRA), ("primary_owner", tuple_of(U64, BYTES32), NO_EXTRA),
                ("request_modes", ordered(U64), length(4, 4)), ("effect_origins", ordered(U64), length(1, 4)),
                ("context", U64, NO_EXTRA),
            ]
        elif catalog_tag == 6:
            descriptor_fields = [
                ("tag", U64, NO_EXTRA), ("name", ASCII, NO_EXTRA), ("primary_owner", tuple_of(U64, BYTES32), NO_EXTRA),
                ("family_tag", U64, NO_EXTRA), ("family_local_tag", U64, NO_EXTRA),
                ("dispatch_reservation_modes", [6, ordered(U64)], NO_EXTRA), ("authority_basis", U64, NO_EXTRA),
                ("effect_origins", ordered(U64), length(0, 23)),
                ("lifecycle_transition", [6, tuple_of(U64, ordered(ASCII), ASCII)], NO_EXTRA),
            ]
        elif catalog_tag in {7, 8}:
            descriptor_fields = [
                ("tag", U64, NO_EXTRA), ("name", ASCII, NO_EXTRA), ("primary_owner", tuple_of(U64, BYTES32), NO_EXTRA),
                ("domain", U64, NO_EXTRA), ("h3_continuity_closure_id", BYTES32, NO_EXTRA),
            ]
        else:
            descriptor_fields = [
                ("tag", U64, NO_EXTRA), ("name", ASCII, NO_EXTRA), ("primary_owner", tuple_of(U64, BYTES32), NO_EXTRA),
                ("family_tag", U64, NO_EXTRA), ("family_local_tag", U64, NO_EXTRA),
                ("dispatch_reservation_modes", [6, ordered(U64)], NO_EXTRA), ("authority_basis", U64, NO_EXTRA),
                ("effect_origins", ordered(U64), length(0, 23)),
                ("lifecycle_transition", [6, tuple_of(U64, ordered(ASCII), ASCII)], NO_EXTRA),
                ("atomic_participants", ordered(U64), length(1, 3)), ("produced_records", ordered(ASCII), length(1, 4)),
                ("result_outcomes", ordered(tuple_of(U64, ASCII)), canonical_set(7, 7)), ("guards", ordered(ASCII), length(3, 8)),
            ]
        descriptor_schema = schema_record(f"maestro.vnext.catalog.{slug}.descriptor-value.v1", descriptor_fields)
        header_schema = schema_record(
            f"maestro.vnext.catalog.{slug}.manifest-header.v1",
            [
                ("catalog_tag", U64, NO_EXTRA), ("catalog_version", U64, NO_EXTRA), ("canonicalization_version", U64, NO_EXTRA),
                ("grammar_id", BYTES32, NO_EXTRA), ("dependencies", ordered(tuple_of(U64, BYTES32)), canonical_set(len(dependency_tags), len(dependency_tags))),
                ("primary_owner_relation_id", BYTES32, NO_EXTRA), ("row_count", U64, NO_EXTRA), ("finite_row_maximum", U64, NO_EXTRA),
                ("owner_profile_set_id", BYTES32, NO_EXTRA), ("predecessor_evidence_sha256", BYTES32, NO_EXTRA),
                ("publication_state", enum("CatalogPublicationStateV1", [[1, "InactiveStage0Candidate"]]), NO_EXTRA),
            ],
        )
        manifest_schema = schema_record(
            f"maestro.vnext.catalog.{slug}.manifest-value.v1",
            [
                ("header", [5, header_schema["value"][0], 1, b32(header_schema["schema_id"])], NO_EXTRA),
                ("rows", ordered(tuple_of(U64, BYTES32, [5, descriptor_schema["value"][0], 1, b32(descriptor_schema["schema_id"])])), canonical_set(len(values), len(values))),
            ],
        )
        descriptor_rows = []
        for value in values:
            row = descriptor(descriptor_domain, descriptor_schema["schema_id"], value)
            descriptor_rows.append(row)
            identity_inputs.append(row["identity_envelope"])
        rows = [[row["value"][0], b32(row["descriptor_id"]), row["value"]] for row in descriptor_rows]
        dependencies = [[tag, b32(manifests_by_tag[tag]["manifest_id"])] for tag in dependency_tags]
        header = [
            catalog_tag,
            1,
            1,
            b32(grammar_id),
            dependencies,
            b32(relation_id),
            len(values),
            len(values),
            b32(owner_profile_set_id),
            b32(sha256_file(predecessors_path)),
            1,
        ]
        manifest_envelope = [
            manifest_domain,
            b32(manifest_schema["schema_id"]),
            b32(descriptor_schema["schema_id"]),
            header,
            rows,
        ]
        manifest_id, manifest_hex, manifest_length = identity(manifest_envelope)
        artifact = {
            "schema_version": "maestro.vnext.catalog.literal.v1",
            "status": STATUS,
            "publication_state": PUBLICATION_STATE,
            "catalog_tag": catalog_tag,
            "catalog_slug": slug,
            "catalog_type": type_name,
            "grammar_id": grammar_id,
            "predecessor_disposition": "immutable_non_current_evidence_only",
            "schemas": {"descriptor": descriptor_schema, "header": header_schema, "manifest": manifest_schema},
            "primary_owner_relation": {
                "rows": relation_rows,
                "relation_id": relation_id,
                "identity_envelope": relation_envelope,
                "cbor_hex": relation_hex,
                "byte_length": relation_length,
            },
            "descriptors": descriptor_rows,
            "manifest_header": header,
            "manifest_rows": rows,
            "manifest_identity_envelope": manifest_envelope,
            "manifest_id": manifest_id,
            "cbor_hex": manifest_hex,
            "byte_length": manifest_length,
        }
        if catalog_tag == 1:
            artifact["semantic_proof"] = {"cma_positive_count": 10, "cma_negative_count": 205}
        elif catalog_tag == 2:
            artifact["semantic_proof"] = {
                "action_branches": action_branch_count,
                "ceremony_branches": ceremony_branch_count,
                "route_count": route_count,
                "action_basis_partition": {"Ordinary": 12, "BootstrapG0": 2, "ContinuityMaintenance": 5},
            }
        elif catalog_tag in {7, 8}:
            artifact["semantic_proof"] = {
                "h3_continuity_closure_id": continuity_closure_id,
                "closure_value": continuity_closure_value,
            }
        manifests_by_tag[catalog_tag] = artifact
        generated_artifacts.append(artifact)
        identity_inputs.extend(
            [
                descriptor_schema["identity_envelope"],
                header_schema["identity_envelope"],
                manifest_schema["identity_envelope"],
                relation_envelope,
                manifest_envelope,
            ]
        )

    if owner_relation_total != 444:
        raise ValueError(f"expected 444 owner-relation rows, got {owner_relation_total}")

    artifact_files = []
    grammar_name = "catalog-profile-grammar-v1.json"
    grammar_sha = write_json(output / grammar_name, grammar_artifact)
    artifact_files.append(
        {"path": grammar_name, "sha256": grammar_sha, "identity": grammar_id, "kind": "grammar", "row_count": 156}
    )
    for artifact in generated_artifacts:
        name = f"catalog-{artifact['catalog_tag']:02d}-{artifact['catalog_slug']}.json"
        artifact_sha = write_json(output / name, artifact)
        artifact_files.append(
            {
                "path": name,
                "sha256": artifact_sha,
                "identity": artifact["manifest_id"],
                "kind": artifact["catalog_type"],
                "row_count": len(artifact["descriptors"]),
            }
        )

    encoder_input_name = "manifest-identity-input.json"
    encoder_input_path = output / encoder_input_name
    write_json(encoder_input_path, identity_inputs)
    python_receipt = encoder_receipt([sys.executable, str(PY_ENCODER)], encoder_input_path)
    ruby_receipt = encoder_receipt(["ruby", str(RB_ENCODER)], encoder_input_path)
    builder_encoded = encode(identity_inputs)
    builder_receipt = {
        "cbor_hex": builder_encoded.hex(),
        "byte_length": len(builder_encoded),
        "sha256": sha256_bytes(builder_encoded),
    }
    if python_receipt != ruby_receipt or python_receipt != builder_receipt:
        raise ValueError("the builder, independent Python encoder and independent Ruby encoder disagree")
    compact_python_receipt = {key: value for key, value in python_receipt.items() if key != "cbor_hex"}
    compact_ruby_receipt = {key: value for key, value in ruby_receipt.items() if key != "cbor_hex"}
    encoder_receipt_value = {
        "schema_version": "maestro.vnext.catalog.encoder-equality-receipt.v1",
        "status": "verified",
        "identity_input_count": len(identity_inputs),
        "encoder_input_sha256": sha256_file(encoder_input_path),
        "python_encoder_sha256": sha256_file(PY_ENCODER),
        "ruby_encoder_sha256": sha256_file(RB_ENCODER),
        "python": compact_python_receipt,
        "ruby": compact_ruby_receipt,
        "equality": "exact_bytes_length_and_sha256",
    }
    encoder_receipt_sha = write_json(output / "encoder-receipt.json", encoder_receipt_value)

    inventory = {
        "schema_version": "maestro.vnext.catalog.stage0-inventory.v1",
        "status": STATUS,
        "publication_state": PUBLICATION_STATE,
        "effective_decision": [CURRENT_DECISION, CURRENT_DECISION_BODY_SHA256],
        "grammar_id": grammar_id,
        "predecessor_grammar_id": PREDECESSOR_GRAMMAR_ID,
        "predecessor_evidence": {
            "artifact_count": 32,
            "missing_count": 0,
            "receipt_sha256": sha256_file(predecessors_path),
            "disposition": "immutable_non_current_predecessor_evidence",
        },
        "semantic_counts": {
            "observations": 43,
            "effect_origins": 23,
            "actions": 145,
            "action_specs": 145,
            "action_families": ACTION_FAMILY_COUNTS,
            "ceremonies": 11,
            "repository_capacity_kinds": 6,
            "installation_capacity_kinds": 6,
            "owner_profiles": 21,
            "grammar_symbols": 156,
            "effect_routes": 139,
            "route_roles": 9,
            "ceremony_request_modes": 4,
            "owner_relation_domains": [43, 23, 6, 6, 11, 145, 35, 30, 145],
            "owner_relation_rows": 444,
            "dependency_dag_edges": 28,
            "execution_attempt_owners": 3,
            "cma_observation_positive": 10,
            "cma_observation_negative": 205,
        },
        "artifacts": artifact_files,
        "encoder_input": {"path": encoder_input_name, "sha256": sha256_file(encoder_input_path)},
        "encoder_receipt": {"path": "encoder-receipt.json", "sha256": encoder_receipt_sha},
        "source_hashes": {
            "e346_nominal_source": sha256_file(nominal_path),
            "e346_semantic_baseline": sha256_file(baseline_path),
            "predecessor_evidence": sha256_file(predecessors_path),
            "builder": sha256_file(Path(__file__)),
        },
    }
    write_json(output / "inventory.json", inventory)

    if not VALIDATOR.is_file():
        raise FileNotFoundError("the independent semantic validator is required")
    validation = subprocess.run(
        [sys.executable, str(VALIDATOR), "--generated", str(output), "--mutants", "--json"],
        check=True,
        capture_output=True,
        text=True,
    )
    validation_receipt = json.loads(validation.stdout)
    write_json(output / "validation-receipt.json", validation_receipt)
    return inventory


def compare_generated(expected: Path, actual: Path) -> None:
    expected_files = sorted(path.name for path in expected.iterdir() if path.is_file())
    actual_files = sorted(path.name for path in actual.iterdir() if path.is_file())
    if expected_files != actual_files:
        raise ValueError(f"generated file set drifted: expected={expected_files}, actual={actual_files}")
    changed = [name for name in expected_files if (expected / name).read_bytes() != (actual / name).read_bytes()]
    if changed:
        raise ValueError(f"generated catalog bytes drifted: {changed}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.check:
        if not DEFAULT_OUTPUT.is_dir():
            raise FileNotFoundError(DEFAULT_OUTPUT)
        with tempfile.TemporaryDirectory(prefix="maestro-vnext-catalogs-") as directory:
            temporary_output = Path(directory)
            inventory = build(temporary_output)
            compare_generated(DEFAULT_OUTPUT, temporary_output)
    else:
        inventory = build(args.output.resolve())
    print(
        json.dumps(
            {
                "grammar_id": inventory["grammar_id"],
                "catalog_manifest_ids": [row["identity"] for row in inventory["artifacts"] if row["kind"] != "grammar"],
                "semantic_counts": inventory["semantic_counts"],
                "check": args.check,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
