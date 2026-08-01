#!/usr/bin/env python3
"""Independent semantic validator and mutation suite for Effect Home artifacts."""

from __future__ import annotations

import argparse
import ast
import copy
import hashlib
import json
import shutil
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
DEFAULT = ROOT / "contracts/vnext/stage0/effect-home"
C325 = ROOT / "contracts/vnext/public/direct_consumers.c325.v1.json"
DOMAIN = "maestro.vnext.stage0.effect-home.v1"
ROLES = [
    "ActionReserve",
    "ActionRecoverReserved",
    "ActionOutcome",
    "ActionReconcile",
    "ActionWithdraw",
    "CeremonyInitiate",
    "CeremonyRecoverReserved",
    "CeremonyResolveResult",
    "CeremonyWithdraw",
]
CONTENDERS = [
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
    "EffectIntent", "EffectOrigin", "DispatchAttempt", "ReconciliationAttempt",
    "RemoteClassification", "EffectWithdrawal", "WithdrawEffectIntent", "RecoverReserved",
    "ControlHead", "ControlRevision", "WriterTerm",
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
    "RepositoryGovernanceFloorSnapshotV1",
    "maestro.vnext.repository-governance-floor-snapshot.v1",
    "maestro.vnext.repository-governance-head-class-8.v1",
]
STAGE2_SEMANTIC_SOURCE_DECLARATIONS = {
    "src/domain/authority/action_basis.rs": ("Authority", "candidate_contract_definition", "exact_stage4_execution_basis_partition"),
    "src/domain/authority/bootstrap_catalog.rs": ("Authority", "candidate_contract_definition", "exact_stage2_bootstrap_target_literal"),
    "src/domain/authority/capacity.rs": ("Authority", "candidate_contract_definition", "exact_stage2_capacity_literal"),
    "src/domain/authority/closed.rs": ("Authority", "candidate_contract_definition", "exact_stage2_closed_sum_literal"),
    "src/domain/authority/continuity/catalog.rs": ("Authority", "candidate_contract_definition", "exact_stage2_continuity_effect_intent_class_literal"),
    "src/domain/authority/continuity/totality.rs": ("Authority", "candidate_contract_definition", "exact_stage2_continuity_owner_census_literal"),
    "src/domain/authority/governance_floor.rs": ("Authority", "candidate_contract_definition", "exact_internal_append_only_authority_schema_tag_25"),
    "src/domain/authority/mod.rs": ("Authority", "candidate_contract_definition", "exact_stage2_authority_facade_literal"),
    "src/domain/authority/publication.rs": ("Authority", "candidate_contract_definition", "exact_internal_authority_schema_registry_prefix_and_tag_25"),
    "src/domain/authority/facade/repository_admission.rs": ("Authority", "candidate_contract_definition", "exact_stage4_execution_authority_admission"),
    "src/domain/authority/facade/repository_leaf_authority.rs": ("Authority", "candidate_contract_definition", "exact_stage4_execution_authority_closed_union"),
    "src/domain/authority/transition.rs": ("Authority", "candidate_contract_definition", "exact_stage2_transition_guard_literal"),
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
    "contracts/vnext/catalogs/evidence/e346-nominal-source.json": ("V1AuditMigrationEvidence", "sealed_v1_audit_migration_consumer", "sealed_v1_e346_nominal_source_evidence"),
    "contracts/vnext/catalogs/evidence/e346-semantic-baseline.json": ("V1AuditMigrationEvidence", "sealed_v1_audit_migration_consumer", "sealed_v1_e346_semantic_baseline_evidence"),
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
    "src/domain/execution/control_head.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/execution/ceremony.rs": ("Execution", "candidate_contract_definition", "direct_stage4_protected_ceremony_literal"),
    "src/domain/execution/dispatch_state.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/execution/effect_home.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/execution/effect_routes.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/execution/effects.rs": ("Execution", "candidate_contract_definition", "direct_stage4_effect_runtime_literal"),
    "src/domain/execution/h3_withdrawal_publication.rs": ("Execution", "candidate_contract_definition", "direct_stage4_h3_withdrawal_publication_literal"),
    "src/domain/execution/mod.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/execution/runtime.rs": ("Execution", "candidate_contract_definition", "direct_stage4_execution_runtime_literal"),
    "src/domain/execution/store.rs": ("Execution", "candidate_contract_definition", "direct_stage4_atomic_store_literal"),
    "src/domain/execution/withdrawal.rs": ("Execution", "candidate_contract_definition", "direct_execution_literal"),
    "src/domain/evidence/observation.rs": ("Evidence", "candidate_contract_definition", "direct_stage5_observation_literal"),
    "src/domain/evidence/store.rs": ("Evidence", "candidate_contract_definition", "direct_stage5_evidence_store_literal"),
    "src/domain/distribution/runtime/model.rs": ("Distribution", "candidate_contract_definition", "direct_stage9_distribution_model_literal"),
    "src/domain/distribution/runtime/records.rs": ("Distribution", "candidate_contract_definition", "direct_stage9_distribution_record_literal"),
    "src/domain/distribution/runtime/transaction.rs": ("Distribution", "candidate_contract_definition", "direct_stage9_distribution_transaction_literal"),
    "src/domain/identity/manifest.rs": ("Identity", "candidate_contract_definition", "direct_identity_literal"),
    "src/domain/integration/public_literals.rs": ("PublicContracts", "candidate_contract_definition", "direct_public_contract_literal"),
    "src/domain/migration/runtime/classification.rs": ("Migration", "candidate_contract_definition", "direct_stage11_migration_classification_literal"),
    "src/domain/persistence/protected_locator_stage9_seed.rs": ("Persistence", "candidate_contract_definition", "direct_stage9_protected_locator_literal"),
    "src/domain/transport/json.rs": ("Transport", "candidate_contract_definition", "direct_stage6_transport_literal"),
    "src/operations/action/service.rs": ("Action", "candidate_contract_definition", "direct_stage6_action_service_literal"),
    "src/operations/installation/agent_resource_release.rs": ("Installation", "candidate_contract_definition", "direct_stage9_agent_resource_release_literal"),
    "src/operations/installation/effects.rs": ("Installation", "candidate_contract_definition", "direct_stage9_installation_effect_literal"),
    "src/operations/migration/tests.rs": ("Migration", "candidate_proof_reader", "direct_stage11_migration_runtime_proof"),
    "src/operations/repository.rs": ("Repository", "candidate_contract_definition", "direct_stage8_repository_literal"),
    "tests/vnext_dispatch_cutover_literals.rs": ("Stage0Proof", "candidate_proof_reader", "direct_stage0_literal_test"),
    "tests/vnext_effect_home_literals.rs": ("Stage0Proof", "candidate_proof_reader", "direct_stage0_literal_test"),
    "tests/vnext_stage11_migration_contracts.rs": ("Stage11Proof", "candidate_proof_reader", "direct_stage11_migration_contract_proof"),
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
    "tools/vnext_contracts/stage4/execution/build.py": ("Stage4Execution", "candidate_contract_definition", "direct_stage4_execution_builder_literal"),
    "tools/vnext_contracts/stage4/execution/validate.py": ("Stage4Proof", "candidate_proof_reader", "independent_stage4_execution_reconstruction"),
    "tools/vnext_contracts/stage4/execution/verify.rb": ("Stage4Proof", "candidate_proof_reader", "independent_stage4_execution_ruby_reconstruction"),
}
SEMANTIC_ROLE_SOURCES = {
    "tools/vnext_contracts/stage0/effect_home/encode.rb": (
        "Stage0Proof", "candidate_proof_reader",
        "independent_cbor_receipt_encoder_for_effect_home_identity_input",
        "independent_cbor_encoder_source",
    ),
    "tools/vnext_contracts/stage0/resource_release/validate.py": (
        "Stage0Proof", "candidate_proof_reader",
        "direct_resource_release_effect_home_reader",
        "function_scoped_resource_release_effect_binding_validation",
    ),
    "tools/vnext_contracts/stage0/proof_matrix/build.py": (
        "Stage0Proof", "candidate_proof_reader",
        "stage0_proof_manifest_effect_home_reader",
        "function_scoped_stage0_proof_manifest_effect_binding",
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
    "tools/vnext_contracts/stage0/proof_matrix/build.py": (
        "build_manifest",
        ('effect_inventory = load("contracts/vnext/stage0/effect-home/inventory.json")',),
    ),
}
SEMANTIC_ROLE_AST_CALLERS = {
    "tools/vnext_contracts/stage0/resource_release/validate.py": (
        "validate_all",
        "validate_resource_release(documents, inventory, resources, bundles, census, release)",
    ),
    "tools/vnext_contracts/stage0/proof_matrix/build.py": (
        "execute",
        "document, encoded = build_manifest(check=check)",
    ),
}
SEMANTIC_ROLE_SOURCE_SHA256 = {
    "tools/vnext_contracts/stage0/resource_release/validate.py": "f02dc1f50903c54c6b30de3c86c56b576e7880f2a986e3f3f1a5a102fc2e8349",
    "tools/vnext_contracts/stage0/proof_matrix/build.py": "c997a02cfe10880bddeac64a45e89cee48c59c367524976d7ebfa7e32c77cc88",
}
DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS = {
    "contracts/vnext/stage4/execution/execution-effects.v1.cbor": (
        ["EffectIntent", "DispatchAttempt", "ReconciliationAttempt", "RecoverReserved", "ControlHead"],
        "Stage4Execution", "pending_downstream_generated_binding", "resolved_by_stage4_execution_manifest",
    ),
    "contracts/vnext/stage4/execution/execution-effects.v1.json": (
        ["EffectIntent", "DispatchAttempt", "ReconciliationAttempt", "RecoverReserved", "ControlHead"],
        "Stage4Execution", "pending_downstream_generated_binding", "resolved_by_stage4_execution_manifest",
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
        ["EffectIntent"], "Stage2Authority", "pending_downstream_generated_binding", "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/authority-continuity-manifest.v1.json": (
        ["EffectIntent"], "Stage2Authority", "pending_downstream_generated_binding", "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/authority-literals.v1.cbor": (
        ["EffectIntent"], "Stage2Authority", "pending_downstream_generated_binding", "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/authority-literals.v1.json": (
        ["EffectIntent"], "Stage2Authority", "pending_downstream_generated_binding", "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.cbor": (
        ["EffectOrigin"], "Stage2Authority", "pending_downstream_generated_binding", "resolved_by_stage2_authority_root_manifest",
    ),
    "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json": (
        ["EffectOrigin"], "Stage2Authority", "pending_downstream_generated_binding", "resolved_by_stage2_authority_root_manifest",
    ),
}
LEGACY_SEMANTIC_REMOVAL_SOURCES = {
    "src/domain/channel.rs": ("delivery_release", "Channel", "replacement_removal_target", "append_delivery_receipt", "legacy_delivery_receipt_and_latest_cursor_writer", "direct_v1_delivery_receipt_and_latest_join"),
    "src/interfaces/cli/msg.rs": ("connector_adapter", "CliMessage", "replacement_removal_target", "send_codex_thread_primary", "legacy_codex_connector_delivery_adapter", "direct_v1_connector_delivery_adapter"),
    "tests/msg_codex_delivery_integration.rs": ("delivery_release", "Stage0Proof", "candidate_proof_reader", "codex_to_codex_msg_send_uses_target_thread_without_unread_local_duplicate", "legacy_codex_delivery_contract_test", "direct_v1_delivery_integration_proof"),
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
    ("src/domain/loop_recipes.rs", "753c9f535ebf219cb60998cb19aa2876b8dfad9134eedab4a878a478a549ea91", "776ba071adaafd17a3eaa45ddee4ec55c652f82bcf83650c4640ed87c2f487e9", "loop_recipes_domain_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("src/domain/mod.rs", "70a6dbbc96645090e77750f12879db081e3b2f4647e04c9785e13617c1f55ce7", "8ecc94ec3520e1b00fc76cd453ace95645af36453fc2888a23948dba49ab2930", "domain_module_facade_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("src/interfaces/cli/loop_recipes.rs", "babfbd7ef04b869e9a86d0022c312b10653b5df5f1be5d7e647d967a0d44e947", "f8c3fcd5d01aaa9a590e4c93703928e018531dd5d1665b385411ae8fe95e91c7", "loop_recipes_cli_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("src/interfaces/cli/mod.rs", "7c43e73ff25ae8c12d378b0a9ead453f7ad89452848b59a9190a852a87f1c7f8", "ddb86f6af0a20bbda80fb03824f3c18ed8ca45c62e688a6cd89099d4654ae000", "cli_module_facade_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("src/interfaces/cli/status.rs", "d560602a5ac888e5ef6f256f487056712536f7915e18f855389b1db2e2695a35", "4f62a633c8398d404605eed8647e15c815fdf5b5ab2ea81ea188f0716c9601f1", "status_cli_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("tests/loop_recipes_integration.rs", "85cc2627b8db9616bdd20976204c660737d726858f546f62fd20a6ea26f1c61c", "5fc7a9808427daf0caea1316c6069d7d31528c0c19c50dc0d6836b5c777fc447", "loop_recipes_test_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("tests/resource_contracts.rs", "1c553df182bbe8a6f5a22ef1c838cb8fd12ee2ddd118cfda61f4f7b145ec4878", "81e07503d036606d3fbeb6514bb475e4ad894336515a1d868b35bd11278b4275", "resource_contract_test_surface_changed_after_c325_baseline_not_effect_semantic_evidence"),
    ("tests/resources_version_guard.rs", "8176e638c10301d1fd5ede30ab006096307895ef229fe3cd197a15eab8c1c0ba", "bdc15354e5b32c85f6db2a7a781559de9a55ff53f0fb3510be9a68bc31c60f4e", "resource_version_guard_changed_after_c325_baseline_not_effect_semantic_evidence"),
]
DENIED = {
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
}


def read(output: Path, name: str) -> dict[str, object]:
    return json.loads((output / f"{name}.json").read_text(encoding="ascii"))


def sha_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha(path: Path) -> str:
    return sha_bytes(path.read_bytes())


def fail(message: str) -> None:
    raise ValueError(message)


def ast_binds_name(node: ast.AST, name: str) -> bool:
    if isinstance(node, ast.Name):
        return node.id == name and isinstance(node.ctx, (ast.Store, ast.Del))
    if isinstance(node, ast.arg):
        return node.arg == name
    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
        return node.name == name
    if isinstance(node, ast.ExceptHandler):
        return node.name == name
    match_binding_nodes = tuple(
        node_type
        for name in ("MatchAs", "MatchStar")
        if (node_type := getattr(ast, name, None)) is not None
    )
    if match_binding_nodes and isinstance(node, match_binding_nodes):
        return node.name == name
    match_mapping_node = getattr(ast, "MatchMapping", None)
    if match_mapping_node is not None and isinstance(node, match_mapping_node):
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
    if sha_bytes(raw_source) != SEMANTIC_ROLE_SOURCE_SHA256[path]:
        fail(f"semantic role source bytes drifted from the executable binding: {path}")
    source = raw_source.decode("utf-8")
    tree = ast.parse(source, filename=path)
    functions = [
        node
        for node in tree.body
        if isinstance(node, ast.FunctionDef)
        and node.name == function_name
    ]
    if len(functions) != 1:
        fail(f"semantic role source must contain one top-level {function_name}: {path}")
    function = functions[0]
    if function.decorator_list or any(
        isinstance(node, (ast.Yield, ast.YieldFrom)) for node in ast.walk(function)
    ):
        fail(f"semantic role source target is not a plain executable function: {path}")
    if any(
        (node is not function and ast_binds_name(node, function_name))
        or has_dynamic_namespace_mutation(node)
        for node in ast.walk(function)
    ):
        fail(f"semantic role source target can rebind its exact target: {path}")
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
        fail(f"semantic role source lost its exact function-scoped proof: {path}")
    if has_static_infinite_loop(body[: matches[0]]) or any(
        isinstance(node, (ast.Return, ast.Raise))
        for statement in body[: matches[0]]
        for node in ast.walk(statement)
    ):
        fail(f"semantic role source made its exact proof unreachable: {path}")

    caller_name, caller_statement = SEMANTIC_ROLE_AST_CALLERS[path]
    callers = [
        node
        for node in tree.body
        if isinstance(node, ast.FunctionDef)
        and node.name == caller_name
    ]
    if len(callers) != 1:
        fail(f"semantic role source must contain one top-level {caller_name}: {path}")
    caller = callers[0]
    if caller.decorator_list or any(
        isinstance(node, (ast.Yield, ast.YieldFrom)) for node in ast.walk(caller)
    ):
        fail(f"semantic role caller is not a plain executable function: {path}")
    if not caller.body or not isinstance(caller.body[0], ast.Try):
        fail(f"semantic role caller must begin with its direct try body: {path}")
    caller_body = caller.body[0].body
    expected_call = ast.dump(ast.parse(caller_statement).body[0], include_attributes=False)
    call_matches = [
        index
        for index, statement in enumerate(caller_body)
        if ast.dump(statement, include_attributes=False) == expected_call
    ]
    if len(call_matches) != 1:
        fail(f"semantic role source lost its exact reachable caller: {path}")
    if has_static_infinite_loop(caller_body[: call_matches[0]]) or any(
        isinstance(node, (ast.Return, ast.Raise))
        for statement in caller_body[: call_matches[0]]
        for node in ast.walk(statement)
    ):
        fail(f"semantic role source made its exact caller unreachable: {path}")
    if any(
        ast_binds_name(node, function_name) or has_dynamic_namespace_mutation(node)
        for node in ast.walk(caller)
    ):
        fail(f"semantic role caller shadows its exact target: {path}")
    target_index = tree.body.index(function)
    caller_index = tree.body.index(caller)
    if target_index >= caller_index or any(
        ast_binds_name(node, function_name) or has_dynamic_namespace_mutation(node)
        for statement in tree.body[target_index + 1 :]
        for node in ast.walk(statement)
    ):
        fail(f"semantic role source rebinds its exact target: {path}")


def cbor_head(major: int, value: int) -> bytes:
    if not isinstance(value, int) or not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        fail("canonical CBOR requires unsigned u64")
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
    if value is False:
        return b"\xf4"
    if value is True:
        return b"\xf5"
    if isinstance(value, int):
        return cbor_head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return cbor_head(3, len(raw)) + raw
    if isinstance(value, list):
        return cbor_head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        raw = bytes.fromhex(str(value["bytes"]))
        return cbor_head(2, len(raw)) + raw
    fail("value outside deterministic CBOR subset")


def validate_artifact_identity(document: dict[str, object]) -> None:
    value = document["canonical_value"]
    cbor = encode(value)
    if document["canonical_cbor_hex"] != cbor.hex():
        fail(f"canonical CBOR drifted: {document['artifact']}")
    if document["canonical_cbor_sha256"] != sha_bytes(cbor):
        fail(f"canonical CBOR digest drifted: {document['artifact']}")
    if document["identity"] != f"sha256:{sha_bytes(encode([DOMAIN, value]))}":
        fail(f"content identity drifted: {document['artifact']}")
    if document["publication_state"] != "candidate_only_runtime_inactive":
        fail(f"artifact activated: {document['artifact']}")


def c325_rows() -> list[dict[str, object]]:
    ledger = json.loads(C325.read_text(encoding="ascii"))
    if not (
        ledger["schema"] == "maestro.vnext.direct-consumer-evidence-ledger.v1"
        and ledger["expected_count"] == 325
        and ledger["expected_digest"] == "9aee8ea371f770e8694131079d4bfb4845f849d59d0b545005a2f0371a42976a"
        and ledger["current_source_equality_claimed"] is False
    ):
        fail("pinned C325 evidence drifted")
    return ledger["rows"]


def stage2_semantic_consumer_rows() -> list[list[object]]:
    rows: list[list[object]] = []
    for path, (owner, disposition, proof) in sorted(
        STAGE2_SEMANTIC_SOURCE_DECLARATIONS.items()
    ):
        source = ROOT / path
        if not source.is_file():
            fail(f"declared Stage 2 semantic consumer is missing: {path}")
        contents = source.read_text(encoding="utf-8", errors="ignore")
        matched = [
            literal for literal in STAGE2_SEMANTIC_LITERAL_PATTERNS if literal in contents
        ]
        if not matched:
            fail(f"declared Stage 2 semantic consumer has no exact literal: {path}")
        worktree_sha256 = sha(source)
        rows.append(
            [
                path,
                f"sha256:{worktree_sha256}",
                worktree_sha256,
                matched,
                "stage2_semantic_consumer_delta",
                owner,
                disposition,
                proof,
            ]
        )
    return rows


def validate_stage2_semantic_delta(document: dict[str, object]) -> None:
    rows = stage2_semantic_consumer_rows()
    canonical_rows = [
        [row[0], row[1], row[2], row[3], row[5], row[6], row[7]] for row in rows
    ]
    rows_digest = sha_bytes(
        "".join(
            f"{row[0]}  {row[1]}  {','.join(row[3])}  {row[4]}  {row[5]}  {row[6]}\n"
            for row in canonical_rows
        ).encode("ascii")
    )
    expected_value = [
        "Stage2SemanticConsumerDeltaV1",
        1,
        STAGE2_PREDECESSOR_CONSUMER_CENSUS_ID,
        STAGE2_PREDECESSOR_CANDIDATE_ROOT_ID,
        canonical_rows,
        [len(canonical_rows), rows_digest, "complete_exact_source_overlay"],
        "candidate_only_runtime_inactive",
    ]
    if document["canonical_value"] != expected_value:
        fail("Stage 2 semantic consumer delta canonical value drifted")
    if document.get("schema_version") != "maestro.vnext.stage2.semantic-consumer-delta.v1":
        fail("Stage 2 semantic consumer delta schema drifted")
    if document.get("predecessor") != {
        "consumer_census_id": STAGE2_PREDECESSOR_CONSUMER_CENSUS_ID,
        "candidate_contract_root_id": STAGE2_PREDECESSOR_CANDIDATE_ROOT_ID,
    }:
        fail("Stage 2 semantic consumer delta predecessor binding drifted")
    expected_body_rows = [
        {
            "path": row[0],
            "resource_identity": row[1],
            "worktree_sha256": row[2],
            "matched_literals": row[3],
            "owner": row[5],
            "consumer_disposition": row[6],
            "proof": row[7],
        }
        for row in rows
    ]
    if document.get("consumer_rows") != expected_body_rows:
        fail("Stage 2 semantic consumer delta rows drifted")
    if document.get("consumer_count") != len(rows):
        fail("Stage 2 semantic consumer delta count drifted")
    if document.get("consumer_digest") != rows_digest:
        fail("Stage 2 semantic consumer delta digest drifted")
    if document.get("closure_status") != "complete_exact_source_overlay":
        fail("Stage 2 semantic consumer delta closure drifted")


def expected_consumer_census() -> tuple[
    list[list[object]],
    list[list[object]],
    list[list[object]],
    list[list[object]],
    list[list[object]],
    str,
    str,
    dict[str, int],
]:
    source_rows = c325_rows()
    source_hashes = {str(row["path"]): str(row["sha256"]) for row in source_rows}
    if len(source_hashes) != 325:
        fail("C325 source path uniqueness drifted")
    baseline_drifts = []
    for path, pinned_sha256, baseline_worktree_sha256, explanation in C325_BASELINE_DRIFTS:
        if source_hashes.get(path) != pinned_sha256:
            fail(f"C325 baseline drift pin changed: {path}")
        baseline_drifts.append([path, pinned_sha256, baseline_worktree_sha256, explanation])
    if [row[0] for row in baseline_drifts] != sorted(row[0] for row in baseline_drifts):
        fail("C325 baseline drift record order changed")

    literal_hits: dict[str, list[str]] = {}
    for scope in (ROOT / "src", ROOT / "contracts/vnext", ROOT / "tools", ROOT / "tests"):
        for path in scope.rglob("*"):
            if not path.is_file() or path.is_relative_to(DEFAULT) or path.suffix == ".pyc":
                continue
            contents = path.read_text(encoding="utf-8", errors="ignore")
            matched = [pattern for pattern in SEMANTIC_LITERAL_PATTERNS if pattern in contents]
            if matched:
                literal_hits[str(path.relative_to(ROOT))] = matched
    for path, (expected_patterns, _, _, _) in DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS.items():
        observed_patterns = literal_hits.pop(path, [])
        if observed_patterns is not None and observed_patterns != expected_patterns:
            fail(f"downstream generated semantic obligation changed: {path}")
    stage2_rows = stage2_semantic_consumer_rows()
    for row in stage2_rows:
        path = str(row[0])
        observed_patterns = literal_hits.pop(path, [])
        expected_patterns = [
            pattern for pattern in row[3] if pattern in SEMANTIC_LITERAL_PATTERNS
        ]
        if observed_patterns != expected_patterns:
            fail(f"Stage 2 semantic overlay does not match the Stage 0 census: {path}")
    if set(literal_hits) != set(SEMANTIC_LITERAL_SOURCES):
        fail("semantic literal source closure changed")
    if set(literal_hits) & set(source_hashes):
        fail("sealed C325 coverage rows became semantic sources")

    effect_rows: list[list[object]] = []
    for path in sorted(literal_hits):
        owner, disposition, proof = SEMANTIC_LITERAL_SOURCES[path]
        worktree_sha256 = sha(ROOT / path)
        effect_rows.append([
            path,
            f"sha256:{worktree_sha256}",
            worktree_sha256,
            literal_hits[path],
            "direct_effect_contract_literal",
            owner,
            disposition,
            proof,
        ])
    for path, (owner, disposition, semantic_role, proof) in SEMANTIC_ROLE_SOURCES.items():
        validate_semantic_role_source(path)
        worktree_sha256 = sha(ROOT / path)
        effect_rows.append([
            path,
            f"sha256:{worktree_sha256}",
            worktree_sha256,
            [],
            semantic_role,
            owner,
            disposition,
            proof,
        ])
    effect_rows.extend(stage2_rows)
    effect_rows.sort(key=lambda row: str(row[0]))
    if len({row[0] for row in effect_rows}) != len(effect_rows):
        fail("semantic consumer source duplication")
    downstream_rows = [
        [
            path,
            expected_patterns,
            "downstream_generated_semantic_consumer",
            producer,
            status,
            proof,
            "ResourceRelease consumes EffectHome",
            False,
        ]
        for path, (expected_patterns, producer, status, proof) in sorted(
            DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS.items()
        )
    ]
    legacy_rows: list[list[object]] = []
    for path, (surface, owner, disposition, evidence_pattern, semantic_role, proof) in sorted(LEGACY_SEMANTIC_REMOVAL_SOURCES.items()):
        source = ROOT / path
        if not source.is_file() or evidence_pattern not in source.read_text(encoding="utf-8", errors="ignore"):
            fail(f"legacy semantic removal direct evidence drifted: {path}")
        worktree_sha256 = sha(source)
        legacy_rows.append([
            path,
            f"sha256:{worktree_sha256}",
            worktree_sha256,
            surface,
            [evidence_pattern],
            semantic_role,
            owner,
            disposition,
            proof,
        ])
    if set(row[0] for row in effect_rows) & set(row[0] for row in legacy_rows):
        fail("effect and legacy semantic source ledgers overlap")
    rows = [*effect_rows, *legacy_rows]
    counts = {
        disposition: sum((row[6] if len(row) == 8 else row[7]) == disposition for row in rows)
        for disposition in [
            "candidate_contract_definition",
            "candidate_proof_reader",
            "sealed_v1_audit_migration_consumer",
            "replacement_removal_target",
        ]
    }
    if sum(counts.values()) != len(rows) or any(
        (row[6] if len(row) == 8 else row[7]) not in counts for row in rows
    ):
        fail("unresolved semantic consumer disposition")
    digest = sha_bytes(
        "".join(
            f"{row[0]}  {row[1]}  {'effect_contract' if len(row) == 8 else row[3]}  {','.join(row[3] if len(row) == 8 else row[4])}  {row[4] if len(row) == 8 else row[5]}  {row[5] if len(row) == 8 else row[6]}  {row[6] if len(row) == 8 else row[7]}  {row[7] if len(row) == 8 else row[8]}\n"
            for row in rows
        ).encode("ascii")
    )
    downstream_digest = sha_bytes(
        "".join(
            f"{row[0]}  {','.join(row[1])}  {row[2]}  {row[3]}  {row[4]}  {row[5]}  {row[6]}\n"
            for row in downstream_rows
        ).encode("ascii")
    )
    coverage = [
        "sealed_c325_coverage_universe",
        sha(C325),
        325,
        "9aee8ea371f770e8694131079d4bfb4845f849d59d0b545005a2f0371a42976a",
        ["contracts/vnext/public/historical_source_coverage_inputs.v1.json", "40b1d37adc0119dfabd461ee7f74ad257a5bd0207a1c41dcceeea979c95a76b0"],
        "sealed_non_promoting_historical_coverage_not_semantic_consumer_set",
    ]
    return (
        coverage,
        baseline_drifts,
        effect_rows,
        legacy_rows,
        downstream_rows,
        digest,
        downstream_digest,
        counts,
    )


def validate_consumer_census(census: dict[str, object]) -> None:
    (
        coverage,
        baseline_drifts,
        expected_rows,
        expected_legacy_rows,
        expected_downstream_rows,
        expected_digest,
        expected_downstream_digest,
        counts,
    ) = expected_consumer_census()
    value = census["canonical_value"]
    if value[0:2] != ["EffectIntentControlConsumerCensusV1", 4]:
        fail("consumer census type drifted")
    if value[2] != coverage:
        fail("consumer census C325 binding drifted")
    if value[3] != baseline_drifts or len(value[3]) != 8:
        fail("consumer census baseline drift snapshot is not exact")
    if value[4] != expected_rows:
        fail("consumer census semantic source rows are not canonical")
    if value[5] != expected_legacy_rows:
        fail("legacy semantic removal source rows are not canonical")
    if value[6] != expected_downstream_rows:
        fail("downstream generated semantic obligations are not canonical")
    if value[7] != [
        expected_digest,
        expected_downstream_digest,
        len(expected_rows) + len(expected_legacy_rows),
        len(expected_downstream_rows),
        len(expected_rows) + len(expected_legacy_rows) + len(expected_downstream_rows),
        counts["candidate_contract_definition"],
        counts["candidate_proof_reader"],
        counts["sealed_v1_audit_migration_consumer"],
        counts["replacement_removal_target"],
        0,
        "complete_upstream_sources_plus_declared_downstream_generated_obligations_runtime_inactive",
    ]:
        fail("consumer census semantic summary drifted")
    body_rows = census["semantic_consumer_rows"]
    if len(body_rows) != len(expected_rows):
        fail("consumer census body count drifted")
    required_keys = {
        "path",
        "resource_identity",
        "worktree_sha256",
        "matched_symbols_or_patterns",
        "semantic_role",
        "owner",
        "consumer_disposition",
        "proof",
    }
    if any(set(row) != required_keys for row in body_rows):
        fail("consumer census admits a category-only or wildcard row")
    if [
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
        for row in body_rows
    ] != expected_rows:
        fail("consumer census body is not canonical")
    legacy_body_rows = census["legacy_semantic_removal_consumer_rows"]
    legacy_required_keys = required_keys | {"legacy_surface"}
    if any(set(row) != legacy_required_keys for row in legacy_body_rows):
        fail("legacy semantic removal ledger admits a category-only or wildcard row")
    if [
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
        for row in legacy_body_rows
    ] != expected_legacy_rows:
        fail("legacy semantic removal ledger body is not canonical")
    downstream_body_rows = census["downstream_generated_semantic_consumer_obligations"]
    downstream_required_keys = {
        "path",
        "matched_symbols_or_patterns",
        "semantic_role",
        "producer",
        "status",
        "proof",
        "dependency_direction",
        "identity_input",
    }
    if any(set(row) != downstream_required_keys for row in downstream_body_rows):
        fail("downstream generated obligation admits an undeclared field")
    if [
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
        for row in downstream_body_rows
    ] != expected_downstream_rows:
        fail("downstream generated obligation body is not canonical")
    if census["closure_status"] != value[7][10]:
        fail("consumer census closure status drifted")
    if census["unresolved_actual_semantic_consumer_count"] != 0:
        fail("consumer census unresolved semantic consumer count drifted")
    if census["semantic_consumer_counts"] != counts:
        fail("consumer census disposition counts drifted")
    if census["effect_contract_consumer_count"] != len(expected_rows):
        fail("effect contract consumer count drifted")
    if census["legacy_semantic_removal_consumer_count"] != len(expected_legacy_rows):
        fail("legacy semantic removal consumer count drifted")
    if census["total_actual_semantic_consumer_count"] != len(expected_rows) + len(expected_legacy_rows):
        fail("total actual semantic consumer count drifted")
    if census["downstream_generated_semantic_consumer_obligation_count"] != len(expected_downstream_rows):
        fail("downstream generated obligation count drifted")
    if census["total_declared_semantic_consumer_count"] != len(expected_rows) + len(expected_legacy_rows) + len(expected_downstream_rows):
        fail("total declared semantic consumer count drifted")
    if census["downstream_generated_semantic_consumer_digest"] != expected_downstream_digest:
        fail("downstream generated semantic consumer digest drifted")


def validate_h2_components(documents: dict[str, dict[str, object]]) -> None:
    expected_kinds = [
        "EffectIntentControlHeadV1",
        "EffectIntentControlRevisionV1",
        "EffectIntentControlTransitionV1",
        "EffectIntentControlWriterTermV1",
    ]
    if len(H2_COMPONENTS) != len(expected_kinds):
        fail("H2 component and kind cardinality drifted")
    for name, kind in zip(H2_COMPONENTS, expected_kinds):
        component = documents[name]
        if component["canonical_value"][0:2] != [kind, 1]:
            fail(f"H2 component type drifted: {name}")
        if component["component_kind"] != kind or component["behavior_bearing"] is not True:
            fail(f"H2 component body drifted: {name}")
    writer_term = documents["effect-intent-control-writer-term-v1"]["canonical_value"]
    if writer_term[2] != [
        ["OriginationWriterTermV1", ["home_local_tenure"]],
        ["SameHomeRestoreWriterTermV1", ["same_home_continuity", "old_writer_fenced"]],
    ]:
        fail("writer term closure drifted")


def validate_cohort(documents: dict[str, dict[str, object]]) -> None:
    cohort = documents["effect-intent-control-cohort-v1"]
    census = documents["effect-intent-control-consumer-census-v1"]
    value = cohort["canonical_value"]
    if value[0:2] != ["EffectIntentControlReadWriteCohortDescriptorV1", 1]:
        fail("cohort descriptor type drifted")
    if value[2] != CONTENDERS or len(value[2]) != 11:
        fail("transition contender closure drifted")
    summary = census["canonical_value"][7]
    if value[3] != [
        census["identity"],
        "complete_upstream_sources_plus_declared_downstream_generated_obligations_runtime_inactive",
        summary[2],
        summary[3],
        summary[4],
        summary[5],
        summary[6],
        summary[7],
        summary[8],
        0,
    ]:
        fail("cohort consumer census binding drifted")
    if value[4] != MIGRATION_MAP:
        fail("cohort migration map is not canonical")
    if value[5] != [
        "single_current_writer_term",
        "transition_contenders_are_not_writer_terms",
        "dual_writer_reader_forbidden",
        "physical_source_consumers_are_not_writer_or_reader_roles",
        "no_hidden_wildcard_consumer",
    ]:
        fail("cohort control separation drifted")
    if cohort["legacy_semantic_removal_consumer_count"] != census["legacy_semantic_removal_consumer_count"]:
        fail("cohort legacy semantic removal count drifted")
    if cohort["transition_contenders"] != [name for _, name in CONTENDERS]:
        fail("cohort body drifted")
    if cohort["migration_map"] != MIGRATION_MAP:
        fail("cohort body migration map drifted")
    if cohort["consumer_census_identity"] != census["identity"]:
        fail("cohort body census identity drifted")
    if cohort["unresolved_actual_semantic_consumer_count"] != 0:
        fail("cohort may not declare unresolved actual semantic consumers")


def validate_withdrawal(documents: dict[str, dict[str, object]]) -> None:
    compat = documents["effect-origin-home-compatibility-v1"]
    withdrawal = documents["effect-withdrawal-v1"]
    routes = compat["routes"]
    compat_value = compat["canonical_value"]
    expected_compat_rows = [
        [row["origin_tag"], row["route_tag"], row["role"], row["home"], row["branch_tag"], row["catalog_descriptor_id"]]
        for row in routes
    ]
    if compat_value[0:2] != ["EffectOriginHomeCompatibilityV1", 1] or compat_value[2] != expected_compat_rows:
        fail("compatibility rows are not canonical")
    if len(routes) != 139 or compat["formula"] != "19x5+11x4":
        fail("route totality drifted")
    if len({row["origin"] for row in routes}) != 23:
        fail("origin totality drifted")
    if any(row["role"] not in ROLES for row in routes):
        fail("unknown route role admitted")
    withdrawal_routes = [row for row in routes if row["role"] in {"ActionWithdraw", "CeremonyWithdraw"}]
    expected_cells = [
        {
            "source_classification": classification,
            "compatibility_identity": compat["identity"],
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
        for row in withdrawal_routes
    ]
    cells = withdrawal["positive_cells"]
    if len(withdrawal_routes) != 30 or len(cells) != 60 or cells != expected_cells:
        fail("withdrawal cells are not the complete exact compatibility projection")
    keys = {
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
    }
    if len(keys) != 60:
        fail("withdrawal cells contain a duplicate origin or route product")
    expected_value_cells = [
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
    ]
    value = withdrawal["canonical_value"]
    if value[0:2] != ["EffectIntentWithdrawalV1", 1] or value[2] != expected_value_cells:
        fail("withdrawal proof rows are not canonical")
    if withdrawal["compatibility_identity"] != compat["identity"]:
        fail("withdrawal does not bind the compatibility identity")
    if set(withdrawal["denied_products"]) != DENIED:
        fail("withdrawal denial product suite drifted")
    if withdrawal["cma_effect_withdrawal_slot_families"] != [
        "MaintenanceExecutorCurrentness",
        "ProspectiveContinuityCarrier",
        "PlannedTurnoverHighWater",
        "RepositoryRecoveryAdmission",
        "InstallationRecoveryAdmission",
    ]:
        fail("CMA withdrawal purpose closure drifted")
    if withdrawal["zero_refund_or_remint"] is not True or withdrawal["terminal_no_reopen"] is not True:
        fail("withdrawal conservation or terminality drifted")
    if withdrawal["late_evidence"] != "observation_only_integrity_block_without_cancelled_rewrite":
        fail("late Evidence non-rewrite law drifted")


def validate_manifest(documents: dict[str, dict[str, object]]) -> None:
    h2 = documents["effect-intent-control-h2-contract-manifest-v1"]
    expected_members = [
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
    if h2["canonical_value"][0:2] != ["EffectIntentControlH2ContractManifestV1", 1]:
        fail("H2 manifest type drifted")
    if h2["canonical_value"][2] != [[name, documents[name]["identity"]] for name in expected_members]:
        fail("H2 manifest member closure drifted")
    if h2["component_count"] != 4 or h2["h2_component_identities"] != [[name, documents[name]["identity"]] for name in H2_COMPONENTS]:
        fail("H2 manifest component binding drifted")
    if h2["consumer_closure_status"] != "complete_upstream_sources_plus_declared_downstream_generated_obligations_runtime_inactive":
        fail("H2 manifest consumer closure status drifted")
    proof = documents["effect-intent-control-proof-profile-v1"]
    if "semantic_consumer_census_complete_runtime_inactive" not in proof["canonical_value"][4]:
        fail("H2 proof profile does not preserve the inactive semantic census gate")
    if "downstream_generated_consumer_obligations_declared_without_identity_cycle" not in proof["canonical_value"][4]:
        fail("H2 proof profile does not preserve the acyclic downstream obligation gate")
    if "legacy_semantic_removal_consumer_ledger_complete" not in proof["canonical_value"][4]:
        fail("H2 proof profile does not close the legacy semantic removal ledger")
    if proof["activation_gate"] != "candidate_only_runtime_inactive_no_activation":
        fail("H2 proof profile activation gate drifted")
    delta = documents["expected-delta-manifest"]
    declared = sorted(name for name in documents if name != "expected-delta-manifest")
    if delta["declared_candidate_artifacts"] != declared:
        fail("expected delta artifact closure drifted")
    if delta["canonical_value"][2] != [
        [name, documents[name]["identity"], documents[name]["canonical_cbor_sha256"]]
        for name in declared
    ]:
        fail("expected delta canonical identity closure drifted")


def canonical_json(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def validate_finalization(
    output: Path,
    documents: dict[str, dict[str, object]],
    inventory: dict[str, object],
    receipt: dict[str, object],
) -> None:
    finalization = json.loads((output / "finalization-receipt.v1.json").read_text(encoding="ascii"))
    required = {
        "schema_version",
        "identity",
        "finalization_state",
        "candidate_only",
        "runtime",
        "runtime_activation",
        "expected_delta_manifest_id",
        "expected_delta_artifact_sha256",
        "expected_delta_canonical_cbor_sha256",
        "encoder_receipt_sha256",
        "encoder_receipt_identity",
        "unresolved_actual_semantic_consumers",
        "legacy_semantic_removal_consumer_count",
        "legacy_semantic_removal_consumer_digest",
        "h2_manifest_identity",
        "h3_withdrawal_identity",
        "semantic_consumer_census_id",
        "expected_delta",
        "encoder_receipt",
        "validator",
        "semantic_consumer_census",
    }
    if set(finalization) != required:
        fail("finalization receipt field closure drifted")
    body = {key: value for key, value in finalization.items() if key != "identity"}
    if finalization["identity"] != f"sha256:{sha_bytes(canonical_json(body).encode('ascii'))}":
        fail("finalization receipt identity drifted")
    delta = documents["expected-delta-manifest"]
    census = documents["effect-intent-control-consumer-census-v1"]
    if not (
        finalization["schema_version"] == "maestro.vnext.stage0.effect-home-finalization-receipt.v1"
        and finalization["finalization_state"] == "final"
        and finalization["candidate_only"] is True
        and finalization["runtime"] == "inactive"
        and finalization["runtime_activation"] is False
        and finalization["expected_delta_manifest_id"] == delta["identity"]
        and finalization["expected_delta_artifact_sha256"] == sha(output / "expected-delta-manifest.json")
        and finalization["expected_delta_canonical_cbor_sha256"] == delta["canonical_cbor_sha256"]
        and finalization["encoder_receipt_sha256"] == sha(output / "encoder-receipt.json")
        and finalization["encoder_receipt_identity"] == receipt["receipt_identity"]
        and finalization["unresolved_actual_semantic_consumers"] == 0
        and finalization["legacy_semantic_removal_consumer_count"] == census["legacy_semantic_removal_consumer_count"]
        and finalization["legacy_semantic_removal_consumer_digest"] == census["semantic_consumer_digest"]
        and finalization["h2_manifest_identity"] == documents["effect-intent-control-h2-contract-manifest-v1"]["identity"]
        and finalization["h3_withdrawal_identity"] == documents["effect-withdrawal-v1"]["identity"]
        and finalization["semantic_consumer_census_id"] == census["identity"]
    ):
        fail("finalization receipt required fields drifted")
    if finalization["expected_delta"] != {
        "identity": delta["identity"],
        "artifact_sha256": sha(output / "expected-delta-manifest.json"),
        "canonical_cbor_sha256": delta["canonical_cbor_sha256"],
    }:
        fail("finalization receipt expected delta binding drifted")
    receipt_identity_input = {
        "schema_version": receipt["schema_version"],
        "equality": receipt["equality"],
        "python": receipt["python"],
        "ruby": receipt["ruby"],
    }
    if receipt.get("receipt_identity") != f"sha256:{sha_bytes(canonical_json(receipt_identity_input).encode('ascii'))}":
        fail("encoder receipt identity drifted")
    if finalization["encoder_receipt"] != {
        "sha256": sha(output / "encoder-receipt.json"),
        "identity": receipt["receipt_identity"],
    }:
        fail("finalization receipt encoder binding drifted")
    if finalization["validator"] != {
        "path": "tools/vnext_contracts/stage0/effect_home/validate.py",
        "sha256": sha(Path(__file__)),
        "semantic_validation": "pass",
        "mutant_suite": {"case_count": 28, "result": "all_rejected"},
    }:
        fail("finalization receipt validator binding drifted")
    if finalization["semantic_consumer_census"] != {
        "identity": census["identity"],
        "closure_status": "complete_upstream_sources_plus_declared_downstream_generated_obligations_runtime_inactive",
        "effect_contract_consumer_count": census["effect_contract_consumer_count"],
        "legacy_semantic_removal_consumer_count": census["legacy_semantic_removal_consumer_count"],
        "total_actual_semantic_consumer_count": census["total_actual_semantic_consumer_count"],
        "downstream_generated_semantic_consumer_obligation_count": census["downstream_generated_semantic_consumer_obligation_count"],
        "total_declared_semantic_consumer_count": census["total_declared_semantic_consumer_count"],
        "downstream_generated_semantic_consumer_digest": census["downstream_generated_semantic_consumer_digest"],
        "unresolved_actual_semantic_consumer_count": 0,
    }:
        fail("finalization receipt semantic consumer binding drifted")
    if inventory.get("finalization_receipt_sha256") != sha(output / "finalization-receipt.v1.json"):
        fail("inventory finalization receipt binding drifted")


def validate(output: Path) -> None:
    names = [
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
        "effect-intent-control-h2-contract-manifest-v1",
        "expected-delta-manifest",
    ]
    documents = {name: read(output, name) for name in names}
    for document in documents.values():
        validate_artifact_identity(document)
    home = documents["effect-intent-home-v1"]
    if {"writer_cohort", "read_cohort", "migration_map"} & set(home):
        fail("material cohort or migration law may not live as EffectIntentHome metadata")
    if set(home["closed_union"]) != {"ActiveStoreHomeV1", "NoStoreCeremonyHomeV1", "PreStoreCeremonyHomeV1"}:
        fail("EffectIntentHomeV1 union drifted")
    if home["no_store_forbidden_fields"] != ["domain_id", "generation", "epoch"]:
        fail("NoStore Home may not fabricate DomainId, Generation, or Epoch")
    if home["origination_fence"] != "immutable" or home["use_fence"] != "fresh_same_home_per_later_dispatch_or_action_reconciliation":
        fail("fence mutability law drifted")
    if set(home["reconciliation_refusal"]) != {"NoStoreCeremonyHomeV1", "PreStoreCeremonyHomeV1"}:
        fail("Ceremony reconciliation refusal drifted")
    if home["resolve_result_effects"] != {"creates_attempt": False, "creates_intent": False, "creates_run": False, "performs_io": False}:
        fail("ResolveResult must create no Intent, Attempt, Run, or I/O")
    validate_consumer_census(documents["effect-intent-control-consumer-census-v1"])
    validate_stage2_semantic_delta(documents["stage2-semantic-consumer-delta-v1"])
    validate_h2_components(documents)
    validate_cohort(documents)
    validate_withdrawal(documents)
    validate_manifest(documents)
    bootstrap = documents["bootstrap-control-withdrawal-v1"]
    if bootstrap["row_count"] != 11 or bootstrap["target_count"] != 3 or bootstrap["hard_exclusion_count"] != 8:
        fail("Bootstrap target census drifted")
    if bootstrap["seventh_interaction_hard_exclusion"] != "WithdrawBootstrapMandateInteractionEffect":
        fail("Bootstrap seventh interaction exclusion drifted")
    inventory = read(output, "inventory")
    receipt = read(output, "encoder-receipt")
    for name, digest in inventory["artifact_sha256"].items():
        if sha(output / name) != digest:
            fail(f"artifact hash drifted: {name}")
    if receipt["equality"] != "exact_bytes_length_and_sha256" or receipt["python"] != receipt["ruby"]:
        fail("independent encoder receipt drifted")
    if set(receipt["python"]) != set(names):
        fail("encoder receipt artifact closure drifted")
    validate_finalization(output, documents, inventory, receipt)


def must_fail(output: Path, mutate) -> None:
    originals = {path.name: path.read_bytes() for path in output.glob("*.json")}
    try:
        mutate()
        try:
            validate(output)
        except ValueError:
            return
        raise AssertionError("semantic mutant was accepted")
    finally:
        for name, data in originals.items():
            (output / name).write_bytes(data)


def mutants(output: Path) -> int:
    with tempfile.TemporaryDirectory(prefix="maestro-effect-home-mutants-") as temporary:
        isolated = Path(temporary) / "effect-home"
        shutil.copytree(output, isolated)
        cases = [
            lambda: mutate_json(isolated, "effect-intent-home-v1", lambda doc: doc.__setitem__("writer_cohort", ["metadata_only"])),
            lambda: mutate_json(isolated, "effect-intent-control-cohort-v1", lambda doc: doc["transition_contenders"].pop(8)),
            lambda: mutate_json(isolated, "effect-intent-control-consumer-census-v1", lambda doc: doc["semantic_consumer_rows"][0].update({"path": "adapter", "consumer_category": "adapter"})),
            lambda: mutate_json(isolated, "effect-intent-control-consumer-census-v1", lambda doc: doc.__setitem__("unresolved_actual_semantic_consumer_count", 1)),
            lambda: mutate_json(isolated, "effect-intent-control-consumer-census-v1", lambda doc: doc["legacy_semantic_removal_consumer_rows"][0].__setitem__("legacy_surface", "wildcard")),
            lambda: mutate_json(isolated, "effect-intent-control-consumer-census-v1", lambda doc: doc["downstream_generated_semantic_consumer_obligations"][0].__setitem__("path", "contracts/vnext/stage0/resource-release/unlisted.json")),
            lambda: mutate_json(isolated, "effect-intent-control-consumer-census-v1", lambda doc: doc["downstream_generated_semantic_consumer_obligations"][0].__setitem__("status", "resolved_without_downstream_proof")),
            lambda: mutate_json(isolated, "effect-intent-control-consumer-census-v1", lambda doc: doc["downstream_generated_semantic_consumer_obligations"][0].__setitem__("identity_input", True)),
            lambda: mutate_json(isolated, "stage2-semantic-consumer-delta-v1", lambda doc: doc["consumer_rows"].pop()),
            lambda: mutate_json(isolated, "stage2-semantic-consumer-delta-v1", lambda doc: doc["consumer_rows"][0].__setitem__("worktree_sha256", "0" * 64)),
            lambda: mutate_json(isolated, "effect-withdrawal-v1", lambda doc: doc["positive_cells"].__setitem__(1, copy.deepcopy(doc["positive_cells"][0]))),
            lambda: mutate_json(isolated, "effect-withdrawal-v1", lambda doc: doc["positive_cells"][0].__setitem__("origin_tag", 999)),
            lambda: mutate_json(isolated, "effect-withdrawal-v1", lambda doc: doc["positive_cells"][0].__setitem__("route_tag", 999)),
            lambda: mutate_json(isolated, "effect-intent-control-h2-contract-manifest-v1", lambda doc: doc["canonical_value"].__setitem__(1, 2)),
            lambda: mutate_json(isolated, "expected-delta-manifest", lambda doc: doc.__setitem__("runtime", "active")),
            lambda: mutate_json(isolated, "encoder-receipt", lambda doc: doc.__setitem__("equality", "length_only")),
        ]
        for case in cases:
            must_fail(isolated, case)
    return len(cases) + semantic_source_mutants()


def semantic_source_mutants() -> int:
    path = "tools/vnext_contracts/stage0/resource_release/validate.py"
    source = (ROOT / path).read_text(encoding="utf-8")
    call = "        validate_resource_release(documents, inventory, resources, bundles, census, release)"
    effect = '    effect_path = "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"'
    caller = "\ndef validate_all("
    mutations = [
        source.replace(call, "        from builtins import print as validate_resource_release\n" + call, 1),
        source.replace(caller, "\nclass validate_resource_release:\n    pass\n\n\ndef validate_all(", 1),
        source.replace(caller, "\nasync def validate_resource_release():\n    pass\n\n\ndef validate_all(", 1),
        source.replace(effect, "    while True:\n        pass\n" + effect, 1),
        source.replace(call, "        while True:\n            pass\n" + call, 1),
        source.replace(call, "        try:\n            pass\n        except Exception as validate_resource_release:\n            pass\n" + call, 1),
        source.replace(effect, "    while 1 == 1:\n        pass\n" + effect, 1),
        source.replace(call, "        while not False:\n            pass\n" + call, 1),
        source.replace(call, "        match lambda *args: None:\n            case validate_resource_release:\n                pass\n" + call, 1),
        source.replace(call, "        globals()[\"validate_resource_release\"] = lambda *args: None\n" + call, 1),
        source.replace(call, "        globals().update(validate_resource_release=lambda *args: None)\n" + call, 1),
        source.replace(call, "        exec(\"validate_resource_release = lambda *args: None\", globals())\n" + call, 1),
    ]
    if len(set(mutations)) != len(mutations) or any(mutant == source for mutant in mutations):
        raise AssertionError("semantic source mutant construction drifted")
    for mutant in mutations:
        try:
            validate_semantic_role_source(path, mutant)
        except ValueError:
            continue
        raise AssertionError("semantic source mutant was accepted")
    return len(mutations)


def mutate_json(output: Path, name: str, mutate) -> None:
    path = output / f"{name}.json"
    document = json.loads(path.read_text(encoding="ascii"))
    mutate(document)
    path.write_text(json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n", encoding="ascii")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=DEFAULT)
    parser.add_argument("--mutants", action="store_true")
    args = parser.parse_args()
    validate(args.output)
    count = mutants(args.output) if args.mutants else 0
    print(json.dumps({"status": "ok", "mutants": count}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
