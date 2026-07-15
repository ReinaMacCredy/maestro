#!/usr/bin/env python3
"""Build the inactive Stage-0 Public/Recipe/Capability identity closure."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
from typing import Any


REPO = Path(__file__).resolve().parents[4]
OUTPUT = REPO / "contracts/vnext/stage0/public-identity"
AUTHORITATIVE_ENV = "MAESTRO_AUTHORITATIVE_SOURCE"
EXPECTED_AUTHORITATIVE_HASHES = {
    ".maestro/cards/maestro-whole-flow-architecture-refoundation/design.md": "85787cfb4fb32eefe078adbf9ede66114b12c6304af10857bd676a1cd9875d18",
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


def sha256(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def head(major: int, value: int) -> bytes:
    if not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("canonical values require unsigned u64 integers")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def cbor(value: Any) -> bytes:
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
        return head(4, len(value)) + b"".join(cbor(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        raw = bytes.fromhex(value["bytes"])
        return head(2, len(raw)) + raw
    raise ValueError(f"value is outside the canonical public-identity subset: {value!r}")


def digest(domain: str, value: Any) -> str:
    return "sha256:" + sha256(cbor([domain, value]))


def raw_bytes(identity: str) -> dict[str, str]:
    if not identity.startswith("sha256:") or len(identity) != 71:
        raise ValueError("expected sha256 identity")
    return {"bytes": identity.removeprefix("sha256:")}


def load(relative: str) -> dict[str, Any]:
    return json.loads((REPO / relative).read_text(encoding="utf-8"))


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def authoritative_root() -> Path:
    raw = os.environ.get(AUTHORITATIVE_ENV)
    require(bool(raw), f"{AUTHORITATIVE_ENV} is required")
    root = Path(raw).expanduser().resolve()
    require(root.is_dir(), f"{AUTHORITATIVE_ENV} is not a directory: {root}")
    return root


def source_commitments() -> list[dict[str, str]]:
    authoritative = authoritative_root()
    rows: list[dict[str, str]] = []
    for relative, expected in EXPECTED_AUTHORITATIVE_HASHES.items():
        raw = (authoritative / relative).read_bytes()
        actual = sha256(raw)
        require(actual == expected, f"frozen authoritative input drifted: {relative}")
        rows.append({"path": f"authoritative:{relative}", "sha256": actual})
    require(len(REQUIRED_SOURCES) == len(set(REQUIRED_SOURCES)), "duplicate required source")
    for relative in REQUIRED_SOURCES:
        rows.append({"path": relative, "sha256": sha256((REPO / relative).read_bytes())})
    return rows


def schema_descriptor(schema: dict[str, Any]) -> dict[str, Any]:
    fields = [
        [position, field["name"], field["type"]]
        for position, field in enumerate(schema["ordered_fields"], start=1)
    ]
    variants = [
        [variant["tag"], variant["name"], variant["payload"]]
        for variant in schema["variants"]
    ]
    canonical = [
        schema["name"],
        schema["version"],
        schema["kind"],
        fields,
        variants,
        schema["cross_constraints"],
        schema["unknown_fields"],
    ]
    schema_id = digest("maestro.vnext.schema.v1", canonical)
    descriptor_id = digest(
        "maestro.vnext.descriptor-id.v1",
        [
            "maestro.vnext.public-identity.descriptor.v1",
            raw_bytes(schema_id),
            canonical,
        ],
    )
    return {
        "schema_name": schema["name"],
        "schema_version": schema["version"],
        "kind": schema["kind"],
        "field_names": [field["name"] for field in schema["ordered_fields"]],
        "variant_names": [variant["name"] for variant in schema["variants"]],
        "canonical_descriptor_value": canonical,
        "schema_id": schema_id,
        "descriptor_id": descriptor_id,
    }


def schema_descriptors(public: dict[str, Any]) -> list[dict[str, Any]]:
    definitions = public["schema_definitions"]
    require(public["schema_definition_count"] == len(definitions) == 79, "public schema closure drifted")
    require(len({row["name"] for row in definitions}) == 79, "duplicate public schema name")
    return [schema_descriptor(row) for row in sorted(definitions, key=lambda row: row["name"])]


def inactive(value: dict[str, Any]) -> bool:
    return (
        value.get("candidate_only") is True
        and value.get("runtime_activation") is False
        and value.get("runtime_registration") is False
    )


def semantic_snapshot() -> dict[str, Any]:
    public = load(PUBLIC_ARTIFACTS[0])
    selection = load(PUBLIC_ARTIFACTS[1])
    returns = load(PUBLIC_ARTIFACTS[2])
    job_recipe = load(PUBLIC_ARTIFACTS[3])
    route = load(PUBLIC_ARTIFACTS[4])
    capability = load(PUBLIC_ARTIFACTS[5])
    context = load(PUBLIC_ARTIFACTS[6])
    setup = load(PUBLIC_ARTIFACTS[7])
    activation = load(PUBLIC_ARTIFACTS[8])
    ledger = load(PUBLIC_ARTIFACTS[9])
    instruction_tree = load(EMBEDDED_SOURCES[0])
    mcp = load(EMBEDDED_SOURCES[3])
    recipe_catalog = load(EMBEDDED_SOURCES[4])
    inactive_sources = [
        public,
        selection,
        returns,
        job_recipe,
        route,
        capability,
        context,
        setup,
        activation,
        instruction_tree,
        mcp,
        recipe_catalog,
    ]
    require(all(inactive(value) for value in inactive_sources), "public identity source activated")
    require(all(public["prohibitions"].values()), "public prohibition opened")
    require(selection["vector_count"] == 30, "selection product drifted")
    require(returns["member_count"] == 30 and returns["application_outcome_vector_count"] == 196, "return closure drifted")
    require(job_recipe["positive_edges"] == 22 and job_recipe["negative_edges"] == 48, "JobRecipe relation drifted")
    require(route["row_count"] == 17, "JobRoute total map drifted")
    require(capability["job_method"]["positive"] == 19 and capability["job_method"]["negative"] == 100, "JobMethod relation drifted")
    require(context["host_profile_count"] == 2 and context["admitted_combined_closure_count"] == 750, "ContextBudget closure drifted")
    require(setup["action_row_count"] == 145 and setup["ceremony_row_count"] == 11, "Setup operation projection drifted")
    require(activation["evidence_catalog_bindings"]["publish_observation_tag"] == 39, "current activation Action tag drifted")
    require(len(mcp["tools"]) == 2 and not mcp["project_tools"], "MCP tool closure drifted")
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


def encoded_json(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def write(path: Path, raw: bytes, check: bool, mismatches: list[str]) -> None:
    if check:
        if not path.is_file() or path.read_bytes() != raw:
            mismatches.append(str(path.relative_to(REPO)))
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(raw)


def build(check: bool) -> dict[str, Any]:
    commitments = source_commitments()
    public = load(PUBLIC_ARTIFACTS[0])
    descriptors = schema_descriptors(public)
    snapshot = semantic_snapshot()
    resource_input_value = [1, [[row["path"], {"bytes": row["sha256"]}] for row in commitments]]
    resource_input_id = digest("maestro.vnext.public-identity-resource-input.v1", resource_input_value)
    manifest_value = [
        "maestro.vnext.public-identity.manifest.v1",
        raw_bytes(resource_input_id),
        [raw_bytes(item["descriptor_id"]) for item in descriptors],
    ]
    manifest_id = digest("maestro.vnext.manifest-id.v1", manifest_value)
    closure_value = [
        1,
        raw_bytes(resource_input_id),
        raw_bytes(manifest_id),
        [raw_bytes(item["schema_id"]) for item in descriptors],
    ]
    closure_id = digest("maestro.vnext.public-identity-closure.v1", closure_value)
    output = {
        "schema": "maestro.vnext.stage0-public-identity-closure.v1",
        "candidate_only": True,
        "runtime_activation": False,
        "runtime_registration": False,
        "closure_id": closure_id,
        "source_input_commitment": {
            "resource_input_id": resource_input_id,
            "canonical_value": resource_input_value,
            "inputs": commitments,
        },
        "schema_descriptors": descriptors,
        "manifest": {
            "manifest_id": manifest_id,
            "canonical_value": manifest_value,
            "descriptor_count": len(descriptors),
        },
        "semantic_snapshot": snapshot,
        "canonical_closure_value": closure_value,
    }
    outputs = {
        OUTPUT / "public-identity-closure.v1.json": encoded_json(output),
        OUTPUT / "public-identity-closure.v1.cbor": cbor(closure_value),
        OUTPUT / "public-identity-closure-input.v1.json": encoded_json({"closure_value": closure_value}),
        OUTPUT / "schema-descriptors.v1.json": encoded_json(
            {
                "schema": "maestro.vnext.stage0-public-identity-schema-descriptors.v1",
                "candidate_only": True,
                "runtime_activation": False,
                "runtime_registration": False,
                "descriptors": descriptors,
            }
        ),
        OUTPUT / "resource-input-commitment.v1.json": encoded_json(
            {
                "schema": "maestro.vnext.stage0-public-identity-resource-input.v1",
                "resource_input_id": resource_input_id,
                "canonical_value": resource_input_value,
                "inputs": commitments,
            }
        ),
    }
    mismatches: list[str] = []
    for path, raw in outputs.items():
        write(path, raw, check, mismatches)
    return {
        "schema": "maestro.vnext.stage0-public-identity-build-receipt.v1",
        "mode": "check" if check else "write",
        "status": "pass" if not mismatches else "fail",
        "mismatches": mismatches,
        "closure_id": closure_id,
        "manifest_id": manifest_id,
        "resource_input_id": resource_input_id,
        "schema_descriptor_count": len(descriptors),
        "source_input_count": len(commitments),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    receipt = build(args.check)
    print(json.dumps(receipt, sort_keys=True))
    return 0 if receipt["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
