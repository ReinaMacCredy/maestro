#!/usr/bin/env python3
"""Focused, read-only Stage-6 candidate validator."""

from __future__ import annotations

import argparse
import copy
import json
import re
import subprocess
from pathlib import Path

BASE = "6bb7e881800cd33a115086c557e315b71b4485c0"
PREFIXES = (
    "src/domain/vnext/capability/generated_catalog/",
    "src/domain/vnext/projection/",
    "src/domain/vnext/transport/",
    "src/interfaces/vnext/cli/",
    "src/operations/vnext/action/",
    "tests/fixtures/vnext/stage6/",
    "tests/vnext_stage6_",
    "tools/vnext_contracts/stage6/",
)
REQUIRED_SOURCE_TOKENS = {
    "src/domain/vnext/capability/generated_catalog/catalog.rs": (
        "OWNER_RELATION_CATALOGS",
        "OWNER_RELATION_TOTAL_ROWS",
        "validate_owner_relation_closure",
        "OwnerRelationClosureMismatch",
    ),
    "src/domain/vnext/projection/engine.rs": (
        "ProjectionReadPortV1",
        "DiscoverSelectionContextV1",
        "packet_semantic_hash",
        "advertised_specs",
        "validate_project_selection",
        "ForeignSelectionBasis",
        "NonMemberSelection",
    ),
    "src/domain/vnext/transport/json.rs": (
        "decode_operation_request",
        "decode_packet_read_request",
        "UnknownOrMissingField",
        "encode_operation_result",
    ),
    "src/operations/vnext/action/service.rs": (
        "GovernedOperationPortV1",
        "OwnerDurableResultV1",
        "SameKeyDifferentMeaning",
        "semantic_request_hash",
        "has_frozen_owner_materialization",
        "RepositoryActionLeafV1::ALL",
        "AuthorityActionLeafV1::ALL",
        "downstream_tags_94_through_145_delegate_to_the_materialized_owner_port",
        "actions_without_a_frozen_owner_materialization_remain_unavailable",
    ),
    "src/interfaces/vnext/cli/adapter.rs": (
        "OperationPrepare",
        "OperationSubmit",
        "OperationResult",
        "CapabilityCatalog",
    ),
}
CATALOGS = (
    ("catalog-01-observation.json", 43),
    ("catalog-02-effect.json", 23),
    ("catalog-03-repository-capacity.json", 6),
    ("catalog-04-installation-capacity.json", 6),
    ("catalog-05-ceremony.json", 11),
    ("catalog-06-action-leaf.json", 145),
    ("catalog-07-repository-continuity.json", 35),
    ("catalog-08-installation-continuity.json", 30),
    ("catalog-09-action-spec.json", 145),
)
ADAPTER_GATE = {
    "candidate_adapters": "test_only",
    "production_adapter": "absent",
    "replacement_requirement": "real_upstream_materialized_owner_and_result_store",
    "parity_gate": "same_request_admission_result_and_replay_vectors_before_activation",
}
ACTION_OWNER_BOUNDARY = {
    "recognized_action_count": 145,
    "materialized_owner_delegation_count": 95,
    "materialized_downstream_tag_range": [94, 145],
    "materialized_downstream_count": 52,
    "pre_port_owner_unavailable_count": 50,
}


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def validate_closure(root: Path) -> None:
    actions = load(
        root / "contracts/vnext/catalogs/generated/catalog-09-action-spec.json"
    )["descriptors"]
    ceremonies = load(
        root / "contracts/vnext/catalogs/generated/catalog-05-ceremony.json"
    )["descriptors"]
    selections = load(
        root / "contracts/vnext/public/recipe_selection_application_vectors.v1.json"
    )["vectors"]
    closure = load(root / "tests/fixtures/vnext/stage6/closure.v1.json")
    validate_documents(actions, ceremonies, selections, closure)
    validate_action_owner_boundary(root, actions, closure)
    catalogs = load_catalogs(root)
    validate_owner_relations(catalogs, closure)
    validate_adapter_classification(root, closure)


