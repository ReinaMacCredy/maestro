#!/usr/bin/env python3
"""Prove Python/Ruby encoding equality and semantic mutant rejection."""

from __future__ import annotations

import copy
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any, Callable


REPO = Path(__file__).resolve().parents[4]
TOOLS = Path(__file__).resolve().parent
ARTIFACT = REPO / "contracts/vnext/stage0/public-identity"
MIRROR_FILES = [
    "public-identity-closure.v1.cbor",
    "public-identity-closure-input.v1.json",
    "schema-descriptors.v1.json",
    "resource-input-commitment.v1.json",
]


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def run(command: list[str], env: dict[str, str]) -> str:
    return subprocess.run(command, check=True, text=True, capture_output=True, env=env).stdout.strip()


def descriptor(value: dict[str, Any], name: str) -> dict[str, Any]:
    return next(item for item in value["schema_descriptors"] if item["schema_name"] == name)


def snapshot(value: dict[str, Any]) -> dict[str, Any]:
    return value["semantic_snapshot"]


def mutate_descriptor_type(value: dict[str, Any]) -> None:
    descriptor(value, "AgentPacketV1")["canonical_descriptor_value"][3][1][2] = "String"


def mutate_descriptor_field(value: dict[str, Any]) -> None:
    descriptor(value, "RecipeSelectionRequestV1")["canonical_descriptor_value"][3].append(
        [5, "caller_frontier_ref", "FrontierRefV1"]
    )


def mutate_descriptor_variant(value: dict[str, Any]) -> None:
    descriptor(value, "SkillActivationRecipeResolutionV1")["canonical_descriptor_value"][4].append(
        [3, "Refused", "RecipeAdmissionRefusalV1"]
    )


def mutate_descriptor_constraint(value: dict[str, Any]) -> None:
    descriptor(value, "PacketRecipeBindingV1")["canonical_descriptor_value"][5].pop()


def mutate_descriptor_unknown_fields(value: dict[str, Any]) -> None:
    descriptor(value, "OperationRequestV1")["canonical_descriptor_value"][6] = "allow"


def mutate_missing_descriptor(value: dict[str, Any]) -> None:
    value["schema_descriptors"].pop()


def mutate_source_hash(value: dict[str, Any]) -> None:
    value["source_input_commitment"]["inputs"][3]["sha256"] = "00" * 32


def mutate_source_path(value: dict[str, Any]) -> None:
    value["source_input_commitment"]["inputs"][4]["path"] = "contracts/vnext/public/other.json"


def mutate_selection_duplicate(value: dict[str, Any]) -> None:
    snapshot(value)["selection"]["commitments"][-1] = copy.deepcopy(
        snapshot(value)["selection"]["commitments"][0]
    )


def mutate_selection_zero_hash(value: dict[str, Any]) -> None:
    snapshot(value)["selection"]["commitments"][0][2] = "sha256:" + "0" * 64


def mutate_selection_fallback(value: dict[str, Any]) -> None:
    snapshot(value)["selection"]["fallback"] = True


def mutate_return_vectors(value: dict[str, Any]) -> None:
    snapshot(value)["returns"]["application_outcome_vector_count"] = 195


def mutate_job_recipe_edge(value: dict[str, Any]) -> None:
    snapshot(value)["job_recipe"]["rows"][0]["eligible_jobs"].append("Setup")


def mutate_job_recipe_fallback(value: dict[str, Any]) -> None:
    snapshot(value)["job_recipe"]["partial_fallback"] = True


def mutate_route_reason(value: dict[str, Any]) -> None:
    snapshot(value)["job_route"]["rows"][0]["reason"] = "StepRunnable"


def mutate_route_recommendation(value: dict[str, Any]) -> None:
    snapshot(value)["job_route"]["owns_recommendation"] = True


def mutate_job_method(value: dict[str, Any]) -> None:
    snapshot(value)["capability"]["job_method"]["positive"] = 18


def mutate_review_subset(value: dict[str, Any]) -> None:
    snapshot(value)["capability"]["review"]["admitted_subset_shapes"] = 14


def mutate_tdd_child(value: dict[str, Any]) -> None:
    snapshot(value)["capability"]["tdd"]["children"].pop()


def mutate_research_example(value: dict[str, Any]) -> None:
    snapshot(value)["capability"]["research_examples"]["rows"][0]["admitted"] = True


def mutate_context_universal(value: dict[str, Any]) -> None:
    snapshot(value)["context_budget"]["universal_product_cap"] = True


