#!/usr/bin/env python3
"""Independent fail-closed validator for the Stage-0 public semantic closure."""

from __future__ import annotations

import argparse
import copy
import hashlib
import itertools
import json
from collections import Counter
from pathlib import Path
from typing import Callable


RECIPE_IDS = [
    "bounded-continuation", "conflict-handoff", "design-relay", "fanout",
    "intake-triage", "learning", "setup", "ship", "synthesize", "wayfinding",
]
PRIMARY_RECIPE_IDS = RECIPE_IDS[1:]
PROFILES = ["attended", "unattended"]
OUTCOMES = ["NotApplicable", "RestrictiveAdvice", "HardStop"]
JOBS = ["Setup", "Research", "Design", "Review", "Execute", "Recover", "Adapt"]
METHOD_ROWS = {
    "Setup": [],
    "Research": [],
    "Design": ["DDD", "DomainModel", "Grilling", "PRD", "ArchitectureDeepening", "Probe", "GenerateAndFilter", "QABaseline"],
    "Review": ["Audit", "ArchitectureReview", "AdversarialReview", "GenerateAndFilter", "QAReplay", "CloseReview", "Verification"],
    "Execute": ["TDD", "Simplify"],
    "Recover": [],
    "Adapt": ["ExtensionLaw", "GenerateAndFilter"],
}
JOB_RECIPE = {
    "bounded-continuation": JOBS,
    "conflict-handoff": ["Execute", "Recover"],
    "design-relay": ["Design"],
    "fanout": ["Execute"],
    "intake-triage": ["Research", "Design"],
    "learning": ["Review", "Recover", "Adapt"],
    "setup": ["Setup"],
    "ship": ["Execute"],
    "synthesize": ["Execute", "Recover"],
    "wayfinding": ["Research", "Design"],
}
ROUTE_SELECTED = {
    "BootstrapRequired": "Setup", "SetupRequired": "Setup",
    "ExplicitResearch": "Research", "ContextUnknown": "Research",
    "DesignRequired": "Design", "ExplicitReview": "Review",
    "ReviewRequired": "Review", "StepRunnable": "Execute",
    "RecoveryRequired": "Recover", "ExtensionRequired": "Adapt",
}
ROUTE_AMBIGUOUS = ["ConflictingReadIntent", "ConflictingRouteFacts"]
ROUTE_BLOCKED = ["MissingRouteMapping", "CanonicalWaitOrStop", "StaleRouteInput", "IncompatibleRouteInput", "UnavailableResourceClosure"]
SETUP_MODES = ["Install", "Adopt", "Migrate", "Update", "Repair", "Rollback", "Uninstall"]
SETUP_BLOCKED = [
    "UnsupportedSchemaOrCatalog", "ContextLegalityMismatch", "LocalityMismatch",
    "CrossDomainAggregate", "StaleFactView", "StaleOrUnadvertisedOperation",
    "RecoveryRequired", "EffectInDoubt", "ConflictingOwnerFacts",
    "UnsafeOrAmbiguousTarget", "GenerationUnresolved", "MigrationNotReady",
    "TargetOrCustodyUnavailable", "DesiredStateUnresolved", "NoEligibleSnapshot",
    "AlreadyCurrent", "NoEligibleMode", "RequestedModeIneligible",
    "OperationModeMismatch", "AuthorityUnavailable", "CapabilityUnavailable",
]
EXPECTED_SCHEMA_NAMES = {
    "AcquisitionContextV1", "ActionRequestV1", "ActionResultV1",
    "AdversarialReviewResultV1", "AgentPacketV1", "AuditResultV1",
    "BootstrapRouteFactViewV1", "CapabilityInstructionLoadPlanV1",
    "CapabilityMethodIntentV1", "CapabilityMethodResolutionOutcomeV1",
    "CapabilityMethodResolutionV1", "CapabilityTypedNeedV1", "CeremonyRequestV1",
    "CeremonyResultV1", "CloseReviewResultV1", "ContextBudgetMeasurementV1",
    "ContextBudgetProfileV1", "ExactRecipeSelectionV1", "InspectResultV1",
    "InstructionResourceRefV1", "JobGuidanceEnvelopeV1", "JobInstructionLoadPlanV1",
    "JobMethodEligibilityRowV1", "JobMethodEligibilityV1",
    "JobRecipeEligibilityRowV1", "JobRecipeEligibilityV1", "JobRouteInputV1",
    "JobRouteOutcomeV1", "JobRouteV1", "LegacySkillActivationImportV1",
    "LoadedResourceClosureV1", "McpCliSearchEnvelopeV1", "McpCliSearchRequestV1",
    "McpPacketReadEnvelopeV1", "McpPacketReadModeV1", "McpPacketReadRequestV1",
    "McpToolDescriptorV1", "OperationRequestV1", "OperationResultV1",
    "OperationSemanticOutcomeV1", "OperationSpecRefV1",
    "PacketRecipeAdviceProvenanceV1", "PacketRecipeBindingV1",
    "PacketRecipeComponentProvenanceV1", "ProjectionScopeV1", "QAReplayResultV1",
    "RecipeApplicationV1", "RecipeComponentEvaluationV1", "RecipeManifestV1",
    "RecipeReturnOccurrenceV1", "RecipeReturnReasonV1", "RecipeSelectionOptionV1",
    "RecipeSelectionRequestV1", "ResearchExampleEligibilityV1", "ReviewCoverageV1",
    "ReviewMethodFailureV1", "ReviewMethodInvocationV1", "ReviewMethodLoadPlanV1",
    "ReviewMethodRefusalV1", "ReviewMethodResultV1", "ReviewModeRequestV1",
    "ReviewModeResolutionOutcomeV1", "ReviewModeResolutionV1", "ReviewResultHeaderV1",
    "SelectedJobRecipeAdmissionOutcomeV1", "SelectedJobRecipeAdmissionV1",
    "SelectionContextV1", "SetupFactViewV1", "SetupModeRequestV1",
    "SetupModeResolutionOutcomeV1", "SetupModeResolutionV1", "SetupModeV1",
    "SkillActivationAcquisitionContextV1", "SkillActivationCandidateV1",
    "SkillActivationPayloadV1", "SkillActivationRecipeResolutionV1",
    "SkillActivationSubjectV1", "TddChildEligibilityRowV1", "TddChildEligibilityV1",
}
CRITICAL_FIELDS = {
    "RecipeSelectionRequestV1": ["schema_version", "resolution_basis_ref", "primary_selection", "continuation_selection"],
    "RecipeApplicationV1": ["schema_version", "resolution_basis_ref", "frontier_ref", "primary", "continuation"],
    "PacketRecipeBindingV1": ["schema_version", "selection_request_hash", "recipe_application", "recipe_application_hash", "component_provenance", "advice_provenance"],
    "AgentPacketV1": ["schema_version", "packet_id", "semantic_audit_hash", "as_of_ref", "valid_until_ref", "visibility_ref", "scope_manifest", "completeness", "bounds", "snapshot_manifest_ref", "projection_result", "blockers", "advertised_specs", "required_inputs", "effect_classes", "idempotency_classes", "retry_classes", "inspect_refs", "recipe_binding"],
    "McpCliSearchEnvelopeV1": ["schema_version", "request_id", "running_binary_release", "binary_digest", "binary_version", "executable_slot", "core_catalog_ref", "public_catalog_ref", "catalog_snapshot_ref", "completeness", "bounds", "cursor", "hits"],
    "JobRouteV1": ["schema_version", "resolution_basis_ref", "input", "explicit_read_intent", "basis", "outcome"],
    "CapabilityMethodIntentV1": ["schema_version", "exact_scope_ref", "requested_direct_methods", "requested_tdd_children", "research_examples", "requested_review_mode"],
    "CapabilityMethodResolutionV1": ["schema_version", "resolution_basis_ref", "exact_selected_job_route_ref", "exact_intent_ref", "outcome"],
    "ReviewMethodInvocationV1": [],
    "SetupFactViewV1": ["schema_version", "resolver_resource_and_release_catalog_closure", "acquisition_context", "locality_subject", "source_owner_fact_commitments", "advertised_operation_binding"],
    "SetupModeResolutionV1": ["schema_version", "resolver_resource_and_release_catalog_closure", "fact_view_commitment", "request_commitment", "advertised_operation_binding", "outcome"],
    "SkillActivationSubjectV1": ["schema_version", "activation_acquisition_id", "acquisition_context", "release_ref", "root_skill_resource_ref"],
    "SkillActivationPayloadV1": ["selected_route", "capability_resolution", "recipe_resolution", "context_budget_profile_ref", "loaded_resource_closure"],
    "SkillActivationCandidateV1": ["schema_version", "subject", "payload", "subject_commitment", "payload_commitment", "candidate_commitment"],
    "LegacySkillActivationImportV1": ["schema_version", "source_format", "source_file_hash", "source_path_bytes", "record_ordinal", "byte_start", "byte_length", "newline_state", "raw_record_hash", "parse_status", "raw_event_spelling", "skill_name", "session_annotation", "agent_runtime_annotation", "activation_mode_annotation", "timestamp_annotations", "disposition", "reason"],
}


