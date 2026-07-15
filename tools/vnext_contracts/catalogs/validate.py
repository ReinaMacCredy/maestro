#!/usr/bin/env python3
"""Independent semantic and identity validator for efa0 Stage-0 catalog literals."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path


REPO = Path(__file__).resolve().parents[3]
EVIDENCE = REPO / "contracts/vnext/catalogs/evidence"
PREDECESSOR_GRAMMAR_ID = "2b428f8444253794cd0abb41b32da482cc0805359c2a37bf0cba90a70e3186e9"
U64_MAX = 0xFFFFFFFFFFFFFFFF

OWNER_NAMES = [
    "Work", "Step", "Contract", "Design", "Decision", "Execution", "Evidence", "GatePolicy", "Authority",
    "Coordination", "Orchestration", "Planning", "Projection", "Persistence", "SearchMaintenance", "Memory",
    "Intake", "Research", "Integration", "Distribution", "Installation",
]
ZERO_PROFILES = {"GatePolicy", "Orchestration", "Projection", "Integration"}
ACTION_FAMILY_COUNTS = {
    "Work": 7, "Step": 4, "Contract": 2, "Design": 4, "Decision": 5, "Execution": 16, "Evidence": 7,
    "Authority": 48, "Coordination": 9, "Planning": 4, "Persistence": 10, "Distribution": 13,
    "SearchMaintenance": 2, "Memory": 7, "Intake": 3, "Research": 4,
}
COMBINED_PROFILE_COUNTS = {
    "Work": 7, "Step": 4, "Contract": 2, "Design": 4, "Decision": 5, "Execution": 16, "Evidence": 7,
    "GatePolicy": 0, "Authority": 52, "Coordination": 9, "Orchestration": 0, "Planning": 4, "Projection": 0,
    "Persistence": 10, "SearchMaintenance": 2, "Memory": 7, "Intake": 3, "Research": 4, "Integration": 0,
    "Distribution": 14, "Installation": 6,
}
ROUTE_ROLES = [
    [1, "ActionReserve"], [2, "ActionRecoverReserved"], [3, "ActionOutcome"], [4, "ActionReconcile"],
    [5, "ActionWithdraw"], [6, "CeremonyInitiate"], [7, "CeremonyRecoverReserved"],
    [8, "CeremonyResolveResult"], [9, "CeremonyWithdraw"],
]
DISPATCH_MODES = [[1, "InitiateNew"], [2, "RecoverReserved"]]
CEREMONY_MODES = [[1, "Initiate"], [2, "RecoverReserved"], [3, "ResolveResult"], [4, "Withdraw"]]
RESULT_OUTCOMES = [
    [1, "committed"], [2, "no_op"], [3, "rejected"], [4, "stale"], [5, "conflict"],
    [6, "unavailable"], [7, "in_doubt"],
]
DAG_EDGES = [
    [1, 2, 1], [2, 5, 1], [3, 5, 2], [4, 5, 3], [5, 5, 4], [6, 6, 1], [7, 6, 2], [8, 6, 3],
    [9, 6, 4], [10, 6, 5], [11, 7, 1], [12, 7, 2], [13, 7, 3], [14, 7, 5], [15, 7, 6],
    [16, 8, 1], [17, 8, 2], [18, 8, 4], [19, 8, 5], [20, 8, 6], [21, 9, 1], [22, 9, 2],
    [23, 9, 3], [24, 9, 4], [25, 9, 5], [26, 9, 6], [27, 9, 7], [28, 9, 8],
]
CATALOG_SPECS = [
    (1, "observation", 43, []), (2, "effect", 23, [1]), (3, "repository-capacity", 6, []),
    (4, "installation-capacity", 6, []), (5, "ceremony", 11, [1, 2, 3, 4]),
    (6, "action-leaf", 145, [1, 2, 3, 4, 5]), (7, "repository-continuity", 35, [1, 2, 3, 5, 6]),
    (8, "installation-continuity", 30, [1, 2, 4, 5, 6]),
    (9, "action-spec", 145, [1, 2, 3, 4, 5, 6, 7, 8]),
]
CATALOG_FILES = {tag: f"catalog-{tag:02d}-{slug}.json" for tag, slug, _count, _deps in CATALOG_SPECS}
RESERVE_ACTIONS = {
    "OriginateEffectIntent", "OriginateCoordinationDelivery", "ReserveBootstrapMandateInteractionEffect",
    "ReserveContinuityMaintenanceEffect",
}
WITHDRAW_BY_BASIS = {1: "WithdrawEffectIntent", 2: "WithdrawBootstrapMandateInteractionEffect", 3: "WithdrawContinuityMaintenanceEffect"}
LIFECYCLE = {
    "CreateDraftWork": [1, ["absent"], "draft"], "PublishInitialContract": [1, ["draft"], "ready"],
    "AcquireStepExecution": [1, ["ready"], "active"],
    "SubmitWorkCompletion": [1, ["active"], "awaiting_acceptance"],
    "CompleteWork": [1, ["awaiting_acceptance"], "completed"],
    "RejectWorkCompletion": [1, ["awaiting_acceptance"], "active"],
    "ReturnWorkForRepair": [1, ["awaiting_acceptance"], "active"],
    "CancelWork": [1, ["draft", "ready", "active", "awaiting_acceptance"], "cancelled"],
    "AbsorbWork": [1, ["draft", "ready", "active", "awaiting_acceptance"], "superseded"],
    "SubmitStep": [2, ["open"], "submitted"], "SatisfyStep": [2, ["submitted"], "satisfied"],
    "RejectStepSubmission": [2, ["submitted"], "open"], "RecoverStepSubmission": [2, ["submitted"], "open"],
}


class ValidationError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def _head(major: int, value: int) -> bytes:
    if not 0 <= value <= U64_MAX:
        raise ValidationError("ManifestIdentityV1 admits only unsigned u64 integers and lengths")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def encode(value: object) -> bytes:
    if value is False:
        return b"\xf4"
    if value is True:
        return b"\xf5"
    if isinstance(value, int) and not isinstance(value, bool):
        return _head(0, value)
    if isinstance(value, str):
        try:
            raw = value.encode("ascii")
        except UnicodeEncodeError as error:
            raise ValidationError("ManifestIdentityV1 text must be ASCII") from error
        return _head(3, len(raw)) + raw
    if isinstance(value, list):
        return _head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        try:
            raw = bytes.fromhex(value["bytes"])
        except (TypeError, ValueError) as error:
            raise ValidationError("invalid raw-byte wrapper") from error
        return _head(2, len(raw)) + raw
    raise ValidationError(f"value is outside the ManifestIdentityV1 subset: {value!r}")


def digest(value: object) -> tuple[str, str, int]:
    raw = encode(value)
    return hashlib.sha256(raw).hexdigest(), raw.hex(), len(raw)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def expected_action_families(nominal: dict[str, object]) -> list[dict[str, object]]:
    result = []
    for original in copy.deepcopy(nominal["action_families"]):
        if original["owner"] == "Work":
            original["leaves"].extend(["SubmitWorkCompletion", "RejectWorkCompletion", "ReturnWorkForRepair"])
            result.append(original)
            result.append({"owner": "Step", "leaves": ["SubmitStep", "SatisfyStep", "RejectStepSubmission", "RecoverStepSubmission"]})
        elif original["owner"] == "Execution":
            original["leaves"] = [leaf for leaf in original["leaves"] if leaf != "SubmitStep"]
            original["leaves"].extend(["WithdrawEffectIntent", "WithdrawBootstrapMandateInteractionEffect", "WithdrawContinuityMaintenanceEffect"])
            result.append(original)
        else:
            result.append(original)
    return result


def load_suite(generated: Path) -> dict[str, object]:
    grammar = json.loads((generated / "catalog-profile-grammar-v1.json").read_text(encoding="ascii"))
    catalogs = {tag: json.loads((generated / filename).read_text(encoding="ascii")) for tag, filename in CATALOG_FILES.items()}
    return {
        "generated": generated,
        "grammar": grammar,
        "catalogs": catalogs,
        "inventory": json.loads((generated / "inventory.json").read_text(encoding="ascii")),
        "encoder_input": json.loads((generated / "manifest-identity-input.json").read_text(encoding="ascii")),
        "encoder_receipt": json.loads((generated / "encoder-receipt.json").read_text(encoding="ascii")),
        "nominal": json.loads((EVIDENCE / "e346-nominal-source.json").read_text(encoding="ascii")),
        "baseline": json.loads((EVIDENCE / "e346-semantic-baseline.json").read_text(encoding="ascii")),
        "predecessors": json.loads((EVIDENCE / "predecessors.json").read_text(encoding="ascii")),
    }


def validate_record(record: dict[str, object], identifier_field: str) -> None:
    identifier, cbor_hex, byte_length = digest(record["identity_envelope"])
    require(record[identifier_field] == identifier, f"{identifier_field} does not match its identity envelope")
    require(record["cbor_hex"] == cbor_hex, f"{identifier_field} canonical CBOR bytes drifted")
    require(record["byte_length"] == byte_length, f"{identifier_field} canonical CBOR length drifted")


def validate_schema(record: dict[str, object]) -> None:
    value = record["value"]
    require(record["identity_envelope"] == ["maestro.vnext.schema.v1", value], "SchemaId envelope drifted")
    require(len(value) == 4 and value[1] == 1 and value[3] == [], "SchemaDescriptorV1 shape/version drifted")
    fields = value[2]
    require([field[0] for field in fields] == list(range(1, len(fields) + 1)), "SchemaDescriptor fields are not dense")
    require(len({field[1] for field in fields}) == len(fields), "SchemaDescriptor field names are not unique")
    validate_record(record, "schema_id")


def validate_identities(suite: dict[str, object]) -> None:
    grammar = suite["grammar"]
    for schema in grammar["schemas"].values():
        validate_schema(schema)
    for row in grammar["owner_profiles"] + grammar["action_leaf_symbols"] + grammar["ceremony_symbols"] + grammar["effect_origin_routes"]:
        validate_record(row, "descriptor_id")
    grammar_record = grammar["catalog_profile_grammar"]
    grammar_id, grammar_hex, grammar_length = digest(grammar_record["identity_envelope"])
    require(grammar_record["catalog_profile_grammar_id"] == grammar_id, "GrammarId does not match its envelope")
    require(grammar_record["cbor_hex"] == grammar_hex and grammar_record["byte_length"] == grammar_length, "Grammar CBOR receipt drifted")

    for tag, catalog in suite["catalogs"].items():
        for schema in catalog["schemas"].values():
            validate_schema(schema)
        for row in catalog["descriptors"]:
            validate_record(row, "descriptor_id")
        relation = catalog["primary_owner_relation"]
        relation_id, relation_hex, relation_length = digest(relation["identity_envelope"])
        require(relation["relation_id"] == relation_id, f"catalog {tag} owner relation identity drifted")
        require(relation["cbor_hex"] == relation_hex and relation["byte_length"] == relation_length, f"catalog {tag} owner relation bytes drifted")
        manifest_id, manifest_hex, manifest_length = digest(catalog["manifest_identity_envelope"])
        require(catalog["manifest_id"] == manifest_id, f"catalog {tag} ManifestId drifted")
        require(catalog["cbor_hex"] == manifest_hex and catalog["byte_length"] == manifest_length, f"catalog {tag} manifest bytes drifted")


def validate_semantics(suite: dict[str, object]) -> dict[str, object]:
    grammar = suite["grammar"]
    catalogs = suite["catalogs"]
    nominal = suite["nominal"]
    baseline = suite["baseline"]
    predecessors = suite["predecessors"]
    inventory = suite["inventory"]

    require(grammar["status"] == "stage0_candidate_not_published", "grammar was admitted as current/active")
    require(grammar["publication_state"] == "inactive_candidate", "grammar publication state is not inactive")
    grammar_id = grammar["catalog_profile_grammar"]["catalog_profile_grammar_id"]
    require(grammar_id != PREDECESSOR_GRAMMAR_ID, "stale e346 GrammarId was admitted")
    require(grammar["predecessor_disposition"] == "immutable_non_current_evidence_only", "grammar predecessor is not evidence-only")

    profiles = grammar["owner_profiles"]
    require(len(profiles) == 21, "OwnerProfile count is not 21")
    require([row["tag"] for row in profiles] == list(range(1, 22)), "OwnerProfile tags are not dense")
    require([row["name"] for row in profiles] == OWNER_NAMES, "OwnerProfile names/order drifted")
    profile_by_tag = {row["tag"]: row for row in profiles}
    profile_by_name = {row["name"]: row for row in profiles}
    profile_members = []
    profile_member_keys = []
    for row in profiles:
        value = row["value"]
        require(value[0] == row["tag"] and value[1] == row["name"], "OwnerProfile metadata/value mismatch")
        require(value[3] == sorted(value[3]), "OwnerProfile members are not canonical")
        require(len(value[3]) == len({tuple(member) for member in value[3]}), "OwnerProfile contains duplicate members")
        require(all(len(member) == 3 and member[1] in {1, 2} for member in value[3]), "OwnerProfile member shape drifted")
        if row["name"] in ZERO_PROFILES:
            require(value[2] == 2 and value[3] == [], f"{row['name']} must use NoGrammarSymbolMembership")
        else:
            require(value[2] == 1 and value[3], f"{row['name']} must have exact grammar membership")
        profile_members.extend(tuple(member) for member in value[3])
        profile_member_keys.extend(member[0] for member in value[3])
    require(len(profile_members) == 156 and len(set(profile_members)) == 156, "OwnerProfiles are not a disjoint 156-symbol partition")
    require(sorted(profile_member_keys) == list(range(1, 157)), "OwnerProfile canonical-set keys are not the exact unique symbol partition")
    require({row["name"]: len(row["value"][3]) for row in profiles} == COMBINED_PROFILE_COUNTS, "OwnerProfile combined counts drifted")

    expected_families = expected_action_families(nominal)
    expected_actions = []
    for family_tag, family in enumerate(expected_families, 1):
        for local_tag, name in enumerate(family["leaves"], 1):
            expected_actions.append((family_tag, local_tag, name, family["owner"]))
    actions = grammar["action_leaf_symbols"]
    require(len(actions) == 145, "Action symbol count is not 145")
    require([row["global_tag"] for row in actions] == list(range(1, 146)), "Action global tags are not dense")
    actual_actions = [(row["family_tag"], row["family_local_tag"], row["name"], row["owner"]) for row in actions]
    require(actual_actions == expected_actions, "Action family/name/owner/tag literal equality failed")
    actual_family_counts = {owner: sum(row["owner"] == owner for row in actions) for owner in ACTION_FAMILY_COUNTS}
    require(actual_family_counts == ACTION_FAMILY_COUNTS, "sixteen Action-family counts drifted")
    require([row["name"] for row in actions if row["owner"] == "Step"] == ["SubmitStep", "SatisfyStep", "RejectStepSubmission", "RecoverStepSubmission"], "Step family is not exact")
    execution_names = {row["name"] for row in actions if row["owner"] == "Execution"}
    require("SubmitStep" not in execution_names and set(WITHDRAW_BY_BASIS.values()) <= execution_names, "Execution retained SubmitStep or lost a withdrawal leaf")
    for row in actions:
        owner_tag, profile_id = row["value"][1]
        require(profile_by_tag[owner_tag]["name"] == row["owner"], f"Action {row['name']} owner ref is wrong")
        require(profile_id["bytes"] == profile_by_tag[owner_tag]["descriptor_id"], f"Action {row['name']} profile identity is stale")
        require(
            [row["global_tag"], 1, row["global_tag"]] in profile_by_tag[owner_tag]["value"][3],
            f"Action {row['name']} is absent from its OwnerProfile",
        )

    ceremonies = grammar["ceremony_symbols"]
    require(len(ceremonies) == 11 and [row["tag"] for row in ceremonies] == list(range(1, 12)), "Ceremony symbols are not exact/dense")
    expected_ceremonies = [(index, row["name"], row["owner"]) for index, row in enumerate(nominal["ceremonies"], 1)]
    require([(row["tag"], row["name"], row["owner"]) for row in ceremonies] == expected_ceremonies, "Ceremony symbol/owner equality failed")
    for row in ceremonies:
        owner_tag, profile_id = row["value"][1]
        require(profile_by_tag[owner_tag]["name"] == row["owner"], f"Ceremony {row['name']} owner ref is wrong")
        require(profile_id["bytes"] == profile_by_tag[owner_tag]["descriptor_id"], f"Ceremony {row['name']} profile identity is stale")
        require(
            [145 + row["tag"], 2, row["tag"]] in profile_by_tag[owner_tag]["value"][3],
            f"Ceremony {row['name']} is absent from its OwnerProfile",
        )

    grammar_value = grammar["catalog_profile_grammar"]["value"]
    require(grammar_value[1] == ROUTE_ROLES, "EffectOriginRouteRoleV1 is not the exact nine-member enum")
    require(grammar_value[2] == DISPATCH_MODES, "DispatchReservationModeV1 drifted")
    require(grammar_value[3] == CEREMONY_MODES, "CeremonyRequestModeV1 is not the exact four-member enum")
    require(grammar["dependency_dag_edges"] == DAG_EDGES and grammar_value[8] == DAG_EDGES, "the unchanged 28-edge DAG drifted")

    routes = grammar["effect_origin_routes"]
    require(len(routes) == 23 and [row["origin_tag"] for row in routes] == list(range(1, 24)), "Effect Origin route descriptors are not exact/dense")
    action_branches = 0
    ceremony_branches = 0
    route_total = 0
    basis_partition = {1: 0, 2: 0, 3: 0}
    action_by_tag = {row["global_tag"]: row for row in actions}
    ceremony_by_tag = {row["tag"]: row for row in ceremonies}
    for route, expected_origin in zip(routes, nominal["effect_origin_routes"], strict=True):
        require(route["origin_name"] == expected_origin["origin"], "Effect Origin name/order drifted")
        entries = route["value"][3]
        require([entry[0] for entry in entries] == list(range(1, len(entries) + 1)), f"route {route['origin_name']} local tags are not dense")
        action_entries = [entry for entry in entries if entry[4] == 1]
        ceremony_entries = [entry for entry in entries if entry[4] == 2]
        if action_entries:
            action_branches += 1
            require([entry[1] for entry in action_entries] == [1, 2, 3, 4, 5], f"route {route['origin_name']} Action branch is not five-role complete")
            require(action_entries[0][5:] == action_entries[1][5:], f"route {route['origin_name']} RecoverReserved does not reuse reserve symbol")
            require(len({entry[2] for entry in action_entries}) == 1 and action_entries[0][2] == 1, "Action branch is not ActiveStore-only")
            basis = action_entries[0][3]
            require(all(entry[3] == basis for entry in action_entries), "Action branch basis is not fixed")
            basis_partition[basis] += 1
            withdrawal = action_by_tag[action_entries[4][5]]
            require(withdrawal["name"] == WITHDRAW_BY_BASIS[basis], f"route {route['origin_name']} selects wrong withdrawal leaf")
        grouped: dict[int, list[list[object]]] = {}
        for entry in ceremony_entries:
            grouped.setdefault(entry[5], []).append(entry)
        for ceremony_tag, group in grouped.items():
            ceremony_branches += 1
            require([entry[1] for entry in group] == [6, 7, 8, 9], f"Ceremony {ceremony_by_tag[ceremony_tag]['name']} branch is not four-role complete")
            require(len({entry[5] for entry in group}) == 1, "Ceremony branch changes symbol across modes")
            expected_context = 2 if ceremony_tag == 1 else 3
            require(all(entry[2] == expected_context and entry[3] == 4 for entry in group), "Ceremony branch context/basis drifted")
        for entry in entries:
            symbol = action_by_tag[entry[5]] if entry[4] == 1 else ceremony_by_tag[entry[5]]
            require(entry[6]["bytes"] == symbol["descriptor_id"], "route references a stale or wrong symbol identity")
        route_total += len(entries)
    require((action_branches, ceremony_branches, route_total) == (19, 11, 139), "route total is not 19x5 plus 11x4")
    require(basis_partition == {1: 12, 2: 2, 3: 5}, "Action branch basis partition is not 12/2/5")

    expected_relation_domains = [43, 23, 6, 6, 11, 145, 35, 30, 145]
    manifest_ids = {}
    relation_total = 0
    predecessor_manifest_ids = {
        path["path"].split("-sha256-")[1].split(".json")[0]
        for path in predecessors["artifacts"]
        if "-sha256-" in path["path"] and "catalog-profile-grammar" not in path["path"]
    }
    for (tag, slug, expected_count, dependencies), relation_count in zip(CATALOG_SPECS, expected_relation_domains, strict=True):
        catalog = catalogs[tag]
        require(catalog["catalog_tag"] == tag and catalog["catalog_slug"] == slug, f"catalog {tag} identity metadata drifted")
        require(catalog["status"] == "stage0_candidate_not_published" and catalog["publication_state"] == "inactive_candidate", f"catalog {tag} was admitted active")
        require(catalog["grammar_id"] == grammar_id and catalog["manifest_header"][3]["bytes"] == grammar_id, f"catalog {tag} does not bind the successor GrammarId")
        require(catalog["manifest_id"] not in predecessor_manifest_ids, f"catalog {tag} reused a predecessor ManifestId")
        require(len(catalog["descriptors"]) == expected_count == relation_count, f"catalog {tag} row/domain count drifted")
        require([row["value"][0] for row in catalog["descriptors"]] == list(range(1, expected_count + 1)), f"catalog {tag} descriptor tags are not dense")
        expected_relation = [[row["value"][0], row["value"][2][0], row["value"][2][1]] for row in catalog["descriptors"]]
        require(catalog["primary_owner_relation"]["rows"] == expected_relation, f"catalog {tag} owner relation is not descriptor-total")
        require(catalog["manifest_header"][5]["bytes"] == catalog["primary_owner_relation"]["relation_id"], f"catalog {tag} Header does not bind its owner relation")
        require(catalog["manifest_header"][6:8] == [expected_count, expected_count], f"catalog {tag} row count/maximum drifted")
        require(catalog["manifest_header"][10] == 1, f"catalog {tag} Header is not inactive Stage-0")
        expected_dependencies = [[dependency, {"bytes": manifest_ids[dependency]}] for dependency in dependencies]
        require(catalog["manifest_header"][4] == expected_dependencies, f"catalog {tag} dependency identities drifted")
        manifest_ids[tag] = catalog["manifest_id"]
        relation_total += len(expected_relation)
    require(relation_total == 444, "nine owner relations do not total 444 rows")

    observations = [row["value"] for row in catalogs[1]["descriptors"]]
    require([row[1] for row in observations] == nominal["observations"], "43 Observation names/order drifted")
    require(all(row[2][0] == profile_by_name["Evidence"]["tag"] for row in observations), "Observation primary owner is not Evidence")
    positive = sum(len(row[5]) for row in observations)
    require((positive, 5 * 43 - positive) == (10, 205), "CMA Observation compatibility is not 10/205")

    effects = [row["value"] for row in catalogs[2]["descriptors"]]
    require([row[1] for row in effects] == nominal["effect_origins"], "23 Effect Origin names/order drifted")
    require(all(row[2][0] == profile_by_name["Execution"]["tag"] for row in effects), "Effect Origin primary owner is not Execution")
    require([row[4] for row in effects] == [row["value"][3] for row in routes], "Effect catalog route closure differs from grammar")

    for tag, domain_name, domain_tag in [(3, "Repository", 1), (4, "Installation", 2)]:
        capacities = [row["value"] for row in catalogs[tag]["descriptors"]]
        require([row[1] for row in capacities] == nominal["capacity_profiles"][domain_name], f"{domain_name} capacity names drifted")
        require(all(row[2][0] == profile_by_name["Authority"]["tag"] for row in capacities), f"{domain_name} capacity owner is not Authority")
        require(all(row[3:] == [domain_tag, [1, 1, U64_MAX], True] for row in capacities), f"{domain_name} capacity maximum/refill law drifted")

    ceremony_values = [row["value"] for row in catalogs[5]["descriptors"]]
    require([row[1] for row in ceremony_values] == [row["name"] for row in nominal["ceremonies"]], "Ceremony descriptor names drifted")
    require(all(row[3] == [1, 2, 3, 4] for row in ceremony_values), "not every Ceremony has the four exact request modes")
    require(sum(row[5] == 2 for row in ceremony_values) == 1 and sum(row[5] == 3 for row in ceremony_values) == 10, "Ceremony context split is not 1 NoStore / 10 PreStore")

    action_leafs = [row["value"] for row in catalogs[6]["descriptors"]]
    require([row[1] for row in action_leafs] == [row["name"] for row in actions], "ActionLeaf names differ from grammar")
    for value, symbol in zip(action_leafs, actions, strict=True):
        require(value[2][0] == OWNER_NAMES.index(symbol["owner"]) + 1, f"ActionLeaf {symbol['name']} owner drifted")
        expected_modes = [1, [1, 2]] if symbol["name"] in RESERVE_ACTIONS else [0]
        require(value[5] == expected_modes, f"ActionLeaf {symbol['name']} reservation modes drifted or lifecycle mode was smuggled")
        expected_lifecycle = [1, LIFECYCLE[symbol["name"]]] if symbol["name"] in LIFECYCLE else [0]
        require(value[8] == expected_lifecycle, f"ActionLeaf {symbol['name']} lifecycle map drifted")
    submit_step = next(row for row in action_leafs if row[1] == "SubmitStep")
    require(submit_step[2][0] == profile_by_name["Step"]["tag"], "SubmitStep is not Step-owned")

    repo_continuity = [row["value"] for row in catalogs[7]["descriptors"]]
    installation_continuity = [row["value"] for row in catalogs[8]["descriptors"]]
    require([row[1] for row in repo_continuity] == [row["name"] for row in baseline["repository_continuity"]], "Repository continuity names drifted")
    require([row[1] for row in installation_continuity] == [row["name"] for row in baseline["installation_continuity"]], "Installation continuity names drifted")
    closure_ids = {row[4]["bytes"] for row in repo_continuity + installation_continuity}
    require(len(closure_ids) == 1, "continuity manifests do not bind one exact H3 closure")
    for tag in (7, 8):
        closure = catalogs[tag]["semantic_proof"]["closure_value"]
        require(len(closure[0]) == 8 and len(closure[1]) == 5, f"catalog {tag} lost Bootstrap/CMA withdrawal closure")
        require(closure[2] == ["no_refill", "no_refund", "no_sixth_purpose", "no_new_capacity_kind"], f"catalog {tag} withdrawal closure weakened")

    action_specs = [row["value"] for row in catalogs[9]["descriptors"]]
    require(len(action_specs) == 145, "ActionSpec count is not 145")
    require([row[:9] for row in action_specs] == action_leafs, "ActionSpec prefixes differ from ActionLeaf descriptors")
    require(all(row[11] == RESULT_OUTCOMES for row in action_specs), "ActionSpec seven-outcome contract drifted")
    submit_spec = next(row for row in action_specs if row[1] == "SubmitStep")
    expected_submit_participants = sorted([profile_by_name[name]["tag"] for name in ["Step", "Execution", "Evidence"]])
    require(submit_spec[9] == expected_submit_participants, "SubmitStep fixed atomic participants drifted")
    complete_work = next(row for row in action_specs if row[1] == "CompleteWork")
    require(complete_work[8] == [1, LIFECYCLE["CompleteWork"]], "CompleteWork is not acceptance-only")

    require(predecessors["artifact_count"] == 32 and predecessors["missing_count"] == 0, "predecessor evidence is not exact 32/0")
    require(len({row["path"] for row in predecessors["artifacts"]}) == 32, "predecessor evidence paths are duplicated")
    require(all(row["disposition"] == "immutable_non_current_predecessor_evidence" for row in predecessors["artifacts"]), "predecessor evidence was treated as current")
    require(all(row["hash_named_by_decisions"] for row in predecessors["artifacts"]), "a predecessor hash is not Decision-verified")

    counts = inventory["semantic_counts"]
    require(inventory["grammar_id"] == grammar_id and inventory["predecessor_grammar_id"] == PREDECESSOR_GRAMMAR_ID, "inventory grammar identity drifted")
    require(inventory["publication_state"] == "inactive_candidate", "inventory claims active publication")
    require(counts["observations"] == 43 and counts["effect_origins"] == 23, "inventory observation/effect counts drifted")
    require(counts["actions"] == 145 and counts["action_specs"] == 145 and counts["action_families"] == ACTION_FAMILY_COUNTS, "inventory Action counts drifted")
    require(counts["ceremonies"] == 11 and counts["repository_capacity_kinds"] == counts["installation_capacity_kinds"] == 6, "inventory Ceremony/capacity counts drifted")
    require(counts["owner_profiles"] == 21 and counts["grammar_symbols"] == 156, "inventory profile/symbol counts drifted")
    require(counts["effect_routes"] == 139 and counts["route_roles"] == 9 and counts["ceremony_request_modes"] == 4, "inventory route/mode counts drifted")
    require(counts["owner_relation_domains"] == expected_relation_domains and counts["owner_relation_rows"] == 444, "inventory owner relation counts drifted")
    require(counts["dependency_dag_edges"] == 28 and counts["execution_attempt_owners"] == 3, "inventory DAG/Attempt-owner counts drifted")

    return {
        "grammar_id": grammar_id,
        "catalog_manifest_ids": [manifest_ids[tag] for tag in range(1, 10)],
        "owner_relation_rows": relation_total,
        "route_count": route_total,
    }


def validate_files(suite: dict[str, object]) -> None:
    generated = suite["generated"]
    inventory = suite["inventory"]
    for row in inventory["artifacts"]:
        path = generated / row["path"]
        require(path.is_file() and sha256_file(path) == row["sha256"], f"inventory file hash drifted: {row['path']}")
        if row["kind"] == "grammar":
            require(row["identity"] == suite["grammar"]["catalog_profile_grammar"]["catalog_profile_grammar_id"], "inventory GrammarId drifted")
        else:
            matching = next(catalog for catalog in suite["catalogs"].values() if catalog["catalog_type"] == row["kind"])
            require(row["identity"] == matching["manifest_id"], f"inventory ManifestId drifted: {row['path']}")
    require(sha256_file(generated / "manifest-identity-input.json") == inventory["encoder_input"]["sha256"], "encoder input file hash drifted")
    require(sha256_file(generated / "encoder-receipt.json") == inventory["encoder_receipt"]["sha256"], "encoder receipt file hash drifted")
    receipt = suite["encoder_receipt"]
    own_sha, _own_hex, own_length = digest(suite["encoder_input"])
    expected = {"sha256": own_sha, "byte_length": own_length}
    require(receipt["python"] == expected and receipt["ruby"] == expected, "independent encoder receipts do not equal validator bytes")
    require(receipt["equality"] == "exact_bytes_length_and_sha256", "encoder equality receipt is not exact")


def mutants(suite: dict[str, object]) -> list[tuple[str, object]]:
    return [
        ("missing_action", lambda value: value["grammar"]["action_leaf_symbols"].pop()),
        ("duplicate_action_tag", lambda value: value["grammar"]["action_leaf_symbols"][-1].__setitem__("global_tag", 144)),
        ("submit_step_execution_owner", lambda value: next(row for row in value["grammar"]["action_leaf_symbols"] if row["name"] == "SubmitStep").__setitem__("owner", "Execution")),
        ("missing_step_profile", lambda value: value["grammar"]["owner_profiles"].pop(1)),
        ("two_owner_membership", lambda value: value["grammar"]["owner_profiles"][2]["value"][3].append([1, 1])),
        ("zero_profile_member", lambda value: next(row for row in value["grammar"]["owner_profiles"] if row["name"] == "GatePolicy")["value"][3].append([1, 1])),
        ("stale_grammar_current", lambda value: value["grammar"]["catalog_profile_grammar"].__setitem__("catalog_profile_grammar_id", PREDECESSOR_GRAMMAR_ID)),
        ("route_role_removed", lambda value: value["grammar"]["catalog_profile_grammar"]["value"][1].pop()),
        ("ceremony_mode_removed", lambda value: value["grammar"]["catalog_profile_grammar"]["value"][3].pop()),
        ("route_tuple_removed", lambda value: value["grammar"]["effect_origin_routes"][0]["value"][3].pop()),
        ("wrong_withdrawal_route", lambda value: value["grammar"]["effect_origin_routes"][0]["value"][3][4].__setitem__(5, 1)),
        ("dag_edge_removed", lambda value: value["grammar"].__setitem__("dependency_dag_edges", value["grammar"]["dependency_dag_edges"][:-1])),
        ("observation_removed", lambda value: value["catalogs"][1]["descriptors"].pop()),
        ("cma_edge_removed", lambda value: value["catalogs"][1]["descriptors"][28]["value"][5].pop()),
        ("capacity_unbounded_sentinel", lambda value: value["catalogs"][3]["descriptors"][0]["value"][4].__setitem__(0, 0)),
        ("ceremony_wrong_owner", lambda value: value["catalogs"][5]["descriptors"][0]["value"][2].__setitem__(0, 9)),
        ("action_leaf_removed", lambda value: value["catalogs"][6]["descriptors"].pop()),
        ("lifecycle_mode_smuggling", lambda value: next(row for row in value["catalogs"][6]["descriptors"] if row["value"][1] == "CompleteWork")["value"].__setitem__(5, [1, [1, 2]])),
        ("complete_work_wrong_edge", lambda value: next(row for row in value["catalogs"][6]["descriptors"] if row["value"][1] == "CompleteWork")["value"].__setitem__(8, [1, [1, ["active"], "completed"]])),
        ("continuity_closure_weakened", lambda value: value["catalogs"][7]["semantic_proof"]["closure_value"][1].pop()),
        ("action_spec_removed", lambda value: value["catalogs"][9]["descriptors"].pop()),
        ("submit_step_participant_removed", lambda value: next(row for row in value["catalogs"][9]["descriptors"] if row["value"][1] == "SubmitStep")["value"][9].pop()),
        ("owner_relation_row_removed", lambda value: value["catalogs"][9]["primary_owner_relation"]["rows"].pop()),
        ("predecessor_admitted_current", lambda value: value["predecessors"]["artifacts"][0].__setitem__("disposition", "current")),
        ("inventory_old_action_count", lambda value: value["inventory"]["semantic_counts"].__setitem__("actions", 139)),
    ]


def run_mutants(suite: dict[str, object]) -> list[str]:
    rejected = []
    for name, mutate in mutants(suite):
        candidate = copy.deepcopy(suite)
        mutate(candidate)
        try:
            validate_semantics(candidate)
        except (ValidationError, KeyError, IndexError, StopIteration, TypeError, ValueError):
            rejected.append(name)
        else:
            raise ValidationError(f"semantic mutant was admitted: {name}")
    return rejected


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--generated", required=True, type=Path)
    parser.add_argument("--mutants", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    suite = load_suite(args.generated.resolve())
    summary = validate_semantics(suite)
    validate_identities(suite)
    validate_files(suite)
    rejected = run_mutants(suite) if args.mutants else []
    receipt = {
        "schema_version": "maestro.vnext.catalog.semantic-validation-receipt.v1",
        "status": "verified",
        "validator_sha256": sha256_file(Path(__file__)),
        "grammar_id": summary["grammar_id"],
        "catalog_manifest_ids": summary["catalog_manifest_ids"],
        "owner_relation_rows": summary["owner_relation_rows"],
        "effect_route_count": summary["route_count"],
        "semantic_checks": 24,
        "mutants_rejected": len(rejected),
        "mutant_names": rejected,
    }
    print(json.dumps(receipt, indent=None if args.json else 2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
