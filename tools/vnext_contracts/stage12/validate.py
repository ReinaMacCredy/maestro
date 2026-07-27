#!/usr/bin/env python3
"""Validate Stage 12 read-only inputs without publishing proof or receipts."""

from __future__ import annotations

import argparse
import copy
import json
import sys
from pathlib import Path, PurePosixPath
from typing import Any, Callable, Mapping, cast


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[2]
FIXTURES = WORKSPACE / "tests/fixtures/vnext/stage12"
POLICY_PATH = FIXTURES / "consumer-census-policy.v1.json"
NEGATIVE_PATH = FIXTURES / "negative-compatibility.v1.json"
RELEASE_PATH = FIXTURES / "release-proof-inputs.v1.json"
POLICY_SCHEMA = "maestro.test-only.vnext-stage12-consumer-census-policy.v1"
NEGATIVE_SCHEMA = "maestro.test-only.vnext-stage12-negative-compatibility.v1"
RELEASE_SCHEMA = "maestro.test-only.vnext-stage12-release-proof-inputs.v1"
FANOUT_COMMIT = "7080fb6cd1e286998ff47fb6205e90dca990ba40"
FANOUT_TREE = "926f6f0f6a169716a8815105adc8609ac289c717"
DESIGN_SHA256 = "3832e005dd165cf47366d4df048f12e5ce633f61c95f5893778c4c57a7029dac"
FANOUT_MANIFEST_SCHEMA = "maestro.external.vnext-successor-fanout.v3"
FANOUT_MANIFEST_SHA256 = (
    "22160433bbf9784317b76fbe2784cd8f155fc7ee83f43e23f47453d564c64cb8"
)
FANOUT_MANIFEST_STATE = "design_locked_not_dispatch_ready"
MATERIALIZATION_DECISIONS = {
    "dec-canonical-authority-materialization-df3b": (
        "0d7c406f68f04fdf47ce00d56e8189b54159f164323c9511504790b941f715d0"
    ),
    "dec-canonical-execution-h3-verified-0939": (
        "b5935c389182a7f3ec6447fb2a13dcb70e912108b399d0b1d25fee5f132186a7"
    ),
    "dec-canonical-foundation-descriptor-a128": (
        "17fb79ef9bc74cf3838d869bf5fb3b0ae0e9ae017670ca7cb207aeb8105c234e"
    ),
    "dec-canonical-installation-consumer-c1fe": (
        "aaba56a8f34fb293a68f26743fbf4ef879d9f5a399a4eb45da74eed70a509e53"
    ),
}
EXPECTED_RULE_IDS = (
    "temporary_vnext_source_path",
    "temporary_domain_namespace_reference",
    "temporary_domain_module_export",
    "legacy_skill_surface",
    "legacy_next_surface",
    "legacy_harness_resource",
)
EXPECTED_CASE_IDS = (
    "temporary-domain-namespace-import",
    "legacy-skill-name",
    "legacy-mcp-mutation-tool",
    "packet-without-recipe-binding",
    "old-five-branch-packet-read",
    "caller-built-recipe-application",
    "supplied-frontier-reference",
    "recipe-unattended-type-confusion",
    "cancel-like-label-promotion",
    "mixed-release-request",
    "caller-forged-scheduling-materialization",
    "migration-minted-h3-withdrawal",
    "pathname-census-substitution",
    "caller-attested-consumer-zero",
    "removal-without-fresh-authority-or-rollback",
    "candidate-ancestry-substitution",
)
EXPECTED_EXTERNAL_SLOTS = (
    "stage6_integrated_unsealed_checkpoint",
    "stage7_integrated_unsealed_checkpoint",
    "stage8_integrated_unsealed_checkpoint",
    "stage9_integrated_unsealed_checkpoint",
    "stage10_integrated_unsealed_checkpoint",
    "stage11_integrated_unsealed_checkpoint",
    "stage6_through_stage11_ordered_integration_ancestry",
    "stage11_migration_association_closure",
    "stage11_active_consumer_closure",
    "stage11_retained_reader_manifest",
    "stage11_retention_hold_manifest",
    "stage12_candidate_ancestry_and_ownership",
    "stage12_frozen_interface_readback",
    "post_promotion_namespace_census",
    "source_move_identity_parity",
    "negative_compatibility_execution",
    "migration_rollback_rehearsal",
    "fresh_removal_authority_receipt",
    "dogfood_recovery_rehearsal",
    "final_full_chain_seal",
)
EXPECTED_OBLIGATIONS = (
    "ordered_stage6_through_stage11_integration_ancestry",
    "active_consumer_zero",
    "sealed_reader_zero",
    "retention_hold_zero",
    "migration_association_and_finality_closure",
    "candidate_ancestry_and_owned_path_closure",
    "frozen_interface_byte_identity",
    "temporary_namespace_zero",
    "canonical_owner_facade_parity",
    "content_identity_byte_parity",
    "negative_compatibility_refusal",
    "fresh_removal_authority",
    "retained_audit_rollback_closure",
    "dogfood_and_recovery_rehearsal",
    "final_edge_sweep",
)
sys.dont_write_bytecode = True
sys.path.insert(0, str(TOOLS))