class ValidationError(Exception):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def inactive(value: dict) -> bool:
    return value.get("candidate_only") is True and value.get("runtime_activation") is False and value.get("runtime_registration") is False


def active(value: dict) -> bool:
    return value.get("candidate_only") is False and value.get("runtime_activation") is True and value.get("runtime_registration") is True


def cbor_head(major: int, value: int) -> bytes:
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def cbor(value: object) -> bytes:
    if isinstance(value, int) and not isinstance(value, bool):
        return cbor_head(0, value)
    if isinstance(value, str):
        raw = value.encode()
        return cbor_head(3, len(raw)) + raw
    if isinstance(value, list):
        return cbor_head(4, len(value)) + b"".join(cbor(item) for item in value)
    raise ValidationError(f"unsupported canonical value {value!r}")


def semantic_hash(domain: str, value: object) -> str:
    raw = cbor(value)
    h = hashlib.sha256()
    domain_raw = domain.encode()
    h.update(len(domain_raw).to_bytes(8, "big"))
    h.update(domain_raw)
    h.update(len(raw).to_bytes(8, "big"))
    h.update(raw)
    return h.hexdigest()


def selection_value(value: dict) -> list:
    if value["variant"] == "Absent":
        return [1]
    return ([2] if len(value["value"]) == 2 else [3]) + value["value"]


def expected_request_hash(request: dict) -> str:
    return semantic_hash("maestro.vnext.recipe-selection-request.v1", [request["schema_version"], request["resolution_basis_ref"], selection_value(request["primary_selection"]), selection_value(request["continuation_selection"])])


def expected_application_hash(application: dict) -> str:
    return semantic_hash("maestro.vnext.recipe-application.v1", [application["schema_version"], application["resolution_basis_ref"], application["frontier_ref"], selection_value(application["primary"]), selection_value(application["continuation"])])


def artifact_data(repo: Path) -> dict[str, dict]:
    public = repo / "contracts/vnext/public"
    data = {
        "public": load(public / "public_contracts.v1.json"),
        "selection": load(public / "recipe_selection_application_vectors.v1.json"),
        "returns": load(public / "recipe_return_reasons.v1.json"),
        "job_recipe": load(public / "job_recipe_eligibility_vectors.v1.json"),
        "route": load(public / "job_route_contract.v1.json"),
        "capability": load(public / "capability_method_contracts.v1.json"),
        "context_index": load(public / "context_budget_profiles.v1.json"),
        "setup": load(public / "setup_operation_compatibility.v1.json"),
        "activation": load(public / "skill_activation_contract.v1.json"),
        "ledger": load(public / "v1_skill_ledger.v1.json"),
        "recipe_source": load(repo / "embedded/vnext/orchestration/recipe-catalog.v1.json"),
        "instruction_source": load(repo / "embedded/vnext/capability/instruction-tree.v1.json"),
        "mcp": load(repo / "embedded/vnext/adapter/mcp-tools.v1.json"),
        "bundle": load(public / "bundle_membership_inputs.v1.json"),
    }
    for profile in data["context_index"]["profiles"]:
        data[f"context:{profile['profile_id']}"] = load(repo / profile["source_path"])
    return data


