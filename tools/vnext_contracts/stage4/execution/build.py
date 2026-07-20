#!/usr/bin/env python3
"""Build the inactive Stage 4 Execution and effects proof artifact."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[3]
OUTPUT = WORKSPACE / "contracts/vnext/stage4/execution"
sys.dont_write_bytecode = True
sys.path.insert(0, str(WORKSPACE / "tools/vnext_contracts/catalogs"))
import cbor_py  # noqa: E402


DOMAIN = "maestro.vnext.stage4.execution-effects.v1"
PUBLICATION_STATE = "inactive_candidate"
DISPATCH_ATTEMPT = "Dispatch" + "AttemptV1"
RECONCILIATION_ATTEMPT = "Reconciliation" + "AttemptV1"
EFFECT_INTENT = "Effect" + "IntentV1"
EFFECT_CONTROL_HEAD = "Effect" + "Intent" + "Control" + "HeadV1"
WITHDRAWAL_SCHEMA = "Effect" + "Intent" + "WithdrawalV1"
PREDECESSOR_RECEIPTS = {
    "stage0_effect_home": [
        "contracts/vnext/stage0/effect-home/encoder-receipt.json",
        "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json",
    ],
    "stage0_dispatch_cutover": [
        "contracts/vnext/stage0/dispatch-cutover/build-receipt.v1.json",
        "contracts/vnext/stage0/dispatch-cutover/validation-receipt.v1.json",
    ],
    "stage2_authority": [
        "contracts/vnext/stage2/authority/python-encoder-receipt.v1.json",
        "contracts/vnext/stage2/authority/semantic-validation-receipt.v1.json",
        "contracts/vnext/stage2/authority/ruby-verification-receipt.v1.json",
    ],
    "stage3_domain": [
        "contracts/vnext/stage3/domain/python-encoder-receipt.v1.json",
        "contracts/vnext/stage3/domain/semantic-validation-receipt.v1.json",
        "contracts/vnext/stage3/domain/ruby-verification-receipt.v1.json",
    ],
}
PREDECESSOR_MANIFESTS = {
    "stage0_effect_home": "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json",
    "stage0_dispatch_cutover": "contracts/vnext/stage0/dispatch-cutover/validation-receipt.v1.json",
    "stage2_authority": "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json",
    "stage3_domain": "contracts/vnext/stage3/domain/domain-kernel.v1.json",
}
PREDECESSOR_COMMANDS = [
    ["python3", "tools/vnext_contracts/stage0/effect_home/build.py", "--check"],
    ["python3", "tools/vnext_contracts/stage0/effect_home/validate.py", "--mutants"],
    ["python3", "tools/vnext_contracts/stage0/dispatch_cutover/build.py", "--check"],
    [
        "python3",
        "tools/vnext_contracts/stage0/dispatch_cutover/validate.py",
        "--mutant-suite",
        "--no-write",
    ],
    ["python3", "tools/vnext_contracts/stage2/authority/build.py", "--check"],
    ["python3", "tools/vnext_contracts/stage3/domain/build.py", "--check"],
]
CATALOG_PATHS = [
    "contracts/vnext/catalogs/generated/inventory.json",
    "contracts/vnext/catalogs/generated/catalog-profile-grammar-v1.json",
    "contracts/vnext/catalogs/generated/catalog-02-effect.json",
    "contracts/vnext/catalogs/generated/catalog-06-action-leaf.json",
    "contracts/vnext/catalogs/generated/catalog-09-action-spec.json",
]
DISPATCH_PATH = "contracts/vnext/stage0/dispatch-cutover/dispatch-attempt-state.v1.json"
WITHDRAWAL_PATH = "contracts/vnext/stage0/effect-home/effect-withdrawal-v1.json"
COMPILATION_ANCESTORS = [
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "src/lib.rs",
    "src/domain/mod.rs",
    "src/domain/vnext/mod.rs",
    "src/foundation/mod.rs",
    "src/foundation/core/mod.rs",
    "src/foundation/core/deterministic_cbor.rs",
]
AUTHORITY_EXTENSION_SOURCES = [
    "src/domain/vnext/authority/action_basis.rs",
    "src/domain/vnext/authority/continuity/trusted_time.rs",
    "src/domain/vnext/authority/facade.rs",
    "src/domain/vnext/authority/facade/repository_admission.rs",
    "src/domain/vnext/authority/facade/repository_leaf_authority.rs",
    "src/domain/vnext/authority/mod.rs",
]
FOCAL_STEP_EVIDENCE_SOURCES = [
    "src/domain/vnext/evidence/mod.rs",
    "src/domain/vnext/evidence/submission_claim.rs",
    "src/domain/vnext/evidence/claim.rs",
    "src/domain/vnext/step/lifecycle.rs",
    "src/domain/vnext/step/submission.rs",
]
TOOL_SOURCES = [
    "tests/vnext_stage4_contracts.rs",
    "tools/vnext_contracts/catalogs/cbor_py.py",
    "tools/vnext_contracts/stage4/execution/build.py",
    "tools/vnext_contracts/stage4/execution/validate.py",
    "tools/vnext_contracts/stage4/execution/verify.rb",
]
BEHAVIOR_COMMANDS = [
    ["cargo", "test", "--lib", "domain::vnext::execution::", "--", "--nocapture"],
    [
        "cargo",
        "test",
        "--lib",
        "domain::vnext::authority::facade::repository_admission::ancestry_tests",
        "--",
        "--nocapture",
    ],
    [
        "cargo",
        "test",
        "--lib",
        "domain::vnext::authority::continuity::trusted_time::tests",
        "--",
        "--nocapture",
    ],
    [
        "cargo",
        "test",
        "--test",
        "vnext_stage4_contracts",
        "stage4_public_effect_facade_exports_are_complete",
        "--",
        "--nocapture",
    ],
    [
        "cargo",
        "test",
        "--test",
        "vnext_stage4_contracts",
        "runtime_withdrawal_catalog_matches_all_sixty_frozen_rows_and_twenty_one_denials",
        "--",
        "--nocapture",
    ],
    [
        "cargo",
        "test",
        "--test",
        "vnext_effect_home_literals",
        "stage0_effect_home_artifacts_are_reproducible_and_reject_mutants",
        "--",
        "--nocapture",
    ],
]
MUTANT_COMMANDS = [
    [
        "cargo",
        "test",
        "--test",
        "vnext_stage4_contracts",
        "stage4_regenerated_",
        "--",
        "--nocapture",
    ],
    [
        "cargo",
        "test",
        "--test",
        "vnext_stage4_contracts",
        "stage4_proof_rejects_",
        "--",
        "--nocapture",
    ],
    [
        "cargo",
        "test",
        "--test",
        "vnext_stage4_contracts",
        "independent_execution_artifact_rejects_semantic_and_shape_mutants",
        "--",
        "--nocapture",
    ],
]
BEHAVIOR_EXPECTED_PASSED = [70, 5, 1, 1, 1, 1]
MUTANT_EXPECTED_PASSED = [10, 6, 1]
SANITIZED_ENVIRONMENT_KEYS = [
    "CARGO_BUILD_TARGET",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_HOME",
    "CARGO_INCREMENTAL",
    "CARGO_TARGET_DIR",
    "CC",
    "CFLAGS",
    "HOME",
    "LDFLAGS",
    "MACOSX_DEPLOYMENT_TARGET",
    "PATH",
    "RUSTC",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_WRAPPER",
    "RUSTDOC",
    "RUSTFLAGS",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
]
UNSET_BUILD_OVERRIDE_KEYS = [
    "CARGO_BUILD_TARGET",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_TARGET_DIR",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_WRAPPER",
    "RUSTDOC",
    "RUSTFLAGS",
]

_PREDECESSOR_COMMAND_RECEIPTS: list[dict[str, object]] | None = None

EXECUTION_MODEL = [
    [
        "ExecutionAttemptV1",
        "closed_union",
        [
            ["StepAttemptV1", "exact_step_binding_and_live_step_lease_fence", "step_execution_runs_only"],
            [DISPATCH_ATTEMPT, "one_durable_effect_intent_and_dispatch_fence", "dispatch_runs_only"],
            [RECONCILIATION_ATTEMPT, "one_durable_effect_intent_and_fresh_action_request", "reconciliation_runs_only"],
        ],
        "one_run_exactly_one_owner",
        "no_fourth_owner",
    ],
    [
        "StepLeaseV1",
        "step_only",
        "one_to_one_with_step_attempt",
        "exact_generation_step_binding",
        "immutable_contiguous_hash_linked_terms",
        "takeover_requires_exact_owner_receipt_binding_predecessor_term_fences_and_trusted_time_cut",
        "stage4_consumes_owner_issued_takeover_safety_stage5_evidence_owns_production_issuance_and_decode",
        "missing_owner_evidence_fails_closed",
        ["submitted", "yielded", "failed", "cancelled", "timed_out", "lost", "fenced"],
        "terminal_never_reopens",
    ],
    [
        "RunV1",
        "finite_and_owned",
        [
            ["reserved", ["active", "definitely_not_started", "cancelled", "timed_out", "lost", "fenced"]],
            ["active", ["succeeded", "failed", "cancelled", "timed_out", "lost", "fenced"]],
        ],
        "terminal_never_reopens",
          "run_success_is_not_step_or_remote_success",
          "definitely_not_started_requires_non_self_attested_run_boundary_receipt",
          "store_issues_no_start_receipt_from_current_authority_time_and_pinned_boundary_observer",
          "timed_out_only_at_or_after_exact_deadline",
    ],
    [
        EFFECT_INTENT,
        "stable_across_step_amendment_removal_and_recovery",
        "home_qualified_stable_subject_semantic_uniqueness_independent_of_provider_envelope",
        "committed_with_dispatch_reservation_before_io",
        "one_current_control_head",
        "durable_uncertainty",
    ],
    [
        EFFECT_CONTROL_HEAD,
        "sole_mutable_selector",
        ["None", "Reserved", "Sealed"],
        [
            "prepared",
            "dispatching",
            "pending",
            "in_doubt",
            "confirmed_applied",
            "confirmed_not_applied",
            "partially_applied",
            "conflicted",
            "cancelled",
        ],
        "ten_legal_seventeen_denied_products",
        "same_store_atomic_expected_old_publication",
    ],
    [
        RECONCILIATION_ATTEMPT,
        "fresh_authorized_action_request_and_use_fence",
        "same_durable_intent",
        "one_action_admission_and_capacity_debit_at_begin",
        "read_and_terminal_reuse_exact_attempt_authority",
        "no_dispatch",
        "no_lease",
        "no_stale_step_mutation",
          "unknown_may_refine_but_never_infer",
          "read_release_is_ephemeral_non_clone_consumed_once_and_deadline_guarded",
          "replayed_begin_never_reconstructs_read_release",
    ],
    [
        "ExecutionAuthorityV1",
        "closed_union",
        [
            ["Ordinary", "ordinary_live_runtime_only"],
            ["BootstrapG0", "exact_nondelegable_genesis_grant"],
            ["ContinuityMaintenance", "exact_branch_phase_slot_and_executor"],
        ],
        "continuity_slot_binds_purpose_request_subject_epoch_and_job_applicability",
        "exact_leaf_selects_one_basis",
        "no_cross_basis_donation",
    ],
    [
        "ActiveStoreDomainParityV1",
        ["RepositoryDomain", "InstallationDomain"],
        "same_atomic_control_product",
        ["RepositoryExternalEffect", "InstallationExternalEffect"],
        "stable_home_and_generation_bound",
        "cross_domain_refused",
    ],
    [
        "ProtectedCeremonyEffectStoreV1",
        ["NoStoreProtectedCas", "PreStoreProtectedCas"],
        11,
        ["Initiate", "RecoverReserved", "ResolveResult", "Withdraw"],
        "durable_expected_old_carrier",
        "exact_managed_root_and_unique_database_leaf_anchor",
        "one_winner_exact_replay",
        "zero_provider_io",
        "request_requires_opaque_owner_issued_authority",
        "carrier_owns_no_authority",
        "external_owner_authority_retains_secret_carrier_persists_commitment_only",
          "managed_root_database_and_rollback_journal_custody_reverified_on_every_operation",
          "rollback_journal_create_is_exclusive_nofollow_and_open_descriptor_verified_before_commit",
          "sqlite_connection_identity_uses_documented_file_controls_only",
    ],
    [
        "ProviderApplicationReleaseV1",
        "ephemeral_non_clone",
        "winner_only",
        "never_persisted_or_reconstructed",
        "sealed_capability_has_no_public_accessor",
        "consuming_single_use_gateway",
          "exact_run_boundary_and_deadline_binding",
          "serialized_store_gateway_loads_current_control_head_and_current_authority_time_at_io_boundary",
          "writer_handoff_linearizes_before_or_after_external_io_never_during_release_validation",
          "fresh_current_trusted_time_must_be_strictly_before_deadline",
        "deadline_refusal_performs_zero_provider_io",
        "terminal_and_withdrawal_require_disposition",
    ],
    [
        "WriterHandoffAndHealthV1",
        "store_issued_same_home_fence",
        "old_writer_fenced",
        "one_head_writer_winner",
        ["Healthy", "RecoveryRequired", "IntegrityBlocked"],
        "unhealthy_blocks_behavior",
        "integrity_blocked_blocks_handoff",
    ],
    [
        "retry_policy",
        "no_retry_engine",
        "no_fresh_key_after_uncertainty",
        "safe_redispatch_is_typed_action_on_same_intent_only_when_conclusive",
        "in_doubt_survives_crash_restore_and_step_disposition",
        "definitely_not_started_requires_boundary_observation_not_caller_assertion",
    ],
    [
        "StepSubmissionV1",
        "step_owned_submission_stores_exact_claim_set_digest_only",
        ["ClaimV1", "SubmissionClaimSetV1", "evidence_owned_immutable_participant"],
        "execution_validates_binding_and_atomic_preconditions_without_owning_claim_semantics",
        "persistence_commits_submission_claimset_claims_and_step_closure_in_one_generation",
        ["one_claim", "n_claims"],
        ["submit_vs_submit", "submit_vs_renew", "submit_vs_takeover"],
        "loser_zero_write_zero_debit",
    ],
]

INVARIANTS = [
    "execution_attempt_union_is_exactly_three_owners",
    "every_run_has_exactly_one_execution_attempt_owner",
    "step_lease_is_step_attempt_only_and_one_to_one",
    "step_takeover_requires_exact_non_self_attested_owner_safety_proof",
    "stage4_takeover_safety_is_consumer_only_and_production_issuance_decode_belongs_to_stage5_evidence",
    "missing_stage5_takeover_evidence_fails_closed_without_self_attestation",
    "dispatch_and_reconciliation_provenance_never_donates_step_authority",
    "effect_intent_and_dispatch_reservation_commit_before_external_io",
    "dispatch_attempt_has_exactly_four_typed_outcomes",
    "sealed_or_ambiguous_crossing_is_durably_in_doubt",
    "reconciliation_requires_fresh_current_authorization",
    "one_reconciliation_attempt_spends_exactly_one_action_capacity",
    "reconciliation_never_dispatches_or_mutates_stale_step_state",
    "execution_authority_is_closed_three_branch_and_rejects_basis_donation",
    "continuity_maintenance_slot_is_nontransferable_across_purpose_subject_request_epoch_or_applicability",
    "effect_semantic_uniqueness_is_stable_across_provider_key_or_envelope_changes",
    "active_store_effects_have_repository_and_installation_domain_parity",
    "provider_release_is_ephemeral_winner_only_and_never_reconstructed",
    "effect_intent_control_head_is_the_only_current_selector",
    "store_publication_is_atomic_expected_old_and_zero_io",
    "protected_ceremony_matrix_is_eleven_by_four_durable_expected_old_cas",
    "protected_ceremony_database_leaf_identity_is_anchored_and_aba_replacement_is_refused",
    "protected_ceremony_carrier_is_non_authorizing_and_publicly_read_only",
    "protected_ceremony_request_requires_opaque_owner_issued_authority",
    "protected_ceremony_requests_have_owner_bound_canonical_bytes_for_post_crash_exact_replay",
    "protected_ceremony_reads_and_commit_acknowledgements_bind_the_sqlite_connection_descriptor_to_the_anchored_leaf",
    "protected_ceremony_revalidates_managed_root_database_and_rollback_journal_custody_on_every_operation",
    "protected_ceremony_uses_only_documented_sqlite_file_controls_and_public_connection_paths",
    "writer_handoff_uses_store_issued_same_home_fence_and_one_head_winner",
    "recovery_required_and_integrity_blocked_products_fail_closed",
    "terminal_attempt_closes_runs_and_clears_live_dispatch_atomically",
      "definitely_not_started_requires_an_opaque_run_term_boundary_and_time_receipt",
      "run_no_start_receipt_is_store_issued_from_current_authority_time_and_pinned_boundary_observation",
      "run_timeout_is_refused_before_the_exact_deadline",
      "provider_and_reconciliation_io_releases_are_consuming_non_clone_capabilities_without_raw_accessors",
      "provider_and_reconciliation_adapters_execute_inside_one_serialized_current_store_view",
      "provider_and_reconciliation_io_are_refused_at_or_after_deadline_before_adapter_invocation",
      "writer_handoff_and_external_io_have_one_serial_order",
      "replayed_reconciliation_begin_never_reconstructs_external_read_authority",
      "persistence_head_cas_loss_projects_to_execution_stale_expected_state",
    "withdrawal_has_exactly_sixty_legal_cells_and_twenty_one_denied_products",
    "withdrawal_performs_no_provider_io_and_creates_no_intent_attempt_or_run",
    "uncertainty_survives_crash_restore_amendment_removal_and_supersession",
    "no_blind_retry_no_fresh_key_retry_and_no_retry_engine",
    "step_submission_contains_only_the_exact_claim_set_digest_and_no_embedded_claim_records",
    "evidence_owns_claim_and_claim_set_semantics_step_owns_submission_and_execution_cannot_reauthor_them",
    "one_and_n_claim_participants_commit_with_step_closure_in_one_atomic_generation",
    "submit_vs_submit_submit_vs_renew_and_submit_vs_takeover_races_have_loser_zero_write_zero_debit",
    "stage5_gate_and_non_submission_evidence_implementation_is_outside_stage4_source_closure",
    "all_runtime_mutations_use_frozen_nominal_action_or_ceremony_owners",
]

REQUIRED_SOURCE_GROUPS = [
    ["ExecutionAttemptV1", "StepAttemptV1", DISPATCH_ATTEMPT, RECONCILIATION_ATTEMPT],
    ["ExecutionAuthorityV1", "BootstrapExecutionAuthorityV1", "ContinuityMaintenanceExecutionAuthorityV1", "job_applicability_commitment"],
    ["StepLeaseV1", "LeaseTermV1", "TakeoverSafetyV1", "owner_receipt_commitment"],
    ["RunV1", "RunNoStartReceiptV1", "RunExecutionTimeReceiptV1", "PinnedExecutionBoundaryObserverV1", "issue_run_no_start_receipt"],
    [EFFECT_INTENT, EFFECT_CONTROL_HEAD],
    ["ProtectedCeremonyOwnerAuthorityV1", "ProtectedCeremonyAuthorityV1", "ProtectedCeremonyCarrierAnchorV1", "ProtectedCeremonyEffectStoreV1", "owner_basis_commitment", "decode_request", "canonical_bytes", "verify_connection_leaf", "verify_live_connection", "verify_rollback_journal_custody", "protected_ceremony_vfs_open", "SQLITE_OPEN_EXCLUSIVE", "SQLITE_FCNTL_JOURNAL_POINTER", "SQLITE_FCNTL_HAS_MOVED", "sqlite3_db_filename", "TransactionBehavior::Immediate"],
    ["ProviderApplicationReleaseV1", "ReconciliationReadReleaseV1", "RunExecutionTimeReceiptV1", "execute_provider_once", "execute_reconciliation_read_once", "current_repository_authority_time", "with_serialized_active_view", "map_store_error", "HeadCasMismatch", "StaleExternalIoRelease", "HandoffWriter", "IntegrityBlocked"],
    ["StoreRoleV1::Repository", "StoreRoleV1::Installation"],
    ["publish", "transaction"],
]
FOCAL_STEP_EVIDENCE_REQUIRED_SOURCE_GROUPS = [
    ["StepSubmissionV1", "claim_set_digest"],
    ["SubmissionClaimSetV1", "digest"],
    ["ClaimV1", "EvidenceClaimPublicationV1"],
]


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_object(relative: str) -> dict[str, object]:
    value = json.loads((WORKSPACE / relative).read_text(encoding="ascii"))
    if not isinstance(value, dict):
        raise ValueError(f"artifact must contain one object: {relative}")
    return value


def canonical_payload(document: dict[str, object]) -> object:
    return document.get("canonical_value", document.get("value", document))


def receipt_rows(paths: list[str]) -> list[dict[str, object]]:
    return [
        {"byte_length": len(data := (WORKSPACE / path).read_bytes()), "path": path, "sha256": digest(data)}
        for path in paths
    ]


def tool_descriptor(name: str) -> dict[str, object]:
    invocation = Path(sys.executable if name == "python-current" else shutil.which(name) or "")
    if not invocation.is_file():
        raise ValueError(f"required Stage 4 proof executable is unavailable: {name}")
    invocation = invocation.absolute()
    resolved = invocation.resolve(strict=True)
    data = resolved.read_bytes()
    return {
        "invocation_path": str(invocation),
        "resolved_path": str(resolved),
        "sha256": digest(data),
        "byte_length": len(data),
    }


def bound_environment_value(key: str, environment: dict[str, str]) -> str:
    value = environment.get(key, "<unset>")
    if key != "PATH" or value == "<unset>":
        return value
    return os.pathsep.join(
        "<codex-transient-arg0>"
        if re.search(r"/\.codex/tmp/arg0/codex-arg0[^/]*$", component)
        else component
        for component in value.split(os.pathsep)
    )


def command_environment() -> dict[str, str]:
    environment = dict(os.environ)
    for key in UNSET_BUILD_OVERRIDE_KEYS:
        environment.pop(key, None)
    environment.setdefault("CARGO_HOME", str(Path.home() / ".cargo"))
    environment.setdefault("RUSTUP_HOME", str(Path.home() / ".rustup"))
    environment["CARGO_INCREMENTAL"] = "0"
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    environment["RUSTC"] = str(tool_descriptor("rustc")["invocation_path"])
    return environment


def command_result_digest(payload: object) -> str:
    return digest(json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("ascii"))


def test_binary_receipt_matches(binary: object) -> bool:
    if not isinstance(binary, dict):
        return False
    path = binary.get("path")
    if not isinstance(path, str):
        return False
    binary_path = Path(path)
    if not binary_path.is_absolute():
        binary_path = WORKSPACE / binary_path
    try:
        binary_data = binary_path.resolve(strict=True).read_bytes()
    except (FileNotFoundError, OSError):
        return False
    return (
        binary.get("byte_length") == len(binary_data)
        and binary.get("sha256") == digest(binary_data)
    )


def execute_commands(commands: list[list[str]], label: str) -> list[dict[str, object]]:
    receipts = []
    for command in commands:
        executable = tool_descriptor(command[0])
        invoked = [str(executable["invocation_path"]), *command[1:]]
        result = subprocess.run(
            invoked,
            cwd=WORKSPACE,
            capture_output=True,
            check=False,
            env=command_environment(),
        )
        if result.returncode != 0:
            detail = (result.stderr.strip() or result.stdout.strip()).decode("utf-8", "replace")
            raise ValueError(f"{label} failed: {' '.join(command)}: {detail}")
        receipts.append(
            {
                "command": command,
                "executable": executable,
                "exit_code": 0,
                "result": "pass",
                "stdout_sha256": digest(result.stdout),
                "stderr_sha256": digest(result.stderr),
            }
        )
    return receipts


def execute_test_commands(
    commands: list[list[str]], expected_passed: list[int], label: str
) -> list[dict[str, object]]:
    if len(commands) != len(expected_passed):
        raise ValueError(f"{label} expectation cardinality drifted")
    receipts = []
    for command, expected in zip(commands, expected_passed, strict=True):
        executable = tool_descriptor(command[0])
        result = subprocess.run(
            [str(executable["invocation_path"]), *command[1:]],
            cwd=WORKSPACE,
            capture_output=True,
            text=True,
            check=False,
            env=command_environment(),
        )
        output = f"{result.stdout}\n{result.stderr}"
        if result.returncode != 0:
            raise ValueError(f"{label} failed: {' '.join(command)}: {output.strip()}")
        outcomes = re.findall(r"^test (.+) \.\.\. (ok|ignored)$", output, re.MULTILINE)
        passed_names = sorted(name for name, status in outcomes if status == "ok")
        ignored_names = sorted(name for name, status in outcomes if status == "ignored")
        summaries = re.findall(
            r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",
            output,
        )
        if summaries != [(str(expected), "0", "0")]:
            raise ValueError(
                f"{label} expected exactly {expected} passed and zero failed/ignored: "
                f"{' '.join(command)}: {summaries}"
            )
        if len(passed_names) != expected or ignored_names:
            raise ValueError(f"{label} did not execute its exact non-ignored test census")
        binaries = re.findall(r"Running [^\n]*\(([^)]+)\)", output)
        if len(binaries) != 1:
            raise ValueError(f"{label} did not expose exactly one compiled test binary")
        binary_path = Path(binaries[0])
        if not binary_path.is_absolute():
            binary_path = WORKSPACE / binary_path
        binary_path = binary_path.resolve(strict=True)
        binary_data = binary_path.read_bytes()
        try:
            display_path = binary_path.relative_to(WORKSPACE).as_posix()
        except ValueError:
            display_path = str(binary_path)
        outcome = {
            "command": command,
            "ignored": 0,
            "passed": expected,
            "test_binary": {
                "byte_length": len(binary_data),
                "path": display_path,
                "sha256": digest(binary_data),
            },
            "test_names": passed_names,
        }
        receipts.append(
            {
                **outcome,
                "executable": executable,
                "exit_code": 0,
                "normalized_output_sha256": command_result_digest(outcome),
                "result": "pass",
            }
        )
    return receipts


def validate_recorded_test_receipts(
    receipts: object, commands: list[list[str]], expected_passed: list[int], label: str
) -> None:
    if not isinstance(receipts, list) or len(receipts) != len(commands):
        raise ValueError(f"{label} receipt cardinality drifted")
    for row, command, expected in zip(receipts, commands, expected_passed, strict=True):
        if not isinstance(row, dict) or row.get("command") != command:
            raise ValueError(f"{label} command binding drifted")
        names = row.get("test_names")
        binary = row.get("test_binary")
        outcome = {
            "command": command,
            "ignored": 0,
            "passed": expected,
            "test_binary": binary,
            "test_names": names,
        }
        if (
            row.get("executable") != tool_descriptor(command[0])
            or row.get("exit_code") != 0
            or row.get("result") != "pass"
            or row.get("ignored") != 0
            or row.get("passed") != expected
            or not isinstance(names, list)
            or len(names) != expected
            or len(set(names)) != expected
            or not test_binary_receipt_matches(binary)
            or row.get("normalized_output_sha256") != command_result_digest(outcome)
        ):
            raise ValueError(f"{label} exact test census receipt drifted")


def predecessor_command_receipts() -> list[dict[str, object]]:
    global _PREDECESSOR_COMMAND_RECEIPTS
    if _PREDECESSOR_COMMAND_RECEIPTS is None:
        _PREDECESSOR_COMMAND_RECEIPTS = execute_commands(
            PREDECESSOR_COMMANDS, "Stage 4 predecessor validation"
        )
    return _PREDECESSOR_COMMAND_RECEIPTS


def predecessor_binding() -> dict[str, object]:
    stage0 = read_object(PREDECESSOR_MANIFESTS["stage0_effect_home"])
    dispatch = read_object(PREDECESSOR_MANIFESTS["stage0_dispatch_cutover"])
    stage2 = read_object(PREDECESSOR_MANIFESTS["stage2_authority"])
    stage3 = read_object(PREDECESSOR_MANIFESTS["stage3_domain"])
    if (
        stage0.get("finalization_state") != "final"
        or stage0.get("candidate_only") is not True
        or stage0.get("runtime_activation") is not False
        or dispatch.get("status") != "pass"
        or dispatch.get("runtime_activated") is not False
        or stage2.get("publication_state") != PUBLICATION_STATE
        or stage3.get("publication_state") != PUBLICATION_STATE
    ):
        raise ValueError("Stage 4 predecessor chain is not a certified inactive full chain")
    roots = {
        "stage0_effect_home": stage0.get("identity"),
        "stage0_dispatch_cutover": f"sha256:{digest((WORKSPACE / PREDECESSOR_MANIFESTS['stage0_dispatch_cutover']).read_bytes())}",
        "stage2_authority": f"sha256:{stage2.get('root_id')}",
        "stage3_domain": stage3.get("identity"),
    }
    if any(
        not isinstance(root, str) or not re.fullmatch(r"sha256:[0-9a-f]{64}", root)
        for root in roots.values()
    ):
        raise ValueError("Stage 4 predecessor chain has a missing semantic root")
    for group, paths in PREDECESSOR_RECEIPTS.items():
        for path in paths:
            receipt = read_object(path)
            if group == "stage3_domain" and receipt.get("validation_mode") != "full_chain":
                raise ValueError(f"predecessor receipt is not full-chain: {path}")
    return {
        "command_receipts": predecessor_command_receipts(),
        "mode": "full_chain",
        "roots": roots,
        "proof_receipts": {group: receipt_rows(paths) for group, paths in PREDECESSOR_RECEIPTS.items()},
    }


def predecessor_canonical() -> list[object]:
    binding = predecessor_binding()
    roots = binding["roots"]
    proof_receipts = binding["proof_receipts"]
    return [
        "full_chain",
        [
            [
                group,
                roots[group],
                [[row["path"], row["byte_length"], row["sha256"]] for row in proof_receipts[group]],
            ]
            for group in [
                "stage0_effect_home",
                "stage0_dispatch_cutover",
                "stage2_authority",
                "stage3_domain",
            ]
        ],
        [
            [
                row["command"],
                [
                    row["executable"]["invocation_path"],
                    row["executable"]["resolved_path"],
                    row["executable"]["byte_length"],
                    row["executable"]["sha256"],
                ],
                row["exit_code"],
                row["result"],
                row["stdout_sha256"],
                row["stderr_sha256"],
            ]
            for row in binding["command_receipts"]
        ],
    ]


def catalog_binding() -> list[object]:
    inventory = read_object(CATALOG_PATHS[0])
    counts = inventory.get("semantic_counts")
    if not isinstance(counts, dict) or {
        "actions": counts.get("actions"),
        "action_specs": counts.get("action_specs"),
        "ceremonies": counts.get("ceremonies"),
        "effect_origins": counts.get("effect_origins"),
        "effect_routes": counts.get("effect_routes"),
        "execution_attempt_owners": counts.get("execution_attempt_owners"),
    } != {
        "actions": 145,
        "action_specs": 145,
        "ceremonies": 11,
        "effect_origins": 23,
        "effect_routes": 139,
        "execution_attempt_owners": 3,
    }:
        raise ValueError("frozen Stage 0 public catalog counts drifted")
    expected = {
        "catalog-profile-grammar-v1.json": ("b7ef635dcd29af4fc41f20cd670b726e5627c2f7210344d058e7c188ace69647", 156),
        "catalog-02-effect.json": ("d28f8e573ddb450c427e628df121dbd516d0e5b05c03caf18d2757782dfd259d", 23),
        "catalog-06-action-leaf.json": ("b2f538d76795db0338448cc8cb837419157c1bebdc8bcc7d7b42fd961790d454", 145),
        "catalog-09-action-spec.json": ("7a7a1311690fcdec194c0d9c9afbbe4e28f1332b567ed23451daf06ebca02970", 145),
    }
    inventory_rows = {row["path"]: row for row in inventory.get("artifacts", [])}
    rows = []
    for relative in CATALOG_PATHS[1:]:
        name = Path(relative).name
        identity, row_count = expected[name]
        document = read_object(relative)
        actual_identity = (
            document.get("catalog_profile_grammar", {}).get("catalog_profile_grammar_id")
            if name.startswith("catalog-profile")
            else document.get("manifest_id")
        )
        inventory_row = inventory_rows.get(name)
        data = (WORKSPACE / relative).read_bytes()
        if (
            actual_identity != identity
            or not isinstance(inventory_row, dict)
            or inventory_row.get("identity") != identity
            or inventory_row.get("row_count") != row_count
            or inventory_row.get("sha256") != digest(data)
        ):
            raise ValueError(f"frozen public catalog commitment drifted: {name}")
        rows.append([name, identity, row_count, digest(data)])
    return ["frozen_public_catalogs", [145, 11, 23, 139, 3, 156], rows]


def dispatch_binding() -> list[object]:
    value = canonical_payload(read_object(DISPATCH_PATH))
    if not isinstance(value, list) or len(value) < 8:
        raise ValueError("dispatch state artifact has the wrong shape")
    outcomes = value[4]
    expected = [
        [1, "locally_rejected", 1],
        [2, "definitely_not_sent", 2],
        [3, "response_received", 2],
        [4, "ambiguous_transport", 2],
    ]
    if outcomes != expected:
        raise ValueError("dispatch state does not have the exact four typed payloads")
    data = (WORKSPACE / DISPATCH_PATH).read_bytes()
    return ["dispatch_attempt", expected, len(data), digest(data)]


def withdrawal_binding() -> list[object]:
    value = canonical_payload(read_object(WITHDRAWAL_PATH))
    if (
        not isinstance(value, list)
        or len(value) != 6
        or value[0:2] != [WITHDRAWAL_SCHEMA, 1]
        or not isinstance(value[2], list)
        or len(value[2]) != 60
        or not isinstance(value[4], list)
        or len(value[4]) != 21
        or value[5] != "withdrawn locally; no provider cancellation performed"
    ):
        raise ValueError("withdrawal artifact does not have exact 60/21 closure")
    if {row[0] for row in value[2]} != {"prepared", "confirmed_not_applied"}:
        raise ValueError("withdrawal legal cells admit an invalid classification")
    data = (WORKSPACE / WITHDRAWAL_PATH).read_bytes()
    return ["withdrawal", 60, 21, value[3], value[4], value[5], len(data), digest(data)]


def execution_sources() -> list[str]:
    root = WORKSPACE / "src/domain/vnext/execution"
    paths = sorted(path.relative_to(WORKSPACE).as_posix() for path in root.rglob("*.rs"))
    if not paths or "src/domain/vnext/execution/mod.rs" not in paths:
        raise ValueError("Stage 4 Execution owner source closure is absent")
    return paths


def persistence_sources() -> list[str]:
    root = WORKSPACE / "src/domain/vnext/persistence"
    paths = sorted(path.relative_to(WORKSPACE).as_posix() for path in root.rglob("*.rs"))
    if not paths or "src/domain/vnext/persistence/mod.rs" not in paths:
        raise ValueError("Stage 4 persistence owner source closure is absent")
    return paths


def contract_ownership_sources() -> list[str]:
    root = WORKSPACE / "src/domain/vnext/contract"
    paths = sorted(path.relative_to(WORKSPACE).as_posix() for path in root.rglob("*.rs"))
    if not paths or "src/domain/vnext/contract/mod.rs" not in paths:
        raise ValueError("Stage 4 Contract ownership source closure is absent")
    return paths


def source_paths() -> list[str]:
    paths = sorted(
        set(
            CATALOG_PATHS
            + [DISPATCH_PATH, WITHDRAWAL_PATH]
            + list(PREDECESSOR_MANIFESTS.values())
            + [path for paths in PREDECESSOR_RECEIPTS.values() for path in paths]
            + COMPILATION_ANCESTORS
            + AUTHORITY_EXTENSION_SOURCES
                + FOCAL_STEP_EVIDENCE_SOURCES
                + contract_ownership_sources()
            + execution_sources()
            + persistence_sources()
            + TOOL_SOURCES
        )
    )
    if any(
        path.startswith("src/domain/vnext/gate/")
        or (
            path.startswith("src/domain/vnext/evidence/")
            and path not in FOCAL_STEP_EVIDENCE_SOURCES
        )
        for path in paths
    ):
        raise ValueError("Stage 5 Gate or non-submission Evidence implementation leaked into Stage 4 source closure")
    return paths


def validate_runtime_source() -> None:
    sources = execution_sources()
    text = "\n".join((WORKSPACE / path).read_text(encoding="utf-8") for path in sources)
    missing = [group for group in REQUIRED_SOURCE_GROUPS if not all(marker in text for marker in group)]
    if missing:
        raise ValueError(f"live Execution source does not yet implement Stage 4 runtime semantics: {missing}")
    definition = "pub struct SubmissionClaimSetV1"
    definition_owners = sorted(
        path.relative_to(WORKSPACE).as_posix()
        for path in (WORKSPACE / "src/domain/vnext").rglob("*.rs")
        if definition in path.read_text(encoding="utf-8")
    )
    if definition_owners != ["src/domain/vnext/evidence/submission_claim.rs"]:
        raise ValueError(
            "SubmissionClaimSetV1 must have exactly one Evidence-owned definition"
        )
    contract_text = "\n".join(
        (WORKSPACE / path).read_text(encoding="utf-8")
        for path in contract_ownership_sources()
    )
    if "SubmissionClaimSetV1" in contract_text:
        raise ValueError("Contract cannot define or re-export SubmissionClaimSetV1")
    focal_text = "\n".join(
        (WORKSPACE / path).read_text(encoding="utf-8")
        for path in FOCAL_STEP_EVIDENCE_SOURCES
    )
    missing_focal = [
        group
        for group in FOCAL_STEP_EVIDENCE_REQUIRED_SOURCE_GROUPS
        if not all(marker in focal_text for marker in group)
    ]
    if missing_focal:
        raise ValueError(
            f"live Step/Evidence submission participants are incomplete: {missing_focal}"
        )
    if "Stage 4 is the only future implementation owner" in text:
        raise ValueError("Stage 4 cannot certify candidate-only execution literals as runtime implementation")
    for constructor in ["from_owner_receipt", "from_canonical_value"]:
        marker = f"#[cfg(test)]\n    pub(crate) fn {constructor}("
        if text.count(marker) != 1:
            raise ValueError(
                "Stage 4 takeover safety must remain consumer-only until Stage 5 owner Evidence"
            )
    persistence = "\n".join(
        (WORKSPACE / path).read_text(encoding="utf-8") for path in persistence_sources()
    )
    if not any(marker in text for marker in ["crate::domain::vnext::persistence", "super::super::persistence"]):
        raise ValueError("Execution does not bind the canonical persistence owner")
    if not all(marker in persistence for marker in ["transaction", "publish"]):
        raise ValueError("persistence source does not expose atomic publication semantics")


def source_rows() -> list[list[object]]:
    rows = []
    for relative in source_paths():
        path = WORKSPACE / relative
        if not path.is_file():
            raise ValueError(f"missing Stage 4 semantic source: {relative}")
        data = path.read_bytes()
        rows.append([relative, len(data), digest(data)])
    return rows


def toolchain_binding() -> list[object]:
    environment = command_environment()
    descriptors = [
        [
            name,
            descriptor["invocation_path"],
            descriptor["resolved_path"],
            descriptor["byte_length"],
            descriptor["sha256"],
        ]
        for name in ["cargo", "rustc", "python3", "ruby"]
        for descriptor in [tool_descriptor(name)]
    ]
    cargo_home = Path(environment.get("CARGO_HOME", str(Path.home() / ".cargo")))
    config_candidates = [
        WORKSPACE / ".cargo/config.toml",
        WORKSPACE / ".cargo/config",
        cargo_home / "config.toml",
        cargo_home / "config",
    ]
    config_rows = []
    for path in config_candidates:
        if path.is_file():
            data = path.read_bytes()
            config_rows.append([str(path.resolve()), len(data), digest(data)])
    rustc = subprocess.run(
        [str(tool_descriptor("rustc")["invocation_path"]), "-vV"],
        cwd=WORKSPACE,
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    cargo = subprocess.run(
        [str(tool_descriptor("cargo")["invocation_path"]), "-Vv"],
        cwd=WORKSPACE,
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    cfg = subprocess.run(
        [str(tool_descriptor("rustc")["invocation_path"]), "--print", "cfg"],
        cwd=WORKSPACE,
        capture_output=True,
        text=True,
        check=False,
        env=environment,
    )
    if rustc.returncode != 0 or cargo.returncode != 0 or cfg.returncode != 0:
        raise ValueError("Stage 4 could not bind the active Rust toolchain and target cfg")
    return [
        "proof_toolchain_environment_and_target_v2",
        descriptors,
        [[key, bound_environment_value(key, environment)] for key in SANITIZED_ENVIRONMENT_KEYS],
        config_rows,
        rustc.stdout.strip().splitlines(),
        cargo.stdout.strip().splitlines(),
        sorted(line for line in cfg.stdout.strip().splitlines() if line),
    ]


def canonical_value() -> list[object]:
    validate_runtime_source()
    return [
        DOMAIN,
        1,
        PUBLICATION_STATE,
        predecessor_canonical(),
        catalog_binding(),
        EXECUTION_MODEL,
        dispatch_binding(),
        withdrawal_binding(),
        INVARIANTS,
        source_rows(),
        toolchain_binding(),
    ]


def json_bytes(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n").encode("ascii")


def write_atomic(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def build_to(
    root: Path, validation_mode: str, parent_certification_identity: str | None
) -> str:
    value = canonical_value()
    encoded = cbor_py.encode(value)
    identity = f"sha256:{digest(encoded)}"
    manifest = {
        "canonical_value": value,
        "identity": identity,
        "publication_state": PUBLICATION_STATE,
        "schema_version": DOMAIN,
    }
    manifest_bytes = json_bytes(manifest)
    write_atomic(root / "execution-effects.v1.cbor", encoded)
    write_atomic(root / "execution-effects.v1.json", manifest_bytes)
    receipt = {
        "artifacts": [
            ["execution-effects.v1.cbor", len(encoded), digest(encoded)],
            ["execution-effects.v1.json", len(manifest_bytes), digest(manifest_bytes)],
        ],
        "encoder": "python-stdlib-plus-frozen-cbor-subset",
        "identity": identity,
        "predecessor_chain": predecessor_binding(),
        "schema_version": f"{DOMAIN}.python-encoder-receipt.v1",
        "validation_mode": validation_mode,
    }
    if parent_certification_identity is not None:
        receipt["parent_certification_identity"] = parent_certification_identity
    write_atomic(root / "python-encoder-receipt.v1.json", json_bytes(receipt))
    return identity


def certify_behavior(
    root: Path,
    run_mutants: bool,
    validation_mode: str,
    parent_certification_identity: str | None,
) -> None:
    manifest = json.loads((root / "execution-effects.v1.json").read_text(encoding="ascii"))
    identity = manifest.get("identity")
    if not isinstance(identity, str):
        raise ValueError("Stage 4 behavioral proof cannot bind a missing artifact identity")
    command_receipts = execute_test_commands(
        BEHAVIOR_COMMANDS,
        BEHAVIOR_EXPECTED_PASSED,
        "Stage 4 behavioral certification",
    )
    mutant_command_receipts = (
        execute_test_commands(
            MUTANT_COMMANDS,
            MUTANT_EXPECTED_PASSED,
            "Stage 4 mutant certification",
        )
        if run_mutants
        else []
    )
    receipt = {
        "command_receipts": command_receipts,
        "commands": BEHAVIOR_COMMANDS,
        "identity": identity,
        "mutant_command_receipts": mutant_command_receipts,
        "mutant_commands": MUTANT_COMMANDS,
        "mutant_validation": "executed" if run_mutants else "nested_skip",
        "result": "pass",
        "schema_version": f"{DOMAIN}.behavioral-proof-receipt.v1",
        "validation_mode": validation_mode,
        "validator": "compiled-rust-execution-contracts",
    }
    if parent_certification_identity is not None:
        receipt["parent_certification_identity"] = parent_certification_identity
    write_atomic(root / "behavioral-proof-receipt.v1.json", json_bytes(receipt))


def certify(
    root: Path,
    run_mutants: bool,
    validation_mode: str,
    parent_certification_identity: str | None,
) -> None:
    certify_behavior(root, run_mutants, validation_mode, parent_certification_identity)
    for command in [
        [sys.executable, str(TOOLS / "validate.py"), "--root", str(root)],
        ["ruby", str(TOOLS / "verify.rb"), "--root", str(root)],
    ]:
        if not run_mutants:
            command.extend(
                [
                    "--skip-mutants",
                    "--parent-certification-identity",
                    parent_certification_identity or "",
                ]
            )
        result = subprocess.run(
            command,
            cwd=WORKSPACE,
            capture_output=True,
            text=True,
            check=False,
            env=command_environment(),
        )
        if result.returncode != 0:
            detail = result.stderr.strip() or result.stdout.strip()
            raise ValueError(f"Stage 4 certification failed: {' '.join(command)}: {detail}")


def compare_trees(expected: Path, actual: Path) -> None:
    expected_paths = sorted(path.relative_to(expected).as_posix() for path in expected.rglob("*") if path.is_file())
    actual_paths = sorted(path.relative_to(actual).as_posix() for path in actual.rglob("*") if path.is_file())
    if expected_paths != actual_paths:
        raise ValueError("Stage 4 artifact file set is stale")
    volatile_proof_receipts = {
        "behavioral-proof-receipt.v1.json",
        "semantic-validation-receipt.v1.json",
        "ruby-verification-receipt.v1.json",
    }
    for relative in expected_paths:
        if relative in volatile_proof_receipts:
            expected_value = stable_receipt_projection(
                json.loads((expected / relative).read_text(encoding="ascii"))
            )
            actual_value = stable_receipt_projection(
                json.loads((actual / relative).read_text(encoding="ascii"))
            )
            if expected_value != actual_value:
                raise ValueError(f"Stage 4 proof receipt is stale: {relative}")
        elif (expected / relative).read_bytes() != (actual / relative).read_bytes():
            raise ValueError(f"Stage 4 artifact is stale: {relative}")
    validate_full_chain_proof_receipts(actual)


def stable_receipt_projection(value: object) -> object:
    if isinstance(value, dict):
        return {
            key: stable_receipt_projection(item)
            for key, item in value.items()
            if key not in {"stdout_sha256", "stderr_sha256"}
        }
    if isinstance(value, list):
        return [stable_receipt_projection(item) for item in value]
    return value


def validate_full_chain_proof_receipts(root: Path) -> None:
    manifest = json.loads((root / "execution-effects.v1.json").read_text(encoding="ascii"))
    identity = manifest.get("identity")
    behavior = json.loads((root / "behavioral-proof-receipt.v1.json").read_text(encoding="ascii"))
    if (
        behavior.get("identity") != identity
        or behavior.get("validation_mode") != "full_chain"
        or behavior.get("mutant_validation") != "executed"
        or behavior.get("commands") != BEHAVIOR_COMMANDS
        or behavior.get("mutant_commands") != MUTANT_COMMANDS
        or "parent_certification_identity" in behavior
    ):
        raise ValueError("Stage 4 behavioral proof receipt is not a full-chain certification")
    validate_recorded_test_receipts(
        behavior.get("command_receipts"),
        BEHAVIOR_COMMANDS,
        BEHAVIOR_EXPECTED_PASSED,
        "stored Stage 4 behavior",
    )
    validate_recorded_test_receipts(
        behavior.get("mutant_command_receipts"),
        MUTANT_COMMANDS,
        MUTANT_EXPECTED_PASSED,
        "stored Stage 4 mutants",
    )
    for name, validator in [
        ("semantic-validation-receipt.v1.json", "independent-python-reconstruction"),
        ("ruby-verification-receipt.v1.json", "independent-ruby-reconstruction"),
    ]:
        receipt = json.loads((root / name).read_text(encoding="ascii"))
        reexecution = receipt.get("behavioral_reexecution")
        if (
            receipt.get("identity") != identity
            or receipt.get("validation_mode") != "full_chain"
            or receipt.get("validator") != validator
            or not isinstance(reexecution, dict)
            or "parent_certification_identity" in receipt
        ):
            raise ValueError(f"Stage 4 independent proof receipt is incomplete: {name}")
        validate_recorded_test_receipts(
            reexecution.get("command_receipts"),
            BEHAVIOR_COMMANDS,
            BEHAVIOR_EXPECTED_PASSED,
            f"{name} behavior reexecution",
        )
        validate_recorded_test_receipts(
            reexecution.get("mutant_command_receipts"),
            MUTANT_COMMANDS,
            MUTANT_EXPECTED_PASSED,
            f"{name} mutant reexecution",
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--root", type=Path, default=OUTPUT)
    parser.add_argument("--skip-mutants", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--parent-certification-identity", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.skip_mutants:
        if args.check or args.root.resolve() == OUTPUT.resolve():
            parser.error("--skip-mutants is only valid for a nested non-canonical proof root")
        if not isinstance(args.parent_certification_identity, str) or not re.fullmatch(
            r"sha256:[0-9a-f]{64}", args.parent_certification_identity
        ):
            parser.error("nested proof requires an exact parent certification identity")
        validation_mode = "nested_subset"
        parent_certification_identity = args.parent_certification_identity
    else:
        if args.parent_certification_identity is not None:
            parser.error("full-chain proof cannot claim a parent certification identity")
        validation_mode = "full_chain"
        parent_certification_identity = None
    if args.check:
        with tempfile.TemporaryDirectory(prefix="maestro-stage4-execution-") as temporary:
            generated = Path(temporary) / "execution"
            identity = build_to(generated, validation_mode, parent_certification_identity)
            certify(
                generated,
                not args.skip_mutants,
                validation_mode,
                parent_certification_identity,
            )
            compare_trees(generated, args.root)
    else:
        identity = build_to(args.root, validation_mode, parent_certification_identity)
        certify(
            args.root,
            not args.skip_mutants,
            validation_mode,
            parent_certification_identity,
        )
    print(identity)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
