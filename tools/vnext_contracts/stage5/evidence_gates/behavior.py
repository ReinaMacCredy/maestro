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
            "domain::vnext::evidence::assessment::tests::all_same_result_assessments_remain_applicable_without_newest_selection",
            "domain::vnext::evidence::assessment::tests::applicability_binds_the_complete_historical_time_basis",
            "domain::vnext::evidence::assessment::tests::claim_assessment_requires_exact_resolved_observations",
            "domain::vnext::evidence::assessment::tests::closed_presence_rules_cannot_self_attest_gate_satisfaction",
            "domain::vnext::evidence::assessment::tests::composite_assessment_consumes_exact_child_resolutions",
            "domain::vnext::evidence::assessment::tests::closed_semantic_evaluator_can_pass_fail_and_derive_satisfaction",
            "domain::vnext::evidence::assessment::tests::conflict_invalidation_and_expiry_never_prefer_pass",
            "domain::vnext::evidence::assessment::tests::foreign_work_and_contract_claims_are_rejected_before_evaluation",
            "domain::vnext::evidence::assessment::tests::leaf_assessment_uses_pinned_evaluator_and_conservative_freshness",
            "domain::vnext::evidence::assessment::tests::quorum_requires_pairwise_contributor_and_source_independence",
            "domain::vnext::evidence::assessment::tests::security_erasure_is_authorized_and_couples_all_invalidations",
            "domain::vnext::evidence::assessment::tests::step_claim_subject_helper_remains_generation_scoped",
            "domain::vnext::evidence::assessment::tests::step_scope_and_mixed_authority_inputs_are_exact",
            "domain::vnext::evidence::assessment::tests::stored_assessment_decoder_rejects_self_consistent_duplicate_inputs",
            "domain::vnext::evidence::assessment::tests::trusted_time_and_store_domain_fail_closed",
        ),
    ),
    (
        "submission-evidence-join",
        "maestro",
        (
            "domain::vnext::execution::store::tests::competing_step_submissions_have_one_atomic_winner",
            "domain::vnext::execution::store::tests::step_submission_and_renewal_race_has_one_atomic_winner",
            "domain::vnext::execution::store::tests::step_submission_and_takeover_boundary_proves_both_atomic_linearizations",
            "domain::vnext::execution::store::tests::step_submission_one_and_many_claims_are_atomic_restart_decodable_and_idempotent",
            "domain::vnext::execution::store::tests::step_submission_rejects_empty_and_wrong_fence_claim_sets_before_publication",
        ),
    ),
    (
        "authorized-evidence-store",
        "maestro",
        (
            "domain::vnext::authority::action_basis::tests::downstream_leaves_are_materialized_but_have_no_stage_five_admission_basis",
            "domain::vnext::authority::action_basis::tests::stage_five_owner_dispatch_is_total_and_never_admits_a_later_owner",
            "domain::vnext::authority::downstream_action_basis::tests::scheduling_policy_publication_has_one_named_exact_typed_leaf",
            "domain::vnext::authority::materialization::tests::downgrade_mandate_and_action_binding_are_one_use_and_exact",
            "domain::vnext::authority::materialization::tests::equivalent_policy_and_cross_transaction_substitution_refuse_without_consumption",
            "domain::vnext::authority::materialization::tests::wrong_owner_action_cannot_enter_the_scheduling_binding",
            "domain::vnext::authority::facade::repository_leaf_authority::tests::inert_downstream_leaves_cannot_enter_the_stage_five_authority_carrier",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_guard_is_non_oracular_across_subjects",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_guard_is_subject_bound_and_zero_write",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_guard_refuses_noncurrent_human_facts",
            "domain::vnext::authority::facade::tests::inactive_store_refusal_mints_no_diagnostic_invocation",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_failed_subject_consumes_host_authentication_event",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_zero_subject_consumes_host_authentication_event",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_final_recheck_rejects_host_claim_turnover",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_final_recheck_rejects_host_fence_turnover",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_joins_every_independent_host_identity_dimension",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_refuses_ambiguous_operator_mapping",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_refuses_every_substituted_store_anchor_dimension",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_refuses_missing_duplicate_and_stale_authority_roots",
            "domain::vnext::authority::facade::tests::protected_continuity_diagnostic_selects_one_authority_root_in_a_heterogeneous_generation",
            "domain::vnext::authority::facade::tests::session_request_commitment_is_snapshot_identity_not_host_authority",
            "domain::vnext::evidence::store::tests::authorized_store_cut_and_security_erasure_are_restart_safe",
            "domain::vnext::persistence::store::tests::controlled_copy_census_fails_closed_on_a_renamed_export_carrier",
            "domain::vnext::persistence::store::tests::controlled_copy_census_includes_an_orphan_pre_receipt_export",
            "domain::vnext::persistence::store::tests::controlled_copy_erasure_recovery_accepts_only_monotonic_disappearance",
            "domain::vnext::persistence::store::tests::failed_sealer_cleanup_cannot_unlink_a_waiting_sealers_committed_carrier",
            "domain::vnext::persistence::store::tests::hard_link_race_blocks_controlled_copy_absence_receipt_after_restart",
            "domain::vnext::persistence::tests::atomic_publication::historical_idempotency_result_is_a_durable_replay_horizon_after_head_advance",
            "domain::vnext::persistence::idempotency::tests::atomic_publication_rejects_unreachable_supplied_objects",
            "domain::vnext::persistence::idempotency::tests::generation_closure_rejects_a_missing_referenced_object",
            "domain::vnext::persistence::idempotency::tests::publication_builder_reduces_a_superset_to_the_exact_generation_closure",
            "domain::vnext::repository::tests::work_completion_atomically_persists_claim_gate_and_submission_proof",
            "domain::vnext::repository::tests::work_completion_requires_and_commits_the_exact_current_satisfied_step_submission_closure",
            "domain::vnext::execution::h3_withdrawal_publication::tests::all_three_homes_require_the_exact_causal_branch_and_one_use_finality",
            "domain::vnext::execution::h3_withdrawal_publication::tests::cross_branch_and_complete_meaning_substitution_refuse_without_consumption",
            "domain::vnext::execution::h3_withdrawal_publication::tests::pre_store_has_no_destination_root_or_candidate_seal_field",
            "domain::vnext::installation::consumer_snapshot::tests::caller_gate_stage_and_post_issue_currentness_substitution_refuse",
            "domain::vnext::installation::consumer_snapshot::tests::owner_issued_snapshot_joins_store_and_host_until_both_final_rechecks",
            "domain::vnext::installation::consumer_snapshot::tests::pre_store_is_pre_currentness_only_and_has_no_final_root_or_candidate_seal",
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
        ("stage5_protected_diagnostic_ports_are_sealed_test_only_and_non_bearer",),
    ),
)
EXPECTED_TESTS = sum(len(row[2]) for row in EXPECTED_RUNS)
EXPECTED_BEHAVIOR_MANIFEST_IDENTITY = (
    "sha256:fe5df73a47fb802b0ef87afafab04267c0b8a540931c8a6e667749f3a60131a5"
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