def validate_schemas(public: dict) -> None:
    require(inactive(public), "public contract is active")
    require(all(public["prohibitions"].values()), "public prohibition opened")
    definitions = public["schema_definitions"]
    require(public["schema_definition_count"] == len(definitions) == 79, "schema count mismatch")
    by_name = {row["name"]: row for row in definitions}
    require(len(by_name) == len(definitions) and set(by_name) == EXPECTED_SCHEMA_NAMES, "schema name closure mismatch")
    forbidden_condensed = {"ReviewResolutionV1", "ReviewInvocationV1", "ReviewResultV1", "TddChildNeedV1", "ResearchExamplesV1"}
    require(not (set(by_name) & forbidden_condensed), "condensed predecessor schema survived")
    for name, fields in CRITICAL_FIELDS.items():
        row = by_name[name]
        require([field["name"] for field in row["ordered_fields"]] == fields, f"{name} fields mismatch")
        require(row["unknown_fields"] == "reject", f"{name} accepts unknown fields")
        require(all(field["type"] not in {"String", "Any", "Json"} for field in row["ordered_fields"]), f"{name} has untyped field")
    require([row["name"] for row in by_name["OperationRequestV1"]["variants"]] == ["Action", "Ceremony"], "OperationRequest branches mismatch")
    require([row["name"] for row in by_name["OperationResultV1"]["variants"]] == ["Action", "Ceremony"], "OperationResult branches mismatch")
    require([row["name"] for row in by_name["ReviewMethodInvocationV1"]["variants"]] == ["Produced", "Refused", "Failed"], "Review invocation branches mismatch")
    require([row["name"] for row in by_name["SkillActivationRecipeResolutionV1"]["variants"]] == ["BootstrapNoRecipe", "PacketAdmission"], "activation Recipe union mismatch")
    activation_recipe_variants = by_name["SkillActivationRecipeResolutionV1"]["variants"]
    require(
        all("Refused" not in row["name"] and "Refused" not in row["payload"] for row in activation_recipe_variants),
        "Refused became activatable",
    )
    totals = public["closed_totals"]
    expected = {
        "recipes": 10, "continuation_profiles": 2, "selection_shapes": 30,
        "recipe_return_reasons": 30, "recipe_return_vectors": 196,
        "job_recipe_positive": 22, "job_recipe_negative": 48,
        "job_recipe_admitted_applications": 66, "job_recipe_refused_applications": 144,
        "jobs": 7, "instruction_resources": 31, "direct_methods": 17,
        "job_method_positive": 19, "job_method_negative": 100,
        "review_auxiliary_positive": 4, "review_auxiliary_negative": 11,
        "review_admitted_subsets": 13, "review_refused_subsets": 27,
        "tdd_children": 5, "tdd_direct_job_child_refusals": 35,
        "research_examples_positive": 1, "research_examples_negative": 6,
        "setup_modes": 7, "setup_ambiguous_reasons": 1, "setup_blocked_reasons": 21,
        "setup_action_rows": 145, "setup_ceremony_rows": 11,
        "observation_kinds": 43, "action_specs": 145, "effect_origins": 23,
        "mcp_tools": 2, "project_mcp_tools": 0,
    }
    require(totals == expected, "closed total map mismatch")


def validate_selection(value: dict) -> None:
    expected_shapes = [{"primary": primary, "continuation": continuation} for primary in ["Absent", *PRIMARY_RECIPE_IDS] for continuation in ["Absent", *PROFILES]]
    require(value["primary_axis"] == ["Absent", *PRIMARY_RECIPE_IDS] and value["continuation_axis"] == ["Absent", *PROFILES], "selection axes mismatch")
    require(value["vector_count"] == len(value["vectors"]) == 30, "selection vector count mismatch")
    recipe_identities: dict[str, tuple[str, str]] = {}
    profile_identities: dict[str, str] = {}
    for ordinal, (row, shape) in enumerate(zip(value["vectors"], expected_shapes), 1):
        require(row["enumeration_ordinal_not_identity"] == ordinal and row["shape"] == shape, "selection order/shape mismatch")
        request, application, binding = row["selection_request"], row["application"], row["packet_recipe_binding_fixture"]
        require(application["primary"] == request["primary_selection"] and application["continuation"] == request["continuation_selection"], "request/application selection mismatch")
        require(binding["recipe_application"] == application, "binding application mismatch")
        require(binding["selection_request_hash"] == "sha256:" + expected_request_hash(request), "request commitment mismatch")
        require(binding["recipe_application_hash"] == "sha256:" + expected_application_hash(application), "application commitment mismatch")
        selected = []
        for slot, selection in (("Primary", request["primary_selection"]), ("Continuation", request["continuation_selection"])):
            if selection["variant"] == "Absent":
                continue
            payload = selection["value"]
            require(payload[1].startswith("sha256:") and payload[1] != "sha256:" + "0" * 64, "zero manifest identity")
            recipe = payload[0].split(":")[-2]
            identity = (payload[0], payload[1])
            require(recipe_identities.setdefault(recipe, identity) == identity, "mixed Recipe identity")
            if slot == "Continuation":
                profile = payload[2].removeprefix(
                    "candidate:orchestration:bounded-continuation-profile:"
                ).split(":v1@sha256:", 1)[0]
                require(profile_identities.setdefault(profile, payload[2]) == payload[2], "mixed profile identity")
            selected.append(slot)
        components = binding["component_provenance"]
        require([component["component_slot"] for component in components] == selected, "component slot/cardinality mismatch")
        hashes = []
        for component in components:
            occurrence = component["recipe_return_occurrence"]
            require(occurrence["frontier_ref"] == application["frontier_ref"] and occurrence["resolution_basis_ref"] == application["resolution_basis_ref"], "component Frontier/basis mismatch")
            require(occurrence["component_slot"] == component["component_slot"] and occurrence["outcome_tag"] == "NotApplicable", "component occurrence mismatch")
            require(component["component_output_hash"].startswith("sha256:") and component["component_output_hash"] != "sha256:" + "0" * 64, "zero component output")
            hashes.append(component["component_output_hash"])
        advice = binding["advice_provenance"]
        require(advice["ordered_component_output_hashes"] == hashes, "Advice hash order mismatch")
        require(advice["composed_output_hash"].startswith("sha256:") and advice["composed_output_hash"] != "sha256:" + "0" * 64, "zero composed Advice")
        require(advice["composition_outcome"] == ("CoreOnly" if not selected else "NotApplicable"), "selected NotApplicable was lost")
    require(set(recipe_identities) == set(RECIPE_IDS) and set(profile_identities) == set(PROFILES), "selection identity coverage mismatch")


def reason_ref(recipe: str, outcome: str) -> str:
    name = "".join(part.capitalize() for part in recipe.split("-")) + outcome
    return f"candidate:orchestration:recipe-return-reason:{name}:v1"


def validate_returns(value: dict) -> None:
    expected_members = [(tag, recipe, outcome, reason_ref(recipe, outcome)) for tag, (recipe, outcome) in enumerate(itertools.product(RECIPE_IDS, OUTCOMES), 1)]
    actual = [(row["tag"], row["recipe_id"], row["outcome"], row["resource_ref"]) for row in value["members"]]
    require(value["member_count"] == len(actual) == 30 and actual == expected_members, "return reason bijection mismatch")
    require(value["manifest_subset_count"] == len(value["manifest_subsets"]) == 10, "return subset mismatch")
    for subset, recipe in zip(value["manifest_subsets"], RECIPE_IDS):
        require(subset == {"recipe_id": recipe, "return_reason_refs": [reason_ref(recipe, outcome) for outcome in OUTCOMES]}, "manifest reason subset mismatch")
    require(value["membership_matrix"] == {"positive": 30, "negative": 270}, "return membership matrix mismatch")
    require(value["compatibility_matrix"] == {"positive": 30, "negative": 870}, "return compatibility matrix mismatch")
    vectors = value["application_outcome_vectors"]
    require(value["application_outcome_vector_count"] == len(vectors) == 196 and len({row["vector_id"] for row in vectors}) == 196, "return occurrence vectors mismatch")
    counts = Counter(len(row["outcomes"]) for row in vectors)
    require(counts == {0: 1, 1: 33, 2: 162}, "return vector arity mismatch")
    for row in vectors:
        outcomes = [item["outcome"] for item in row["outcomes"]]
        expected = "CoreOnly" if not outcomes else "HardStop" if "HardStop" in outcomes else "NotApplicable" if "NotApplicable" in outcomes else "RestrictiveAdvice"
        require(row["composition_outcome"] == expected, "return composition mismatch")
        for item, shape in zip(row["outcomes"], row["component_shape"]):
            recipe = shape["component"].split(":", 1)[0]
            require(item["return_reason_ref"] == reason_ref(recipe, item["outcome"]), "return occurrence reason mismatch")


