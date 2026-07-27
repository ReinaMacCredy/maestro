#!/usr/bin/env python3
"""Read-only Stage-7 candidate contract and architecture validator."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

REPOSITORY = Path(__file__).resolve().parents[3]

PUBLIC_FIXTURES = {
    "contracts/vnext/public/recipe_selection_application_vectors.v1.json": (
        "d286a98d5d6d7146652a5a114bef15ef15e30fe49bab29ed996100c6d2357635"
    ),
    "contracts/vnext/public/recipe_return_reasons.v1.json": (
        "30b80df9a97ff63852bfbf27694660c9aa26a803195c381701acaab85045c215"
    ),
    "contracts/vnext/public/job_recipe_eligibility_vectors.v1.json": (
        "c6d9f2c21b8860302b7efcb0ce843c2953862acd5e024d0a9b975cc165bf1870"
    ),
}

COORDINATION_ACTIONS = [
    (94, "PublishInitialMessage"),
    (95, "PublishMessage"),
    (96, "AcknowledgeMessage"),
    (97, "ReplaceFocus"),
    (98, "WithdrawFocus"),
    (99, "PublishScope"),
    (100, "WithdrawScope"),
    (101, "AssertConflict"),
    (102, "ResolveConflict"),
]
PLANNING_ACTIONS = [
    (103, "PublishPlanningProposal"),
    (104, "DisposePlanningProposal"),
    (105, "PublishSchedulingPolicyBinding"),
    (106, "PublishSchedulingAssessment"),
]

PRODUCTION_STAGE7 = [
    "src/domain/vnext/coordination/model.rs",
    "src/domain/vnext/coordination/projection.rs",
    "src/domain/vnext/coordination/state.rs",
    "src/domain/vnext/orchestration/runtime/advice.rs",
    "src/domain/vnext/orchestration/runtime/catalog.rs",
    "src/domain/vnext/orchestration/runtime/continuation.rs",
    "src/domain/vnext/planning/evaluation.rs",
    "src/domain/vnext/planning/model.rs",
    "src/domain/vnext/planning/publication.rs",
    "src/domain/vnext/planning/state.rs",
    "src/operations/vnext/orchestration/mod.rs",
]


def fail(message: str) -> None:
    raise ValueError(message)


def read_json(relative: str) -> dict:
    return json.loads((REPOSITORY / relative).read_text(encoding="utf-8"))


def sha256(relative: str) -> str:
    return hashlib.sha256((REPOSITORY / relative).read_bytes()).hexdigest()


def validate_public_fixtures() -> None:
    for relative, expected in PUBLIC_FIXTURES.items():
        if sha256(relative) != expected:
            fail(f"frozen Stage-0 public fixture changed: {relative}")
    selections = read_json(next(iter(PUBLIC_FIXTURES)))
    returns = read_json(
        "contracts/vnext/public/recipe_return_reasons.v1.json"
    )
    eligibility = read_json(
        "contracts/vnext/public/job_recipe_eligibility_vectors.v1.json"
    )
    if (
        selections["vector_count"] != 30
        or len(selections["vectors"]) != 30
        or len(selections["primary_axis"]) != 10
        or len(selections["continuation_axis"]) != 3
    ):
        fail("Recipe selection/application fixture is not the exact 10 x 3 closure")
    if (
        returns["member_count"] != 30
        or returns["manifest_subset_count"] != 10
        or returns["application_outcome_vector_count"] != 196
    ):
        fail("Recipe return fixture is not the exact frozen closure")
    if (
        eligibility["application_vector_count"] != 210
        or eligibility["positive_edges"] != 22
        or eligibility["negative_edges"] != 48
    ):
        fail("Job/Recipe eligibility fixture is not the exact frozen closure")


def validate_catalog_bytes() -> None:
    catalog_path = (
        REPOSITORY / "embedded/vnext/orchestration/recipe-catalog.v1.json"
    )
    catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
    if (
        catalog["schema"] != "maestro.vnext.recipe-catalog-source.v1"
        or not catalog["candidate_only"]
        or catalog["runtime_activation"]
        or catalog["runtime_registration"]
        or len(catalog["recipes"]) != 10
        or len(catalog["bounded_continuation_profiles"]) != 2
    ):
        fail("embedded Recipe catalog is not the candidate-only ten-plus-two closure")

    embedded_root = catalog_path.parent
    manifest_refs: dict[str, str] = {}
    for row in catalog["recipes"]:
        path = embedded_root / row["manifest_path"]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        manifest_refs[row["id"]] = f"sha256:{digest}"
    profile_refs: dict[str, str] = {}
    for row in catalog["bounded_continuation_profiles"]:
        path = embedded_root / row["resource_path"]
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        profile_refs[row["id"]] = (
            "candidate:orchestration:bounded-continuation-profile:"
            f"{row['id']}:v1@sha256:{digest}"
        )

    vectors = read_json(
        "contracts/vnext/public/recipe_selection_application_vectors.v1.json"
    )["vectors"]
    for vector in vectors:
        primary = vector["selection_request"]["primary_selection"]
        if primary["variant"] == "Present":
            recipe_ref, manifest_ref = primary["value"]
            recipe_id = recipe_ref.removeprefix(
                "candidate:orchestration:recipe:"
            ).removesuffix(":v1")
            if manifest_refs.get(recipe_id) != manifest_ref:
                fail(f"primary manifest bytes drifted for {recipe_id}")
        continuation = vector["selection_request"]["continuation_selection"]
        if continuation["variant"] == "Present":
            recipe_ref, manifest_ref, profile_ref = continuation["value"]
            if (
                recipe_ref
                != "candidate:orchestration:recipe:bounded-continuation:v1"
                or manifest_refs["bounded-continuation"] != manifest_ref
                or profile_ref not in profile_refs.values()
            ):
                fail("bounded-continuation manifest or profile bytes drifted")


def validate_action_rows() -> None:
    catalog = read_json(
        "contracts/vnext/catalogs/generated/catalog-06-action-leaf.json"
    )
    rows = {
        descriptor["value"][0]: descriptor["value"][1]
        for descriptor in catalog["descriptors"]
    }
    expected = dict(COORDINATION_ACTIONS + PLANNING_ACTIONS)
    if {tag: rows.get(tag) for tag in expected} != expected:
        fail("frozen Action catalog tags 94..106 do not match Stage-7 owners")

    coordination = (
        REPOSITORY / "src/domain/vnext/coordination/state.rs"
    ).read_text(encoding="utf-8")
    planning = (
        REPOSITORY / "src/domain/vnext/planning/state.rs"
    ).read_text(encoding="utf-8")
    for _, literal in COORDINATION_ACTIONS:
        if coordination.count(f'"{literal}"') != 1:
            fail(f"Coordination Action literal is missing or duplicated: {literal}")
    for _, literal in PLANNING_ACTIONS:
        if planning.count(f'"{literal}"') != 1:
            fail(f"Planning Action literal is missing or duplicated: {literal}")


def validate_architecture() -> None:
    forbidden = [
        re.compile(r"\bstruct\s+(?:Scheduler|Queue|Retry|WorkerRuntime|Cursor)\b"),
        re.compile(r"\b(?:std|tokio)::(?:net|process|thread|task)\b"),
        re.compile(r"\b(?:sleep|spawn|retry)\s*\("),
    ]
    for relative in PRODUCTION_STAGE7:
        source = (REPOSITORY / relative).read_text(encoding="utf-8")
        for pattern in forbidden:
            if pattern.search(source):
                fail(f"hidden scheduler/retry/effect runtime in {relative}")

    coordination_mod = (
        REPOSITORY / "src/domain/vnext/coordination/mod.rs"
    ).read_text(encoding="utf-8")
    planning_mod = (
        REPOSITORY / "src/domain/vnext/planning/mod.rs"
    ).read_text(encoding="utf-8")
    if (
        "#[cfg(test)]\nmod authority_test_adapter;" not in coordination_mod
        or "#[cfg(test)]\nmod authority_test_adapter;" not in planning_mod
    ):
        fail("Stage-6 Authority parity adapters must remain explicitly test-only")
    for relative in [
        "src/domain/vnext/coordination/authority_test_adapter.rs",
        "src/domain/vnext/planning/authority_test_adapter.rs",
    ]:
        if "frozen Stage-6 Authority facade" not in (
            REPOSITORY / relative
        ).read_text(encoding="utf-8"):
            fail(f"test-only adapter parity/replacement marker missing: {relative}")

    continuation = (
        REPOSITORY
        / "src/domain/vnext/orchestration/runtime/continuation.rs"
    ).read_text(encoding="utf-8")
    for boundary in [
        "MissingAuthority",
        "EvidenceBoundary",
        "MutationBoundary",
        "ExternalEffectBoundary",
        "MaterialAmbiguity",
        "OperatingLimit",
        "Conflict",
        "NoProgress",
        "Terminal",
        "UnknownState",
        "WaveBoundary",
    ]:
        if boundary not in continuation:
            fail(f"bounded-continuation stop boundary missing: {boundary}")

    planning_model = (
        REPOSITORY / "src/domain/vnext/planning/model.rs"
    ).read_text(encoding="utf-8")
    if "downgrade_mandate_consumption_ref" in planning_model:
        fail("Scheduling Policy Binding persists forbidden downgrade authority")

    publication = (
        REPOSITORY / "src/domain/vnext/planning/publication.rs"
    ).read_text(encoding="utf-8")
    if (
        publication.count("publish_scheduling_policy_from_stage7(") != 1
        or "publish_scheduling_policy_without_downgrade" in publication
        or "publish_scheduling_policy_with_downgrade" in publication
        or "admit_repository_action" in publication
        or "RepositoryActionBindingFactsV1" in publication
        or "SchedulingPolicyDowngradeMandateFactsV1" in publication
        or "SchedulingPolicyDowngradeMandateV1" in publication
    ):
        fail("Scheduling publication is not the exact one-call Authority facade path")
    input_match = re.search(
        r"struct SchedulingPolicyPublicationInputV1 \{(?P<body>.*?)\n\}",
        publication,
        re.DOTALL,
    )
    if input_match is None:
        fail("Scheduling publication input is missing")
    forbidden_input = [
        "safety_floor",
        "safety_floor_strength",
        "repository_governance_floor_strength",
        "evaluator_revision",
        "classifier_revision",
        "mandate",
        "path",
    ]
    if any(field in input_match.group("body") for field in forbidden_input):
        fail("Scheduling publication accepts caller-supplied owner or Authority facts")
    forbidden_authority_fact = [
        "governance_floor",
        "evaluator_revision",
        "classifier_revision",
        "authority_epoch",
        "trust_root",
    ]
    if any(fact in publication for fact in forbidden_authority_fact):
        fail("Scheduling publication hands Authority-owned governance facts to the facade")
    for retired_floor_transport in [
        "safety_floor_strength",
        "validated_safety_floor_strength",
        "safety_floor.strength()",
    ]:
        if retired_floor_transport in publication:
            fail("Scheduling publication derives or transports an Authority-owned governance floor")
    for owner_fact in [
        ".scheduling_safety_floor()",
        "PlanningSchedulingPolicyInputV1::from_stage7_planning(",
    ]:
        if owner_fact not in publication:
            fail(f"Scheduling publication does not derive live owner fact: {owner_fact}")
    for mandate_gate in [
        "SemanticPolicyDiffKindV1::Weakening | SemanticPolicyDiffKindV1::Incomparable",
        "SchedulingPolicyPublicationKindV1::WeakeningOrIncomparableWithMandate",
        "SchedulingPolicyPublicationKindV1::EquivalentOrStrengthening",
    ]:
        if mandate_gate not in publication:
            fail(f"Scheduling publication does not gate the downgrade Mandate: {mandate_gate}")


def validate_frozen_authority_seam() -> None:
    seed = (
        REPOSITORY
        / "src/domain/vnext/authority/governance_attestation_stage7_seed.rs"
    ).read_text(encoding="utf-8")
    if (
        seed.count("publish_scheduling_policy_without_downgrade(") != 1
        or seed.count("publish_scheduling_policy_with_downgrade(") != 1
        or "SchedulingPolicyPublicationKindV1::EquivalentOrStrengthening" not in seed
        or "SchedulingPolicyPublicationKindV1::WeakeningOrIncomparableWithMandate"
        not in seed
    ):
        fail("frozen Stage-7 Authority seed is not the two-kind one-call entry")

    facade = (
        REPOSITORY / "src/domain/vnext/authority/facade.rs"
    ).read_text(encoding="utf-8")
    for owner_derivation in [
        "resolve_repository_governance_floor_current_view(",
        "GovernanceAttestationV1::derive(",
    ]:
        if facade.count(owner_derivation) != 1:
            fail(
                "Authority does not derive the live governance view exactly once "
                f"inside the Store transaction: {owner_derivation}"
            )

    attestation = (
        REPOSITORY / "src/domain/vnext/authority/governance_attestation.rs"
    ).read_text(encoding="utf-8")
    for live_fact in [
        "current_view.snapshot().semantic_hash()",
        "current_view.scheduling_evaluator_revision()",
        "current_view.scheduling_classifier_revision()",
    ]:
        if live_fact not in attestation:
            fail(f"Authority does not mint the attestation from live owner facts: {live_fact}")


def main() -> int:
    validate_public_fixtures()
    validate_catalog_bytes()
    validate_action_rows()
    validate_architecture()
    validate_frozen_authority_seam()
    print(
        "stage7 candidate validation: ok "
        "(13 actions, 10 recipes, 2 profiles, 30/196/210 vectors)"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (KeyError, OSError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"stage7 candidate validation: failed: {error}", file=sys.stderr)
        raise SystemExit(1)