from architecture_guard import (  # type: ignore[import-not-found]  # noqa: E402
    ArchitectureGuardError,
    evaluate,
    parse_evidence,
)
from census import (  # type: ignore[import-not-found]  # noqa: E402
    CensusError,
    build_census,
    canonical_json,
    validate_policy as validate_census_policy,
)


class ValidationError(RuntimeError):
    """A Stage 12 provisional input violates its test-only contract."""


def _reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise ValidationError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def load_fixture(path: Path) -> dict[str, Any]:
    try:
        data = path.read_bytes()
    except OSError as error:
        raise ValidationError(f"cannot read Stage 12 fixture {path}: {error}") from error
    if data.startswith(b"\xef\xbb\xbf") or b"\r" in data:
        raise ValidationError(f"fixture must be UTF-8 LF-only without BOM: {path}")
    if not data.endswith(b"\n") or data.endswith(b"\n\n"):
        raise ValidationError(f"fixture must have exactly one final LF: {path}")
    for line_number, line in enumerate(data[:-1].split(b"\n"), start=1):
        if line.rstrip(b" \t") != line:
            raise ValidationError(
                f"fixture has trailing whitespace at {path}:{line_number}"
            )
    try:
        value = json.loads(data, object_pairs_hook=_reject_duplicates)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValidationError(f"invalid Stage 12 fixture {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValidationError(f"Stage 12 fixture must be one object: {path}")
    return cast(dict[str, Any], value)


def _safe_relative(value: str) -> None:
    if not value or value.startswith("/") or "\\" in value:
        raise ValidationError(f"unsafe repository-relative path: {value!r}")
    parts = PurePosixPath(value).parts
    if not parts or any(part in {"", ".", ".."} for part in parts):
        raise ValidationError(f"non-normalized repository-relative path: {value!r}")


def validate_policy(policy: Mapping[str, Any]) -> None:
    if policy.get("schema_version") != POLICY_SCHEMA:
        raise ValidationError("consumer census policy schema differs")
    if policy.get("authority_state") != "noncanonical_read_only_test_input":
        raise ValidationError("consumer census policy authority state differs")
    if (
        policy.get("fanout_commit") != FANOUT_COMMIT
        or policy.get("fanout_tree") != FANOUT_TREE
        or policy.get("design_sha256") != DESIGN_SHA256
        or policy.get("fanout_manifest_schema") != FANOUT_MANIFEST_SCHEMA
        or policy.get("fanout_manifest_sha256") != FANOUT_MANIFEST_SHA256
        or policy.get("fanout_manifest_state") != FANOUT_MANIFEST_STATE
        or policy.get("fanout_manifest_preservation_only") is not True
        or policy.get("materialization_decisions") != MATERIALIZATION_DECISIONS
    ):
        raise ValidationError("consumer census policy input binding differs")
    coverage = policy.get("coverage")
    if not isinstance(coverage, dict) or coverage != {
        "closed_world": False,
        "kind": "provisional_stage12_seed",
        "requires_ordered_stage6_through_stage11_integration": True,
        "requires_stage11_consumer_closure": True,
        "semantics": "literal_and_path_evidence_only",
    }:
        raise ValidationError("consumer census policy overstates its coverage")
    try:
        rules = validate_census_policy(policy)
    except CensusError as error:
        raise ValidationError(str(error)) from error
    if tuple(rule["id"] for rule in rules) != EXPECTED_RULE_IDS:
        raise ValidationError("consumer census policy rule closure differs")


def validate_negative_fixture(value: Mapping[str, Any]) -> None:
    if value.get("schema_version") != NEGATIVE_SCHEMA:
        raise ValidationError("negative compatibility fixture schema differs")
    if value.get("authority_state") != "noncanonical_test_fixture":
        raise ValidationError("negative compatibility fixture authority differs")
    if (
        value.get("design_sha256") != DESIGN_SHA256
        or value.get("fanout_manifest_schema") != FANOUT_MANIFEST_SCHEMA
        or value.get("fanout_manifest_sha256") != FANOUT_MANIFEST_SHA256
        or value.get("fanout_manifest_state") != FANOUT_MANIFEST_STATE
        or value.get("fanout_manifest_preservation_only") is not True
        or value.get("materialization_decisions") != MATERIALIZATION_DECISIONS
    ):
        raise ValidationError("negative compatibility design binding differs")
    if value.get("claim_state") != "fixtures_only_not_executed_proof":
        raise ValidationError("negative compatibility fixture claims proof")
    cases = value.get("cases")
    if not isinstance(cases, list) or not all(isinstance(case, dict) for case in cases):
        raise ValidationError("negative compatibility cases must be an object array")
    typed_cases = cast(list[dict[str, Any]], cases)
    if tuple(case.get("id") for case in typed_cases) != EXPECTED_CASE_IDS:
        raise ValidationError("negative compatibility case closure differs")
    for case in typed_cases:
        if set(case) != {
            "expected_outcome",
            "id",
            "input",
            "input_kind",
            "prohibited_behaviors",
            "surface",
        }:
            raise ValidationError(f"negative case {case.get('id')} fields differ")
        if case["expected_outcome"] not in {
            "refuse_incompatible",
            "remain_sealed_non_executable",
        }:
            raise ValidationError(f"negative case {case['id']} is not refusal-only")
        if not isinstance(case["input"], dict) or not case["input"]:
            raise ValidationError(f"negative case {case['id']} has no input")
        prohibited = case["prohibited_behaviors"]
        if (
            not isinstance(prohibited, list)
            or len(prohibited) < 2
            or not all(isinstance(item, str) and item for item in prohibited)
            or len(prohibited) != len(set(prohibited))
        ):
            raise ValidationError(
                f"negative case {case['id']} prohibited behaviors are incomplete"
            )


def validate_release_inputs(value: Mapping[str, Any]) -> None:
    if value.get("schema_version") != RELEASE_SCHEMA:
        raise ValidationError("release-proof input schema differs")
    if value.get("authority_state") != "none":
        raise ValidationError("release-proof input must carry no authority")
    if (
        value.get("fanout_commit") != FANOUT_COMMIT
        or value.get("fanout_tree") != FANOUT_TREE
        or value.get("design_sha256") != DESIGN_SHA256
        or value.get("fanout_manifest_schema") != FANOUT_MANIFEST_SCHEMA
        or value.get("fanout_manifest_sha256") != FANOUT_MANIFEST_SHA256
        or value.get("fanout_manifest_state") != FANOUT_MANIFEST_STATE
        or value.get("fanout_manifest_preservation_only") is not True
        or value.get("materialization_decisions") != MATERIALIZATION_DECISIONS
    ):
        raise ValidationError("release-proof input binding differs")
    if value.get("input_state") != "provisional_read_only":
        raise ValidationError("release-proof input state differs")
    claims = value.get("claims")
    if (
        not isinstance(claims, dict)
        or set(claims)
        != {
            "certified",
              "consumer_zero",
              "current",
              "integration_complete",
            "ancestry_verified",
            "migration_closed",
            "namespace_promoted",
            "pruning_authorized",
            "release_ready",
            "removal_authorized",
            "rollback_rehearsed",
            "sealed_reader_zero",
        }
        or any(claims.values())
    ):
        raise ValidationError("release-proof input makes a positive claim")
    if tuple(value.get("external_input_slots", [])) != EXPECTED_EXTERNAL_SLOTS:
        raise ValidationError("release-proof external input closure differs")
    if tuple(value.get("proof_obligations", [])) != EXPECTED_OBLIGATIONS:
        raise ValidationError("release-proof obligation closure differs")
    local_inputs = value.get("local_inputs")
    if not isinstance(local_inputs, list) or not all(
        isinstance(path, str) for path in local_inputs
    ):
        raise ValidationError("release-proof local inputs must be paths")
    for path in local_inputs:
        _safe_relative(path)
    forbidden = value.get("forbidden_operations")
    if not isinstance(forbidden, list) or set(forbidden) != {
        "authority_claim",
        "canonical_namespace_promotion",
        "certification",
        "currentness_claim",
        "legacy_removal",
        "migration_activation",
        "physical_pruning",
        "production_deletion",
        "production_mutation",
        "receipt_publication",
        "removal_execution",
        "stage6_through_stage11_product_mutation",
    }:
        raise ValidationError("release-proof forbidden operation closure differs")


def require_census_sight(census: Mapping[str, Any]) -> None:
    """Refuse a candidate census that lost sight of known-present consumers.

    Candidate state is pre-promotion: every expected rule must still yield
    rows. A rule with zero rows means the census went blind (for example a
    mutated values list), not that consumers were removed; release-time zero
    is judged by the release preflight with bound external inputs instead.
    """
    rule_counts = cast(Mapping[str, int], census["rule_counts"])
    blind = [rule_id for rule_id in EXPECTED_RULE_IDS if not rule_counts.get(rule_id)]
    if blind:
        raise ValidationError(
            "consumer census lost sight of known-present consumers: "
            + ", ".join(blind)
        )


def _expect_rejected(action: Callable[[], None], label: str) -> None:
    try:
        action()
    except ValidationError:
        return
    raise ValidationError(f"mutant was accepted: {label}")


def mutant_suite(
    policy: Mapping[str, Any],
    negative: Mapping[str, Any],
    release: Mapping[str, Any],
) -> dict[str, object]:
    coverage_mutant = copy.deepcopy(policy)
    coverage_mutant["coverage"]["closed_world"] = True
    _expect_rejected(lambda: validate_policy(coverage_mutant), "closed-world census")

    compatibility_mutant = copy.deepcopy(negative)
    compatibility_mutant["cases"][0]["expected_outcome"] = "accept"
    _expect_rejected(
        lambda: validate_negative_fixture(compatibility_mutant),
        "accepted compatibility case",
    )

    authority_mutant = copy.deepcopy(release)
    authority_mutant["claims"]["certified"] = True
    _expect_rejected(
        lambda: validate_release_inputs(authority_mutant),
        "positive certification claim",
    )
    return {
        "accepted_mutants": 0,
        "authority_state": "none",
        "rejected_mutants": 3,
        "status": "pass",
    }


def candidate_validation(repo: Path) -> dict[str, object]:
    policy = load_fixture(POLICY_PATH)
    negative = load_fixture(NEGATIVE_PATH)
    release = load_fixture(RELEASE_PATH)
    validate_policy(policy)
    validate_negative_fixture(negative)
    validate_release_inputs(release)
    first = build_census(repo, policy)
    second = build_census(repo, policy)
    if canonical_json(first) != canonical_json(second):
        raise ValidationError("read-only consumer census is nondeterministic")
    require_census_sight(first)
    guard, exit_code = evaluate(
        repo,
        policy,
        release,
        {},
        release_preflight=False,
    )
    if exit_code != 0 or guard.get("status") != "pass":
        raise ValidationError("candidate architecture guard failed")
    return {
        "authority_state": "none",
        "candidate_ready_claim": False,
        "candidate_state": "stage_12_candidate_read_only_wip_unverified",
        "census_row_count": first["row_count"],
        "census_sha256": first["scan_sha256"],
        "classification_counts": first["classification_counts"],
        "compile_lane_needed": True,
        "negative_case_count": len(cast(list[object], negative["cases"])),
        "release_evaluated": False,
        "scan_warnings": first["scan_warnings"],
        "status": "pass",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=WORKSPACE)
    parser.add_argument(
        "--mode", choices=("candidate", "release-preflight"), default="candidate"
    )
    parser.add_argument("--evidence", action="append", default=[])
    parser.add_argument("--mutant-suite", action="store_true")
    args = parser.parse_args()
    try:
        policy = load_fixture(POLICY_PATH)
        negative = load_fixture(NEGATIVE_PATH)
        release = load_fixture(RELEASE_PATH)
        validate_policy(policy)
        validate_negative_fixture(negative)
        validate_release_inputs(release)
        if args.mutant_suite:
            payload = mutant_suite(policy, negative, release)
            exit_code = 0
        elif args.mode == "candidate":
            payload = candidate_validation(args.repo)
            exit_code = 0
        else:
            payload, exit_code = evaluate(
                args.repo,
                policy,
                release,
                parse_evidence(args.evidence),
                release_preflight=True,
            )
    except (ArchitectureGuardError, CensusError, ValidationError) as error:
        print(json.dumps({"status": "error", "error": str(error)}, sort_keys=True))
        return 1
    print(json.dumps(payload, indent=2, sort_keys=True))
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
