#!/usr/bin/env python3
"""Independent semantic validator for the design-only CatalogProfileGrammarV1."""

from __future__ import annotations

import copy
import hashlib
import json
import sys
from pathlib import Path


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
    raise ValueError(f"unsupported deterministic-CBOR value: {value!r}")


def sha(value) -> str:
    return hashlib.sha256(encode(value)).hexdigest()


OWNER_TAGS = {
    "Work": 1,
    "Contract": 2,
    "Design": 3,
    "Decision": 4,
    "Execution": 5,
    "Evidence": 6,
    "GatePolicy": 7,
    "Authority": 8,
    "Coordination": 9,
    "Orchestration": 10,
    "Planning": 11,
    "Projection": 12,
    "Persistence": 13,
    "SearchMaintenance": 14,
    "Memory": 15,
    "Intake": 16,
    "Research": 17,
    "Integration": 18,
    "Distribution": 19,
    "Installation": 20,
}
SOURCE_OWNER_TAGS = {
    "Execution": 1,
    "Coordination": 2,
    "Authority": 3,
    "Installation": 4,
    "Distribution": 5,
    "Evidence": 6,
}
CONTEXT_TAGS = {"ActiveStore": 1, "NoStore": 2, "PreStore": 3}
BASIS_TAGS = {"Ordinary": 1, "BootstrapG0": 2, "ContinuityMaintenance": 3, "CeremonyExternal": 4}
ACTION_PHASE_SETS = {
    "OrdinaryGeneric": [(1, "OriginateEffectIntent"), (2, "RecordDispatchOutcome"), (3, "ReconcileEffectIntent")],
    "CoordinationDelivery": [(1, "OriginateCoordinationDelivery"), (2, "RecordDispatchOutcome"), (3, "ReconcileEffectIntent")],
    "BootstrapInteraction": [(1, "ReserveBootstrapMandateInteractionEffect"), (2, "PublishBootstrapMandateInteractionOutcome"), (3, "ReconcileBootstrapMandateInteractionEffect")],
    "ContinuityMaintenance": [(1, "ReserveContinuityMaintenanceEffect"), (2, "PublishContinuityMaintenanceEffectOutcome"), (3, "ReconcileContinuityMaintenanceEffect")],
}
EXPECTED_DAG = [
    [1, 2, 1],
    [2, 5, 1], [3, 5, 2], [4, 5, 3], [5, 5, 4],
    [6, 6, 1], [7, 6, 2], [8, 6, 3], [9, 6, 4], [10, 6, 5],
    [11, 7, 1], [12, 7, 2], [13, 7, 3], [14, 7, 5], [15, 7, 6],
    [16, 8, 1], [17, 8, 2], [18, 8, 4], [19, 8, 5], [20, 8, 6],
    [21, 9, 1], [22, 9, 2], [23, 9, 3], [24, 9, 4], [25, 9, 5], [26, 9, 6], [27, 9, 7], [28, 9, 8],
]


