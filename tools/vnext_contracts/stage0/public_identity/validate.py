#!/usr/bin/env python3
"""Independently validate the Stage-0 public identity closure and its semantics."""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path
from typing import Any


REPO = Path(__file__).resolve().parents[4]
DEFAULT_ARTIFACT_ROOT = REPO / "contracts/vnext/stage0/public-identity"
AUTHORITATIVE_ENV = "MAESTRO_AUTHORITATIVE_SOURCE"
EXPECTED_AUTHORITATIVE_HASHES = {
    ".maestro/cards/maestro-whole-flow-architecture-refoundation/design.md": "16a2f079f6ebf3dd3a2fb1a171cd0c6811203fe5f84dda73a7e2e91f67d6f9f7",
    ".maestro/cards/maestro-whole-flow-architecture-refoundation/decisions.yaml": "1f97e67b156d5a17d13b94ff955ad17efeb3bb71a4b74b1aec14e20dac1100dd",
    ".maestro/cards/maestro-whole-flow-architecture-refoundation/card.yaml": "2cdf1f74843a6eca926ff3bc48e060654350e6a03b65342f8d7be48d111379b4",
}
PUBLIC_ARTIFACTS = [
    "contracts/vnext/public/public_contracts.v1.json",
    "contracts/vnext/public/recipe_selection_application_vectors.v1.json",
    "contracts/vnext/public/recipe_return_reasons.v1.json",
    "contracts/vnext/public/job_recipe_eligibility_vectors.v1.json",
    "contracts/vnext/public/job_route_contract.v1.json",
    "contracts/vnext/public/capability_method_contracts.v1.json",
    "contracts/vnext/public/context_budget_profiles.v1.json",
    "contracts/vnext/public/setup_operation_compatibility.v1.json",
    "contracts/vnext/public/skill_activation_contract.v1.json",
    "contracts/vnext/public/v1_skill_ledger.v1.json",
    "contracts/vnext/public/census_admission_report.v1.json",
    "contracts/vnext/public/physical_census.commitment.v1.json",
    "contracts/vnext/public/physical_census.historical-output.txt",
    "contracts/vnext/public/direct_consumers.c325.v1.json",
    "contracts/vnext/public/embedded_resources.e204.v1.json",
    "contracts/vnext/public/historical_source_coverage_inputs.v1.json",
    "contracts/vnext/public/bundle_membership_inputs.v1.json",
]
EMBEDDED_SOURCES = [
    "embedded/vnext/capability/instruction-tree.v1.json",
    "embedded/vnext/capability/context-budget/agents-compatible-cli.v1.json",
    "embedded/vnext/capability/context-budget/claude-code.v1.json",
    "embedded/vnext/adapter/mcp-tools.v1.json",
    "embedded/vnext/orchestration/recipe-catalog.v1.json",
]
CATALOG_SOURCES = [
    "contracts/vnext/catalogs/generated/catalog-profile-grammar-v1.json",
    "contracts/vnext/catalogs/generated/catalog-01-observation.json",
    "contracts/vnext/catalogs/generated/catalog-02-effect.json",
    "contracts/vnext/catalogs/generated/catalog-05-ceremony.json",
    "contracts/vnext/catalogs/generated/catalog-09-action-spec.json",
]
RECIPE_IDS = [
    "bounded-continuation",
    "conflict-handoff",
    "design-relay",
    "fanout",
    "intake-triage",
    "learning",
    "setup",
    "ship",
    "synthesize",
    "wayfinding",
]
RECIPE_SOURCES = [
    *(f"embedded/vnext/orchestration/recipes/{recipe}/manifest.v1.json" for recipe in RECIPE_IDS),
    *(
        f"embedded/vnext/orchestration/profiles/bounded-continuation/{profile}.v1.json"
        for profile in ("attended", "unattended")
    ),
]
REQUIRED_SOURCES = [*PUBLIC_ARTIFACTS, *EMBEDDED_SOURCES, *CATALOG_SOURCES, *RECIPE_SOURCES]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def source(relative: str) -> dict[str, Any]:
    return load(REPO / relative)