def validate_documents(
    actions: list[dict],
    ceremonies: list[dict],
    selections: list[dict],
    closure: dict,
) -> None:
    assert closure["action_owner_boundary"] == ACTION_OWNER_BOUNDARY
    assert [row["value"][0] for row in actions] == list(range(1, 146))
    assert [row["value"][0] for row in ceremonies] == list(range(1, 12))
    assert len(actions) + len(ceremonies) == closure["operation_count"] == 156
    assert all(
        [outcome[1] for outcome in row["value"][-2]]
        == closure["action_outcomes"]
        for row in actions
    )
    assert all(
        row["value"][3] == closure["ceremony_request_modes"] for row in ceremonies
    )
    assert len(selections) == closure["selection_option_count"] == 30
    actual = {"0": 0, "1": 0, "2": 0}
    for vector in selections:
        count = len(
            vector["packet_recipe_binding_fixture"]["component_provenance"]
        )
        actual[str(count)] += 1
    assert actual == closure["component_provenance_cardinality"]


def load_catalogs(root: Path) -> list[dict]:
    catalog_root = root / "contracts/vnext/catalogs/generated"
    return [load(catalog_root / name) for name, _ in CATALOGS]


def validate_action_owner_boundary(
    root: Path,
    actions: list[dict],
    closure: dict,
) -> None:
    authority = (
        root / "src/domain/vnext/authority/action_basis.rs"
    ).read_text(encoding="utf-8")
    repository_match = re.search(
        r"pub const ADMITTED_STAGE5: \[Self; 38\] = \[(.*?)\n    \];",
        authority,
        re.DOTALL,
    )
    assert repository_match is not None
    repository_names = re.findall(r"Self::([A-Za-z0-9_]+)", repository_match[1])
    assert len(repository_names) == len(set(repository_names)) == 38

    authority_impl = authority.split("impl AuthorityActionLeafV1 {", maxsplit=1)[1]
    authority_match = re.search(
        r"pub const ALL: \[Self; 5\] = \[(.*?)\n    \];",
        authority_impl,
        re.DOTALL,
    )
    assert authority_match is not None
    authority_names = re.findall(r"Self::([A-Za-z0-9_]+)", authority_match[1])
    assert len(authority_names) == len(set(authority_names)) == 5

    downstream = (
        root / "src/domain/vnext/authority/downstream_action_basis.rs"
    ).read_text(encoding="utf-8")
    downstream_first = re.search(
        r"DOWNSTREAM_ACTION_FIRST_GLOBAL_TAG_V1: u64 = (\d+);",
        downstream,
    )
    downstream_last = re.search(
        r"DOWNSTREAM_ACTION_LAST_GLOBAL_TAG_V1: u64 = (\d+);",
        downstream,
    )
    assert downstream_first is not None and downstream_last is not None
    first = int(downstream_first[1])
    last = int(downstream_last[1])
    downstream_tags = set(range(first, last + 1))
    assert [first, last] == ACTION_OWNER_BOUNDARY["materialized_downstream_tag_range"]
    assert len(downstream_tags) == ACTION_OWNER_BOUNDARY["materialized_downstream_count"]
    downstream_rows = re.findall(
        r'RepositoryDownstreamActionMetadataV1 \{\s*literal: "([^"]+)",'
        r"\s*global_tag: (\d+),",
        downstream,
    )
    assert len(downstream_rows) == len(set(downstream_rows)) == len(downstream_tags)
    assert [int(tag) for _, tag in downstream_rows] == list(range(first, last + 1))

    catalog_tags = {row["value"][1]: row["value"][0] for row in actions}
    assert {catalog_tags[name] for name, _ in downstream_rows} == downstream_tags
    specialized_names = set(repository_names) | set(authority_names)
    assert len(specialized_names) == 43
    assert specialized_names <= catalog_tags.keys()
    specialized_tags = {catalog_tags[name] for name in specialized_names}
    assert specialized_tags.isdisjoint(downstream_tags)
    materialized_tags = specialized_tags | downstream_tags
    assert len(materialized_tags) == ACTION_OWNER_BOUNDARY[
        "materialized_owner_delegation_count"
    ]
    assert len(set(range(1, 146)) - materialized_tags) == ACTION_OWNER_BOUNDARY[
        "pre_port_owner_unavailable_count"
    ]
    assert closure["action_owner_boundary"] == ACTION_OWNER_BOUNDARY


