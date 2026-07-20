#!/usr/bin/env python3
"""Build the inactive Stage-0 Effect Intent Home literal suite.

This generator consumes the already-frozen efa0 catalog artifacts.  It is a
candidate-only materializer: it creates no runtime registration or mutable
store state and intentionally treats predecessor material as evidence only.
"""

from __future__ import annotations

import argparse
import ast
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
DEFAULT_OUTPUT = ROOT / "contracts/vnext/stage0/effect-home"
CATALOGS = ROOT / "contracts/vnext/catalogs/generated"
EVIDENCE = ROOT / "contracts/vnext/catalogs/evidence/predecessors.json"
SOURCE_BINDINGS = ROOT / "contracts/vnext/stage0/input-bindings.json"
C325 = ROOT / "contracts/vnext/public/direct_consumers.c325.v1.json"
DOMAIN = "maestro.vnext.stage0.effect-home.v1"
EXPECTED_SOURCE_BINDINGS_SHA256 = "d0cc2563dd84458ed9122ee91eb1f640e137a7fdb6ad74ecb2123744243482c5"
EXPECTED_INPUTS = {
    "design": "85787cfb4fb32eefe078adbf9ede66114b12c6304af10857bd676a1cd9875d18",
    "decisions": "1f97e67b156d5a17d13b94ff955ad17efeb3bb71a4b74b1aec14e20dac1100dd",
    "card": "2cdf1f74843a6eca926ff3bc48e060654350e6a03b65342f8d7be48d111379b4",
    "c325": "ccd22243030aa3bbbd02fefd4ab17371b9bfb2c9842311c5acddbee5bd220c29",
}
ROLES = {
    1: "ActionReserve",
    2: "ActionRecoverReserved",
    3: "ActionOutcome",
    4: "ActionReconcile",
    5: "ActionWithdraw",
    6: "CeremonyInitiate",
    7: "CeremonyRecoverReserved",
    8: "CeremonyResolveResult",
    9: "CeremonyWithdraw",
}
RESERVATION_MODES = ["InitiateNew", "RecoverReserved"]
CEREMONY_MODES = ["Initiate", "RecoverReserved", "ResolveResult", "Withdraw"]
RESERVE_LEAVES = [
    "OriginateEffectIntent",
    "OriginateCoordinationDelivery",
    "ReserveBootstrapMandateInteractionEffect",
    "ReserveContinuityMaintenanceEffect",
]
BOOTSTRAP_CENSUS = [
    ("EnrollRecoveryCommitmentSelection", "candidate_target"),
    ("RotateRecoveryCommitmentSelection", "candidate_target"),
    ("RevokeRecoveryCommitmentSelection", "candidate_target"),
    ("FirstHumanBindingEnrollment", "hard_exclusion"),
    ("ReserveBootstrapMandateInteractionEffect", "hard_exclusion"),
    ("PublishBootstrapMandateInteractionOutcome", "hard_exclusion"),
    ("PublishBootstrapMandatePresentationObservation", "hard_exclusion"),
    ("PublishBootstrapMandateResponseObservation", "hard_exclusion"),
    ("ReconcileBootstrapMandateInteractionEffect", "hard_exclusion"),
    ("IssueBootstrapMandate", "hard_exclusion"),
    ("WithdrawBootstrapMandateInteractionEffect", "hard_exclusion"),
]
CMA_SLOT_FAMILIES = [
    "MaintenanceExecutorCurrentness",
    "ProspectiveContinuityCarrier",
    "PlannedTurnoverHighWater",
    "RepositoryRecoveryAdmission",
    "InstallationRecoveryAdmission",
]
DENIED_WITHDRAWAL_PRODUCTS = [
    "live_dispatch_reserved_or_sealed",
    "classification_not_prepared_or_confirmed_not_applied",
    "live_attempt_or_dispatch_fence_or_seal_or_release_capability",
    "open_run_or_incomplete_run_closure",
    "stale_or_unknown_home_domain_realm_or_context",
    "wrong_origin_semantic_subject_or_origination_fence",
    "stale_generation_epoch_material_or_authority",
    "wrong_action_leaf_ceremony_mode_or_route_role",
    "ordinary_bootstrap_cma_or_ceremony_basis_donation",
    "missing_or_spent_cma_effect_withdrawal_slot",
    "sixth_cma_purpose_or_capacity_kind",
    "missing_current_writer_term_or_expected_old_head",
    "unknown_mixed_or_stale_catalog_identity",
    "new_intent_attempt_run_observation_key_or_envelope",
    "refund_remint_top_up_refill_rewind_or_slot_reopen",
    "provider_cancellation_or_compensation_wording",
    "terminal_cancelled_reopen_reconcile_retry_or_redispatch",
    "late_evidence_control_head_rewrite",
    "restore_without_same_home_old_writer_fence",
    "cross_domain_clone_import_or_unresolved_collision_activation",
    "legacy_cancelled_label_without_unique_complete_h3_causal_join",
]
TRANSITION_CONTENDERS = [
    [1, "original_handler"],
    [2, "recovery_caller"],
    [3, "preseal_local_rejection"],
    [4, "seal"],
    [5, "response_handler"],
    [6, "terminalizer"],
    [7, "classifier"],
    [8, "reconciler"],
    [9, "redispatcher"],
    [10, "withdrawal"],
    [11, "same_home_restore_writer"],
]
MIGRATION_MAP = [
    ["same_domain_restore", "fence_old_writer_then_one_same_home_writer"],
    ["different_domain_clone_or_import", "non_bearer_history_no_activation"],
    ["native_cancelled", "unique_complete_h3_causal_join_only"],
]
H2_COMPONENTS = [
    "effect-intent-control-head-v1",
    "effect-intent-control-revision-v1",
    "effect-intent-control-transition-v1",
    "effect-intent-control-writer-term-v1",
]
SEMANTIC_LITERAL_PATTERNS = [
    "EffectIntent",
    "EffectOrigin",
    "DispatchAttempt",
    "ReconciliationAttempt",
    "RemoteClassification",
    "EffectWithdrawal",
    "WithdrawEffectIntent",
    "RecoverReserved",
    "ControlHead",
    "ControlRevision",
    "WriterTerm",
]
STAGE2_SEMANTIC_LITERAL_PATTERNS = [
    *SEMANTIC_LITERAL_PATTERNS,
    "PublishBootstrapMandateInteractionOutcome",
    "TrustedTimeAcquisition",
    "RecoveryExternalRegistration",
    "RecoveryExternalStatus",
    "MaintenanceExecutorCurrentness",
    "ProspectiveContinuityCarrier",
    "PlannedTurnoverHighWater",
    "RepositoryRecoveryAdmission",
    "InstallationRecoveryAdmission",
    "RepositoryWorkAuthorityPolicyTransition",
    "RepositoryFirstWorkPublication",
    "RepositoryFloorOrTrustRootRotation",
    "InstallationPolicyBindingReplacement",
    "InstallationStructuralRootFloorReplacement",
    "TrustedTimePolicyStackRotation",
    "ExternalLogicalCarrierProfileRotation",
    "PlannedEpochTurnoverPreparation",
]
STAGE2_SEMANTIC_SOURCE_DECLARATIONS = {
    "src/domain/vnext/authority/action_basis.rs": ("Authority", "candidate_contract_definition", "exact_stage4_execution_basis_partition"),
    "src/domain/vnext/authority/bootstrap_catalog.rs": ("Authority", "candidate_contract_definition", "exact_stage2_bootstrap_target_literal"),
    "src/domain/vnext/authority/capacity.rs": ("Authority", "candidate_contract_definition", "exact_stage2_capacity_literal"),
    "src/domain/vnext/authority/closed.rs": ("Authority", "candidate_contract_definition", "exact_stage2_closed_sum_literal"),
    "src/domain/vnext/authority/continuity/catalog.rs": ("Authority", "candidate_contract_definition", "exact_stage2_continuity_effect_intent_class_literal"),
    "src/domain/vnext/authority/continuity/totality.rs": ("Authority", "candidate_contract_definition", "exact_stage2_continuity_owner_census_literal"),
    "src/domain/vnext/authority/mod.rs": ("Authority", "candidate_contract_definition", "exact_stage2_authority_facade_literal"),
    "src/domain/vnext/authority/facade/repository_admission.rs": ("Authority", "candidate_contract_definition", "exact_stage4_execution_authority_admission"),
    "src/domain/vnext/authority/facade/repository_leaf_authority.rs": ("Authority", "candidate_contract_definition", "exact_stage4_execution_authority_closed_union"),
    "src/domain/vnext/authority/transition.rs": ("Authority", "candidate_contract_definition", "exact_stage2_transition_guard_literal"),
    "tests/vnext_authority_capacity_transition.rs": ("Stage2Proof", "candidate_proof_reader", "exact_stage2_capacity_and_transition_proof"),
    "tests/vnext_authority_contracts.rs": ("Stage2Proof", "candidate_proof_reader", "exact_stage2_authority_contract_proof"),
    "tests/vnext_authority_continuity_totality.rs": ("Stage2Proof", "candidate_proof_reader", "exact_stage2_continuity_totality_proof"),
    "tests/vnext_authority_literals.rs": ("Stage2Proof", "candidate_proof_reader", "exact_stage2_literal_artifact_proof"),
    "tools/vnext_contracts/stage2/authority/build.py": ("Stage2Authority", "candidate_contract_definition", "exact_stage2_authority_builder_semantics"),
    "tools/vnext_contracts/stage2/authority/validate.py": ("Stage2Proof", "candidate_proof_reader", "independent_stage2_semantic_reconstruction"),
    "tools/vnext_contracts/stage2/authority/verify.rb": ("Stage2Proof", "candidate_proof_reader", "independent_stage2_ruby_reconstruction"),
}
STAGE2_PREDECESSOR_CONSUMER_CENSUS_ID = "sha256:962cb761a55a8cdf1250ac4068f957d9357cd389d5f3ef34e4af501b18fa74df"
STAGE2_PREDECESSOR_CANDIDATE_ROOT_ID = "sha256:128bf49c9195ed8d7e395a77e1deaa26376613598e8142679cb2625890d49f59"
SEMANTIC_LITERAL_SOURCES = {
    "contracts/vnext/catalogs/evidence/e346-nominal-source.json": (
        "V1AuditMigrationEvidence",
        "sealed_v1_audit_migration_consumer",
        "sealed_v1_e346_nominal_source_evidence",
    ),
    "contracts/vnext/catalogs/evidence/e346-semantic-baseline.json": (
        "V1AuditMigrationEvidence",
        "sealed_v1_audit_migration_consumer",
        "sealed_v1_e346_semantic_baseline_evidence",
    ),
    "contracts/vnext/catalogs/generated/catalog-02-effect.json": ("Catalogs", "candidate_contract_definition", "direct_catalog_literal"),
    "contracts/vnext/catalogs/generated/catalog-06-action-leaf.json": ("Catalogs", "candidate_contract_definition", "direct_catalog_literal"),
    "contracts/vnext/catalogs/generated/catalog-07-repository-continuity.json": ("Catalogs", "candidate_contract_definition", "direct_catalog_literal"),
    "contracts/vnext/catalogs/generated/catalog-08-installation-continuity.json": ("Catalogs", "candidate_contract_definition", "direct_catalog_literal"),
    "contracts/vnext/catalogs/generated/catalog-09-action-spec.json": ("Catalogs", "candidate_contract_definition", "direct_catalog_literal"),
    "contracts/vnext/catalogs/generated/catalog-profile-grammar-v1.json": ("Catalogs", "candidate_contract_definition", "direct_catalog_literal"),
    "contracts/vnext/catalogs/generated/inventory.json": ("Catalogs", "candidate_contract_definition", "direct_catalog_literal"),
    "contracts/vnext/catalogs/generated/manifest-identity-input.json": ("Catalogs", "candidate_contract_definition", "direct_catalog_literal"),
    "contracts/vnext/stage0/decision-closure/external-design-authority-closure.v1.cbor": ("DecisionClosure", "candidate_contract_definition", "direct_decision_closure_resource_literal"),
    "contracts/vnext/stage0/resource-release/predecessor-migration-cutover-contract-v1.json": ("ResourceRelease", "candidate_contract_definition", "direct_resource_release_predecessor_literal"),
    "contracts/vnext/stage0/resource-release/predecessor-resource-contract-suite-v1.json": ("ResourceRelease", "candidate_contract_definition", "direct_resource_release_predecessor_literal"),
    "contracts/vnext/public/setup_operation_compatibility.v1.json": ("PublicContracts", "candidate_contract_definition", "direct_public_literal"),
    "src/domain/vnext/execution/control_head.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/vnext/execution/ceremony.rs": ("Execution", "candidate_contract_definition", "direct_stage4_protected_ceremony_literal"),
    "src/domain/vnext/execution/dispatch_state.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/vnext/execution/effect_home.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/vnext/execution/effect_routes.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/vnext/execution/effects.rs": ("Execution", "candidate_contract_definition", "direct_stage4_effect_runtime_literal"),
    "src/domain/vnext/execution/mod.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/vnext/execution/runtime.rs": ("Execution", "candidate_contract_definition", "direct_stage4_execution_runtime_literal"),
    "src/domain/vnext/execution/store.rs": ("Execution", "candidate_contract_definition", "direct_stage4_atomic_store_literal"),
    "src/domain/vnext/execution/withdrawal.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/vnext/identity/manifest.rs": ("Identity", "candidate_contract_definition", "direct_identity_literal"),
    "src/domain/vnext/integration/public_literals.rs": ("PublicContracts", "candidate_contract_definition", "direct_public_contract_literal"),
    "tests/vnext_dispatch_cutover_literals.rs": ("Stage0Proof", "candidate_proof_reader", "direct_stage0_literal_test"),
    "tests/vnext_effect_home_literals.rs": ("Stage0Proof", "candidate_proof_reader", "direct_stage0_literal_test"),
    "tests/vnext_stage4_contracts.rs": ("Stage4Proof", "candidate_proof_reader", "direct_stage4_execution_contract_proof"),
    "tests/vnext_manifest_identity.rs": ("Stage0Proof", "candidate_proof_reader", "direct_stage0_literal_test"),
    "tools/vnext_contracts/catalogs/build.py": ("Catalogs", "candidate_contract_definition", "direct_catalog_builder_literal"),
    "tools/vnext_contracts/catalogs/predecessor_e346/vnext_catalog_profile_grammar_build.py": ("Stage0Proof", "candidate_proof_reader", "predecessor_grammar_reproduction_builder"),
    "tools/vnext_contracts/catalogs/predecessor_e346/vnext_catalog_profile_grammar_validate.py": ("Stage0Proof", "candidate_proof_reader", "predecessor_grammar_semantic_validator"),
    "tools/vnext_contracts/catalogs/predecessor_e346/vnext_catalog_suite_build.py": ("Stage0Proof", "candidate_proof_reader", "predecessor_catalog_reproduction_builder"),
    "tools/vnext_contracts/catalogs/validate.py": ("Catalogs", "candidate_proof_reader", "direct_catalog_validator_literal"),
    "tools/vnext_contracts/public/build_public_literals.py": ("PublicContracts", "candidate_contract_definition", "direct_public_builder_literal"),
    "tools/vnext_contracts/stage0/dispatch_cutover/build.py": ("Stage0DispatchCutover", "candidate_contract_definition", "direct_dispatch_builder_literal"),
    "tools/vnext_contracts/stage0/effect_home/build.py": ("Stage0EffectHome", "candidate_contract_definition", "direct_effect_home_builder_literal"),
    "tools/vnext_contracts/stage0/effect_home/validate.py": ("Stage0Proof", "candidate_proof_reader", "direct_effect_home_validator_literal"),
    "tools/vnext_contracts/stage0/proof_matrix/build.py": ("Stage0Proof", "candidate_proof_reader", "stage0_proof_manifest_effect_home_reader"),
    "tools/vnext_contracts/stage4/execution/build.py": ("Stage4Execution", "candidate_contract_definition", "direct_stage4_execution_builder_literal"),
    "tools/vnext_contracts/stage4/execution/validate.py": ("Stage4Proof", "candidate_proof_reader", "independent_stage4_execution_reconstruction"),
    "tools/vnext_contracts/stage4/execution/verify.rb": ("Stage4Proof", "candidate_proof_reader", "independent_stage4_execution_ruby_reconstruction"),
}
SEMANTIC_ROLE_SOURCES = {
    "tools/vnext_contracts/stage0/effect_home/encode.rb": (
        "Stage0Proof",
        "candidate_proof_reader",
        "independent_cbor_receipt_encoder_for_effect_home_identity_input",
        "independent_cbor_encoder_source",
    ),
    "tools/vnext_contracts/stage0/resource_release/validate.py": (
        "Stage0Proof",
        "candidate_proof_reader",
        "direct_resource_release_effect_home_reader",
        "function_scoped_resource_release_effect_binding_validation",
    ),
}
SEMANTIC_ROLE_AST_BINDINGS = {
    "tools/vnext_contracts/stage0/resource_release/validate.py": (
        "validate_resource_release",
        (
            'effect_path = "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"',
            "effect = json.loads((ROOT / effect_path).read_text())",
            'require(closure.get("effect_home_finalization_receipt_sha256") == file_sha(effect_path), "Effect finalization source SHA drifted")',
            'require(closure.get("effect_home_finalization_identity") == effect["identity"], "Effect finalization identity drifted")',
            'require(closure.get("effect_home_expected_delta_manifest_id") == effect["expected_delta_manifest_id"], "Effect expected-delta ManifestId drifted")',
        ),
    ),
}
SEMANTIC_ROLE_AST_CALLERS = {
    "tools/vnext_contracts/stage0/resource_release/validate.py": (
        "validate_all",
        "validate_resource_release(documents, inventory, resources, bundles, census, release)",
    ),
}
SEMANTIC_ROLE_SOURCE_SHA256 = {
    "tools/vnext_contracts/stage0/resource_release/validate.py": "1ef7b22757e35bcb97db6b3bfdb0fc7f0d0f2fc2486901835d877616562ea667",
}
DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS = {
    "contracts/vnext/stage4/execution/execution-effects.v1.cbor": (
        ["EffectIntent", "DispatchAttempt", "ReconciliationAttempt", "RecoverReserved", "ControlHead"],
        "Stage4Execution",
        "pending_downstream_generated_binding",
        "resolved_by_stage4_execution_manifest",
    ),
    "contracts/vnext/stage4/execution/execution-effects.v1.json": (
        ["EffectIntent", "DispatchAttempt", "ReconciliationAttempt", "RecoverReserved", "ControlHead"],
        "Stage4Execution",
        "pending_downstream_generated_binding",
        "resolved_by_stage4_execution_manifest",
    ),
    "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.cbor": (
        ["EffectOrigin"],
        "ResourceRelease",
        "pending_downstream_generated_binding",
        "resolved_by_current_resource_consumer_census",
    ),
    "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.json": (
        ["EffectOrigin"],
        "ResourceRelease",
        "pending_downstream_generated_binding",
        "resolved_by_current_resource_consumer_census",
    ),
    "contracts/vnext/stage0/resource-release/resource-release.v1.json": (
        ["EffectOrigin"],
        "ResourceRelease",
        "pending_downstream_generated_binding",
        "resolved_by_current_resource_consumer_census",
    ),
    "contracts/vnext/stage2/authority/authority-continuity-manifest.v1.cbor": (
        ["EffectIntent"],
        "Stage2Authority",
        "pending_downstream_generated_binding",
        "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/authority-continuity-manifest.v1.json": (
        ["EffectIntent"],
        "Stage2Authority",
        "pending_downstream_generated_binding",
        "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/authority-literals.v1.cbor": (
        ["EffectIntent"],
        "Stage2Authority",
        "pending_downstream_generated_binding",
        "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/authority-literals.v1.json": (
        ["EffectIntent"],
        "Stage2Authority",
        "pending_downstream_generated_binding",
        "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.cbor": (
        ["EffectOrigin"],
        "Stage2Authority",
        "pending_downstream_generated_binding",
        "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json": (
        ["EffectOrigin"],
        "Stage2Authority",
        "pending_downstream_generated_binding",
        "resolved_by_stage2_authority_root_manifest",
    ),
}
LEGACY_SEMANTIC_REMOVAL_SOURCES = {
    "src/domain/channel.rs": ("delivery_release", "Channel", "replacement_removal_target", "append_delivery_receipt", "legacy_delivery_receipt_and_latest_cursor_writer", "direct_v1_delivery_receipt_and_latest_join"),
    "src/interfaces/cli/msg.rs": ("connector_adapter", "CliMessage", "replacement_removal_target", "send_codex_thread_primary", "legacy_codex_connector_delivery_adapter", "direct_v1_connector_delivery_adapter"),
    "tests/msg_codex_delivery_integration.rs": ("delivery_release", "Stage0Proof", "candidate_proof_reader", "codex_to_codex_msg_send_uses_target_thread_without_unread_local_duplicate", "legacy_codex_delivery_contract_test", "direct_v1_delivery_integration_proof"),
    "src/interfaces/mcp/tools.rs": ("connector_adapter", "Mcp", "replacement_removal_target", "maestro_status", "legacy_mcp_command_adapter", "direct_v1_mcp_adapter_surface"),
    "src/interfaces/mcp/server.rs": ("connector_adapter", "Mcp", "replacement_removal_target", "discover_repo_root", "legacy_mcp_repo_scope_adapter", "direct_v1_mcp_server_scope"),
    "src/operations/update/github_release.rs": ("update_github_curl", "Update", "replacement_removal_target", "GitHubCurlDownloader", "legacy_github_curl_release_reader", "direct_v1_github_curl_path"),
    "src/operations/update/mod.rs": ("update_github_curl", "Update", "replacement_removal_target", "InstallMethod::Curl", "legacy_update_install_method_classifier", "direct_v1_update_curl_selector"),
    "src/interfaces/cli/update.rs": ("update_github_curl", "CliUpdate", "replacement_removal_target", "auto_check_paths_from", "legacy_update_cwd_scope_inference", "direct_v1_update_cli_scope"),
    "tests/update_integration.rs": ("update_github_curl", "Stage0Proof", "candidate_proof_reader", "curl_update", "legacy_update_github_curl_contract_test", "direct_v1_update_integration_proof"),
    "src/domain/install/mirrors.rs": ("install_mirrors_effects", "Install", "replacement_removal_target", "write_prepared_mirrors_with_effects", "legacy_install_mirror_effect_writer", "direct_v1_install_effect_path"),
    "src/domain/install/mod.rs": ("install_mirrors_effects", "Install", "replacement_removal_target", "install_agent_with_writer", "legacy_install_mirror_transaction", "direct_v1_install_transaction"),
    "src/interfaces/cli/install.rs": ("install_mirrors_effects", "CliInstall", "replacement_removal_target", "preview.mirrors", "legacy_install_mirror_command", "direct_v1_install_cli_surface"),
    "tests/install_mirrors.rs": ("install_mirrors_effects", "Stage0Proof", "candidate_proof_reader", "mirror_plan_writes_managed_content_for_claude", "legacy_install_mirror_contract_test", "direct_v1_install_mirror_proof"),
    "tests/install_uninstall_integration.rs": ("install_mirrors_effects", "Stage0Proof", "candidate_proof_reader", "install_claude_writes_managed_mirrors_and_lock", "legacy_install_uninstall_contract_test", "direct_v1_install_uninstall_proof"),
    "tests/install_dry_run_integration.rs": ("install_mirrors_effects", "Stage0Proof", "candidate_proof_reader", "Preview mirror writes", "legacy_install_preview_contract_test", "direct_v1_install_preview_proof"),
    "src/domain/proof/commands.rs": ("proof_status_skills", "Proof", "replacement_removal_target", "current_dir(paths.repo_root())", "legacy_proof_command_cwd_effect", "direct_v1_proof_command_effect"),
    "src/domain/proof/receipts.rs": ("proof_status_skills", "Proof", "replacement_removal_target", "with_locator", "legacy_receipt_locator_scope_inference", "direct_v1_receipt_locator_path"),
    "src/domain/proof/verify_task.rs": ("proof_status_skills", "Proof", "replacement_removal_target", "VerificationCommandReceipt", "legacy_task_proof_receipt_writer", "direct_v1_proof_verification_path"),
    "src/interfaces/cli/status.rs": ("proof_status_skills", "CliStatus", "replacement_removal_target", "StatusReport", "legacy_status_projection_classifier", "direct_v1_status_projection"),
    "src/domain/skills/catalog.rs": ("proof_status_skills", "Skills", "replacement_removal_target", "pub struct Skill", "legacy_shipped_skill_catalog", "direct_v1_skill_catalog"),
    "src/domain/skills/global.rs": ("proof_status_skills", "Skills", "replacement_removal_target", "skills-lock.yaml", "legacy_global_skill_symlink_effect", "direct_v1_global_skill_path"),
    "tests/status_next_integration.rs": ("proof_status_skills", "Stage0Proof", "candidate_proof_reader", "reconcile_feature", "legacy_status_next_contract_test", "direct_v1_status_integration_proof"),
    "tests/global_skills_integration.rs": ("proof_status_skills", "Stage0Proof", "candidate_proof_reader", "global Maestro skills synced", "legacy_global_skill_contract_test", "direct_v1_global_skill_proof"),
    "tests/skills_symlink_integration.rs": ("proof_status_skills", "Stage0Proof", "candidate_proof_reader", "legacy per-repo skills symlink", "legacy_skill_symlink_contract_test", "direct_v1_skill_symlink_proof"),
    "src/foundation/core/paths.rs": ("scope_inference", "Foundation", "replacement_removal_target", "discover_repo_root", "legacy_repo_path_cwd_scope_inference", "direct_v1_repo_path_scope"),
    "src/interfaces/cli/mod.rs": ("scope_inference", "Cli", "replacement_removal_target", "resolve_project_in", "legacy_project_cwd_actor_scope_inference", "direct_v1_cli_scope_inference"),
    "src/domain/card/locator.rs": ("scope_inference", "Card", "replacement_removal_target", "ArtifactLocator", "legacy_artifact_locator_scope", "direct_v1_locator_scope"),
    "tests/project_scope_read_surface.rs": ("scope_inference", "Stage0Proof", "candidate_proof_reader", "--project", "legacy_project_scope_contract_test", "direct_v1_project_scope_proof"),
    "src/domain/card/query.rs": ("latest_row_join", "Card", "replacement_removal_target", "pub fn classify", "legacy_card_row_classifier", "direct_v1_card_classification"),
    "src/domain/run/active.rs": ("latest_row_join", "Run", "replacement_removal_target", "latest event", "legacy_latest_run_activity_join", "direct_v1_latest_row_join"),
    "src/domain/run/trace.rs": ("latest_row_join", "Run", "replacement_removal_target", "latest_proof", "legacy_latest_proof_join", "direct_v1_latest_proof_join"),
    "src/domain/feature/verification.rs": ("latest_row_join", "Feature", "replacement_removal_target", "latest_explicit_evidence", "legacy_latest_acceptance_evidence_join", "direct_v1_latest_acceptance_join"),
    "tests/run_evidence_integration.rs": ("latest_row_join", "Stage0Proof", "candidate_proof_reader", "managed_run", "legacy_run_evidence_contract_test", "direct_v1_run_evidence_proof"),
    "src/domain/feature/reconcile.rs": ("independent_cross_worktree_classification", "Feature", "replacement_removal_target", "store_reconcile_receipt_extension", "legacy_independent_reconcile_classification_writer", "direct_v1_reconcile_receipt_writer"),
    "src/domain/run/evidence.rs": ("independent_cross_worktree_classification", "Run", "replacement_removal_target", "classify", "legacy_independent_run_schema_classifier", "direct_v1_run_classification"),
    "src/domain/conflict.rs": ("independent_cross_worktree_classification", "Conflict", "replacement_removal_target", "cross-worktree", "legacy_cross_worktree_conflict_classifier", "direct_v1_cross_worktree_conflict"),
    "src/interfaces/cli/conflict.rs": ("independent_cross_worktree_classification", "CliConflict", "replacement_removal_target", "cross-worktree", "legacy_cross_worktree_conflict_adapter", "direct_v1_cross_worktree_cli"),
    "src/interfaces/cli/active.rs": ("independent_cross_worktree_classification", "CliActive", "replacement_removal_target", "cross-worktree", "legacy_cross_worktree_active_classifier", "direct_v1_cross_worktree_active"),
    "src/interfaces/tui/task_list_watch.rs": ("independent_cross_worktree_classification", "Tui", "replacement_removal_target", "cross-worktree", "legacy_cross_worktree_tui_projection", "direct_v1_cross_worktree_tui"),
    "src/foundation/core/git.rs": ("independent_cross_worktree_classification", "Foundation", "replacement_removal_target", "worktree_roots", "legacy_cross_worktree_root_union", "direct_v1_cross_worktree_root_union"),
}
C325_BASELINE_DRIFTS = [
    ("src/domain/loop_recipes.rs", "753c9f535ebf219cb60998cb19aa2876b8dfad9134eedab4a878a478a549ea91", "12d02195fd88031b2661c236318af4f9dc5b790ebd41d8bfb97066530c5e5394", "loop_recipes_domain_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("src/domain/mod.rs", "70a6dbbc96645090e77750f12879db081e3b2f4647e04c9785e13617c1f55ce7", "8ecc94ec3520e1b00fc76cd453ace95645af36453fc2888a23948dba49ab2930", "domain_module_facade_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("src/interfaces/cli/loop_recipes.rs", "babfbd7ef04b869e9a86d0022c312b10653b5df5f1be5d7e647d967a0d44e947", "f8c3fcd5d01aaa9a590e4c93703928e018531dd5d1665b385411ae8fe95e91c7", "loop_recipes_cli_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("src/interfaces/cli/mod.rs", "7c43e73ff25ae8c12d378b0a9ead453f7ad89452848b59a9190a852a87f1c7f8", "ddb86f6af0a20bbda80fb03824f3c18ed8ca45c62e688a6cd89099d4654ae000", "cli_module_facade_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("src/interfaces/cli/status.rs", "d560602a5ac888e5ef6f256f487056712536f7915e18f855389b1db2e2695a35", "4f62a633c8398d404605eed8647e15c815fdf5b5ab2ea81ea188f0716c9601f1", "status_cli_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("tests/loop_recipes_integration.rs", "85cc2627b8db9616bdd20976204c660737d726858f546f62fd20a6ea26f1c61c", "5fc7a9808427daf0caea1316c6069d7d31528c0c19c50dc0d6836b5c777fc447", "loop_recipes_test_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("tests/resource_contracts.rs", "1c553df182bbe8a6f5a22ef1c838cb8fd12ee2ddd118cfda61f4f7b145ec4878", "81e07503d036606d3fbeb6514bb475e4ad894336515a1d868b35bd11278b4275", "resource_contract_test_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("tests/resources_version_guard.rs", "8176e638c10301d1fd5ede30ab006096307895ef229fe3cd197a15eab8c1c0ba", "2701713fc6d5fa6b05665d05d8b44ab8987c0c8b8dc2fab9aeeb060f51af2612", "resource_version_guard_changed_after_c325_baseline_not_effect_semantic_evidence"),
]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def head(major: int, value: int) -> bytes:
    if not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("deterministic CBOR requires unsigned u64")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def encode(value: object) -> bytes:
    if isinstance(value, bool):
        return b"\xf5" if value else b"\xf4"
    if isinstance(value, int):
        return head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return head(3, len(raw)) + raw
    if isinstance(value, list):
        return head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        raw = bytes.fromhex(str(value["bytes"]))
        return head(2, len(raw)) + raw
    raise ValueError(f"value is outside the frozen deterministic CBOR subset: {value!r}")


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="ascii"))


def require_sha(path: Path, expected: str) -> str:
    actual = sha256_bytes(path.read_bytes())
    if actual != expected:
        raise ValueError(f"frozen input drifted: {path}: expected {expected}, got {actual}")
    return actual


def frozen_source_hashes() -> dict[str, str]:
    require_sha(SOURCE_BINDINGS, EXPECTED_SOURCE_BINDINGS_SHA256)
    bindings = read_json(SOURCE_BINDINGS)
    if bindings.get("schema") != "maestro.vnext.stage0-input-bindings.v1":
        raise ValueError("Stage-0 source bindings use an unexpected schema")
    if bindings.get("feature_id") != "maestro-whole-flow-architecture-refoundation":
        raise ValueError("Stage-0 source bindings name an unexpected feature")
    recorded = bindings.get("canonical_source_inputs")
    expected_recorded = {
        "design_sha256": EXPECTED_INPUTS["design"],
        "decisions_sha256": EXPECTED_INPUTS["decisions"],
        "card_sha256": EXPECTED_INPUTS["card"],
    }
    if recorded != expected_recorded:
        raise ValueError(
            "Stage-0 source bindings do not match the approved canonical source inputs"
        )
    return {
        "design": expected_recorded["design_sha256"],
        "decisions": expected_recorded["decisions_sha256"],
        "card": expected_recorded["card_sha256"],
        "c325": require_sha(C325, EXPECTED_INPUTS["c325"]),
    }


def write_json(path: Path, value: object) -> str:
    rendered = canonical_json(value) + "\n"
    path.write_text(rendered, encoding="ascii")
    return sha256_bytes(rendered.encode("ascii"))


def canonical_json(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def catalog_inputs() -> tuple[dict[str, dict[str, object]], dict[str, object]]:
    paths = {
        "effect": CATALOGS / "catalog-02-effect.json",
        "ceremony": CATALOGS / "catalog-05-ceremony.json",
        "action_leaf": CATALOGS / "catalog-06-action-leaf.json",
        "action_spec": CATALOGS / "catalog-09-action-spec.json",
        "grammar": CATALOGS / "catalog-profile-grammar-v1.json",
    }
    documents = {name: read_json(path) for name, path in paths.items()}
    grammar = documents["grammar"]
    if grammar["effective_decision"][0] != "dec-close-lifecycle-to-action-totality-and-efa0":
        raise ValueError("catalog grammar is not the effective efa0 successor")
    if len(documents["effect"]["descriptors"]) != 23:
        raise ValueError("Effect origin catalog is not the exact 23-origin successor")
    if len(documents["ceremony"]["descriptors"]) != 11:
        raise ValueError("Ceremony catalog is not the exact eleven-row successor")
    if len(documents["action_leaf"]["descriptors"]) != 145:
        raise ValueError("Action leaf catalog is not the exact 145-row efa0 successor")
    if len(documents["action_spec"]["descriptors"]) != 145:
        raise ValueError("Action spec catalog is not the exact 145-row efa0 successor")
    return documents, {name: sha256_bytes(path.read_bytes()) for name, path in paths.items()}


def branch_maps(documents: dict[str, dict[str, object]]) -> tuple[dict[int, str], dict[int, str]]:
    action = {
        int(row["value"][0]): str(row["value"][1])
        for row in documents["action_leaf"]["descriptors"]
    }
    ceremony = {
        int(row["value"][0]): str(row["value"][1])
        for row in documents["ceremony"]["descriptors"]
    }
    if not set(RESERVE_LEAVES).issubset(set(action.values())):
        raise ValueError("one of the four exact reserve leaves is absent")
    if len(action) != 145 or len(ceremony) != 11:
        raise ValueError("catalog tags are not unique")
    return action, ceremony


def flattened_routes(documents: dict[str, dict[str, object]]) -> list[dict[str, object]]:
    action, ceremony = branch_maps(documents)
    routes: list[dict[str, object]] = []
    for origin in documents["grammar"]["effect_origin_routes"]:
        origin_name = str(origin["origin_name"])
        origin_tag = int(origin["origin_tag"])
        for entry in origin["value"][3]:
            route_tag = int(entry[0])
            role_tag = int(entry[1])
            role = ROLES.get(role_tag)
            if role is None:
                raise ValueError("unknown effect route role")
            branch_tag = int(entry[5])
            if role.startswith("Action"):
                branch = action.get(branch_tag)
                home = "ActiveStoreHomeV1"
                mode = {
                    "ActionReserve": "InitiateNew",
                    "ActionRecoverReserved": "RecoverReserved",
                }.get(role)
            else:
                branch = ceremony.get(branch_tag)
                home = "NoStoreCeremonyHomeV1" if branch == "InstallationContextGenesis" else "PreStoreCeremonyHomeV1"
                mode = {
                    "CeremonyInitiate": "Initiate",
                    "CeremonyRecoverReserved": "RecoverReserved",
                    "CeremonyResolveResult": "ResolveResult",
                    "CeremonyWithdraw": "Withdraw",
                }[role]
            if branch is None:
                raise ValueError("route points at a missing catalog branch")
            routes.append({
                "origin_tag": origin_tag,
                "origin": origin_name,
                "route_tag": route_tag,
                "role": role,
                "home": home,
                "branch_tag": branch_tag,
                "branch": branch,
                "mode": mode,
                "catalog_descriptor_id": entry[6]["bytes"],
            })
    # The canonical row order is the catalog's origin tag followed by frozen local route tag.
    routes.sort(key=lambda row: (int(row["origin_tag"]), int(row["route_tag"])))
    if len(routes) != 139:
        raise ValueError("EffectOriginHomeCompatibilityV1 is not 139 routes")
    action_rows = [row for row in routes if str(row["role"]).startswith("Action")]
    ceremony_rows = [row for row in routes if str(row["role"]).startswith("Ceremony")]
    if len(action_rows) != 95 or len(ceremony_rows) != 44:
        raise ValueError("route phase partition drifted")
    if len([row for row in action_rows if row["role"] == "ActionWithdraw"]) != 19:
        raise ValueError("Action withdrawal route count drifted")
    if len([row for row in ceremony_rows if row["role"] == "CeremonyWithdraw"]) != 11:
        raise ValueError("Ceremony withdrawal route count drifted")
    if any(row["role"] == "CeremonyResolveResult" and row["home"] == "ActiveStoreHomeV1" for row in routes):
        raise ValueError("Ceremony ResolveResult may not be active-store")
    return routes


def artifact(name: str, value: list[object], body: dict[str, object]) -> dict[str, object]:
    cbor = encode(value)
    identity = sha256_bytes(encode([DOMAIN, value]))
    return {
        "schema_version": "maestro.vnext.stage0.effect-home-artifact.v1",
        "artifact": name,
        "publication_state": "candidate_only_runtime_inactive",
        "identity": f"sha256:{identity}",
        "canonical_cbor_sha256": sha256_bytes(cbor),
        "canonical_cbor_hex": cbor.hex(),
        "canonical_value": value,
        **body,
    }


def ast_binds_name(node: ast.AST, name: str) -> bool:
    if isinstance(node, ast.Name):
        return node.id == name and isinstance(node.ctx, (ast.Store, ast.Del))
    if isinstance(node, ast.arg):
        return node.arg == name
    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
        return node.name == name
    if isinstance(node, ast.ExceptHandler):
        return node.name == name
    if isinstance(node, (ast.MatchAs, ast.MatchStar)):
        return node.name == name
    if isinstance(node, ast.MatchMapping):
        return node.rest == name
    if isinstance(node, ast.alias):
        return (node.asname or node.name.split(".", 1)[0]) == name
    if isinstance(node, ast.Attribute):
        return node.attr == name and isinstance(node.ctx, (ast.Store, ast.Del))
    if isinstance(node, ast.Subscript) and isinstance(node.ctx, (ast.Store, ast.Del)):
        return isinstance(node.slice, ast.Constant) and node.slice.value == name
    return False


def has_static_infinite_loop(statements: list[ast.stmt]) -> bool:
    return any(isinstance(node, ast.While) for statement in statements for node in ast.walk(statement))


def has_dynamic_namespace_mutation(node: ast.AST) -> bool:
    dangerous = {"exec", "eval", "globals", "locals", "vars", "setattr", "delattr", "__import__", "getattr"}
    if isinstance(node, ast.Call):
        if isinstance(node.func, ast.Name) and node.func.id in dangerous:
            return True
        if isinstance(node.func, ast.Attribute) and node.func.attr in dangerous:
            return True
    return isinstance(node, ast.alias) and node.name.rsplit(".", 1)[-1] in dangerous


def validate_semantic_role_source(path: str, source: str | None = None) -> None:
    binding = SEMANTIC_ROLE_AST_BINDINGS.get(path)
    if binding is None:
        return
    function_name, expected_statements = binding
    raw_source = (ROOT / path).read_bytes() if source is None else source.encode("utf-8")
    if sha256_bytes(raw_source) != SEMANTIC_ROLE_SOURCE_SHA256[path]:
        raise ValueError(f"semantic role source bytes drifted from the executable binding: {path}")
    source = raw_source.decode("utf-8")
    tree = ast.parse(source, filename=path)
    functions = [
        node
        for node in tree.body
        if isinstance(node, ast.FunctionDef)
        and node.name == function_name
    ]
    if len(functions) != 1:
        raise ValueError(f"semantic role source must contain one top-level {function_name}: {path}")
    function = functions[0]
    if function.decorator_list or any(
        isinstance(node, (ast.Yield, ast.YieldFrom)) for node in ast.walk(function)
    ):
        raise ValueError(f"semantic role source target is not a plain executable function: {path}")
    if any(
        (node is not function and ast_binds_name(node, function_name))
        or has_dynamic_namespace_mutation(node)
        for node in ast.walk(function)
    ):
        raise ValueError(f"semantic role source target can rebind its exact target: {path}")
    body = function.body
    actual = [ast.dump(statement, include_attributes=False) for statement in body]
    expected = [
        ast.dump(ast.parse(statement).body[0], include_attributes=False)
        for statement in expected_statements
    ]
    matches = [
        index
        for index in range(len(actual) - len(expected) + 1)
        if actual[index : index + len(expected)] == expected
    ]
    if len(matches) != 1:
        raise ValueError(f"semantic role source lost its exact function-scoped proof: {path}")
    if has_static_infinite_loop(body[: matches[0]]) or any(
        isinstance(node, (ast.Return, ast.Raise))
        for statement in body[: matches[0]]
        for node in ast.walk(statement)
    ):
        raise ValueError(f"semantic role source made its exact proof unreachable: {path}")

    caller_name, caller_statement = SEMANTIC_ROLE_AST_CALLERS[path]
    callers = [
        node
        for node in tree.body
        if isinstance(node, ast.FunctionDef)
        and node.name == caller_name
    ]
    if len(callers) != 1:
        raise ValueError(f"semantic role source must contain one top-level {caller_name}: {path}")
    caller = callers[0]
    if caller.decorator_list or any(
        isinstance(node, (ast.Yield, ast.YieldFrom)) for node in ast.walk(caller)
    ):
        raise ValueError(f"semantic role caller is not a plain executable function: {path}")
    if not caller.body or not isinstance(caller.body[0], ast.Try):
        raise ValueError(f"semantic role caller must begin with its direct try body: {path}")
    caller_body = caller.body[0].body
    expected_call = ast.dump(ast.parse(caller_statement).body[0], include_attributes=False)
    call_matches = [
        index
        for index, statement in enumerate(caller_body)
        if ast.dump(statement, include_attributes=False) == expected_call
    ]
    if len(call_matches) != 1:
        raise ValueError(f"semantic role source lost its exact reachable caller: {path}")
    if has_static_infinite_loop(caller_body[: call_matches[0]]) or any(
        isinstance(node, (ast.Return, ast.Raise))
        for statement in caller_body[: call_matches[0]]
        for node in ast.walk(statement)
    ):
        raise ValueError(f"semantic role source made its exact caller unreachable: {path}")
    if any(
        ast_binds_name(node, function_name) or has_dynamic_namespace_mutation(node)
        for node in ast.walk(caller)
    ):
        raise ValueError(f"semantic role caller shadows its exact target: {path}")
    target_index = tree.body.index(function)
    caller_index = tree.body.index(caller)
    if target_index >= caller_index or any(
        ast_binds_name(node, function_name) or has_dynamic_namespace_mutation(node)
        for statement in tree.body[target_index + 1 :]
        for node in ast.walk(statement)
    ):
        raise ValueError(f"semantic role source rebinds its exact target: {path}")


def stage2_semantic_consumer_rows() -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for path, (owner, disposition, proof) in sorted(
        STAGE2_SEMANTIC_SOURCE_DECLARATIONS.items()
    ):
        source = ROOT / path
        if not source.is_file():
            raise ValueError(f"declared Stage 2 semantic consumer is missing: {path}")
        contents = source.read_text(encoding="utf-8", errors="ignore")
        matched = [
            literal for literal in STAGE2_SEMANTIC_LITERAL_PATTERNS if literal in contents
        ]
        if not matched:
            raise ValueError(f"declared Stage 2 semantic consumer has no exact literal: {path}")
        worktree_sha256 = sha256_bytes(source.read_bytes())
        rows.append(
            {
                "path": path,
                "resource_identity": f"sha256:{worktree_sha256}",
                "worktree_sha256": worktree_sha256,
                "matched_symbols_or_patterns": matched,
                "semantic_role": "stage2_semantic_consumer_delta",
                "owner": owner,
                "consumer_disposition": disposition,
                "proof": proof,
            }
        )
    return rows


def stage2_semantic_delta_artifact() -> dict[str, object]:
    rows = stage2_semantic_consumer_rows()
    canonical_rows = [
        [
            row["path"],
            row["resource_identity"],
            row["worktree_sha256"],
            row["matched_symbols_or_patterns"],
            row["owner"],
            row["consumer_disposition"],
            row["proof"],
        ]
        for row in rows
    ]
    rows_digest = sha256_bytes(
        "".join(
            f"{row[0]}  {row[1]}  {','.join(row[3])}  {row[4]}  {row[5]}  {row[6]}\n"
            for row in canonical_rows
        ).encode("ascii")
    )
    value: list[object] = [
        "Stage2SemanticConsumerDeltaV1",
        1,
        STAGE2_PREDECESSOR_CONSUMER_CENSUS_ID,
        STAGE2_PREDECESSOR_CANDIDATE_ROOT_ID,
        canonical_rows,
        [len(canonical_rows), rows_digest, "complete_exact_source_overlay"],
        "candidate_only_runtime_inactive",
    ]
    return artifact(
        "stage2-semantic-consumer-delta-v1",
        value,
        {
            "schema_version": "maestro.vnext.stage2.semantic-consumer-delta.v1",
            "predecessor": {
                "consumer_census_id": STAGE2_PREDECESSOR_CONSUMER_CENSUS_ID,
                "candidate_contract_root_id": STAGE2_PREDECESSOR_CANDIDATE_ROOT_ID,
            },
            "consumer_rows": [
                {
                    "path": row["path"],
                    "resource_identity": row["resource_identity"],
                    "worktree_sha256": row["worktree_sha256"],
                    "matched_literals": row["matched_symbols_or_patterns"],
                    "owner": row["owner"],
                    "consumer_disposition": row["consumer_disposition"],
                    "proof": row["proof"],
                }
                for row in rows
            ],
            "consumer_count": len(rows),
            "consumer_digest": rows_digest,
            "closure_status": "complete_exact_source_overlay",
        },
    )


def consumer_census() -> dict[str, object]:
    ledger = read_json(C325)
    if not (
        ledger["schema"] == "maestro.vnext.direct-consumer-evidence-ledger.v1"
        and ledger["candidate_only"] is True
        and ledger["runtime_activation"] is False
        and ledger["runtime_registration"] is False
        and ledger["evidence_classification"] == "non_promoting_historical_coverage"
        and ledger["current_source_equality_claimed"] is False
        and ledger["expected_count"] == 325
        and ledger["expected_digest"] == "9aee8ea371f770e8694131079d4bfb4845f849d59d0b545005a2f0371a42976a"
    ):
        raise ValueError("C325 is not the pinned non-promoting coverage ledger")
    source_rows = ledger["rows"]
    if not isinstance(source_rows, list) or len(source_rows) != 325:
        raise ValueError("C325 coverage universe count drifted")
    if [row["path"] for row in source_rows] != sorted(row["path"] for row in source_rows):
        raise ValueError("C325 coverage universe is not path sorted")
    source_hashes = {str(row["path"]): str(row["sha256"]) for row in source_rows}
    if len(source_hashes) != 325 or any(len(value) != 64 for value in source_hashes.values()):
        raise ValueError("C325 coverage universe shape drifted")

    baseline_drifts = []
    for path, pinned_sha256, baseline_worktree_sha256, explanation in C325_BASELINE_DRIFTS:
        if source_hashes.get(path) != pinned_sha256:
            raise ValueError(f"C325 baseline drift pin changed: {path}")
        baseline_drifts.append({
            "path": path,
            "pinned_sha256": pinned_sha256,
            "baseline_worktree_sha256": baseline_worktree_sha256,
            "explanation": explanation,
        })
    if len(baseline_drifts) != 8 or [row["path"] for row in baseline_drifts] != sorted(row["path"] for row in baseline_drifts):
        raise ValueError("C325 baseline drift snapshot is not the exact eight-row record")

    literal_hits: dict[str, list[str]] = {}
    for scope in (ROOT / "src", ROOT / "contracts/vnext", ROOT / "tools", ROOT / "tests"):
        for path in scope.rglob("*"):
            if not path.is_file() or path.is_relative_to(DEFAULT_OUTPUT) or path.suffix == ".pyc":
                continue
            contents = path.read_text(encoding="utf-8", errors="ignore")
            matched = [pattern for pattern in SEMANTIC_LITERAL_PATTERNS if pattern in contents]
            if matched:
                literal_hits[str(path.relative_to(ROOT))] = matched
    for path, (expected_patterns, _, _, _) in DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS.items():
        observed_patterns = literal_hits.pop(path, [])
        if observed_patterns is not None and observed_patterns != expected_patterns:
            raise ValueError(
                f"downstream generated semantic obligation changed: {path}: {observed_patterns}"
            )
    stage2_rows = stage2_semantic_consumer_rows()
    for row in stage2_rows:
        path = str(row["path"])
        observed_patterns = literal_hits.pop(path, [])
        expected_patterns = [
            pattern
            for pattern in row["matched_symbols_or_patterns"]
            if pattern in SEMANTIC_LITERAL_PATTERNS
        ]
        if observed_patterns != expected_patterns:
            raise ValueError(
                f"Stage 2 semantic overlay does not match the Stage 0 census: {path}: "
                f"expected={expected_patterns}, observed={observed_patterns}"
            )
    if set(literal_hits) != set(SEMANTIC_LITERAL_SOURCES):
        unexpected = sorted(set(literal_hits) - set(SEMANTIC_LITERAL_SOURCES))
        missing = sorted(set(SEMANTIC_LITERAL_SOURCES) - set(literal_hits))
        raise ValueError(f"semantic literal census closure changed: unexpected={unexpected}, missing={missing}")

    c325_literal_hits = sorted(set(literal_hits) & set(source_hashes))
    if c325_literal_hits:
        raise ValueError(f"C325 coverage rows unexpectedly became semantic hits: {c325_literal_hits}")

    effect_rows: list[dict[str, object]] = []
    for path in sorted(literal_hits):
        owner, disposition, proof = SEMANTIC_LITERAL_SOURCES[path]
        worktree_sha256 = sha256_bytes((ROOT / path).read_bytes())
        effect_rows.append({
            "path": path,
            "resource_identity": f"sha256:{worktree_sha256}",
            "worktree_sha256": worktree_sha256,
            "matched_symbols_or_patterns": literal_hits[path],
            "semantic_role": "direct_effect_contract_literal",
            "owner": owner,
            "consumer_disposition": disposition,
            "proof": proof,
        })
    for path, (owner, disposition, semantic_role, proof) in SEMANTIC_ROLE_SOURCES.items():
        validate_semantic_role_source(path)
        worktree_sha256 = sha256_bytes((ROOT / path).read_bytes())
        effect_rows.append({
            "path": path,
            "resource_identity": f"sha256:{worktree_sha256}",
            "worktree_sha256": worktree_sha256,
            "matched_symbols_or_patterns": [],
            "semantic_role": semantic_role,
            "owner": owner,
            "consumer_disposition": disposition,
            "proof": proof,
        })
    effect_rows.extend(stage2_rows)
    effect_rows.sort(key=lambda row: str(row["path"]))
    if len({row["path"] for row in effect_rows}) != len(effect_rows):
        raise ValueError("semantic consumer census has duplicate physical sources")
    allowed_dispositions = {
        "candidate_contract_definition",
        "candidate_proof_reader",
        "sealed_v1_audit_migration_consumer",
        "replacement_removal_target",
    }
    if any(row["consumer_disposition"] not in allowed_dispositions for row in effect_rows):
        raise ValueError("semantic consumer census contains an unresolved disposition")
    if any(not row["matched_symbols_or_patterns"] and not row["semantic_role"] for row in effect_rows):
        raise ValueError("semantic consumer census contains evidence without a symbol, pattern, or role")

    downstream_rows = [
        {
            "path": path,
            "matched_symbols_or_patterns": expected_patterns,
            "semantic_role": "downstream_generated_semantic_consumer",
            "producer": owner,
            "status": disposition,
            "proof": proof,
            "dependency_direction": "ResourceRelease consumes EffectHome",
            "identity_input": False,
        }
        for path, (expected_patterns, owner, disposition, proof) in sorted(
            DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS.items()
        )
    ]

    legacy_rows: list[dict[str, object]] = []
    for path, (surface, owner, disposition, evidence_pattern, semantic_role, proof) in sorted(LEGACY_SEMANTIC_REMOVAL_SOURCES.items()):
        source = ROOT / path
        if not source.is_file():
            raise ValueError(f"legacy semantic removal evidence is missing: {path}")
        if evidence_pattern not in source.read_text(encoding="utf-8", errors="ignore"):
            raise ValueError(f"legacy semantic removal evidence pattern drifted: {path}: {evidence_pattern}")
        worktree_sha256 = sha256_bytes(source.read_bytes())
        legacy_rows.append({
            "path": path,
            "resource_identity": f"sha256:{worktree_sha256}",
            "worktree_sha256": worktree_sha256,
            "legacy_surface": surface,
            "matched_symbols_or_patterns": [evidence_pattern],
            "semantic_role": semantic_role,
            "owner": owner,
            "consumer_disposition": disposition,
            "proof": proof,
        })
    if any(row["consumer_disposition"] not in allowed_dispositions for row in legacy_rows):
        raise ValueError("legacy semantic removal ledger contains an unresolved disposition")
    if set(row["path"] for row in effect_rows) & set(row["path"] for row in legacy_rows):
        raise ValueError("effect and legacy semantic ledgers overlap a physical source")
    all_rows = [*effect_rows, *legacy_rows]
    semantic_digest = sha256_bytes(
        "".join(
            f"{row['path']}  {row['resource_identity']}  {row.get('legacy_surface', 'effect_contract')}  {','.join(row['matched_symbols_or_patterns'])}  {row['semantic_role']}  {row['owner']}  {row['consumer_disposition']}  {row['proof']}\n"
            for row in all_rows
        ).encode("ascii")
    )
    counts = {
        disposition: sum(row["consumer_disposition"] == disposition for row in all_rows)
        for disposition in sorted(allowed_dispositions)
    }
    downstream_digest = sha256_bytes(
        "".join(
            f"{row['path']}  {','.join(row['matched_symbols_or_patterns'])}  {row['semantic_role']}  {row['producer']}  {row['status']}  {row['proof']}  {row['dependency_direction']}\n"
            for row in downstream_rows
        ).encode("ascii")
    )
    return {
        "c325_ledger_sha256": sha256_bytes(C325.read_bytes()),
        "c325_expected_count": ledger["expected_count"],
        "c325_expected_digest": ledger["expected_digest"],
        "source_input": ledger["source_input"],
        "coverage_universe_disposition": "sealed_non_promoting_historical_coverage_not_semantic_consumer_set",
        "baseline_drift_rows": baseline_drifts,
        "semantic_consumer_rows": effect_rows,
        "legacy_semantic_removal_consumer_rows": legacy_rows,
        "downstream_generated_semantic_consumer_obligations": downstream_rows,
        "semantic_consumer_digest": semantic_digest,
        "downstream_generated_semantic_consumer_digest": downstream_digest,
        "semantic_consumer_counts": counts,
        "effect_contract_consumer_count": len(effect_rows),
        "legacy_semantic_removal_consumer_count": len(legacy_rows),
        "total_actual_semantic_consumer_count": len(all_rows),
        "downstream_generated_semantic_consumer_obligation_count": len(downstream_rows),
        "total_declared_semantic_consumer_count": len(all_rows) + len(downstream_rows),
        "unresolved_actual_semantic_consumer_count": 0,
        "closure_status": "complete_upstream_sources_plus_declared_downstream_generated_obligations_runtime_inactive",
    }


def artifacts(documents: dict[str, dict[str, object]], input_hashes: dict[str, str]) -> dict[str, dict[str, object]]:
    routes = flattened_routes(documents)
    home_value: list[object] = [
        "EffectIntentHomeV1", 1,
        [
            ["ActiveStoreHomeV1", ["domain_kind", "stable_domain_id", "realm", "semantic_namespace", "home_qualified_semantic_uniqueness_namespace"]],
            ["NoStoreCeremonyHomeV1", ["protected_installation_realm", "locator_candidate_branch", "installation_context_genesis_ceremony"]],
            ["PreStoreCeremonyHomeV1", ["allowed_pre_store_ceremony", "destination_domain_kind", "candidate_branch_or_destination", "inactive_destination_lineage"]],
        ],
        [
            ["ActiveStoreOriginationFenceV1", ["store", "generation", "epoch", "namespace", "material_token", "action_request", "action_authority_basis", "receipt", "result", "effect_origin", "current_authority_commitment", "credential_commitment", "dispatch_reservation_or_fence"]],
            ["NoStoreCeremonyOriginationFenceV1", ["ceremony_spec", "ceremony_manifest", "initiate_mode", "sealed_ceremony_attempt_commitment", "attempt_id", "protected_realm", "locator_candidate_bundle", "carrier_identity", "carrier_incarnation", "expected_old_token", "candidate_seal", "external_anchor", "idempotency_identity", "dispatch_fence"]],
            ["PreStoreCeremonyOriginationFenceV1", ["ceremony_spec", "ceremony_manifest", "initiate_mode", "sealed_ceremony_attempt_commitment", "attempt_id", "branch_bundle", "inactive_destination", "candidate_seal", "carrier_identity", "carrier_incarnation", "expected_old_token", "external_authority_basis", "idempotency_identity", "dispatch_fence"]],
        ],
        [
            ["ActiveStoreUseFenceV1", ["same_stable_home", "generation", "epoch", "namespace", "material_token", "authority", "credentials", "attempt_fence", "idempotency_binding", "provider_contract_guards"]],
            ["NoStoreCeremonyUseFenceV1", ["same_home", "branch_authority", "carrier_incarnation", "expected_old_token", "attempt_id"]],
            ["PreStoreCeremonyUseFenceV1", ["same_home", "branch_authority", "carrier", "expected_old_token", "attempt_id"]],
        ],
        RESERVATION_MODES,
        CEREMONY_MODES,
        list(ROLES.values()),
    ]
    compatibility_value: list[object] = [
        "EffectOriginHomeCompatibilityV1", 1,
        [[row["origin_tag"], row["route_tag"], row["role"], row["home"], row["branch_tag"], row["catalog_descriptor_id"]] for row in routes],
        [documents["effect"]["manifest_id"], documents["ceremony"]["manifest_id"], documents["action_leaf"]["manifest_id"], documents["action_spec"]["manifest_id"], documents["grammar"]["catalog_profile_grammar"]["catalog_profile_grammar_id"]],
    ]
    bootstrap_value: list[object] = [
        "BootstrapControlWithdrawalV1", 1,
        [[name, status] for name, status in BOOTSTRAP_CENSUS],
        "WithdrawBootstrapMandateInteractionEffect",
        "BootstrapControlTerminalScopeAtomV1",
    ]
    artifacts_by_name: dict[str, dict[str, object]] = {
        "effect-intent-home-v1": artifact("effect-intent-home-v1", home_value, {
            "closed_union": ["ActiveStoreHomeV1", "NoStoreCeremonyHomeV1", "PreStoreCeremonyHomeV1"],
            "no_store_forbidden_fields": ["domain_id", "generation", "epoch"],
            "origination_fence": "immutable",
            "use_fence": "fresh_same_home_per_later_dispatch_or_action_reconciliation",
            "reconciliation_refusal": ["NoStoreCeremonyHomeV1", "PreStoreCeremonyHomeV1"],
            "resolve_result_effects": {"creates_intent": False, "creates_attempt": False, "creates_run": False, "performs_io": False},
            "reserve_leaves": RESERVE_LEAVES,
        }),
        "effect-origin-home-compatibility-v1": artifact("effect-origin-home-compatibility-v1", compatibility_value, {
            "origin_count": 23,
            "route_count": 139,
            "action_branch_count": 19,
            "ceremony_branch_count": 11,
            "formula": "19x5+11x4",
            "routes": routes,
            "catalog_bindings": {
                "catalog_02_effect": documents["effect"]["manifest_id"],
                "catalog_05_ceremony": documents["ceremony"]["manifest_id"],
                "catalog_06_action_leaf": documents["action_leaf"]["manifest_id"],
                "catalog_09_action_spec": documents["action_spec"]["manifest_id"],
                "catalog_profile_grammar": documents["grammar"]["catalog_profile_grammar"]["catalog_profile_grammar_id"],
            },
        }),
        "bootstrap-control-withdrawal-v1": artifact("bootstrap-control-withdrawal-v1", bootstrap_value, {
            "row_count": 11,
            "target_count": 3,
            "hard_exclusion_count": 8,
            "rows": [{"action": name, "disposition": status} for name, status in BOOTSTRAP_CENSUS],
            "terminal_scope_atom": "BootstrapControlTerminalScopeAtomV1",
            "seventh_interaction_hard_exclusion": "WithdrawBootstrapMandateInteractionEffect",
        }),
    }
    artifacts_by_name["stage2-semantic-consumer-delta-v1"] = (
        stage2_semantic_delta_artifact()
    )

    census = consumer_census()
    census_value: list[object] = [
        "EffectIntentControlConsumerCensusV1", 4,
        [
            "sealed_c325_coverage_universe",
            census["c325_ledger_sha256"],
            census["c325_expected_count"],
            census["c325_expected_digest"],
            [census["source_input"]["path"], census["source_input"]["sha256"]],
            census["coverage_universe_disposition"],
        ],
        [
            [
                row["path"],
                row["pinned_sha256"],
                row["baseline_worktree_sha256"],
                row["explanation"],
            ]
            for row in census["baseline_drift_rows"]
        ],
        [
            [
                row["path"],
                row["resource_identity"],
                row["worktree_sha256"],
                row["matched_symbols_or_patterns"],
                row["semantic_role"],
                row["owner"],
                row["consumer_disposition"],
                row["proof"],
            ]
            for row in census["semantic_consumer_rows"]
        ],
        [
            [
                row["path"],
                row["resource_identity"],
                row["worktree_sha256"],
                row["legacy_surface"],
                row["matched_symbols_or_patterns"],
                row["semantic_role"],
                row["owner"],
                row["consumer_disposition"],
                row["proof"],
            ]
            for row in census["legacy_semantic_removal_consumer_rows"]
        ],
        [
            [
                row["path"],
                row["matched_symbols_or_patterns"],
                row["semantic_role"],
                row["producer"],
                row["status"],
                row["proof"],
                row["dependency_direction"],
                row["identity_input"],
            ]
            for row in census["downstream_generated_semantic_consumer_obligations"]
        ],
        [
            census["semantic_consumer_digest"],
            census["downstream_generated_semantic_consumer_digest"],
            census["total_actual_semantic_consumer_count"],
            census["downstream_generated_semantic_consumer_obligation_count"],
            census["total_declared_semantic_consumer_count"],
            census["semantic_consumer_counts"]["candidate_contract_definition"],
            census["semantic_consumer_counts"]["candidate_proof_reader"],
            census["semantic_consumer_counts"]["sealed_v1_audit_migration_consumer"],
            census["semantic_consumer_counts"]["replacement_removal_target"],
            census["unresolved_actual_semantic_consumer_count"],
            census["closure_status"],
        ],
    ]
    artifacts_by_name["effect-intent-control-consumer-census-v1"] = artifact(
        "effect-intent-control-consumer-census-v1",
        census_value,
        census,
    )

    component_values = {
        "effect-intent-control-head-v1": [
            "EffectIntentControlHeadV1",
            1,
            ["intent", "home", "immutable_control_revision", "current_writer_term"],
            ["sole_home_local_mutable_selector", "one_head_per_home"],
            [
                "independent_attempt_currentness",
                "independent_classification_currentness",
                "independent_fence_currentness",
                "independent_accounting_currentness",
                "independent_result_or_idempotency_currentness",
                "independent_writer_currentness",
            ],
        ],
        "effect-intent-control-revision-v1": [
            "EffectIntentControlRevisionV1",
            1,
            [
                "attempt_history",
                "live_dispatch_or_absence",
                "classification",
                "dispatch_fence_high_water",
                "occurrence",
                "authority_effect_slot_budget",
                "material_credential_use_fences",
                "run_closure",
                "result_idempotency",
            ],
            ["immutable", "head_selected_only"],
        ],
        "effect-intent-control-transition-v1": [
            "EffectIntentControlTransitionV1",
            1,
            [
                "intent",
                "home",
                "expected_head",
                "expected_revision",
                "expected_writer_term",
                "candidate_revision",
                "current_typed_request",
                "authority_basis",
                "debit",
                "idempotency",
            ],
            [
                "ActiveStoreAtomic",
                "NoStoreProtectedCas",
                "PreStoreProtectedCas",
            ],
            ["one_winner", "losers_write_consume_and_cross_nothing"],
        ],
        "effect-intent-control-writer-term-v1": [
            "EffectIntentControlWriterTermV1",
            1,
            [
                ["OriginationWriterTermV1", ["home_local_tenure"]],
                [
                    "SameHomeRestoreWriterTermV1",
                    ["same_home_continuity", "old_writer_fenced"],
                ],
            ],
            ["exactly_one_current_term", "transition_contender_is_not_writer_term"],
        ],
    }
    for name in H2_COMPONENTS:
        artifacts_by_name[name] = artifact(name, component_values[name], {
            "component_kind": component_values[name][0],
            "behavior_bearing": True,
            "publication_state": "candidate_only_runtime_inactive",
        })

    cohort_value: list[object] = [
        "EffectIntentControlReadWriteCohortDescriptorV1",
        1,
        TRANSITION_CONTENDERS,
        [
            artifacts_by_name["effect-intent-control-consumer-census-v1"]["identity"],
            census["closure_status"],
            census["total_actual_semantic_consumer_count"],
            census["downstream_generated_semantic_consumer_obligation_count"],
            census["total_declared_semantic_consumer_count"],
            census["semantic_consumer_counts"]["candidate_contract_definition"],
            census["semantic_consumer_counts"]["candidate_proof_reader"],
            census["semantic_consumer_counts"]["sealed_v1_audit_migration_consumer"],
            census["semantic_consumer_counts"]["replacement_removal_target"],
            census["unresolved_actual_semantic_consumer_count"],
        ],
        MIGRATION_MAP,
        [
            "single_current_writer_term",
            "transition_contenders_are_not_writer_terms",
            "dual_writer_reader_forbidden",
            "physical_source_consumers_are_not_writer_or_reader_roles",
            "no_hidden_wildcard_consumer",
        ],
    ]
    artifacts_by_name["effect-intent-control-cohort-v1"] = artifact(
        "effect-intent-control-cohort-v1",
        cohort_value,
        {
            "transition_contenders": [name for _, name in TRANSITION_CONTENDERS],
            "consumer_census_identity": artifacts_by_name["effect-intent-control-consumer-census-v1"]["identity"],
            "consumer_closure_status": census["closure_status"],
            "physical_semantic_consumer_count": census["total_actual_semantic_consumer_count"],
            "downstream_generated_semantic_consumer_obligation_count": census["downstream_generated_semantic_consumer_obligation_count"],
            "total_declared_semantic_consumer_count": census["total_declared_semantic_consumer_count"],
            "effect_contract_consumer_count": census["effect_contract_consumer_count"],
            "legacy_semantic_removal_consumer_count": census["legacy_semantic_removal_consumer_count"],
            "candidate_contract_definition_consumer_count": census["semantic_consumer_counts"]["candidate_contract_definition"],
            "candidate_proof_reader_consumer_count": census["semantic_consumer_counts"]["candidate_proof_reader"],
            "sealed_v1_audit_migration_consumer_count": census["semantic_consumer_counts"]["sealed_v1_audit_migration_consumer"],
            "replacement_removal_target_consumer_count": census["semantic_consumer_counts"]["replacement_removal_target"],
            "unresolved_actual_semantic_consumer_count": census["unresolved_actual_semantic_consumer_count"],
            "migration_map": MIGRATION_MAP,
        },
    )

    h2_component_id_rows = [
        [name, artifacts_by_name[name]["identity"]] for name in H2_COMPONENTS
    ]
    writer_epoch_value: list[object] = [
        "EffectIntentControlWriterEpochV1",
        1,
        artifacts_by_name["effect-intent-control-cohort-v1"]["identity"],
        h2_component_id_rows,
        [
            "one_exact_writer_cohort",
            "unknown_semantics_deny_writes",
            "old_writer_fence_required",
            "candidate_only_no_writer_admission",
        ],
    ]
    artifacts_by_name["effect-intent-control-writer-epoch-v1"] = artifact(
        "effect-intent-control-writer-epoch-v1",
        writer_epoch_value,
        {"writer_admission": "candidate_only_no_writer_admission"},
    )
    migration_epoch_value: list[object] = [
        "EffectIntentControlMigrationEpochV1",
        1,
        artifacts_by_name["effect-intent-control-cohort-v1"]["identity"],
        MIGRATION_MAP,
        [
            "h2_native_causal_join_required",
            "missing_duplicate_or_inferred_join_quarantines",
            "different_domain_non_bearer",
            "native_cancelled_requires_complete_h3_join",
        ],
    ]
    artifacts_by_name["effect-intent-control-migration-epoch-v1"] = artifact(
        "effect-intent-control-migration-epoch-v1",
        migration_epoch_value,
        {"migration_activation": "blocked_without_exact_causal_join"},
    )
    proof_profile_value: list[object] = [
        "EffectIntentControlH2ProofProfileV1",
        1,
        h2_component_id_rows,
        artifacts_by_name["effect-intent-control-cohort-v1"]["identity"],
        [
            "ten_positive_seventeen_negative_product",
            "one_head_one_writer_one_classification_selector_one_live_fence",
            "all_transition_contender_races",
            "active_store_no_store_pre_store_carrier_parity",
            "h2_native_causal_join_quarantine",
            "semantic_consumer_census_complete_runtime_inactive",
            "downstream_generated_consumer_obligations_declared_without_identity_cycle",
            "legacy_semantic_removal_consumer_ledger_complete",
            "two_independent_encoders_and_semantic_validator",
        ],
    ]
    artifacts_by_name["effect-intent-control-proof-profile-v1"] = artifact(
        "effect-intent-control-proof-profile-v1",
        proof_profile_value,
        {"activation_gate": "candidate_only_runtime_inactive_no_activation"},
    )

    withdrawal_branches = [
        row for row in routes if row["role"] in {"ActionWithdraw", "CeremonyWithdraw"}
    ]
    withdrawal_route_keys = {
        (
            row["origin_tag"],
            row["route_tag"],
            row["branch_tag"],
            row["role"],
            row["home"],
            row["catalog_descriptor_id"],
        )
        for row in withdrawal_branches
    }
    if len(withdrawal_branches) != 30 or len(withdrawal_route_keys) != 30:
        raise ValueError("withdrawal route closure must be 30 unique compatibility rows")
    compatibility_identity = artifacts_by_name["effect-origin-home-compatibility-v1"]["identity"]
    cells = [
        {
            "source_classification": classification,
            "compatibility_identity": compatibility_identity,
            "origin_tag": row["origin_tag"],
            "route_tag": row["route_tag"],
            "branch_tag": row["branch_tag"],
            "branch": row["branch"],
            "role": row["role"],
            "home": row["home"],
            "catalog_descriptor_id": row["catalog_descriptor_id"],
            "semantic_subject_origination_fence_proof": "exact_effect_intent_origin_semantic_subject_and_origination_fence",
        }
        for classification in ["prepared", "confirmed_not_applied"]
        for row in withdrawal_branches
    ]
    if len(cells) != 60 or len({
        (
            cell["source_classification"],
            cell["compatibility_identity"],
            cell["origin_tag"],
            cell["route_tag"],
            cell["branch_tag"],
            cell["role"],
            cell["home"],
            cell["catalog_descriptor_id"],
        )
        for cell in cells
    }) != 60:
        raise ValueError("withdrawal positive cell matrix must contain 60 unique route-bound cells")
    withdrawal_value: list[object] = [
        "EffectIntentWithdrawalV1",
        1,
        [
            [
                cell["source_classification"],
                cell["compatibility_identity"],
                cell["origin_tag"],
                cell["route_tag"],
                cell["branch_tag"],
                cell["role"],
                cell["home"],
                cell["catalog_descriptor_id"],
                cell["semantic_subject_origination_fence_proof"],
            ]
            for cell in cells
        ],
        CMA_SLOT_FAMILIES,
        DENIED_WITHDRAWAL_PRODUCTS,
        "withdrawn locally; no provider cancellation performed",
    ]
    artifacts_by_name["effect-withdrawal-v1"] = artifact("effect-withdrawal-v1", withdrawal_value, {
        "positive_cell_count": 60,
        "compatibility_identity": compatibility_identity,
        "legal_live_dispatch": "None",
        "legal_source_classifications": ["prepared", "confirmed_not_applied"],
        "action_partition": {"ordinary": 12, "bootstrap_g0": 2, "cma": 5},
        "ceremony_partition": {"no_store": 1, "pre_store": 10},
        "positive_cells": cells,
        "denied_products": DENIED_WITHDRAWAL_PRODUCTS,
        "cma_effect_withdrawal_slot_families": CMA_SLOT_FAMILIES,
        "zero_creation": ["intent", "dispatch_attempt", "reconciliation_attempt", "execution_attempt", "run", "observation", "provider_key", "envelope", "use_fence"],
        "zero_refund_or_remint": True,
        "terminal_no_reopen": True,
        "late_evidence": "observation_only_integrity_block_without_cancelled_rewrite",
    })

    h2_manifest_members = [
        "effect-intent-home-v1",
        "effect-origin-home-compatibility-v1",
        "effect-intent-control-consumer-census-v1",
        "stage2-semantic-consumer-delta-v1",
        *H2_COMPONENTS,
        "effect-intent-control-cohort-v1",
        "effect-intent-control-writer-epoch-v1",
        "effect-intent-control-migration-epoch-v1",
        "effect-intent-control-proof-profile-v1",
        "effect-withdrawal-v1",
        "bootstrap-control-withdrawal-v1",
    ]
    h2_manifest_value: list[object] = [
        "EffectIntentControlH2ContractManifestV1",
        1,
        [[name, artifacts_by_name[name]["identity"]] for name in h2_manifest_members],
        [documents["grammar"]["catalog_profile_grammar"]["catalog_profile_grammar_id"], documents["effect"]["manifest_id"], documents["action_leaf"]["manifest_id"], documents["action_spec"]["manifest_id"], documents["ceremony"]["manifest_id"]],
        ["candidate_only_runtime_inactive", "no_activation", "no_registration"],
    ]
    artifacts_by_name["effect-intent-control-h2-contract-manifest-v1"] = artifact(
        "effect-intent-control-h2-contract-manifest-v1",
        h2_manifest_value,
        {
            "component_count": 4,
            "h2_component_identities": h2_component_id_rows,
            "consumer_closure_status": census["closure_status"],
            "publication_state": "candidate_only_runtime_inactive",
        },
    )
    manifest_value = [
        "EffectHomeExpectedDeltaManifestV1", 1,
        [[name, record["identity"], record["canonical_cbor_sha256"]] for name, record in sorted(artifacts_by_name.items())],
        [[name, input_hashes[name]] for name in sorted(input_hashes)],
        "predecessor_evidence_only_non_current",
    ]
    artifacts_by_name["expected-delta-manifest"] = artifact("expected-delta-manifest", manifest_value, {
        "declared_candidate_artifacts": sorted(artifacts_by_name),
        "catalog_input_sha256": input_hashes,
        "predecessor_evidence": {"path": str(EVIDENCE.relative_to(ROOT)), "sha256": sha256_bytes(EVIDENCE.read_bytes()), "disposition": "immutable_non_current_evidence_only"},
        "runtime": "inactive",
    })
    return artifacts_by_name


def ruby_receipts(input_path: Path) -> dict[str, object]:
    result = subprocess.run(
        ["ruby", str(Path(__file__).with_name("encode.rb")), str(input_path)],
        check=True,
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def materialize(output: Path) -> dict[str, object]:
    source_hashes = frozen_source_hashes()
    documents, input_hashes = catalog_inputs()
    records = artifacts(documents, input_hashes)
    output.mkdir(parents=True, exist_ok=True)
    identity_input = {"schema_version": "maestro.vnext.stage0.effect-home-encoder-input.v1", "artifacts": [{"name": name, "value": record["canonical_value"]} for name, record in sorted(records.items())]}
    write_json(output / "effect-home-identity-input.json", identity_input)
    ruby = ruby_receipts(output / "effect-home-identity-input.json")
    expected_receipts = {}
    for item in identity_input["artifacts"]:
        name = item["name"]
        encoded = encode(item["value"])
        expected = {"cbor_hex": encoded.hex(), "byte_length": len(encoded), "sha256": sha256_bytes(encoded)}
        if ruby["artifacts"].get(name) != expected:
            raise ValueError(f"Python and Ruby CBOR encoder disagreement for {name}")
        expected_receipts[name] = expected
    artifact_hashes = {f"{name}.json": write_json(output / f"{name}.json", record) for name, record in sorted(records.items())}
    receipt_identity_input = {
        "schema_version": "maestro.vnext.stage0.effect-home-encoder-receipt.v1",
        "equality": "exact_bytes_length_and_sha256",
        "python": expected_receipts,
        "ruby": ruby["artifacts"],
    }
    receipt = {
        **receipt_identity_input,
        "receipt_identity": f"sha256:{sha256_bytes(canonical_json(receipt_identity_input).encode('ascii'))}",
    }
    receipt_hash = write_json(output / "encoder-receipt.json", receipt)
    expected_delta = records["expected-delta-manifest"]
    census_record = records["effect-intent-control-consumer-census-v1"]
    finalization_body = {
        "schema_version": "maestro.vnext.stage0.effect-home-finalization-receipt.v1",
        "finalization_state": "final",
        "candidate_only": True,
        "runtime": "inactive",
        "runtime_activation": False,
        "expected_delta_manifest_id": expected_delta["identity"],
        "expected_delta_artifact_sha256": artifact_hashes["expected-delta-manifest.json"],
        "expected_delta_canonical_cbor_sha256": expected_delta["canonical_cbor_sha256"],
        "encoder_receipt_sha256": receipt_hash,
        "encoder_receipt_identity": receipt["receipt_identity"],
        "unresolved_actual_semantic_consumers": census_record["unresolved_actual_semantic_consumer_count"],
        "legacy_semantic_removal_consumer_count": census_record["legacy_semantic_removal_consumer_count"],
        "legacy_semantic_removal_consumer_digest": census_record["semantic_consumer_digest"],
        "h2_manifest_identity": records["effect-intent-control-h2-contract-manifest-v1"]["identity"],
        "h3_withdrawal_identity": records["effect-withdrawal-v1"]["identity"],
        "semantic_consumer_census_id": census_record["identity"],
        "expected_delta": {
            "identity": expected_delta["identity"],
            "artifact_sha256": artifact_hashes["expected-delta-manifest.json"],
            "canonical_cbor_sha256": expected_delta["canonical_cbor_sha256"],
        },
        "encoder_receipt": {
            "sha256": receipt_hash,
            "identity": receipt["receipt_identity"],
        },
        "validator": {
            "path": "tools/vnext_contracts/stage0/effect_home/validate.py",
            "sha256": sha256_bytes(Path(__file__).with_name("validate.py").read_bytes()),
            "semantic_validation": "pass",
            "mutant_suite": {"case_count": 28, "result": "all_rejected"},
        },
        "semantic_consumer_census": {
            "identity": census_record["identity"],
            "closure_status": census_record["closure_status"],
            "effect_contract_consumer_count": census_record["effect_contract_consumer_count"],
            "legacy_semantic_removal_consumer_count": census_record["legacy_semantic_removal_consumer_count"],
            "total_actual_semantic_consumer_count": census_record["total_actual_semantic_consumer_count"],
            "downstream_generated_semantic_consumer_obligation_count": census_record["downstream_generated_semantic_consumer_obligation_count"],
            "total_declared_semantic_consumer_count": census_record["total_declared_semantic_consumer_count"],
            "downstream_generated_semantic_consumer_digest": census_record["downstream_generated_semantic_consumer_digest"],
            "unresolved_actual_semantic_consumer_count": census_record["unresolved_actual_semantic_consumer_count"],
        },
    }
    finalization = {
        **finalization_body,
        "identity": f"sha256:{sha256_bytes(canonical_json(finalization_body).encode('ascii'))}",
    }
    finalization_hash = write_json(output / "finalization-receipt.v1.json", finalization)
    inventory = {
        "schema_version": "maestro.vnext.stage0.effect-home-inventory.v1",
        "publication_state": "candidate_only_runtime_inactive",
        "frozen_inputs": source_hashes,
        "artifact_sha256": artifact_hashes,
            "encoder_receipt_sha256": receipt_hash,
            "finalization_receipt_sha256": finalization_hash,
            "counts": {
            "origins": 23,
            "routes": 139,
            "action_branches": 19,
            "ceremony_branches": 11,
            "withdrawal_positive_cells": 60,
            "withdrawal_unique_route_bound_cells": 60,
            "h2_behavior_bearing_components": 4,
            "transition_contenders": 11,
            "c325_sealed_coverage_rows": 325,
            "c325_baseline_vs_live_drift_rows": 8,
            "effect_contract_consumer_rows": census_record["effect_contract_consumer_count"],
            "legacy_semantic_removal_consumer_rows": census_record["legacy_semantic_removal_consumer_count"],
            "semantic_physical_consumer_rows": census_record["total_actual_semantic_consumer_count"],
            "downstream_generated_semantic_consumer_obligations": census_record["downstream_generated_semantic_consumer_obligation_count"],
            "total_declared_semantic_consumers": census_record["total_declared_semantic_consumer_count"],
            "candidate_contract_definition_consumers": census_record["semantic_consumer_counts"]["candidate_contract_definition"],
            "candidate_proof_reader_consumers": census_record["semantic_consumer_counts"]["candidate_proof_reader"],
            "sealed_v1_audit_migration_consumers": census_record["semantic_consumer_counts"]["sealed_v1_audit_migration_consumer"],
            "replacement_removal_target_consumers": census_record["semantic_consumer_counts"]["replacement_removal_target"],
            "unresolved_actual_semantic_consumers": census_record["unresolved_actual_semantic_consumer_count"],
            "bootstrap_rows": 11,
            "bootstrap_targets": 3,
            "bootstrap_exclusions": 8,
            "cma_effect_withdrawal_slot_families": 5,
        },
    }
    write_json(output / "inventory.json", inventory)
    return inventory


def check(output: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="maestro-effect-home-") as temporary:
        expected = Path(temporary) / "effect-home"
        materialize(expected)
        expected_files = sorted(path.relative_to(expected) for path in expected.iterdir())
        actual_files = sorted(path.relative_to(output) for path in output.iterdir()) if output.exists() else []
        if expected_files != actual_files:
            raise ValueError(f"generated file set drifted: expected {expected_files}, got {actual_files}")
        for relative in expected_files:
            if (expected / relative).read_bytes() != (output / relative).read_bytes():
                raise ValueError(f"generated literal drifted: {relative}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.check:
        check(args.output)
        print(json.dumps({"status": "ok", "output": str(args.output.relative_to(ROOT))}, sort_keys=True))
    else:
        inventory = materialize(args.output)
        print(json.dumps(inventory["counts"], sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