def shapes() -> list[dict[str, str]]:
    return [{"primary": p, "continuation": c} for p in ["Absent", *PRIMARY_RECIPE_IDS] for c in ["Absent", *PROFILES]]


def validate_job_recipe(value: dict) -> None:
    require(value["row_count"] == len(value["rows"]) == 10, "JobRecipe row count mismatch")
    for row, recipe in zip(value["rows"], RECIPE_IDS):
        require(row["recipe_id"] == recipe and row["eligible_jobs"] == JOB_RECIPE[recipe] and row["refused_jobs"] == [job for job in JOBS if job not in JOB_RECIPE[recipe]], "JobRecipe row mismatch")
    require(value["positive_edges"] == 22 and value["negative_edges"] == 48, "JobRecipe edge counts mismatch")
    vectors = value["application_vectors"]
    require(value["application_vector_count"] == len(vectors) == 210, "JobRecipe application vector count mismatch")
    for row, (job, ordinal, shape) in zip(vectors, ((job, ordinal, shape) for job in JOBS for ordinal, shape in enumerate(shapes(), 1))):
        selected = ([] if shape["primary"] == "Absent" else [shape["primary"]]) + ([] if shape["continuation"] == "Absent" else ["bounded-continuation"])
        admitted = all(job in JOB_RECIPE[recipe] for recipe in selected)
        outcome = "NoRecipe" if not selected else "Admitted" if admitted else "Refused"
        require(row["job"] == job and row["selection_shape_ordinal"] == ordinal and row["shape"] == shape and row["selected_recipe_ids"] == selected and row["outcome"] == outcome, "JobRecipe application mismatch")
        require(row["partial_fallback"] is False and row["admitted_recipe_resource_refs"] == ([f"candidate:orchestration:recipe:{recipe}:v1" for recipe in selected] if admitted else []), "JobRecipe fallback/admission mismatch")
    require(value["admitted_application_count"] == 66 and value["refused_application_count"] == 144, "JobRecipe 66/144 mismatch")


def validate_route(value: dict) -> None:
    expected = [(reason, "Selected", job) for reason, job in ROUTE_SELECTED.items()] + [(reason, "Ambiguous", None) for reason in ROUTE_AMBIGUOUS] + [(reason, "Blocked", None) for reason in ROUTE_BLOCKED]
    actual = [(row["reason"], row["status"], row["job"]) for row in value["rows"]]
    require(value["row_count"] == 17 and actual == expected, "JobRoute total map mismatch")
    require(value["selected_count"] == 10 and value["ambiguous_count"] == 2 and value["blocked_count"] == 5, "JobRoute partition mismatch")
    for row in value["rows"][:10]:
        require(row["initial_load_plan"] == {"job_resources": 1, "method_resources": 0, "recipe_resources": 0}, "JobRoute initial load plan widened")
    require(
        all(row["instruction_resources"] == [] for row in value["rows"][10:]),
        "non-selected JobRoute loaded instruction Resources",
    )
    require(value["owns_recommendation"] is False and value["guidance_is_separate_from_packet"] is True, "JobRoute ownership mismatch")


def validate_capability(value: dict) -> None:
    require(value["skill_ids"] == ["maestro"] and value["jobs"] == JOBS and value["instruction_resource_count"] == 31, "Capability root mismatch")
    methods = list(METHOD_ROWS["Design"]) + [method for job in JOBS for method in METHOD_ROWS[job] if method not in METHOD_ROWS["Design"]]
    methods = list(dict.fromkeys(methods))
    require(value["direct_methods"] == methods, "method catalog mismatch")
    rows = value["job_method"]["rows"]
    require(len(rows) == 119, "JobMethod matrix size mismatch")
    for row, (job, method) in zip(rows, itertools.product(JOBS, methods)):
        require(row["job"] == job and row["method"] == method and row["admitted"] == (method in METHOD_ROWS[job]), "JobMethod cell mismatch")
        require(row["exact_resource_ref"].startswith("candidate:capability:instruction-resource:") and "@sha256:" in row["exact_resource_ref"], "method Resource not content-qualified")
    require(value["job_method"]["positive"] == 19 and value["job_method"]["negative"] == 100, "JobMethod counts mismatch")
    require(value["job_method"]["method_degrees"] == {method: (3 if method == "GenerateAndFilter" else 1) for method in methods}, "method degrees mismatch")
    review = value["review"]
    require(review["modes"] == ["Inspect", "Audit", "AdversarialReview", "QAReplay", "CloseReview"], "Review modes mismatch")
    require(review["auxiliary_positive"] == 4 and review["auxiliary_negative"] == 11 and len(review["auxiliary_rows"]) == 15, "Review auxiliary matrix mismatch")
    expected_auxiliary_rows = [
        {
            "mode": mode,
            "method": method,
            "admitted": method in {
                "Inspect": [],
                "Audit": ["ArchitectureReview", "GenerateAndFilter", "Verification"],
                "AdversarialReview": [],
                "QAReplay": [],
                "CloseReview": ["Verification"],
            }[mode],
        }
        for mode in review["modes"]
        for method in ["ArchitectureReview", "GenerateAndFilter", "Verification"]
    ]
    require(review["auxiliary_rows"] == expected_auxiliary_rows, "Review auxiliary matrix cell mismatch")
    require(review["subset_shape_count"] == 40 and review["admitted_subset_shapes"] == 13 and review["refused_subset_shapes"] == 27, "Review subset matrix mismatch")
    for row in review["subset_rows"]:
        allowed = {"Inspect": [], "Audit": ["ArchitectureReview", "GenerateAndFilter", "Verification"], "AdversarialReview": [], "QAReplay": [], "CloseReview": ["Verification"]}[row["mode"]]
        require((row["outcome"] == "Admitted") == all(method in allowed for method in row["requested_auxiliaries"]), "Review subset outcome mismatch")
    require(review["invocation_outer_outcomes"] == ["Produced", "Refused", "Failed"] and review["transport_or_crash_has_result_envelope"] is False, "Review invocation boundary mismatch")
    require(value["tdd"]["positive_children"] == 5 and value["tdd"]["direct_job_child_refusals"] == 35 and len(value["tdd"]["job_child_refusals"]) == 35, "TDD matrix mismatch")
    require(all(row["only_parent"] == "Execute/TDD" for row in value["tdd"]["children"]), "TDD parent widened")
    research = value["research_examples"]
    require(research["positive"] == 1 and research["negative"] == 6 and [row["admitted"] for row in research["rows"]] == [False, True, False, False, False, False, False], "Research example matrix mismatch")