def validate_owner_relations(catalogs: list[dict], closure: dict) -> None:
    expected_counts = [count for _, count in CATALOGS]
    assert len(catalogs) == len(expected_counts) == 9
    assert closure["owner_relation_cardinalities"] == expected_counts

    relation_ids: set[str] = set()
    descriptor_ids: set[str] = set()
    relation_rows: set[str] = set()
    total_rows = 0
    for domain, (catalog, expected_count) in enumerate(
        zip(catalogs, expected_counts, strict=True),
        start=1,
    ):
        descriptors = catalog["descriptors"]
        relation = catalog["primary_owner_relation"]
        rows = relation["rows"]
        assert len(descriptors) == len(rows) == expected_count
        assert relation["identity_envelope"] == [
            f"maestro.vnext.catalog.primary-owner-relation.{domain}.v1",
            rows,
        ]
        assert relation["relation_id"] not in relation_ids
        relation_ids.add(relation["relation_id"])

        for descriptor, row in zip(descriptors, rows, strict=True):
            value = descriptor["value"]
            assert row == [value[0], value[2][0], value[2][1]]
            assert descriptor["identity_envelope"][2] == value
            assert descriptor["descriptor_id"] not in descriptor_ids
            descriptor_ids.add(descriptor["descriptor_id"])
            row_identity = json.dumps(
                [domain, row],
                sort_keys=True,
                separators=(",", ":"),
            )
            assert row_identity not in relation_rows
            relation_rows.add(row_identity)
        total_rows += len(rows)

    assert total_rows == closure["owner_relation_total_rows"] == 444


