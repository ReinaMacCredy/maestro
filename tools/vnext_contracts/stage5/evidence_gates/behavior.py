"""Exact compiled-behavior runner shared by the independent Python Stage 5 engines."""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
from pathlib import Path
from typing import Any


EXPECTED_RUNS = (
    (
        "assessment-kernel",
        "maestro",
        (
            "domain::evidence::assessment::tests::all_same_result_assessments_remain_applicable_without_newest_selection",
            "domain::evidence::assessment::tests::applicability_binds_the_complete_historical_time_basis",
            "domain::evidence::assessment::tests::claim_assessment_requires_exact_resolved_observations",
            "domain::evidence::assessment::tests::closed_presence_rules_cannot_self_attest_gate_satisfaction",
            "domain::evidence::assessment::tests::composite_assessment_consumes_exact_child_resolutions",
            "domain::evidence::assessment::tests::closed_semantic_evaluator_can_pass_fail_and_derive_satisfaction",
            "domain::evidence::assessment::tests::conflict_invalidation_and_expiry_never_prefer_pass",
            "domain::evidence::assessment::tests::foreign_work_and_contract_claims_are_rejected_before_evaluation",
            "domain::evidence::assessment::tests::leaf_assessment_uses_pinned_evaluator_and_conservative_freshness",
            "domain::evidence::assessment::tests::quorum_requires_pairwise_contributor_and_source_independence",
            "domain::evidence::assessment::tests::security_erasure_is_authorized_and_couples_all_invalidations",
            "domain::evidence::assessment::tests::step_claim_subject_helper_remains_generation_scoped",
            "domain::evidence::assessment::tests::step_scope_and_mixed_authority_inputs_are_exact",
            "domain::evidence::assessment::tests::stored_assessment_decoder_rejects_self_consistent_duplicate_inputs",
            "domain::evidence::assessment::tests::trusted_time_and_store_domain_fail_closed",
        ),
    ),
    (
        "submission-evidence-join",
        "maestro",
        (
            "domain::execution::store::tests::competing_step_submissions_have_one_atomic_winner",
            "domain::execution::store::tests::step_submission_and_renewal_race_has_one_atomic_winner",
            "domain::execution::store::tests::step_submission_and_takeover_boundary_proves_both_atomic_linearizations",
            "domain::execution::store::tests::step_submission_one_and_many_claims_are_atomic_restart_decodable_and_idempotent",
            "domain::execution::store::tests::step_submission_rejects_empty_and_wrong_fence_claim_sets_before_publication",
        ),
    ),
    (
        "authorized-evidence-store",
        "maestro",
        (
            "domain::authority::action_basis::tests::downstream_leaves_are_materialized_but_have_no_stage_five_admission_basis",
            "domain::authority::action_basis::tests::stage_five_owner_dispatch_is_total_and_never_admits_a_later_owner",
            "domain::authority::downstream_action_basis::tests::scheduling_policy_publication_has_one_named_exact_typed_leaf",
            "domain::authority::materialization::tests::downgrade_mandate_and_action_binding_are_one_use_and_exact",
            "domain::authority::materialization::tests::equivalent_policy_and_cross_transaction_substitution_refuse_without_consumption",
            "domain::authority::materialization::tests::wrong_owner_action_cannot_enter_the_scheduling_binding",
            "domain::authority::governance_floor::tests::explicit_legacy_migration_is_a_distinct_genesis_basis",
            "domain::authority::governance_floor::tests::rotation_requires_action_68_and_a_gap_free_advancing_lineage",
            "domain::authority::governance_floor::tests::restore_or_gap_in_floor_history_refuses_while_the_exact_same_domain_chain_passes",
            "domain::authority::governance_floor::tests::semantic_or_action_105_substitution_refuses",
            "domain::authority::governance_floor::tests::tag_25_snapshot_round_trips_and_preserves_the_action_105_totality_row",
            "domain::authority::facade::tests::actual_scheduling_weakening_requires_a_live_mandate_and_writes_nothing_without_it",
            "domain::authority::facade::tests::scheduling_publication_derives_the_unique_active_root_inside_authority",
            "domain::authority::facade::repository_leaf_authority::tests::inert_downstream_leaves_cannot_enter_the_stage_five_authority_carrier",
            "domain::authority::facade::tests::protected_continuity_diagnostic_guard_is_non_oracular_across_subjects",
            "domain::authority::facade::tests::protected_continuity_diagnostic_guard_is_subject_bound_and_zero_write",
            "domain::authority::facade::tests::protected_continuity_diagnostic_guard_refuses_noncurrent_human_facts",
            "domain::authority::facade::tests::inactive_store_refusal_mints_no_diagnostic_invocation",
            "domain::authority::facade::tests::protected_continuity_diagnostic_failed_subject_consumes_host_authentication_event",
            "domain::authority::facade::tests::protected_continuity_diagnostic_zero_subject_consumes_host_authentication_event",
            "domain::authority::facade::tests::protected_continuity_diagnostic_final_recheck_rejects_host_claim_turnover",
            "domain::authority::facade::tests::protected_continuity_diagnostic_final_recheck_rejects_host_fence_turnover",
            "domain::authority::facade::tests::protected_continuity_diagnostic_joins_every_independent_host_identity_dimension",
            "domain::authority::facade::tests::protected_continuity_diagnostic_refuses_ambiguous_operator_mapping",
            "domain::authority::facade::tests::protected_continuity_diagnostic_refuses_every_substituted_store_anchor_dimension",
            "domain::authority::facade::tests::protected_continuity_diagnostic_refuses_missing_duplicate_and_stale_authority_roots",
            "domain::authority::facade::tests::protected_continuity_diagnostic_selects_one_authority_root_in_a_heterogeneous_generation",
            "domain::authority::facade::tests::session_request_commitment_is_snapshot_identity_not_host_authority",
            "domain::evidence::store::tests::authorized_store_cut_and_security_erasure_are_restart_safe",
            "domain::persistence::store::tests::controlled_copy_census_fails_closed_on_a_renamed_export_carrier",
            "domain::persistence::store::tests::controlled_copy_census_includes_an_orphan_pre_receipt_export",
            "domain::persistence::store::tests::controlled_copy_erasure_recovery_accepts_only_monotonic_disappearance",
            "domain::persistence::store::tests::failed_sealer_cleanup_cannot_unlink_a_waiting_sealers_committed_carrier",
            "domain::persistence::store::tests::hard_link_race_blocks_controlled_copy_absence_receipt_after_restart",
            "domain::persistence::tests::atomic_publication::historical_idempotency_result_is_a_durable_replay_horizon_after_head_advance",
            "domain::persistence::idempotency::tests::atomic_publication_rejects_unreachable_supplied_objects",
            "domain::persistence::idempotency::tests::generation_closure_rejects_a_missing_referenced_object",
            "domain::persistence::idempotency::tests::publication_builder_reduces_a_superset_to_the_exact_generation_closure",
            "domain::repository::tests::work_completion_atomically_persists_claim_gate_and_submission_proof",
            "domain::repository::tests::work_completion_requires_and_commits_the_exact_current_satisfied_step_submission_closure",
            "domain::execution::h3_withdrawal_publication::tests::all_three_homes_require_the_exact_causal_branch_and_one_use_finality",
            "domain::execution::h3_withdrawal_publication::tests::cross_branch_and_complete_meaning_substitution_refuse_without_consumption",
            "domain::execution::h3_withdrawal_publication::tests::pre_store_has_no_destination_root_or_candidate_seal_field",
            "domain::installation::consumer_snapshot::tests::caller_gate_stage_and_post_issue_currentness_substitution_refuse",
            "domain::installation::consumer_snapshot::tests::owner_issued_snapshot_joins_store_and_host_until_both_final_rechecks",
            "domain::installation::consumer_snapshot::tests::pre_store_is_pre_currentness_only_and_has_no_final_root_or_candidate_seal",
            "domain::installation::durable_finality::tests::active_store_no_op_cannot_mint_finality_from_an_echoed_nonzero_digest",
            "domain::installation::durable_finality::tests::active_store_owner_effect_and_readback_are_one_typed_operation",
            "domain::installation::durable_finality::tests::false_success_and_partial_readback_cannot_mint_finality",
            "domain::installation::durable_finality::tests::post_write_outcomes_are_never_reported_as_ordinary_refusal",
            "domain::installation::durable_finality::tests::production_owner_entry_points_are_frozen_for_stage9_and_stage11",
            "domain::installation::durable_finality_stage9_seed::tests::stage9_owner_test_provider_is_constructible_only_in_its_owner_module",
            "domain::installation::durable_finality_stage11_seed::tests::stage11_owner_test_provider_is_constructible_only_in_its_owner_module",
            "domain::persistence::protected_locator_lease::tests::owner_observes_and_rereads_the_same_locator_through_finality",
            "domain::persistence::protected_locator_lease::tests::pre_store_false_success_with_a_write_is_rejected_through_the_same_live_lease",
            "domain::persistence::protected_locator_lease::tests::pre_store_hands_the_live_lease_to_the_ceremony_cas_continuation",
            "domain::persistence::protected_locator_lease::tests::stale_pre_dispatch_tuple_refuses_without_cas",
            "domain::persistence::protected_locator_lease::tests::unknown_and_old_root_readback_preserve_recovery_laws",
            "foundation::core::aggregate_census::tests::aliases_and_aggregate_overflow_refuse",
            "foundation::core::aggregate_census::tests::optional_absence_overlap_and_early_fence_release_refuse",
            "foundation::core::aggregate_census::tests::owner_holds_the_complete_root_set_across_both_passes",
            "foundation::core::aggregate_census::tests::partial_or_sequential_component_results_refuse",
            "foundation::core::aggregate_census::tests::production_stage11_seed_is_fail_closed_until_the_backend_integrates",
            "foundation::core::aggregate_census_stage11_seed::tests::test_provider_is_owner_local_and_fail_closed",
            "foundation::core::secure_fs::tests::descriptor_census_binds_regular_files_and_symlinks_without_following",
            "foundation::core::secure_fs::tests::descriptor_census_refuses_every_hard_linked_leaf",
            "foundation::core::secure_fs::tests::descriptor_census_refuses_mutation_fence_turnover",
            "foundation::core::secure_fs::tests::digest_addressed_removal_recovers_after_payload_unlink_and_marker_crashes",
            "foundation::core::secure_fs::tests::digest_addressed_removal_recovers_after_the_quarantine_rename",
            "foundation::core::secure_fs::tests::crash_residual_temp_blocks_absence_until_digest_bound_cleanup",
            "foundation::core::secure_fs::tests::hard_link_after_sentinel_check_never_publishes_resolution",
            "foundation::core::secure_fs::tests::hard_link_race_leaves_durable_removal_debt_across_restart",
        ),
    ),
    (
        "work-completion-boundary",
        "vnext_work_lifecycle",
        ("pure_lifecycle_appends_revision_facts_and_refuses_unverified_completion",),
    ),
    (
        "claim-contracts",
        "vnext_evidence_claims",
        (
            "authoritative_claim_set_has_no_second_claim_count_cap",
            "authoritative_claim_set_is_derived_only_from_claims_bound_to_one_submission",
            "claim_identity_and_record_bind_one_exact_submission_deterministically",
            "stage3_claim_and_work_submission_v1_vectors_remain_exact",
            "zero_or_missing_claim_identity_material_is_rejected_before_publication",
        ),
    ),
    (
        "submission-claim-carrier",
        "vnext_submission_claim_set",
        (
            "freezes_one_and_many_claim_vectors",
            "freezes_the_schema_identity_and_rejects_shape_mutants",
            "reference_encoder_matches_the_rust_encoder",
            "rejects_every_malformed_set_product",
        ),
    ),
    (
        "evidence-gate-contracts",
        "vnext_stage5_evidence_gates",
        (
            "claim_publication_requires_exact_resolved_observation_records",
            "composite_gate_grammars_are_fail_closed_and_order_independent",
            "gate_snapshot_is_canonical_closed_and_root_reachable",
            "observation_kind_runtime_matches_all_frozen_catalog_semantics",
            "payload_manifest_requires_current_authenticated_zero_secret_scan",
            "observation_publication_route_rejects_wrong_action_route_and_profile",
            "observations_bind_effect_free_and_exact_derivation_provenance",
            "pure_composite_evaluator_refuses_leaf_self_attestation",
        ),
    ),
    (
        "diagnostic-architecture",
        "architecture_imports",
        (
            "stage5_protected_diagnostic_ports_are_sealed_test_only_and_non_bearer",
            "stage5_successor_seams_are_owner_private_and_production_replaceable",
        ),
    ),
)
EXPECTED_TESTS = sum(len(row[2]) for row in EXPECTED_RUNS)
EXPECTED_BEHAVIOR_MANIFEST_IDENTITY = (
    "sha256:5cd440155141ef65814ef42aadf9ad27f1f02a9557a0155d71ba905b72b2a65e"
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode("ascii")


def behavior_manifest_rows() -> list[list[str]]:
    rows = [[target, test] for _, target, tests in EXPECTED_RUNS for test in tests]
    if len(rows) != EXPECTED_TESTS or len({tuple(row) for row in rows}) != EXPECTED_TESTS:
        raise RuntimeError("Stage 5 behavior manifest is not an exact unique target/test closure")
    return rows


def behavior_manifest_identity() -> str:
    return f"sha256:{sha256(canonical_json(behavior_manifest_rows()))}"


def semantic_test_receipt(
    target: str,
    test_name: str,
    stdout: bytes,
    stderr: bytes,
    returncode: int,
) -> dict[str, object]:
    output = stdout + stderr
    match = re.search(rb"test result: ok\. (\d+) passed; 0 failed", output)
    passed = int(match.group(1)) if match else -1
    if returncode != 0 or passed != 1:
        raise RuntimeError(output[-8_000:].decode("utf-8", errors="replace"))
    return {
        "command": [target, test_name, "--exact", "--nocapture"],
        "name": test_name,
        "result": "pass",
    }


def proof_environment(rustc: Path) -> dict[str, str]:
    required = (
        "AR",
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
        "CC",
        "CXX",
        "HOME",
        "PATH",
        "RANLIB",
        "SDKROOT",
        "TEMP",
        "TMP",
        "TMPDIR",
    )
    missing = [name for name in required if name not in os.environ]
    if missing:
        raise RuntimeError(f"proof environment is incomplete: {missing}")
    return {
        name: os.environ[name]
        for name in required
    } | {
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "LANG": "C",
        "LC_ALL": "C",
        "MAESTRO_VERSION": os.environ["MAESTRO_VERSION"],
        "RUSTC": str(rustc),
        "TZ": "UTC",
    }


def compiled_behavior(cargo: Path, rustc: Path, workspace: Path) -> list[dict[str, Any]]:
    if behavior_manifest_identity() != EXPECTED_BEHAVIOR_MANIFEST_IDENTITY:
        raise RuntimeError("Stage 5 behavior manifest identity differs")
    environment = proof_environment(rustc)
    compile_command = (
        str(cargo),
        "test",
        "--frozen",
        "--offline",
        "--no-run",
        "--message-format=json",
        "--lib",
        "--test",
        "vnext_evidence_claims",
        "--test",
        "vnext_submission_claim_set",
        "--test",
        "vnext_work_lifecycle",
        "--test",
        "vnext_stage5_evidence_gates",
        "--test",
        "architecture_imports",
    )
    compiled = subprocess.run(
        compile_command,
        cwd=workspace,
        env=environment,
        capture_output=True,
        check=False,
    )
    if compiled.returncode != 0:
        raise RuntimeError(compiled.stderr[-8_000:].decode("utf-8", errors="replace"))
    executables: dict[str, Path] = {}
    for line in compiled.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact" or not message.get("profile", {}).get(
            "test"
        ):
            continue
        executable = message.get("executable")
        target = message.get("target", {})
        name = target.get("name")
        if executable and name in {row[1] for row in EXPECTED_RUNS}:
            path = Path(executable).resolve(strict=True)
            if name in executables and executables[name] != path:
                raise RuntimeError(f"compiled test target {name} is ambiguous")
            executables[name] = path
    expected_targets = {row[1] for row in EXPECTED_RUNS}
    if set(executables) != expected_targets:
        raise RuntimeError(
            f"compiled test target closure differs: {sorted(executables)} != {sorted(expected_targets)}"
        )
    receipts = []
    for label, target, test_names in EXPECTED_RUNS:
        executable = executables[target]
        test_receipts = []
        for test_name in test_names:
            args = [test_name, "--exact", "--nocapture"]
            completed = subprocess.run(
                [str(executable), *args],
                cwd=workspace,
                env=environment,
                capture_output=True,
                check=False,
            )
            test_receipts.append(
                semantic_test_receipt(
                    target,
                    test_name,
                    completed.stdout,
                    completed.stderr,
                    completed.returncode,
                )
            )
        receipts.append(
            {
                "binary_sha256": sha256(executable.read_bytes()),
                "label": label,
                "passed": len(test_names),
                "tests": test_receipts,
            }
        )
    target = EXPECTED_RUNS[0][1]
    exact_name = EXPECTED_RUNS[0][2][0]
    substituted_name = f"{exact_name}_same_count_substitution_mutant"
    executable = executables[target]
    args = [substituted_name, "--exact", "--nocapture"]
    completed = subprocess.run(
        [str(executable), *args],
        cwd=workspace,
        env=environment,
        capture_output=True,
        check=False,
    )
    output = completed.stdout + completed.stderr
    match = re.search(rb"test result: ok\. (\d+) passed; 0 failed", output)
    passed = int(match.group(1)) if match else -1
    if completed.returncode != 0 or passed != 0:
        raise RuntimeError("same-count exact-test substitution was not rejected")
    receipts.append(
        {
            "binary_sha256": sha256(executable.read_bytes()),
            "command": [target, *args],
            "label": "same-count-substitution-mutant",
            "passed": 0,
            "rejected": True,
            "result": "rejected",
            "substituted_for": exact_name,
        }
    )
    return receipts