RESOURCE_CACHE: dict[tuple[Path, str], tuple[Path, int]] = {}


def resource_path_for(repo: Path, resource_ref: str) -> tuple[Path, int]:
    cache_key = (repo, resource_ref)
    if cache_key in RESOURCE_CACHE:
        return RESOURCE_CACHE[cache_key]
    if resource_ref.startswith("candidate:capability:instruction-resource:"):
        body = resource_ref.removeprefix("candidate:capability:instruction-resource:")
        logical, digest = body.rsplit(":v1@sha256:", 1)
        path = repo / "embedded/vnext/capability" / logical
    elif resource_ref.startswith("candidate:orchestration:recipe:"):
        body = resource_ref.removeprefix("candidate:orchestration:recipe:")
        recipe, digest = body.split(":v1@sha256:", 1)
        path = repo / f"embedded/vnext/orchestration/recipes/{recipe}/manifest.v1.json"
    elif resource_ref.startswith("candidate:orchestration:bounded-continuation-profile:"):
        body = resource_ref.removeprefix("candidate:orchestration:bounded-continuation-profile:")
        profile, digest = body.split(":v1@sha256:", 1)
        path = repo / f"embedded/vnext/orchestration/profiles/bounded-continuation/{profile}.v1.json"
    else:
        raise ValidationError("unknown ContextBudget Resource")
    raw = path.read_bytes() if path.is_file() else b""
    require(path.is_file() and hashlib.sha256(raw).hexdigest() == digest, "ContextBudget Resource digest mismatch")
    result = (path, len(raw))
    RESOURCE_CACHE[cache_key] = result
    return result


def validate_context(repo: Path, data: dict) -> None:
    index = data["context_index"]
    require(inactive(index) and index["host_profile_count"] == 2 and index["universal_product_cap"] is False, "context profile index mismatch")
    profiles = [data[f"context:{profile['profile_id']}"] for profile in index["profiles"]]
    require([profile["profile_id"] for profile in profiles] == ["agents-compatible-cli", "claude-code"], "supported host profiles mismatch")
    require(index["admitted_combined_closure_count"] == 750, "combined closure count mismatch")
    for summary, profile in zip(index["profiles"], profiles):
        source = repo / summary["source_path"]
        require(hashlib.sha256(source.read_bytes()).hexdigest() == summary["profile_sha256"], "profile source digest mismatch")
        require(inactive(profile) and profile["universal_product_cap"] is False and profile["release_ref"] == index["release_ref"], "profile Release/host boundary mismatch")
        rows = profile["measurements"]
        require(profile["measurement_count"] == len(rows) == 750 and len({row["closure_ref"] for row in rows}) == 750, "profile measurement closure mismatch")
        require(len(profile["admitted_resource_refs"]) == 43 and len(set(profile["admitted_resource_refs"])) == 43, "profile admitted Resource set mismatch")
        for row in rows:
            refs = row["ordered_resource_refs"]
            require(len(refs) == len(set(refs)) and set(refs) <= set(profile["admitted_resource_refs"]), "profile Resource closure mismatch")
            byte_count = sum(resource_path_for(repo, ref)[1] for ref in refs)
            require(row["utf8_bytes"] == byte_count, "profile UTF-8 measurement mismatch")
            expected_units = byte_count if profile["profile_id"] == "agents-compatible-cli" else (byte_count + 3) // 4
            require(row["host_observed_units"] == expected_units, "profile host measurement mismatch")
            key = f"{row['capability_shape']}|selection:{row['selection_shape_ordinal']:02d}"
            expected_ref = "candidate:capability:context-closure:v1@sha256:" + semantic_hash("maestro.vnext.context-budget-closure.v1", [key, *refs])
            require(row["closure_ref"] == expected_ref, "profile closure commitment mismatch")
        require(profile["maximum_utf8_bytes"] == max(row["utf8_bytes"] for row in rows) and profile["maximum_host_observed_units"] == max(row["host_observed_units"] for row in rows), "profile maxima are not exact evidence")


def action_compatibility(family: str, action: str) -> tuple[list[str], str]:
    if action in {"AdoptManagedRegion", "TransferWholeFileCustody"}:
        return ["Adopt"], "MatchingAdoptTargetOrCustodySubject"
    if action in {"RecoverDistributionTransaction", "RollbackDistributionTransaction"}:
        return [], "TransactionRecoveryOrRollbackDoesNotValidateOrdinarySetupMode"
    if family == "Distribution":
        return ["Install", "Migrate", "Update", "Repair", "Rollback", "Uninstall"], "ExactSameDomainDistributionPlanIntent"
    return [], "NoSetupMode"


def ceremony_compatibility(name: str) -> tuple[str, list[str], str]:
    if name == "InstallationContextGenesis":
        return "NoStoreInstallationGenesis", ["Install", "Adopt", "Migrate"], "ExplicitFactFirstJourneyLabelOnly"
    if name in {"RepositoryV1Cutover", "InstallationV1Cutover"}:
        return "PreStore", ["Migrate"], "ExactTypedCutover"
    return "PreStore", [], "RecoveryOrAdmissionDoesNotValidateOrdinarySetupMode"