def mutate_context_count(value: dict[str, Any]) -> None:
    snapshot(value)["context_budget"]["admitted_combined_closure_count"] = 749


def mutate_setup_transaction(value: dict[str, Any]) -> None:
    snapshot(value)["setup"]["transaction_non_modes"][0]["compatible_setup_modes"] = ["Rollback"]


def mutate_setup_manifest(value: dict[str, Any]) -> None:
    snapshot(value)["setup"]["catalog_bindings"]["action_spec_manifest_id"] = "0" * 64


def mutate_activation_ambiguous(value: dict[str, Any]) -> None:
    snapshot(value)["skill_activation"]["selected_route_reason_set"].append("ConflictingReadIntent")


def mutate_activation_refused(value: dict[str, Any]) -> None:
    snapshot(value)["skill_activation"]["recipe_resolution"].append("PacketAdmission.Refused")


def mutate_activation_current_tag(value: dict[str, Any]) -> None:
    snapshot(value)["skill_activation"]["evidence_catalog_bindings"]["publish_observation_tag"] = 30


def mutate_activation_predecessor(value: dict[str, Any]) -> None:
    snapshot(value)["skill_activation"]["predecessor_non_current_evidence"]["current_selector"] = True


def mutate_activation_domain(value: dict[str, Any]) -> None:
    activation = snapshot(value)["skill_activation"]
    activation["commitment_domains"]["payload"] = activation["commitment_domains"]["subject"]


def mutate_activation_publication(value: dict[str, Any]) -> None:
    snapshot(value)["skill_activation"]["publication"]["passive_writes"] = 1


def mutate_activation_legacy(value: dict[str, Any]) -> None:
    snapshot(value)["skill_activation"]["legacy_import"]["dispositions"].append("MappedNormative")


def mutate_mcp_third_tool(value: dict[str, Any]) -> None:
    snapshot(value)["global_mcp"]["tools"].append(copy.deepcopy(snapshot(value)["global_mcp"]["tools"][0]))


def mutate_mcp_project(value: dict[str, Any]) -> None:
    snapshot(value)["global_mcp"]["project_tools"].append("maestro_packet")


def mutate_mcp_cursor(value: dict[str, Any]) -> None:
    snapshot(value)["global_mcp"]["tools"][1]["cursor_contract"] = "optional"


def mutate_runtime_activation(value: dict[str, Any]) -> None:
    value["runtime_activation"] = True


def mutate_alias(value: dict[str, Any]) -> None:
    snapshot(value)["aliases"] = True


def mutate_manifest_id(value: dict[str, Any]) -> None:
    value["manifest"]["manifest_id"] = "sha256:" + "f" * 64


def mutate_closure_id(value: dict[str, Any]) -> None:
    value["closure_id"] = "sha256:" + "e" * 64


MUTANTS: dict[str, Callable[[dict[str, Any]], None]] = {
    "descriptor_untyped_field": mutate_descriptor_type,
    "descriptor_caller_frontier": mutate_descriptor_field,
    "descriptor_refused_variant": mutate_descriptor_variant,
    "descriptor_missing_constraint": mutate_descriptor_constraint,
    "descriptor_unknown_fields_allowed": mutate_descriptor_unknown_fields,
    "descriptor_missing_member": mutate_missing_descriptor,
    "source_stale_hash": mutate_source_hash,
    "source_wrong_path": mutate_source_path,
    "selection_duplicate_shape": mutate_selection_duplicate,
    "selection_zero_hash": mutate_selection_zero_hash,
    "selection_present_absent_fallback": mutate_selection_fallback,
    "return_vector_count": mutate_return_vectors,
    "job_recipe_extra_edge": mutate_job_recipe_edge,
    "job_recipe_partial_fallback": mutate_job_recipe_fallback,
    "route_wrong_reason": mutate_route_reason,
    "route_as_recommendation": mutate_route_recommendation,
    "capability_wrong_job_method": mutate_job_method,
    "review_wrong_subset": mutate_review_subset,
    "tdd_missing_child": mutate_tdd_child,
    "research_extra_example": mutate_research_example,
    "context_universal_cap": mutate_context_universal,
    "context_wrong_count": mutate_context_count,
    "setup_transaction_mode": mutate_setup_transaction,
    "setup_mixed_manifest": mutate_setup_manifest,
    "activation_ambiguous_route": mutate_activation_ambiguous,
    "activation_refused_recipe": mutate_activation_refused,
    "activation_predecessor_tag_as_current": mutate_activation_current_tag,
    "activation_predecessor_promoted": mutate_activation_predecessor,
    "activation_shared_commitment_domain": mutate_activation_domain,
    "activation_passive_write": mutate_activation_publication,
    "activation_legacy_normative": mutate_activation_legacy,
    "mcp_third_tool": mutate_mcp_third_tool,
    "mcp_project_tool": mutate_mcp_project,
    "mcp_cursor_parity": mutate_mcp_cursor,
    "runtime_activation": mutate_runtime_activation,
    "alias_injection": mutate_alias,
    "manifest_identity": mutate_manifest_id,
    "closure_identity": mutate_closure_id,
}