def sha256(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def head(major: int, value: int) -> bytes:
    if not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("non-u64 canonical integer")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def encode(value: Any) -> bytes:
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
    raise ValueError("unsupported canonical-CBOR value")


def identity(domain: str, value: Any) -> str:
    return "sha256:" + sha256(encode([domain, value]))


def raw_bytes(identity_value: str) -> dict[str, str]:
    require(identity_value.startswith("sha256:") and len(identity_value) == 71, "invalid identity")
    return {"bytes": identity_value[7:]}


def artifact_root() -> Path:
    raw = os.environ.get("PUBLIC_IDENTITY_ARTIFACT_ROOT")
    return Path(raw).expanduser().resolve() if raw else DEFAULT_ARTIFACT_ROOT


def authoritative_root() -> Path:
    raw = os.environ.get(AUTHORITATIVE_ENV)
    require(bool(raw), f"{AUTHORITATIVE_ENV} is required")
    root = Path(raw).expanduser().resolve()
    require(root.is_dir(), f"{AUTHORITATIVE_ENV} is not a directory: {root}")
    return root


def expected_source_rows() -> list[dict[str, str]]:
    authoritative = authoritative_root()
    rows: list[dict[str, str]] = []
    for relative, expected in EXPECTED_AUTHORITATIVE_HASHES.items():
        actual = sha256((authoritative / relative).read_bytes())
        require(actual == expected, f"authoritative source drift: {relative}")
        rows.append({"path": f"authoritative:{relative}", "sha256": actual})
    require(len(REQUIRED_SOURCES) == len(set(REQUIRED_SOURCES)), "duplicate required source")
    for relative in REQUIRED_SOURCES:
        rows.append({"path": relative, "sha256": sha256((REPO / relative).read_bytes())})
    return rows


def validate_sources(closure: dict[str, Any]) -> None:
    expected = expected_source_rows()
    commitment = closure["source_input_commitment"]
    require(commitment["inputs"] == expected, "source input set/order/hash mismatch")
    canonical = [1, [[row["path"], {"bytes": row["sha256"]}] for row in expected]]
    require(commitment["canonical_value"] == canonical, "resource canonical value mismatch")
    require(
        commitment["resource_input_id"]
        == identity("maestro.vnext.public-identity-resource-input.v1", canonical),
        "resource input identity mismatch",
    )


def canonical_schema_value(schema: dict[str, Any]) -> list[Any]:
    return [
        schema["name"],
        schema["version"],
        schema["kind"],
        [
            [position, field["name"], field["type"]]
            for position, field in enumerate(schema["ordered_fields"], start=1)
        ],
        [
            [variant["tag"], variant["name"], variant["payload"]]
            for variant in schema["variants"]
        ],
        schema["cross_constraints"],
        schema["unknown_fields"],
    ]


def contains_bytes(value: Any, payload: str) -> bool:
    if isinstance(value, dict):
        return value.get("bytes") == payload or any(contains_bytes(item, payload) for item in value.values())
    if isinstance(value, list):
        return any(contains_bytes(item, payload) for item in value)
    return False


def validate_descriptors(closure: dict[str, Any], root: Path) -> None:
    public = source(PUBLIC_ARTIFACTS[0])
    definitions = sorted(public["schema_definitions"], key=lambda row: row["name"])
    require(public["schema_definition_count"] == len(definitions) == 79, "public schema count mismatch")
    descriptors = closure["schema_descriptors"]
    require(len(descriptors) == 79, "identity descriptor count mismatch")
    require([row["schema_name"] for row in descriptors] == [row["name"] for row in definitions], "descriptor name/order mismatch")
    for descriptor, schema in zip(descriptors, definitions):
        canonical = canonical_schema_value(schema)
        require(descriptor["schema_version"] == schema["version"], "descriptor version mismatch")
        require(descriptor["kind"] == schema["kind"], "descriptor kind mismatch")
        require(descriptor["field_names"] == [row["name"] for row in schema["ordered_fields"]], "descriptor field names mismatch")
        require(descriptor["variant_names"] == [row["name"] for row in schema["variants"]], "descriptor variant names mismatch")
        require(descriptor["canonical_descriptor_value"] == canonical, f"descriptor canonical value mismatch: {schema['name']}")
        require(schema["unknown_fields"] == "reject", f"schema accepts unknown fields: {schema['name']}")
        require(not contains_bytes(canonical, descriptor["schema_id"][7:]), "containing-ID backreference")
        schema_id = identity("maestro.vnext.schema.v1", canonical)
        require(descriptor["schema_id"] == schema_id, f"SchemaId mismatch: {schema['name']}")
        descriptor_id = identity(
            "maestro.vnext.descriptor-id.v1",
            [
                "maestro.vnext.public-identity.descriptor.v1",
                raw_bytes(schema_id),
                canonical,
            ],
        )
        require(descriptor["descriptor_id"] == descriptor_id, f"DescriptorId mismatch: {schema['name']}")
    mirror = load(root / "schema-descriptors.v1.json")
    require(
        mirror
        == {
            "schema": "maestro.vnext.stage0-public-identity-schema-descriptors.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "descriptors": descriptors,
        },
        "descriptor mirror mismatch",
    )


def inactive(value: dict[str, Any]) -> bool:
    return (
        value.get("candidate_only") is True
        and value.get("runtime_activation") is False
        and value.get("runtime_registration") is False
    )


def expected_semantic_snapshot() -> dict[str, Any]:
    public = source(PUBLIC_ARTIFACTS[0])
    selection = source(PUBLIC_ARTIFACTS[1])
    returns = source(PUBLIC_ARTIFACTS[2])
    job_recipe = source(PUBLIC_ARTIFACTS[3])
    route = source(PUBLIC_ARTIFACTS[4])
    capability = source(PUBLIC_ARTIFACTS[5])
    context = source(PUBLIC_ARTIFACTS[6])
    setup = source(PUBLIC_ARTIFACTS[7])
    activation = source(PUBLIC_ARTIFACTS[8])
    ledger = source(PUBLIC_ARTIFACTS[9])
    instruction_tree = source(EMBEDDED_SOURCES[0])
    mcp = source(EMBEDDED_SOURCES[3])
    recipe_catalog = source(EMBEDDED_SOURCES[4])
    for value in [public, selection, returns, job_recipe, route, capability, context, setup, activation, instruction_tree, mcp, recipe_catalog]:
        require(inactive(value), "semantic source activated")
    require(all(public["prohibitions"].values()), "public prohibition opened")
    require(public["schema_definition_count"] == 79, "schema total mismatch")
    require(selection["vector_count"] == 30 and len(selection["vectors"]) == 30, "selection product mismatch")
    require(returns["member_count"] == 30 and returns["application_outcome_vector_count"] == 196, "return taxonomy mismatch")
    require(
        job_recipe["positive_edges"] == 22
        and job_recipe["negative_edges"] == 48
        and job_recipe["admitted_application_count"] == 66
        and job_recipe["refused_application_count"] == 144,
        "JobRecipe closure mismatch",
    )
    require(route["row_count"] == 17 and route["selected_count"] == 10 and route["ambiguous_count"] == 2 and route["blocked_count"] == 5, "JobRoute partition mismatch")
    require(capability["job_method"]["positive"] == 19 and capability["job_method"]["negative"] == 100, "JobMethod relation mismatch")
    require(capability["review"]["admitted_subset_shapes"] == 13 and capability["review"]["refused_subset_shapes"] == 27, "Review subset closure mismatch")
    require(context["host_profile_count"] == 2 and context["admitted_combined_closure_count"] == 750 and context["universal_product_cap"] is False, "ContextBudget evidence mismatch")
    require(setup["action_row_count"] == 145 and setup["ceremony_row_count"] == 11, "Setup operation projection mismatch")
    require(activation["capability_outcomes"] == ["Selected"] and all("Refused" not in row for row in activation["recipe_resolution"]), "invalid activation acquisition union")
    require(activation["evidence_catalog_bindings"]["skill_activation_tag"] == 12, "SkillActivation Observation mismatch")
    require(activation["evidence_catalog_bindings"]["publish_observation_tag"] == 39, "current PublishObservation mismatch")
    require(activation["predecessor_non_current_evidence"]["publish_observation_tag"] == 30 and activation["predecessor_non_current_evidence"]["current_selector"] is False, "predecessor Action evidence promoted")
    require(len(mcp["tools"]) == 2 and not mcp["project_tools"], "MCP tool closure mismatch")
    transactions = [
        {
            "name": row["name"],
            "descriptor_id": row["descriptor_id"],
            "compatible_setup_modes": row["compatible_setup_modes"],
        }
        for row in setup["action_rows"]
        if row["name"] in {"RecoverDistributionTransaction", "RollbackDistributionTransaction"}
    ]
    require(len(transactions) == 2 and all(not row["compatible_setup_modes"] for row in transactions), "transaction Operation established Setup mode")
    census = source("contracts/vnext/public/census_admission_report.v1.json")
    physical = source("contracts/vnext/public/physical_census.commitment.v1.json")
    require(census["stage0_historical_evidence_admission"] == "pass", "Stage-0 historical census admission failed")
    require(census["stage11_live_migration_admission"] == "blocked_pending_recensus", "Stage-11 recensus gate opened")
    require(physical["historical_attested_receipt"]["node_count"] == 28102, "historical 28,102 commitment drifted")
    require(physical["current_live_rows_equal_historical_snapshot"] is False, "current census equality falsely claimed")
    return {
        "schema_descriptor_names": sorted(row["name"] for row in public["schema_definitions"]),
        "closed_totals": public["closed_totals"],
        "semantic_artifacts": public["semantic_artifacts"],
        "recipe_catalog": [row["id"] for row in recipe_catalog["recipes"]],
        "selection": {
            "primary_axis": selection["primary_axis"],
            "continuation_axis": selection["continuation_axis"],
            "vector_count": selection["vector_count"],
            "commitments": [
                [
                    row["enumeration_ordinal_not_identity"],
                    row["shape"],
                    row["packet_recipe_binding_fixture"]["selection_request_hash"],
                    row["packet_recipe_binding_fixture"]["recipe_application_hash"],
                ]
                for row in selection["vectors"]
            ],
            "fallback": False,
        },
        "returns": {
            "member_count": returns["member_count"],
            "manifest_subset_count": returns["manifest_subset_count"],
            "membership_matrix": returns["membership_matrix"],
            "compatibility_matrix": returns["compatibility_matrix"],
            "application_outcome_vector_count": returns["application_outcome_vector_count"],
        },
        "job_recipe": {
            "rows": job_recipe["rows"],
            "positive_edges": job_recipe["positive_edges"],
            "negative_edges": job_recipe["negative_edges"],
            "application_vector_count": job_recipe["application_vector_count"],
            "admitted_application_count": job_recipe["admitted_application_count"],
            "refused_application_count": job_recipe["refused_application_count"],
            "partial_fallback": False,
        },
        "job_route": {
            "rows": route["rows"],
            "precedence": route["precedence"],
            "guidance_is_separate_from_packet": route["guidance_is_separate_from_packet"],
            "owns_recommendation": route["owns_recommendation"],
        },
        "capability": {
            "skill_ids": capability["skill_ids"],
            "jobs": capability["jobs"],
            "direct_methods": capability["direct_methods"],
            "instruction_resources": instruction_tree["logical_paths"],
            "job_method": capability["job_method"],
            "review": capability["review"],
            "tdd": capability["tdd"],
            "research_examples": capability["research_examples"],
        },
        "context_budget": context,
        "setup": {
            "catalog_bindings": setup["catalog_bindings"],
            "action_family_counts": setup["action_family_counts"],
            "action_row_count": setup["action_row_count"],
            "ceremony_row_count": setup["ceremony_row_count"],
            "transaction_non_modes": transactions,
        },
        "skill_activation": activation,
        "skill_ledger": {
            "row_count": len(ledger["rows"]),
            "disposition_totals": ledger["disposition_totals"],
            "semantic_destination_count": ledger["semantic_destination_count"],
        },
        "global_mcp": mcp,
        "runtime_active": False,
        "aliases": False,
        "authority": False,
        "recommendation_or_next_action": False,
    }


def validate_semantics(closure: dict[str, Any]) -> None:
    require(
        closure["candidate_only"] is True
        and closure["runtime_activation"] is False
        and closure["runtime_registration"] is False,
        "identity closure activated",
    )
    require(closure["semantic_snapshot"] == expected_semantic_snapshot(), "semantic snapshot mismatch")


def validate_identities(closure: dict[str, Any], root: Path) -> None:
    descriptors = closure["schema_descriptors"]
    commitment = closure["source_input_commitment"]
    manifest = closure["manifest"]
    expected_manifest_value = [
        "maestro.vnext.public-identity.manifest.v1",
        raw_bytes(commitment["resource_input_id"]),
        [raw_bytes(row["descriptor_id"]) for row in descriptors],
    ]
    require(manifest["descriptor_count"] == len(descriptors) == 79, "manifest descriptor count mismatch")
    require(manifest["canonical_value"] == expected_manifest_value, "manifest binding mismatch")
    require(manifest["manifest_id"] == identity("maestro.vnext.manifest-id.v1", expected_manifest_value), "ManifestId mismatch")
    expected_closure_value = [
        1,
        raw_bytes(commitment["resource_input_id"]),
        raw_bytes(manifest["manifest_id"]),
        [raw_bytes(row["schema_id"]) for row in descriptors],
    ]
    require(closure["canonical_closure_value"] == expected_closure_value, "closure binding mismatch")
    require(closure["closure_id"] == identity("maestro.vnext.public-identity-closure.v1", expected_closure_value), "closure identity mismatch")
    require((root / "public-identity-closure.v1.cbor").read_bytes() == encode(expected_closure_value), "closure CBOR mismatch")
    require(load(root / "public-identity-closure-input.v1.json") == {"closure_value": expected_closure_value}, "closure encoder input mismatch")
    require(
        load(root / "resource-input-commitment.v1.json")
        == {
            "schema": "maestro.vnext.stage0-public-identity-resource-input.v1",
            "resource_input_id": commitment["resource_input_id"],
            "canonical_value": commitment["canonical_value"],
            "inputs": commitment["inputs"],
        },
        "resource input mirror mismatch",
    )


def main() -> int:
    try:
        root = artifact_root()
        closure = load(root / "public-identity-closure.v1.json")
        validate_sources(closure)
        validate_descriptors(closure, root)
        validate_semantics(closure)
        validate_identities(closure, root)
        print(
            json.dumps(
                {
                    "schema": "maestro.vnext.stage0-public-identity-validation-receipt.v1",
                    "status": "pass",
                    "closure_id": closure["closure_id"],
                    "manifest_id": closure["manifest"]["manifest_id"],
                    "resource_input_id": closure["source_input_commitment"]["resource_input_id"],
                    "schema_descriptor_count": len(closure["schema_descriptors"]),
                    "source_input_count": len(closure["source_input_commitment"]["inputs"]),
                    "historical_census_count": 28102,
                    "current_live_equality": False,
                    "stage11_recensus_required": True,
                },
                sort_keys=True,
            )
        )
    except (KeyError, TypeError, ValueError, OSError, json.JSONDecodeError) as error:
        print(f"public identity validation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