def setup_projection(repo: Path) -> dict:
    root = repo / "contracts/vnext/catalogs/generated"
    grammar_path, action_path, ceremony_path = root / "catalog-profile-grammar-v1.json", root / "catalog-09-action-spec.json", root / "catalog-05-ceremony.json"
    grammar, actions, ceremonies = load(grammar_path), load(action_path), load(ceremony_path)
    owners = {row["tag"]: row["name"] for row in grammar["owner_profiles"]}
    action_symbols = {row["global_tag"]: row for row in grammar["action_leaf_symbols"]}
    ceremony_symbols = {row["tag"]: row for row in grammar["ceremony_symbols"]}
    action_owner = {row[0]: row for row in actions["primary_owner_relation"]["rows"]}
    ceremony_owner = {row[0]: row for row in ceremonies["primary_owner_relation"]["rows"]}
    action_rows, family_counts = [], Counter()
    for descriptor in actions["descriptors"]:
        tag, name, owner_ref, family_tag, family_local_tag = descriptor["value"][:5]
        owner_tag, owner_identity = owner_ref
        symbol = action_symbols[tag]
        require(symbol["name"] == name and symbol["owner"] == owners[owner_tag] and action_owner[tag] == [tag, owner_tag, owner_identity], "Action catalog binding mismatch")
        family = symbol["owner"]
        family_counts[family] += 1
        modes, predicate = action_compatibility(family, name)
        action_rows.append({"catalog_tag": tag, "operation_kind": "Action", "name": name, "descriptor_id": descriptor["descriptor_id"], "family": family, "family_tag": family_tag, "family_local_tag": family_local_tag, "primary_owner": owners[owner_tag], "primary_owner_tag": owner_tag, "primary_owner_descriptor_id": owner_identity["bytes"], "catalog_context": "ActiveStore", "compatible_setup_modes": modes, "required_binding_predicate": predicate, "operation_never_contributes_to_eligibility": True})
    ceremony_rows = []
    contexts = {2: "NoStoreInstallationGenesis", 3: "PreStore"}
    for descriptor in ceremonies["descriptors"]:
        tag, name, owner_ref, request_modes, _origins, context_tag = descriptor["value"]
        owner_tag, owner_identity = owner_ref
        require(ceremony_symbols[tag]["name"] == name and ceremony_owner[tag] == [tag, owner_tag, owner_identity], "Ceremony catalog binding mismatch")
        context, modes, predicate = ceremony_compatibility(name)
        require(context == contexts[context_tag], "Ceremony context mismatch")
        ceremony_rows.append({"catalog_tag": tag, "operation_kind": "Ceremony", "name": name, "descriptor_id": descriptor["descriptor_id"], "primary_owner": owners[owner_tag], "primary_owner_tag": owner_tag, "primary_owner_descriptor_id": owner_identity["bytes"], "catalog_context": context, "catalog_context_tag": context_tag, "request_mode_tags": request_modes, "compatible_setup_modes": modes, "required_binding_predicate": predicate, "operation_never_contributes_to_eligibility": True})
    return {"catalog_bindings": {"catalog_profile_grammar_id": grammar["catalog_profile_grammar"]["catalog_profile_grammar_id"], "action_spec_manifest_id": actions["manifest_id"], "action_spec_file_sha256": hashlib.sha256(action_path.read_bytes()).hexdigest(), "ceremony_manifest_id": ceremonies["manifest_id"], "ceremony_file_sha256": hashlib.sha256(ceremony_path.read_bytes()).hexdigest()}, "action_family_counts": dict(family_counts), "action_rows": action_rows, "ceremony_rows": ceremony_rows}


def validate_setup(repo: Path, value: dict) -> None:
    expected = setup_projection(repo)
    for key in expected:
        require(value[key] == expected[key], f"Setup catalog projection mismatch: {key}")
    require(value["action_row_count"] == len(value["action_rows"]) == 145 and value["ceremony_row_count"] == len(value["ceremony_rows"]) == 11, "Setup operation row count mismatch")
    rollback = next(row for row in value["action_rows"] if row["name"] == "RollbackDistributionTransaction")
    recovery = next(row for row in value["action_rows"] if row["name"] == "RecoverDistributionTransaction")
    require(rollback["compatible_setup_modes"] == recovery["compatible_setup_modes"] == [], "transaction rollback/recovery incorrectly establishes Setup mode")
    require(all(row["operation_never_contributes_to_eligibility"] for row in [*value["action_rows"], *value["ceremony_rows"]]), "Operation contributes to Setup eligibility")


def validate_activation(repo: Path, value: dict) -> None:
    require(inactive(value), "activation contract active")
    require(value["subject_ordered_fields"] == CRITICAL_FIELDS["SkillActivationSubjectV1"] and value["payload_ordered_fields"] == CRITICAL_FIELDS["SkillActivationPayloadV1"] and value["candidate_ordered_fields"] == CRITICAL_FIELDS["SkillActivationCandidateV1"], "activation field order mismatch")
    require(value["acquisition_contexts"] == ["ActiveStore.Repository", "ActiveStore.Installation", "Bootstrap.RepositoryBootstrap", "Bootstrap.InstallationBootstrap"], "activation acquisition union mismatch")
    require(value["selected_route_reason_set"] == list(ROUTE_SELECTED) and value["candidate_forbidden_route_reasons"] == ROUTE_AMBIGUOUS + ROUTE_BLOCKED, "activation route partition mismatch")
    require(value["capability_outcomes"] == ["Selected"] and value["recipe_resolution"] == ["BootstrapNoRecipe", "PacketAdmission.NoRecipe", "PacketAdmission.Admitted[1..2]"], "activation selected-only union mismatch")
    require(value["loaded_resource_closure_cardinality"] == {"job": "1", "direct_methods": "0..4", "tdd_children": "0..5", "research_example": "0..1", "recipes": "0..2", "closure_digest": "1"}, "activation closure bounds mismatch")
    require(len(set(value["commitment_domains"].values())) == 4 and all(domain.startswith("maestro.vnext.skill-activation-") for domain in value["commitment_domains"].values()), "activation domains are not separate")
    root = repo / "contracts/vnext/catalogs/generated"
    observation, action, effect = load(root / "catalog-01-observation.json"), load(root / "catalog-09-action-spec.json"), load(root / "catalog-02-effect.json")
    observation_row = next(row for row in observation["descriptors"] if row["value"][1] == "SkillActivation")
    action_row = next(row for row in action["descriptors"] if row["value"][1] == "PublishObservation")
    expected = {"observation_member_count": 43, "skill_activation_tag": 12, "skill_activation_descriptor_id": observation_row["descriptor_id"], "observation_manifest_id": observation["manifest_id"], "action_member_count": 145, "publish_observation_tag": action_row["value"][0], "publish_observation_descriptor_id": action_row["descriptor_id"], "action_manifest_id": action["manifest_id"], "effect_origin_member_count": 23, "effect_origin_manifest_id": effect["manifest_id"], "activation_specific_membership_delta": 0}
    require(value["evidence_catalog_bindings"] == expected and action_row["value"][7] == [], "current activation catalog binding mismatch")
    require(value["predecessor_non_current_evidence"] == {"action_manifest_id": "sha256:bcc5d3ca6c84ae1d293bd31d5729d852435279b132d6b36feff303877dafb050", "publish_observation_tag": 30, "descriptor_id": "b6a89e1621b0a21bd5473dd4d8b88ab42b836210e6f8f106070e8c16d986f7a1", "current_selector": False}, "activation predecessor evidence mismatch")
    require(value["publication"] == {"candidate_per_complete_acquisition": 1, "observations_per_candidate": "0..1", "passive_writes": 0, "additional_resource_reads": 0, "mcp_tools_added": 0}, "activation publication cardinality widened")
    legacy = value["legacy_import"]
    require(legacy["dispositions"] == ["MappedHistoricalNonBearer", "OpaquePreserved", "Quarantined", "UnavailablePreexistingLoss"] and legacy["forbidden_disposition"] == "MappedNormative" and len(legacy["inactive_skill_names"]) == 8, "legacy activation promotion mismatch")
    require(len(value["illegal_union_mutants"]) == 13 and len(set(value["illegal_union_mutants"])) == 13, "activation illegal union set mismatch")