def rust_test_module_spans(source: str) -> list[tuple[int, int]]:
    masked = list(source)
    state = "code"
    index = 0
    while index < len(source):
        pair = source[index : index + 2]
        char = source[index]
        if state == "code" and pair == "//":
            state = "line-comment"
            masked[index : index + 2] = "  "
            index += 2
            continue
        if state == "code" and pair == "/*":
            state = "block-comment"
            masked[index : index + 2] = "  "
            index += 2
            continue
        if state == "code" and char == '"':
            state = "string"
            masked[index] = " "
        elif state == "code" and char == "'" and (
            source[index + 2 : index + 3] == "'"
            or source[index + 1 : index + 2] == "\\"
        ):
            state = "character"
            masked[index] = " "
        elif state == "line-comment":
            if char == "\n":
                state = "code"
            else:
                masked[index] = " "
        elif state == "block-comment":
            masked[index] = " "
            if pair == "*/":
                masked[index : index + 2] = "  "
                state = "code"
                index += 2
                continue
        elif state in {"string", "character"}:
            masked[index] = " "
            if char == "\\":
                if index + 1 < len(source):
                    masked[index + 1] = " "
                    index += 2
                    continue
            elif (state == "string" and char == '"') or (
                state == "character" and char == "'"
            ):
                state = "code"
        index += 1

    structure = "".join(masked)
    marker = re.compile(
        r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+\w+\s*\{"
    )
    spans = []
    for match in marker.finditer(structure):
        open_brace = structure.find("{", match.start(), match.end())
        depth = 0
        for cursor in range(open_brace, len(structure)):
            if structure[cursor] == "{":
                depth += 1
            elif structure[cursor] == "}":
                depth -= 1
                if depth == 0:
                    spans.append((match.start(), cursor + 1))
                    break
        else:
            raise AssertionError("unterminated #[cfg(test)] module")
    return spans


def validate_adapter_classification(root: Path, closure: dict) -> None:
    assert closure["adapter_gate"] == ADAPTER_GATE
    stage6_roots = (
        root / "src/domain/vnext/capability/generated_catalog",
        root / "src/domain/vnext/projection",
        root / "src/domain/vnext/transport",
        root / "src/interfaces/vnext/cli",
        root / "src/operations/vnext/action",
    )
    implementations = []
    pattern = re.compile(
        r"\bimpl(?:\s*<[^>]+>)?\s+"
        r"(GovernedOperationPortV1|OperationResultReadPortV1)\s+for\b"
    )
    for stage6_root in stage6_roots:
        for path in stage6_root.rglob("*.rs"):
            source = path.read_text(encoding="utf-8")
            spans = rust_test_module_spans(source)
            for match in pattern.finditer(source):
                assert any(start <= match.start() < end for start, end in spans), (
                    f"{path.relative_to(root)}: concrete {match.group(1)} adapter "
                    "is not inside #[cfg(test)]"
                )
                implementations.append((path, match.group(1)))
    assert any(kind == "GovernedOperationPortV1" for _, kind in implementations)


def validate_sources(root: Path) -> None:
    for relative, tokens in REQUIRED_SOURCE_TOKENS.items():
        source = (root / relative).read_text(encoding="utf-8")
        missing = [token for token in tokens if token not in source]
        assert not missing, f"{relative}: missing {missing}"
    action_source = (
        root / "src/operations/vnext/action/service.rs"
    ).read_text(encoding="utf-8")
    unavailable = action_source.index("!has_frozen_owner_materialization(entry)")
    admission = action_source.index("let admission = self.admission(entry, request)")
    owner_port = action_source.index("let result = match port.submit(request, &admission)")
    assert unavailable < admission < owner_port
    forbidden_owner_semantics = (
        "AuthorityFacadeV1",
        "GovernedCapacityDebitV1",
        "admit_repository_action",
        "exact_authority_basis_for_action",
    )
    assert not [
        token for token in forbidden_owner_semantics if token in action_source
    ], "Stage-6 Action service contains owner-local Authority semantics"


def validate_ownership(root: Path, base: str) -> None:
    result = subprocess.run(
        ["git", "diff", "--name-only", base, "--"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    paths = sorted(
        {
            line
            for output in (result.stdout, untracked.stdout)
            for line in output.splitlines()
            if line
        }
    )
    outside = [path for path in paths if not path.startswith(PREFIXES)]
    assert not outside, f"outside Stage-6 ownership: {outside}"


def mutant_preflight(root: Path) -> None:
    actions = load(
        root / "contracts/vnext/catalogs/generated/catalog-09-action-spec.json"
    )["descriptors"]
    ceremonies = load(
        root / "contracts/vnext/catalogs/generated/catalog-05-ceremony.json"
    )["descriptors"]
    selections = load(
        root / "contracts/vnext/public/recipe_selection_application_vectors.v1.json"
    )["vectors"]
    closure = load(root / "tests/fixtures/vnext/stage6/closure.v1.json")
    catalogs = load_catalogs(root)
    mutants = []

    mutated_actions = copy.deepcopy(actions)
    mutated_actions[93]["value"][0] = 93
    mutants.append((mutated_actions, ceremonies, selections, closure))

    mutated_selections = copy.deepcopy(selections)
    mutated_selections.pop()
    mutants.append((actions, ceremonies, mutated_selections, closure))

    mutated_closure = copy.deepcopy(closure)
    mutated_closure["action_outcomes"].pop()
    mutants.append((actions, ceremonies, selections, mutated_closure))

    for mutant in mutants:
        try:
            validate_documents(*mutant)
        except AssertionError:
            continue
        raise AssertionError("Stage-6 proof preflight accepted a known mutant")

    relation_mutants = []

    substituted = copy.deepcopy(catalogs)
    substituted[0]["primary_owner_relation"]["rows"][0][1:] = substituted[1][
        "primary_owner_relation"
    ]["rows"][0][1:]
    substituted[0]["primary_owner_relation"]["identity_envelope"][1] = substituted[0][
        "primary_owner_relation"
    ]["rows"]
    relation_mutants.append(substituted)

    duplicated = copy.deepcopy(catalogs)
    duplicated[5]["primary_owner_relation"]["rows"][1] = copy.deepcopy(
        duplicated[5]["primary_owner_relation"]["rows"][0]
    )
    duplicated[5]["primary_owner_relation"]["identity_envelope"][1] = duplicated[5][
        "primary_owner_relation"
    ]["rows"]
    relation_mutants.append(duplicated)

    missing = copy.deepcopy(catalogs)
    missing[8]["primary_owner_relation"]["rows"].pop()
    missing[8]["primary_owner_relation"]["identity_envelope"][1] = missing[8][
        "primary_owner_relation"
    ]["rows"]
    relation_mutants.append(missing)

    for mutant in relation_mutants:
        try:
            validate_owner_relations(mutant, closure)
        except AssertionError:
            continue
        raise AssertionError("Stage-6 owner-relation proof accepted a known mutant")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", default=BASE)
    parser.add_argument("--mutant-preflight", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    validate_closure(root)
    validate_sources(root)
    validate_ownership(root, args.base)
    if args.mutant_preflight:
        mutant_preflight(root)
    print("stage6 candidate: ok")


if __name__ == "__main__":
    main()
