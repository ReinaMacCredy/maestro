#!/usr/bin/env python3
"""Independent validator for the nine design-only literal catalog artifacts."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INDEX_PATH = ROOT / "vnext-catalog-literal-suite-v1-index.json"
NOMINAL_PATH = ROOT / "vnext-catalog-nominal-source-v1.json"
GRAMMAR_PATH = ROOT / "vnext-catalog-profile-grammar-v1-sha256-2b428f8444253794cd0abb41b32da482cc0805359c2a37bf0cba90a70e3186e9.json"
PY_ENCODER = ROOT / "vnext_manifest_encode_py.py"
RB_ENCODER = ROOT / "vnext_manifest_encode_rb.rb"


class ValidationError(Exception):
    pass


def check(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def head(major: int, value: int) -> bytes:
    check(isinstance(value, int) and not isinstance(value, bool), "u64 type")
    check(0 <= value <= 0xFFFFFFFFFFFFFFFF, "u64 range")
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
        text = value["bytes"]
        check(isinstance(text, str) and len(text) % 2 == 0 and text.lower() == text, "bytes wrapper")
        raw = bytes.fromhex(text)
        return head(2, len(raw)) + raw
    raise ValidationError(f"unsupported value {value!r}")


def digest(value) -> str:
    return hashlib.sha256(encode(value)).hexdigest()


def sha_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def bhex(value) -> str:
    check(isinstance(value, dict) and list(value) == ["bytes"], "bytes identity")
    text = value["bytes"]
    check(len(text) == 64 and text.lower() == text, "bytes32 identity")
    bytes.fromhex(text)
    return text


def load_inputs():
    index = json.loads(INDEX_PATH.read_text(encoding="ascii"))
    nominal = json.loads(NOMINAL_PATH.read_text(encoding="ascii"))
    grammar = json.loads(GRAMMAR_PATH.read_text(encoding="ascii"))
    artifacts = {}
    for row in index["catalogs"]:
        path = ROOT / row["artifact_path"]
        check(path.is_file(), f"artifact missing {path.name}")
        check(sha_file(path) == row["artifact_sha256"], f"artifact file hash {row['catalog_tag']}")
        artifact = json.loads(path.read_text(encoding="ascii"))
        artifacts[row["catalog_tag"]] = artifact
    return index, nominal, grammar, artifacts


EXPECTED_DAG = {
    1: [],
    2: [1],
    3: [],
    4: [],
    5: [1, 2, 3, 4],
    6: [1, 2, 3, 4, 5],
    7: [1, 2, 3, 5, 6],
    8: [1, 2, 4, 5, 6],
    9: [1, 2, 3, 4, 5, 6, 7, 8],
}
EXPECTED_COUNTS = {1: 43, 2: 23, 3: 6, 4: 6, 5: 11, 6: 136, 7: 35, 8: 30, 9: 136}
EXPECTED_SLUGS = {
    1: "observation",
    2: "effect",
    3: "repository-capacity",
    4: "installation-capacity",
    5: "ceremony",
    6: "action-leaf",
    7: "repository-continuity",
    8: "installation-continuity",
    9: "action-spec",
}
CMA_EDGES = [
    [1, 29, 1], [1, 30, 2], [2, 34, 3], [2, 33, 4], [3, 35, 5],
    [3, 33, 6], [4, 31, 7], [5, 32, 8], [5, 36, 9], [5, 37, 10],
]


def schema_refs(type_expr):
    if not isinstance(type_expr, list) or not type_expr:
        return []
    tag = type_expr[0]
    if tag == 5:
        return [(type_expr[1], type_expr[2], bhex(type_expr[3]))]
    if tag in {6, 7}:
        return schema_refs(type_expr[1])
    if tag == 8:
        result = []
        for item in type_expr[1]:
            result.extend(schema_refs(item))
        return result
    return []


def validate_schemas(artifact):
    schemas = artifact["schemas"]
    by_name = {}
    for row in schemas.values():
        value = row["value"]
        name = value[0]
        check(value[1] == 1, "schema version")
        check(row["identity_envelope"] == ["maestro.vnext.schema.v1", value], "schema envelope")
        check(digest(row["identity_envelope"]) == row["schema_id"], f"schema id {name}")
        check(row["cbor_hex"] == encode(row["identity_envelope"]).hex(), f"schema bytes {name}")
        check(row["byte_length"] == len(encode(row["identity_envelope"])), f"schema length {name}")
        positions = [field[0] for field in value[2]]
        names = [field[1] for field in value[2]]
        check(positions == list(range(1, len(positions) + 1)), f"schema positions {name}")
        check(len(names) == len(set(names)), f"schema field names {name}")
        by_name[name] = row["schema_id"]
    for row in schemas.values():
        for schema_field in row["value"][2]:
            for name, version, schema_id in schema_refs(schema_field[2]):
                check(version == 1, "schema ref version")
                check(by_name.get(name) == schema_id, f"schema ref closure {name}")


def validate_identity(artifact, prior):
    tag = artifact["catalog_tag"]
    check(artifact["catalog_slug"] == EXPECTED_SLUGS[tag], "catalog slug")
    check(len(artifact["descriptors"]) == EXPECTED_COUNTS[tag], f"row count {tag}")
    values = [row["value"] for row in artifact["descriptors"]]
    tags = [value[0] for value in values]
    check(tags == list(range(1, EXPECTED_COUNTS[tag] + 1)), f"dense tags {tag}")
    validate_schemas(artifact)
    descriptor_schema_id = artifact["schemas"]["descriptor"]["schema_id"]
    for row, value in zip(artifact["descriptors"], values):
        envelope = [artifact["descriptor_domain"], {"bytes": descriptor_schema_id}, value]
        check(row["identity_envelope"] == envelope, f"descriptor envelope {tag}/{value[0]}")
        check(digest(envelope) == row["descriptor_id"], f"descriptor id {tag}/{value[0]}")
        check(row["cbor_hex"] == encode(envelope).hex(), f"descriptor bytes {tag}/{value[0]}")
        check(row["byte_length"] == len(encode(envelope)), f"descriptor length {tag}/{value[0]}")
    relation = artifact["primary_owner_relation"]
    expected_relation = [[value[0], value[2][0], value[2][1]] for value in values]
    check(relation["rows"] == expected_relation, f"owner relation rows {tag}")
    check(digest(relation["identity_envelope"]) == relation["relation_id"], f"owner relation id {tag}")
    header = artifact["manifest_header"]
    check(header[0:3] == [tag, 1, 1], f"header versions {tag}")
    check(bhex(header[3]) == artifact["grammar_id"], f"grammar binding {tag}")
    check(bhex(header[4]) == artifact["schemas"]["generated_sum"]["schema_id"], "sum schema binding")
    check(bhex(header[5]) == descriptor_schema_id, "descriptor schema binding")
    check(bhex(header[6]) == artifact["schemas"]["header"]["schema_id"], "header schema binding")
    check(bhex(header[7]) == artifact["schemas"]["manifest"]["schema_id"], "manifest schema binding")
    deps = [[row[0], bhex(row[1])] for row in header[8]]
    check([row[0] for row in deps] == EXPECTED_DAG[tag], f"dependency tags {tag}")
    for dep_tag, manifest_id in deps:
        check(dep_tag < tag and prior[dep_tag]["manifest_id"] == manifest_id, f"dependency id {tag}/{dep_tag}")
    check(bhex(header[10]) == relation["relation_id"], "owner relation binding")
    check(header[14:17] == [EXPECTED_COUNTS[tag], 1, EXPECTED_COUNTS[tag]], f"header counts {tag}")
    rows = [[value[0], {"bytes": row["descriptor_id"]}, value] for row, value in zip(artifact["descriptors"], values)]
    check(artifact["manifest_value"] == [header, rows], f"manifest value {tag}")
    envelope = [artifact["manifest_domain"], {"bytes": artifact["schemas"]["manifest"]["schema_id"]}, {"bytes": descriptor_schema_id}, header, rows]
    check(artifact["manifest_identity_envelope"] == envelope, f"manifest envelope {tag}")
    check(digest(envelope) == artifact["manifest_id"], f"manifest id {tag}")
    check(artifact["cbor_hex"] == encode(envelope).hex(), f"manifest bytes {tag}")
    check(artifact["byte_length"] == len(encode(envelope)), f"manifest length {tag}")


def validate_observation(artifact, nominal, action_by_name):
    values = [row["value"] for row in artifact["descriptors"]]
    check([value[1] for value in values] == nominal["observations"], "observation names")
    check({value[2][0] for value in values} == {6}, "observation owner")
    check(artifact["semantic_proof"]["cma_positive_edges"] == CMA_EDGES, "CMA edge equality")
    check(artifact["semantic_proof"]["cma_positive_count"] == 10, "CMA positive count")
    check(artifact["semantic_proof"]["cma_negative_count"] == 205, "CMA negative count")
    cma_action = action_by_name["PublishContinuityMaintenanceObservation"]["global_tag"]
    for purpose, kind, route in CMA_EDGES:
        value = values[kind - 1]
        check(cma_action in value[7], f"CMA producer {purpose}/{kind}")
        check(route in value[8], f"CMA route {purpose}/{kind}")


def validate_effect(artifact, nominal, grammar):
    values = [row["value"] for row in artifact["descriptors"]]
    routes = sorted(grammar["effect_origin_routes"], key=lambda row: row["origin_tag"])
    check([value[1] for value in values] == nominal["effect_origins"], "effect names")
    check({value[2][0] for value in values} == {5}, "effect primary owner")
    check(sum(len(value[7]) for value in values) == 79, "effect route count")
    for value, route in zip(values, routes):
        check(value[7] == route["value"][3], f"effect route equality {value[0]}")


def validate_capacity(artifact, nominal, domain):
    values = [row["value"] for row in artifact["descriptors"]]
    check([value[1] for value in values] == nominal["capacity_profiles"][domain], f"{domain} capacity names")
    check({value[2][0] for value in values} == {8}, f"{domain} capacity owner")
    check(all(value[6] and value[7] and value[8] for value in values), f"{domain} finite/refill law")


def validate_ceremony(artifact, grammar):
    values = [row["value"] for row in artifact["descriptors"]]
    symbols = sorted(grammar["ceremony_symbols"], key=lambda row: row["tag"])
    check([value[1] for value in values] == [row["name"] for row in symbols], "ceremony names")
    for value, symbol in zip(values, symbols):
        check(value[2][0] == symbol["value"][1][0], f"ceremony owner {value[0]}")
        check(value[7] == [1, 2], f"ceremony modes {value[0]}")


def validate_action_leaf(artifact, grammar):
    values = [row["value"] for row in artifact["descriptors"]]
    symbols = sorted(grammar["action_leaf_symbols"], key=lambda row: row["global_tag"])
    check([value[1] for value in values] == [row["name"] for row in symbols], "action names")
    for value, symbol in zip(values, symbols):
        check(value[2][0] == symbol["value"][1][0], f"action owner {value[0]}")
        check(value[4:6] == [symbol["family_tag"], symbol["family_local_tag"]], f"action tags {value[0]}")
        if "BootstrapMandate" in value[1]:
            check(value[6] == 2, f"bootstrap basis {value[1]}")
        if "ContinuityMaintenance" in value[1]:
            check(value[6] == 3, f"CMA basis {value[1]}")
    check(artifact["semantic_proof"]["family_counts"] == {
        "Work": 4, "Contract": 2, "Design": 4, "Decision": 5, "Execution": 14,
        "Evidence": 7, "Authority": 48, "Coordination": 9, "Planning": 4,
        "Persistence": 10, "Distribution": 13, "SearchMaintenance": 2,
        "Memory": 7, "Intake": 3, "Research": 4,
    }, "action family counts")


def validate_continuity(artifact, domain):
    values = [row["value"] for row in artifact["descriptors"]]
    proof = artifact["semantic_proof"]
    obligations = proof["atomic_obligations"]
    check(proof["domain"] == domain, "continuity domain")
    check([row[0] for row in obligations] == list(range(1, len(obligations) + 1)), "obligation dense tags")
    class_tags = {value[0] for value in values}
    included = 0
    excluded = 0
    reverse = {tag: set() for tag in class_tags}
    semantic_keys = []
    for row in obligations:
        check(len(row) == 9, "obligation shape")
        semantic_keys.append((row[1], row[2], row[3], row[4], row[5], row[6], tuple(row[7])))
        if row[6] == 1:
            included += 1
            check(row[7], "IncludedBy nonempty")
            check(set(row[7]) <= class_tags, "IncludedBy class closure")
            check(row[8] == 0, "IncludedBy invariant absent")
            for tag in row[7]:
                reverse[tag].add(row[0])
        elif row[6] == 2:
            excluded += 1
            check(not row[7] and row[8] > 0, "ExplicitlyNonContinuity proof")
        else:
            raise ValidationError("obligation disposition")
    check(semantic_keys == sorted(semantic_keys), "obligation semantic order")
    check(included == proof["included_count"] and excluded == proof["explicitly_non_continuity_count"], "obligation counts")
    for value in values:
        check(set(value[7]) == reverse[value[0]], f"reverse obligation relation {value[0]}")
        check(len(value[8]) == 5 and all(mode in {1, 2, 3} for mode in value[8]), "facet closure")
    check(all(reverse.values()), "continuity class nonempty")


def validate_action_spec(artifact, action_artifact):
    specs = [row["value"] for row in artifact["descriptors"]]
    leaves = [row["value"] for row in action_artifact["descriptors"]]
    check(len(specs) == len(leaves) == 136, "ActionSpec equality count")
    for spec, leaf in zip(specs, leaves):
        check(spec[0:6] == leaf[0:6], f"ActionSpec leaf identity {spec[0]}")
        check(spec[8] == leaf[6], f"ActionSpec basis {spec[0]}")
        check(spec[19] == leaf[12], f"ActionSpec produced records {spec[0]}")
        check(spec[20] == leaf[10], f"ActionSpec observation refs {spec[0]}")
        check(spec[21] == leaf[11], f"ActionSpec origin refs {spec[0]}")
        check(spec[22] == leaf[13], f"ActionSpec forbidden refs {spec[0]}")


def validate_external_encoders(artifact):
    path = ROOT / artifact["encoder_input_path"]
    check(path.is_file(), "encoder input exists")
    check(sha_file(path) == artifact["encoder_input_sha256"], "encoder input hash")
    py = subprocess.run(["python3", str(PY_ENCODER), str(path)], check=True, capture_output=True, text=True).stdout.splitlines()
    rb = subprocess.run(["ruby", str(RB_ENCODER), str(path)], check=True, capture_output=True, text=True).stdout.splitlines()
    check(py == rb and len(py) == 3, "independent encoder equality")
    receipt = artifact["encoder_receipts"]["aggregate"]
    check(py == [receipt["cbor_hex"], str(receipt["byte_length"]), receipt["sha256"]], "encoder receipt")


def validate_suite(index, nominal, grammar, artifacts, external=True):
    check(index["schema_version"] == "maestro.vnext.catalog.literal-suite.v1", "index schema")
    check(index["grammar_id"] == grammar["catalog_profile_grammar"]["catalog_profile_grammar_id"], "suite grammar id")
    check(index["grammar_artifact_sha256"] == sha_file(GRAMMAR_PATH), "suite grammar artifact")
    check(index["nominal_source_sha256"] == sha_file(NOMINAL_PATH), "suite nominal source")
    check(sorted(artifacts) == list(range(1, 10)), "nine catalogs")
    action_by_name = {row["name"]: row for row in grammar["action_leaf_symbols"]}
    for tag in range(1, 10):
        validate_identity(artifacts[tag], artifacts)
    validate_observation(artifacts[1], nominal, action_by_name)
    validate_effect(artifacts[2], nominal, grammar)
    validate_capacity(artifacts[3], nominal, "Repository")
    validate_capacity(artifacts[4], nominal, "Installation")
    validate_ceremony(artifacts[5], grammar)
    validate_action_leaf(artifacts[6], grammar)
    validate_continuity(artifacts[7], "Repository")
    validate_continuity(artifacts[8], "Installation")
    validate_action_spec(artifacts[9], artifacts[6])
    check(index["aggregate_counts"] == {
        "catalogs": 9,
        "rows": sum(EXPECTED_COUNTS.values()),
        "schemas": 54,
        "effect_routes": 79,
        "cma_positive": 10,
        "cma_negative": 205,
    }, "aggregate counts")
    if external:
        for artifact in artifacts.values():
            validate_external_encoders(artifact)


def reject_mutants(index, nominal, grammar, artifacts):
    mutants = []

    def add(name, mutate):
        copied = copy.deepcopy(artifacts)
        mutate(copied)
        mutants.append((name, copied))

    add("observation_missing", lambda a: a[1]["descriptors"].pop())
    add("observation_name", lambda a: a[1]["descriptors"][0]["value"].__setitem__(1, "Other"))
    add("observation_owner", lambda a: a[1]["descriptors"][0]["value"][2].__setitem__(0, 5))
    add("cma_edge", lambda a: a[1]["semantic_proof"].__setitem__("cma_positive_edges", CMA_EDGES[:-1]))
    add("effect_route", lambda a: a[2]["descriptors"][0]["value"][7].pop())
    add("effect_owner", lambda a: a[2]["descriptors"][0]["value"][2].__setitem__(0, 8))
    add("effect_name", lambda a: a[2]["descriptors"][0]["value"].__setitem__(1, "GenericEffect"))
    add("repo_capacity_duplicate", lambda a: a[3]["descriptors"][1]["value"].__setitem__(0, 1))
    add("repo_capacity_refill", lambda a: a[3]["descriptors"][0]["value"].__setitem__(8, False))
    add("install_capacity_name", lambda a: a[4]["descriptors"][0]["value"].__setitem__(1, "CustomCapacity"))
    add("ceremony_owner", lambda a: a[5]["descriptors"][0]["value"][2].__setitem__(0, 8))
    add("ceremony_mode", lambda a: a[5]["descriptors"][0]["value"].__setitem__(7, [1]))
    add("action_missing", lambda a: a[6]["descriptors"].pop())
    add("action_owner", lambda a: a[6]["descriptors"][0]["value"][2].__setitem__(0, 8))
    bootstrap_index = next(i for i, row in enumerate(artifacts[6]["descriptors"]) if "BootstrapMandate" in row["value"][1])
    add("action_bootstrap_basis", lambda a: a[6]["descriptors"][bootstrap_index]["value"].__setitem__(6, 1))
    add("action_family_count", lambda a: a[6]["semantic_proof"]["family_counts"].__setitem__("Execution", 13))
    add("repo_obligation_drop", lambda a: a[7]["semantic_proof"]["atomic_obligations"].pop())
    included_index = next(i for i, row in enumerate(artifacts[7]["semantic_proof"]["atomic_obligations"]) if row[6] == 1)
    add("repo_included_empty", lambda a: a[7]["semantic_proof"]["atomic_obligations"][included_index].__setitem__(7, []))
    add("repo_facet", lambda a: a[7]["descriptors"][0]["value"].__setitem__(8, [1, 1]))
    add("install_class_missing", lambda a: a[8]["descriptors"].pop())
    add("install_disposition", lambda a: a[8]["semantic_proof"]["atomic_obligations"][0].__setitem__(6, 3))
    add("action_spec_missing", lambda a: a[9]["descriptors"].pop())
    add("action_spec_owner", lambda a: a[9]["descriptors"][0]["value"][2].__setitem__(0, 8))
    add("action_spec_basis", lambda a: a[9]["descriptors"][0]["value"].__setitem__(8, 3))
    add("header_dependency", lambda a: a[9]["manifest_header"][8].pop())
    add("header_grammar", lambda a: a[1]["manifest_header"][3].__setitem__("bytes", "00" * 32))
    add("owner_relation", lambda a: a[1]["primary_owner_relation"]["rows"][0].__setitem__(1, 5))
    add("descriptor_id", lambda a: a[2]["descriptors"][0].__setitem__("descriptor_id", "00" * 32))
    add("manifest_id", lambda a: a[3].__setitem__("manifest_id", "00" * 32))
    add("schema_id", lambda a: a[4]["schemas"]["descriptor"].__setitem__("schema_id", "00" * 32))

    rejected = []
    for name, mutant in mutants:
        try:
            validate_suite(index, nominal, grammar, mutant, external=False)
        except (ValidationError, ValueError, KeyError, IndexError):
            rejected.append(name)
        else:
            raise ValidationError(f"mutant accepted: {name}")
    return rejected


def main() -> None:
    index, nominal, grammar, artifacts = load_inputs()
    validate_suite(index, nominal, grammar, artifacts, external=True)
    rejected = reject_mutants(index, nominal, grammar, artifacts)
    print(json.dumps({
        "valid": True,
        "catalogs": 9,
        "rows": sum(EXPECTED_COUNTS.values()),
        "manifest_ids": {str(tag): artifacts[tag]["manifest_id"] for tag in range(1, 10)},
        "effect_routes": 79,
        "cma_positive": 10,
        "cma_negative": 205,
        "mutants_rejected": len(rejected),
        "mutants": rejected,
    }, indent=2))


if __name__ == "__main__":
    try:
        main()
    except ValidationError as error:
        print(f"invalid: {error}", file=sys.stderr)
        raise SystemExit(1)