def materialize_mutant(root: Path, value: dict[str, Any]) -> None:
    (root / "public-identity-closure.v1.json").write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    descriptor_mirror = load(root / "schema-descriptors.v1.json")
    descriptor_mirror["descriptors"] = value["schema_descriptors"]
    (root / "schema-descriptors.v1.json").write_text(
        json.dumps(descriptor_mirror, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    resource_mirror = load(root / "resource-input-commitment.v1.json")
    resource_mirror["resource_input_id"] = value["source_input_commitment"]["resource_input_id"]
    resource_mirror["canonical_value"] = value["source_input_commitment"]["canonical_value"]
    resource_mirror["inputs"] = value["source_input_commitment"]["inputs"]
    (root / "resource-input-commitment.v1.json").write_text(
        json.dumps(resource_mirror, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    authoritative = {
        key: value
        for key, value in os.environ.items()
        if key.startswith("MAESTRO_AUTHORITATIVE_")
    }
    if not authoritative.get("MAESTRO_AUTHORITATIVE_SOURCE"):
        raise SystemExit("MAESTRO_AUTHORITATIVE_SOURCE is required")
    env = {
        **authoritative,
        "HOME": tempfile.gettempdir(),
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "PYTHONDONTWRITEBYTECODE": "1",
        "RUBYOPT": "",
    }
    run([sys.executable, str(TOOLS / "build.py"), "--check"], env)
    validation = json.loads(run([sys.executable, str(TOOLS / "validate.py")], env))
    ruby = json.loads(
        run(
            ["/usr/bin/ruby", str(TOOLS / "encode.rb"), str(ARTIFACT / "public-identity-closure-input.v1.json")],
            env,
        )
    )
    cbor = (ARTIFACT / "public-identity-closure.v1.cbor").read_bytes()
    if ruby["hex"] != cbor.hex() or ruby["sha256"] != hashlib.sha256(cbor).hexdigest():
        raise SystemExit("independent Ruby encoder disagreed with canonical closure bytes")

    original = load(ARTIFACT / "public-identity-closure.v1.json")
    rejected: list[str] = []
    with tempfile.TemporaryDirectory(prefix="maestro-public-identity-mutants-") as temp:
        temp_root = Path(temp)
        for filename in MIRROR_FILES:
            shutil.copy2(ARTIFACT / filename, temp_root / filename)
        for name, mutate in MUTANTS.items():
            value = copy.deepcopy(original)
            mutate(value)
            materialize_mutant(temp_root, value)
            result = subprocess.run(
                [sys.executable, str(TOOLS / "validate.py")],
                text=True,
                capture_output=True,
                env={**env, "PUBLIC_IDENTITY_ARTIFACT_ROOT": str(temp_root)},
            )
            if result.returncode == 0:
                raise SystemExit(f"semantic validator accepted mutant: {name}")
            rejected.append(name)

    receipt = {
        "schema": "maestro.vnext.stage0-public-identity-encoder-receipt.v1",
        "closure_id": validation["closure_id"],
        "manifest_id": validation["manifest_id"],
        "resource_input_id": validation["resource_input_id"],
        "schema_descriptor_count": validation["schema_descriptor_count"],
        "source_input_count": validation["source_input_count"],
        "python_semantic_validator": "pass",
        "ruby_encoder": "pass",
        "encoder_equality": "pass",
        "encoder_equality_scope": "byte_exact",
        "mutants": {"count": len(rejected), "rejected": rejected},
        "historical_census_count": validation["historical_census_count"],
        "current_live_equality": validation["current_live_equality"],
        "stage11_recensus_required": validation["stage11_recensus_required"],
    }
    receipt_bytes = (json.dumps(receipt, indent=2, sort_keys=True) + "\n").encode()
    receipt_path = ARTIFACT / "encoder-receipt.v1.json"
    if args.check:
        if not receipt_path.is_file() or receipt_path.read_bytes() != receipt_bytes:
            raise SystemExit("public-identity encoder receipt drifted or is missing")
    else:
        receipt_path.write_bytes(receipt_bytes)
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