def validate_sources(repo: Path, data: dict) -> None:
    for key in ["recipe_source", "instruction_source"]:
        require(inactive(data[key]), f"{key} activated")
    require(active(data["mcp"]), "mcp activation tuple mismatch")
    require([row["id"] for row in data["recipe_source"]["recipes"]] == RECIPE_IDS, "Recipe source order mismatch")
    tree = data["instruction_source"]
    require(tree["public_skills"] == ["maestro"] and len(tree["logical_paths"]) == len(set(tree["logical_paths"])) == 31, "instruction tree mismatch")
    for path in tree["logical_paths"]:
        raw = (repo / "embedded/vnext/capability" / path).read_text()
        frontmatter = raw.split("---", 2)[1]
        require("candidate_only: true" in frontmatter and "runtime_activation: false" in frontmatter and "runtime_registration: false" in frontmatter, f"instruction activated: {path}")
    mcp = data["mcp"]
    require([row["name"] for row in mcp["tools"]] == ["maestro_packet", "maestro_cli_search"] and mcp["project_tools"] == [], "MCP tool closure mismatch")
    require(all(row["read_only"] and not row["writes"] and not row["network_io"] for row in mcp["tools"]), "MCP mutation surface opened")
    require(mcp["tools"][1]["cursor_contract"] == "Complete=Absent; BoundedTruncated=Present", "MCP search cursor parity missing")
    ledger = data["ledger"]
    require(len(ledger["rows"]) == 35 and len({row["path"] for row in ledger["rows"]}) == 35 and Counter(row["disposition"] for row in ledger["rows"]) == Counter({"Rewrite": 19, "Replace": 9, "MigrationOnly": 7}) and ledger["semantic_destination_count"] == 21, "Skill ledger mismatch")
    bundle = data["bundle"]
    require(bundle["bundle_kinds"] == ["Release", "AgentBootstrap", "Capability", "Orchestration", "SharedContract", "Adapter", "ExternalPattern", "Migration"] and bundle["containing_id_backreferences"] is False, "Bundle identity direction mismatch")


def validate_data(repo: Path, data: dict) -> dict:
    validate_schemas(data["public"])
    validate_selection(data["selection"])
    validate_returns(data["returns"])
    validate_job_recipe(data["job_recipe"])
    validate_route(data["route"])
    validate_capability(data["capability"])
    validate_context(repo, data)
    validate_setup(repo, data["setup"])
    validate_activation(repo, data["activation"])
    validate_sources(repo, data)
    return {
        "schema": "maestro.vnext.public-contract-validation-receipt.v1",
        "status": "pass", "runtime_activated": True, "inactive_source_roots": 2,
        "recipes": 10, "recipe_manifests": 10, "bounded_continuation_profiles": 2,
        "selection_application_vectors": 30, "recipe_return_reasons": 30,
        "recipe_return_vectors": 196, "job_recipe_edges": 22, "job_recipe_non_edges": 48,
        "job_recipe_admitted": 66, "job_recipe_refused": 144,
        "setup_action_rows": 145, "setup_ceremony_rows": 11,
        "instruction_resources": 31, "mcp_tools": 2, "project_mcp_tools": 0,
        "schema_descriptors": 79, "context_budget_profiles": 2,
        "context_budget_closures_per_profile": 750, "setup_catalog_bound": True,
        "skill_activation_catalog_counts": [43, 145, 23],
    }


Mutation = tuple[str, str, Callable[[dict], None]]