def check(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def verify_identity_record(row: dict, id_field: str, label: str) -> None:
    raw = encode(row["identity_envelope"])
    identifier = hashlib.sha256(raw).hexdigest()
    check(row[id_field] == identifier, f"{label} identity")
    check(row["sha256"] == identifier, f"{label} sha256")
    check(row["cbor_hex"] == raw.hex(), f"{label} cbor")
    check(row["byte_length"] == len(raw), f"{label} byte length")


def validate_type_expr(expr, schemas: dict, refs: set[str]) -> None:
    check(isinstance(expr, list) and expr, "type expression shape")
    tag = expr[0]
    if tag in (1, 2, 3):
        check(len(expr) == 1, "primitive type arity")
    elif tag == 4:
        check(len(expr) == 2 and isinstance(expr[1], int) and expr[1] > 0, "bytes type")
    elif tag == 5:
        check(len(expr) == 4 and expr[2] == 1, "schema ref shape")
        name = expr[1]
        check(name in schemas, f"missing schema ref {name}")
        check(expr[3] == {"bytes": schemas[name]["schema_id"]}, f"schema ref identity {name}")
        refs.add(name)
    elif tag in (6, 7):
        check(len(expr) == 2, "container type arity")
        validate_type_expr(expr[1], schemas, refs)
    elif tag == 8:
        check(len(expr) == 2 and isinstance(expr[1], list) and expr[1], "tuple type")
        for child in expr[1]:
            validate_type_expr(child, schemas, refs)
    elif tag == 9:
        check(len(expr) == 3 and isinstance(expr[1], str) and isinstance(expr[2], list) and expr[2], "enum type")
        tags = [row[0] for row in expr[2]]
        names = [row[1] for row in expr[2]]
        check(tags == list(range(1, len(tags) + 1)), "enum dense tags")
        check(len(names) == len(set(names)) and all(isinstance(name, str) and name for name in names), "enum names")
    else:
        raise ValueError(f"unknown type expression {tag}")


def resolve_type_path(type_expr, path, schemas: dict):
    check(isinstance(path, list) and path, "field path")
    current = type_expr
    for index, step in enumerate(path):
        check(isinstance(step, list) and len(step) == 2, "path step shape")
        check(current[0] != 6, "optional path traversal")
        if step[0] == 1:
            check(current[0] == 5 and isinstance(step[1], int) and step[1] > 0, "schema field path step")
            descriptor = schemas[current[1]]["value"]
            fields = {field[0]: field for field in descriptor[2]}
            check(step[1] in fields, "schema field path target")
            current = fields[step[1]][2]
        elif step[0] == 2:
            check(current[0] == 8 and isinstance(step[1], int) and 0 <= step[1] < len(current[1]), "tuple path step")
            current = current[1][step[1]]
        else:
            raise ValueError("unknown path step")
        if index < len(path) - 1:
            check(current[0] != 6, "optional path traversal")
    return current


def resolve_schema_path(schema_name: str, path, schemas: dict):
    check(isinstance(path, list) and path, "schema field path")
    first = path[0]
    check(isinstance(first, list) and len(first) == 2 and first[0] == 1 and isinstance(first[1], int) and first[1] > 0, "schema root path step")
    fields = {field[0]: field for field in schemas[schema_name]["value"][2]}
    check(first[1] in fields, "schema root field target")
    current = fields[first[1]][2]
    if len(path) == 1:
        return current
    check(current[0] != 6, "optional path traversal")
    return resolve_type_path(current, path[1:], schemas)


def validate_schema_descriptor(name: str, row: dict, schemas: dict) -> set[str]:
    value = row["value"]
    check(value[0] == name and value[1] == 1 and len(value) == 4, f"schema descriptor {name}")
    fields = value[2]
    positions = [field[0] for field in fields]
    names = [field[1] for field in fields]
    check(positions == list(range(1, len(fields) + 1)), f"schema field positions {name}")
    check(len(names) == len(set(names)), f"schema field names {name}")
    refs: set[str] = set()
    for field in fields:
        check(len(field) == 4 and isinstance(field[1], str) and field[1], f"schema field shape {name}")
        validate_type_expr(field[2], schemas, refs)
        constraints = field[3]
        check(isinstance(constraints, list) and constraints, f"constraints present {name}")
        check(len({encode(item) for item in constraints}) == len(constraints), f"constraint duplicates {name}")
        check([encode(item) for item in constraints] == sorted(encode(item) for item in constraints), f"constraint order {name}")
        for constraint in constraints:
            check(isinstance(constraint, list) and constraint, f"constraint shape {name}")
            tag = constraint[0]
            if tag == 1:
                check(constraint == [1] and len(constraints) == 1, f"no-extra constraint {name}")
            elif tag == 2:
                check(len(constraint) == 3 and 0 <= constraint[1] <= constraint[2], f"length constraint {name}")
            elif tag == 3:
                check(len(constraint) == 4 and constraint[1] and 0 <= constraint[2] <= constraint[3], f"canonical-set constraint {name}")
                check(field[2][0] == 7, f"canonical-set list type {name}")
                terminal = resolve_type_path(field[2][1], constraint[1], schemas)
                check(terminal == [1], f"canonical-set terminal u64 {name}")
            elif tag == 4:
                check(len(constraint) == 3 and 0 <= constraint[1] <= constraint[2], f"range constraint {name}")
            elif tag == 5:
                check(len(constraint) == 2 and constraint[1], f"equality constraint {name}")
                terminal = resolve_schema_path(name, constraint[1], schemas)
                check(terminal == field[2], f"equality path type {name}")
            else:
                raise ValueError(f"unknown constraint {tag}")
    cross = value[3]
    check(isinstance(cross, list), f"cross constraints {name}")
    check(len({encode(item) for item in cross}) == len(cross), f"cross constraint duplicates {name}")
    check([encode(item) for item in cross] == sorted(encode(item) for item in cross), f"cross constraint order {name}")
    for constraint in cross:
        check(isinstance(constraint, list) and constraint, f"cross constraint shape {name}")
        if constraint[0] == 1:
            check(len(constraint) == 3, f"cross equality shape {name}")
            left = resolve_schema_path(name, constraint[1], schemas)
            right = resolve_schema_path(name, constraint[2], schemas)
            check(left == right or (left[0] in (1, 9) and right[0] in (1, 9)), f"cross equality types {name}")
        elif constraint[0] in (2, 3):
            check(len(constraint) == 2 and isinstance(constraint[1], list) and constraint[1], f"cross presence shape {name}")
            paths = constraint[1]
            check(len({encode(path) for path in paths}) == len(paths), f"cross presence path duplicates {name}")
            check([encode(path) for path in paths] == sorted(encode(path) for path in paths), f"cross presence path order {name}")
            for path in paths:
                terminal = resolve_schema_path(name, path, schemas)
                check(terminal[0] == 6, f"cross presence optional target {name}")
        else:
            raise ValueError(f"unknown cross constraint {constraint[0]}")
    return refs


def validate_schema_graph(schemas: dict) -> None:
    graph = {}
    for name, row in schemas.items():
        verify_identity_record(row, "schema_id", f"schema {name}")
        check(row["identity_envelope"] == ["maestro.vnext.schema.v1", row["value"]], f"schema envelope {name}")
        graph[name] = validate_schema_descriptor(name, row, schemas)
    visiting = set()
    visited = set()

    def visit(name: str) -> None:
        if name in visiting:
            raise ValueError(f"schema cycle {name}")
        if name in visited:
            return
        visiting.add(name)
        for child in graph[name]:
            visit(child)
        visiting.remove(name)
        visited.add(name)

    for name in graph:
        visit(name)


def validate(artifact: dict, source: dict) -> None:
    actions = artifact["action_leaf_symbols"]
    ceremonies = artifact["ceremony_symbols"]
    routes = artifact["effect_origin_routes"]
    check(len(actions) == 136, "action count")
    check(len(ceremonies) == 11, "ceremony count")
    check(len(routes) == 23, "origin count")
    check(artifact["effect_origin_route_entry_count"] == 79, "route count")
    check(artifact["dependency_dag_edges"] == EXPECTED_DAG, "dependency DAG")
    check(len(artifact["dictionaries"]) == 13, "dictionary count")
    check(len(artifact["schemas"]) == 41, "schema count")
    validate_schema_graph(artifact["schemas"])
    for name, row in artifact["dictionaries"].items():
        verify_identity_record(row, "dictionary_id", f"dictionary {name}")
    verify_identity_record(artifact["owner_dictionary"], "dictionary_id", "owner dictionary")
    verify_identity_record(artifact["profile_dictionary"], "dictionary_id", "profile dictionary")
    grammar_value = artifact["catalog_profile_grammar"]["value"]
    check(grammar_value[1] == {"bytes": artifact["dictionaries"]["CatalogTagV1"]["dictionary_id"]}, "catalog dictionary binding")
    check(grammar_value[2] == {"bytes": artifact["owner_dictionary"]["dictionary_id"]}, "owner dictionary binding")
    check(grammar_value[3] == {"bytes": artifact["profile_dictionary"]["dictionary_id"]}, "profile dictionary binding")
    check(grammar_value[7:12] == [13, 41, 20, 6, 6], "grammar counts")
    schema_refs = grammar_value[4]
    check([row[0] for row in schema_refs] == list(range(1, 42)), "grammar schema-ref tags")
    check({row[1] for row in schema_refs} == set(artifact["schemas"]), "grammar schema-ref set")
    for _, name, version, schema_id in schema_refs:
        check(version == 1 and schema_id == {"bytes": artifact["schemas"][name]["schema_id"]}, f"grammar schema ref {name}")
    expected_profile_set_order = ["OwnerProfileSetV1", "ProtocolProfileSetV1", "PolicyProfileSetV1"]
    check(grammar_value[5] == [
        [tag, tag, {"bytes": artifact["profile_sets"][name]["profile_set_id"]}]
        for tag, name in enumerate(expected_profile_set_order, 1)
    ], "grammar profile set refs")
    check(grammar_value[6] == artifact["dependency_dag_edges"], "grammar DAG binding")
    check(grammar_value[12] == [row["value"] for row in actions], "grammar Action binding")
    check(grammar_value[13] == [row["value"] for row in ceremonies], "grammar Ceremony binding")
    check(grammar_value[14] == [row["value"] for row in routes], "grammar route binding")
    check(len(grammar_value[15]) == 5, "grammar clause count")
    check(artifact["owner_dictionary"]["value"][1:] == [
        {"bytes": artifact["dictionaries"]["CatalogOwnerTagV1"]["dictionary_id"]},
        {"bytes": artifact["dictionaries"]["CatalogOwnerScopeModeV1"]["dictionary_id"]},
    ], "owner dictionary closure")
    expected_profile_dictionary_refs = [
        "ProfileKindTagV1", "PolicyKindTagV1", "CatalogEqualityModeV1", "CatalogDagRoleV1",
        "CatalogSchemaRoleV1", "EffectOriginRouteRoleV1", "EffectOriginRouteBasisV1",
        "EffectOriginRouteContextV1", "EffectOriginRouteSymbolKindV1", "EffectOriginSourceOwnerV1",
    ]
    check(artifact["profile_dictionary"]["value"][1:] == [
        {"bytes": artifact["dictionaries"][name]["dictionary_id"]} for name in expected_profile_dictionary_refs
    ], "profile dictionary closure")

    owner_ids = {row["tag"]: row["profile_id"] for row in artifact["owner_profiles"]}
    check(set(owner_ids) == set(range(1, 21)), "owner profile tags")
    expected_members = {tag: [] for tag in range(1, 21)}
    action_by_name = {}
    global_tag = 0
    for family_tag, family in enumerate(source["action_families"], 1):
        owner_tag = OWNER_TAGS[family["owner"]]
        for local_tag, name in enumerate(family["leaves"], 1):
            global_tag += 1
            row = actions[global_tag - 1]
            expected_value = [global_tag, [owner_tag, {"bytes": owner_ids[owner_tag]}], family_tag, local_tag, name, 1]
            check(row["value"] == expected_value, f"action value {global_tag}")
            verify_identity_record(row, "descriptor_id", f"action {global_tag}")
            check(len(row["identity_envelope"]) == 3, f"action envelope arity {global_tag}")
            action_by_name[name] = row
            expected_members[owner_tag].append([6_000_000 + global_tag, 6, global_tag, name, 1])

    ceremony_by_name = {}
    for tag, item in enumerate(source["ceremonies"], 1):
        owner_tag = OWNER_TAGS[item["owner"]]
        row = ceremonies[tag - 1]
        expected_value = [tag, [owner_tag, {"bytes": owner_ids[owner_tag]}], item["name"], 1]
        check(row["value"] == expected_value, f"ceremony value {tag}")
        verify_identity_record(row, "descriptor_id", f"ceremony {tag}")
        check(len(row["identity_envelope"]) == 3, f"ceremony envelope arity {tag}")
        ceremony_by_name[item["name"]] = row
        expected_members[owner_tag].append([5_000_000 + tag, 5, tag, item["name"], 1])

    all_members = []
    for profile in artifact["owner_profiles"]:
        value = profile["value"]
        tag = profile["tag"]
        check(len(profile["identity_envelope"]) == 3, f"owner envelope arity {tag}")
        verify_identity_record(profile, "profile_id", f"owner {tag}")
        check(value[4] == [[5], [6]], f"owner grammar catalog scope {tag}")
        check(value[5] == (1 if expected_members[tag] else 2), f"owner grammar membership mode {tag}")
        check(value[6] == expected_members[tag], f"owner members {tag}")
        check(value[3] == {"bytes": artifact["owner_dictionary"]["dictionary_id"]}, f"owner dictionary value {tag}")
        all_members.extend(value[6])
    check(len(all_members) == 147, "owner member projection count")
    check(len({row[0] for row in all_members}) == 147, "owner member projection uniqueness")

    route_total = 0
    for tag, source_row in enumerate(source["effect_origin_routes"], 1):
        row = routes[tag - 1]
        expected_entries = []
        route_tag = 0
        if source_row["action_phase_set"] is not None:
            for role, name in ACTION_PHASE_SETS[source_row["action_phase_set"]]:
                route_tag += 1
                symbol = action_by_name[name]
                expected_entries.append([route_tag, role, CONTEXT_TAGS[source_row["action_context"]], BASIS_TAGS[source_row["action_basis"]], 1, symbol["global_tag"], {"bytes": symbol["descriptor_id"]}])
        for name in source_row["ceremony_symbols"]:
            symbol = ceremony_by_name[name]
            for role in (4, 5):
                route_tag += 1
                expected_entries.append([route_tag, role, CONTEXT_TAGS["NoStore" if symbol["tag"] == 1 else "PreStore"], 4, 2, symbol["tag"], {"bytes": symbol["descriptor_id"]}])
        expected_value = [tag, source_row["origin"], SOURCE_OWNER_TAGS[source_row["origin_source_owner"]], expected_entries]
        check(row["value"] == expected_value, f"origin route {tag}")
        verify_identity_record(row, "descriptor_id", f"origin route {tag}")
        check(len(row["identity_envelope"]) == 3, f"origin route envelope arity {tag}")
        route_total += len(expected_entries)
    check(route_total == 79, "expanded route total")

    for collection_index, collection in enumerate((artifact["protocol_profiles"], artifact["policy_profiles"]), 1):
        for row in collection:
            check(len(row["identity_envelope"]) == 3, "profile envelope arity")
            verify_identity_record(row, "profile_id", "profile")
            dictionary_index = 3 if collection_index == 1 else 4
            check(row["value"][dictionary_index] == {"bytes": artifact["profile_dictionary"]["dictionary_id"]}, "profile dictionary value")
    for name, row in artifact["profile_sets"].items():
        verify_identity_record(row, "profile_set_id", f"profile set {name}")
    profile_collections = {
        "OwnerProfileSetV1": artifact["owner_profiles"],
        "ProtocolProfileSetV1": artifact["protocol_profiles"],
        "PolicyProfileSetV1": artifact["policy_profiles"],
    }
    for name, collection in profile_collections.items():
        id_field = "profile_id"
        expected_rows = [[row["tag"], {"bytes": row[id_field]}, row["value"]] for row in collection]
        check(artifact["profile_sets"][name]["value"][4] == expected_rows, f"profile set rows {name}")
    grammar = artifact["catalog_profile_grammar"]
    check(grammar["value"] == grammar["identity_envelope"][2], "grammar value-envelope binding")
    check(len(grammar["identity_envelope"]) == 3, "grammar envelope arity")
    check(grammar["catalog_profile_grammar_id"] == sha(grammar["identity_envelope"]), "grammar id")
    check(grammar["sha256"] == grammar["catalog_profile_grammar_id"], "grammar digest alias")
    check(grammar["cbor_hex"] == encode(grammar["identity_envelope"]).hex(), "grammar bytes")
    check(grammar["byte_length"] == len(encode(grammar["identity_envelope"])), "grammar byte length")


def reseal_schema_graph_and_grammar(artifact: dict) -> None:
    schemas = artifact["schemas"]
    visited: set[str] = set()
    visiting: set[str] = set()

    def dependencies(value) -> set[str]:
        found: set[str] = set()
        if isinstance(value, list):
            if len(value) == 4 and value[0] == 5 and isinstance(value[1], str) and value[1] in schemas:
                found.add(value[1])
            for item in value:
                found.update(dependencies(item))
        return found

    def refresh_refs(value) -> None:
        if not isinstance(value, list):
            return
        if len(value) == 4 and value[0] == 5 and isinstance(value[1], str) and value[1] in schemas:
            value[3] = {"bytes": schemas[value[1]]["schema_id"]}
        for item in value:
            refresh_refs(item)

    def reseal(name: str) -> None:
        if name in visited:
            return
        check(name not in visiting, f"mutation schema cycle {name}")
        visiting.add(name)
        row = schemas[name]
        for dependency in dependencies(row["value"]):
            reseal(dependency)
        refresh_refs(row["value"])
        row["identity_envelope"] = ["maestro.vnext.schema.v1", row["value"]]
        raw = encode(row["identity_envelope"])
        identifier = hashlib.sha256(raw).hexdigest()
        row["schema_id"] = identifier
        row["sha256"] = identifier
        row["cbor_hex"] = raw.hex()
        row["byte_length"] = len(raw)
        visiting.remove(name)
        visited.add(name)

    for schema_name in schemas:
        reseal(schema_name)

    grammar = artifact["catalog_profile_grammar"]
    for row in grammar["value"][4]:
        row[3] = {"bytes": schemas[row[1]]["schema_id"]}
    grammar_schema_id = schemas["maestro.vnext.catalog.profile-grammar-value.v1"]["schema_id"]
    grammar["identity_envelope"] = [
        "maestro.vnext.catalog-profile-grammar.v1",
        {"bytes": grammar_schema_id},
        grammar["value"],
    ]
    raw = encode(grammar["identity_envelope"])
    identifier = hashlib.sha256(raw).hexdigest()
    grammar["catalog_profile_grammar_id"] = identifier
    grammar["sha256"] = identifier
    grammar["cbor_hex"] = raw.hex()
    grammar["byte_length"] = len(raw)


def mutate_invalid_canonical_set_path(artifact: dict) -> None:
    schema = artifact["schemas"]["maestro.vnext.catalog.profile-grammar-value.v1"]["value"]
    schema[2][4][3][0][1] = [[1, 999]]
    reseal_schema_graph_and_grammar(artifact)


def mutate_invalid_cross_constraint_path(artifact: dict) -> None:
    schema = artifact["schemas"]["maestro.vnext.catalog.profile-set-ref.v1"]["value"]
    schema[3][0][2] = [[1, 999]]
    reseal_schema_graph_and_grammar(artifact)


def mutation_suite(artifact: dict, source: dict) -> int:
    mutations = []
    mutations.append(lambda x: x["action_leaf_symbols"].pop())
    mutations.append(lambda x: x["action_leaf_symbols"][1].__setitem__("value", copy.deepcopy(x["action_leaf_symbols"][0]["value"])))
    mutations.append(lambda x: x["action_leaf_symbols"][0]["value"][1].__setitem__(0, 8))
    mutations.append(lambda x: x["ceremony_symbols"][0]["value"].__setitem__(1, [8, x["ceremony_symbols"][0]["value"][1][1]]))
    mutations.append(lambda x: x["effect_origin_routes"][0]["value"][3][0][6].__setitem__("bytes", "00" * 32))
    mutations.append(lambda x: x["effect_origin_routes"][0]["value"][3][0].__setitem__(1, 2))
    mutations.append(lambda x: x["effect_origin_routes"][0]["value"][3][0].__setitem__(3, 4))
    mutations.append(lambda x: x["effect_origin_routes"][0]["value"][3][0].__setitem__(2, 3))
    mutations.append(lambda x: x["dependency_dag_edges"][0].__setitem__(2, 2))
    mutations.append(lambda x: x["catalog_profile_grammar"].__setitem__("catalog_profile_grammar_id", "00" * 32))
    mutations.append(lambda x: x["action_leaf_symbols"].append(copy.deepcopy(x["action_leaf_symbols"][-1])))
    mutations.append(lambda x: x["action_leaf_symbols"][1]["value"].__setitem__(0, 3))
    mutations.append(lambda x: x["effect_origin_routes"][0]["value"].__setitem__(2, 3))
    mutations.append(lambda x: x["owner_profiles"][0].__setitem__("value", copy.deepcopy(x["owner_profiles"][1]["value"])))
    mutations.append(lambda x: x["catalog_profile_grammar"]["value"][15][0].__setitem__(2, "changed clause"))
    mutations.append(lambda x: x["action_leaf_symbols"][0]["value"].__setitem__(5, 2))
    mutations.append(lambda x: x["dependency_dag_edges"][0].__setitem__(2, 9))
    mutations.append(lambda x: x["catalog_profile_grammar"]["value"][12].append([999]))
    mutations.append(mutate_invalid_canonical_set_path)
    mutations.append(mutate_invalid_cross_constraint_path)
    rejected = 0
    for mutate in mutations:
        candidate = copy.deepcopy(artifact)
        mutate(candidate)
        try:
            validate(candidate, source)
        except (KeyError, IndexError, TypeError, ValueError):
            rejected += 1
        else:
            raise RuntimeError("semantic mutant was accepted")
    return rejected


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: validator ARTIFACT.json NOMINAL_SOURCE.json")
    artifact_path = Path(sys.argv[1])
    source_path = Path(sys.argv[2])
    artifact = json.loads(artifact_path.read_text(encoding="ascii"))
    source = json.loads(source_path.read_text(encoding="ascii"))
    check(artifact["nominal_source_sha256"] == hashlib.sha256(source_path.read_bytes()).hexdigest(), "nominal source hash")
    validate(artifact, source)
    grammar_id = artifact["catalog_profile_grammar"]["catalog_profile_grammar_id"]
    if "sha256-" in artifact_path.name:
        check(artifact_path.name == f"vnext-catalog-profile-grammar-v1-sha256-{grammar_id}.json", "immutable filename suffix")
    rejected = mutation_suite(artifact, source)
    print(json.dumps({"status": "valid", "mutants_rejected": rejected, "artifact": str(artifact_path)}, sort_keys=True))


if __name__ == "__main__":
    main()