def mutations() -> list[Mutation]:
    result: list[Mutation] = []
    for key in ["recipe_source", "instruction_source"]:
        for field, value in [("candidate_only", False), ("runtime_activation", True), ("runtime_registration", True)]:
            result.append((f"{key}:{field}", key, lambda data, field=field, value=value: data.__setitem__(field, value)))
    for field, value in [("candidate_only", True), ("runtime_activation", False), ("runtime_registration", False)]:
        result.append((f"mcp:{field}", "mcp", lambda data, field=field, value=value: data.__setitem__(field, value)))
    for name in sorted(CRITICAL_FIELDS):
        result.append((f"schema:remove:{name}", "public", lambda data, name=name: data["schema_definitions"].__setitem__(slice(None), [row for row in data["schema_definitions"] if row["name"] != name])))
    result += [
        ("schema:untyped-AgentPacket", "public", lambda d: next(row for row in d["schema_definitions"] if row["name"] == "AgentPacketV1")["ordered_fields"][1].__setitem__("type", "String")),
        ("schema:condensed-Review", "public", lambda d: d["schema_definitions"].append({"name": "ReviewResultV1", "version": 1, "kind": "record", "ordered_fields": [], "variants": [], "cross_constraints": [], "unknown_fields": "reject"})),
        ("schema:OperationRequest-third-branch", "public", lambda d: next(row for row in d["schema_definitions"] if row["name"] == "OperationRequestV1")["variants"].append({"tag": 3, "name": "Generic", "payload": "Any"})),
        ("schema:activation-Refused", "public", lambda d: next(row for row in d["schema_definitions"] if row["name"] == "SkillActivationRecipeResolutionV1")["variants"].append({"tag": 3, "name": "Refused", "payload": "String"})),
        ("selection:duplicate-option", "selection", lambda d: d["vectors"].__setitem__(29, copy.deepcopy(d["vectors"][0]))),
        ("selection:swap-order", "selection", lambda d: d["vectors"].__setitem__(slice(0, 2), [d["vectors"][1], d["vectors"][0]])),
        ("selection:zero-request-hash", "selection", lambda d: d["vectors"][0]["packet_recipe_binding_fixture"].__setitem__("selection_request_hash", "sha256:" + "0" * 64)),
        ("selection:wrong-application-hash", "selection", lambda d: d["vectors"][1]["packet_recipe_binding_fixture"].__setitem__("recipe_application_hash", "sha256:" + "f" * 64)),
        ("selection:zero-composed-hash", "selection", lambda d: d["vectors"][2]["packet_recipe_binding_fixture"]["advice_provenance"].__setitem__("composed_output_hash", "sha256:" + "0" * 64)),
        ("selection:drop-component", "selection", lambda d: d["vectors"][-1]["packet_recipe_binding_fixture"]["component_provenance"].pop()),
        ("selection:lose-NotApplicable", "selection", lambda d: d["vectors"][1]["packet_recipe_binding_fixture"]["advice_provenance"].__setitem__("composition_outcome", "RestrictiveAdvice")),
        ("return:wrong-member", "returns", lambda d: d["members"][0].__setitem__("outcome", "HardStop")),
        ("return:drop-subset", "returns", lambda d: d["manifest_subsets"].pop()),
        ("return:wrong-membership", "returns", lambda d: d["membership_matrix"].__setitem__("negative", 269)),
        ("return:drop-vector", "returns", lambda d: d["application_outcome_vectors"].pop()),
        ("return:wrong-composition", "returns", lambda d: d["application_outcome_vectors"][-1].__setitem__("composition_outcome", "NotApplicable")),
        ("return:wrong-reason-ref", "returns", lambda d: d["application_outcome_vectors"][1]["outcomes"][0].__setitem__("return_reason_ref", "candidate:wrong")),
        ("job-recipe:add-edge", "job_recipe", lambda d: d["rows"][1]["eligible_jobs"].append("Setup")),
        ("job-recipe:partial-fallback", "job_recipe", lambda d: d["application_vectors"][-1].__setitem__("partial_fallback", True)),
        ("job-recipe:wrong-outcome", "job_recipe", lambda d: d["application_vectors"][-1].__setitem__("outcome", "Admitted")),
        ("job-recipe:wrong-total", "job_recipe", lambda d: d.__setitem__("admitted_application_count", 67)),
        ("route:wrong-selected-job", "route", lambda d: d["rows"][0].__setitem__("job", "Execute")),
        ("route:resource-in-refusal", "route", lambda d: d["rows"][10].__setitem__("instruction_resources", ["candidate:x"])),
        ("route:method-in-initial-plan", "route", lambda d: d["rows"][0]["initial_load_plan"].__setitem__("method_resources", 1)),
        ("capability:flip-edge", "capability", lambda d: d["job_method"]["rows"][0].__setitem__("admitted", True)),
        ("capability:wrong-degree", "capability", lambda d: d["job_method"]["method_degrees"].__setitem__("GenerateAndFilter", 2)),
        ("capability:Review-aux", "capability", lambda d: d["review"]["auxiliary_rows"][0].__setitem__("admitted", True)),
        ("capability:Review-subset", "capability", lambda d: d["review"]["subset_rows"][-1].__setitem__("outcome", "Admitted")),
        ("capability:Review-transport-result", "capability", lambda d: d["review"].__setitem__("transport_or_crash_has_result_envelope", True)),
        ("capability:TDD-parent", "capability", lambda d: d["tdd"]["children"][0].__setitem__("only_parent", "Design/TDD")),
        ("capability:TDD-refusal", "capability", lambda d: d["tdd"]["job_child_refusals"].pop()),
        ("capability:research-edge", "capability", lambda d: d["research_examples"]["rows"][0].__setitem__("admitted", True)),
        ("context:index-universal", "context_index", lambda d: d.__setitem__("universal_product_cap", True)),
        ("context:index-count", "context_index", lambda d: d.__setitem__("admitted_combined_closure_count", 749)),
        ("context:profile-universal", "context:agents-compatible-cli", lambda d: d.__setitem__("universal_product_cap", True)),
        ("context:measurement-byte", "context:agents-compatible-cli", lambda d: d["measurements"][0].__setitem__("utf8_bytes", d["measurements"][0]["utf8_bytes"] + 1)),
        ("context:measurement-unit", "context:claude-code", lambda d: d["measurements"][0].__setitem__("host_observed_units", d["measurements"][0]["host_observed_units"] + 1)),
        ("context:zero-max", "context:agents-compatible-cli", lambda d: d.__setitem__("maximum_utf8_bytes", 0)),
        ("setup:bogus-action", "setup", lambda d: d["action_rows"][0].__setitem__("name", "BogusAction")),
        ("setup:rollback-is-mode", "setup", lambda d: next(row for row in d["action_rows"] if row["name"] == "RollbackDistributionTransaction").__setitem__("compatible_setup_modes", ["Rollback"])),
        ("setup:operation-contributes", "setup", lambda d: d["ceremony_rows"][0].__setitem__("operation_never_contributes_to_eligibility", False)),
        ("setup:wrong-context", "setup", lambda d: d["ceremony_rows"][0].__setitem__("catalog_context", "PreStore")),
        ("setup:wrong-manifest", "setup", lambda d: d["catalog_bindings"].__setitem__("action_spec_manifest_id", "0" * 64)),
        ("activation:Ambiguous-candidate", "activation", lambda d: d["selected_route_reason_set"].append("ConflictingReadIntent")),
        ("activation:Refused-recipe", "activation", lambda d: d["recipe_resolution"].append("PacketAdmission.Refused")),
        ("activation:zero-job", "activation", lambda d: d["loaded_resource_closure_cardinality"].__setitem__("job", "0..1")),
        ("activation:shared-domain", "activation", lambda d: d["commitment_domains"].__setitem__("payload", d["commitment_domains"]["subject"])),
        ("activation:wrong-observation-tag", "activation", lambda d: d["evidence_catalog_bindings"].__setitem__("skill_activation_tag", 13)),
        ("activation:wrong-current-action-tag", "activation", lambda d: d["evidence_catalog_bindings"].__setitem__("publish_observation_tag", 30)),
        ("activation:predecessor-current", "activation", lambda d: d["predecessor_non_current_evidence"].__setitem__("current_selector", True)),
        ("activation:mandatory-publication", "activation", lambda d: d["publication"].__setitem__("observations_per_candidate", "1")),
        ("activation:passive-write", "activation", lambda d: d["publication"].__setitem__("passive_writes", 1)),
        ("activation:MappedNormative", "activation", lambda d: d["legacy_import"]["dispositions"].append("MappedNormative")),
        ("activation:drop-illegal-union", "activation", lambda d: d["illegal_union_mutants"].pop()),
        ("mcp:third-tool", "mcp", lambda d: d["tools"].append(copy.deepcopy(d["tools"][0]))),
        ("mcp:project-tool", "mcp", lambda d: d["project_tools"].append("maestro_packet")),
        ("mcp:search-writes", "mcp", lambda d: d["tools"][1].__setitem__("writes", True)),
        ("mcp:cursor-parity", "mcp", lambda d: d["tools"][1].__setitem__("cursor_contract", "optional")),
        ("ledger:drop-row", "ledger", lambda d: d["rows"].pop()),
        ("ledger:promote", "ledger", lambda d: d["rows"][0].__setitem__("disposition", "Rewrite" if d["rows"][0]["disposition"] != "Rewrite" else "Replace")),
    ]
    return result


def mutant_suite(repo: Path) -> dict:
    base = artifact_data(repo)
    rejected, escaped = [], []
    for name, key, mutate in mutations():
        data = dict(base)
        data[key] = copy.deepcopy(base[key])
        mutate(data[key])
        try:
            validate_data(repo, data)
        except ValidationError:
            rejected.append(name)
        else:
            escaped.append(name)
    receipt = {
        "schema": "maestro.vnext.public-contract-mutant-receipt.v1",
        "status": "pass" if not escaped else "fail",
        "semantic_mutant_categories": 12,
        "total_mutants": len(rejected) + len(escaped),
        "rejected_mutants": len(rejected),
        "escaped": escaped,
    }
    if escaped:
        raise ValidationError(json.dumps(receipt, sort_keys=True))
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--mutant-suite", action="store_true")
    args = parser.parse_args()
    repo = args.repo.resolve()
    try:
        receipt = mutant_suite(repo) if args.mutant_suite else validate_data(repo, artifact_data(repo))
    except ValidationError as error:
        print(json.dumps({"status": "fail", "error": str(error)}, indent=2, sort_keys=True))
        return 1
    print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
