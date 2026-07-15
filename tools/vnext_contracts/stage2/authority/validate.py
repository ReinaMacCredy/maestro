#!/usr/bin/env python3
"""Independently reconstruct and validate every Stage 2 Authority projection."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[3]
DEFAULT_ROOT = WORKSPACE / "contracts/vnext/stage2/authority"
STAGE2_DELTA_PATH = WORKSPACE / "contracts/vnext/stage0/effect-home/stage2-semantic-consumer-delta-v1.json"
PUBLICATION_STATE = "inactive_candidate"
PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID = "bf2075863bfa3ec7e5269560464182264e78fbeec6dff8197d5dae7bf278a0b4"
PREDECESSOR_CATALOG_IDS = [
    [1, "ObservationKindV1", "9997ffa7f1597c07abd23a3955a634cb321670198e040fb7b5d015b021159688"],
    [2, "EffectOriginV1", "d28f8e573ddb450c427e628df121dbd516d0e5b05c03caf18d2757782dfd259d"],
    [3, "RepositoryGovernedCapacitySlotKindV1", "a1dd1dc6210548029d3e1f2f8697c80a8c1dd9a7c80cf4144f062a95cd54806e"],
    [4, "InstallationGovernedCapacitySlotKindV1", "bcccb791b3b136c9cd328a2677f4fe00826e162cda2e9fb7768c49d919aca2a1"],
    [5, "CeremonySpecV1", "fb9ba972eb2fe8f6861e71cd6c2c6af23a9fdb75986ffbed8c0a2ce319288485"],
    [6, "ActionLeafCensusV1", "b2f538d76795db0338448cc8cb837419157c1bebdc8bcc7d7b42fd961790d454"],
    [7, "RepositoryAuthorityContinuityClassV1", "c67e88788f8d6636277ae15ba87102ec8fa7ede146256ca99ffc36a356267c1e"],
    [8, "InstallationAuthorityContinuityClassV1", "387bf53ada1fc03bbf8004f1eec366267d7881a81f401d9806994df8b9862795"],
    [9, "ActionSpecV1", "7a7a1311690fcdec194c0d9c9afbbe4e28f1332b567ed23451daf06ebca02970"],
]
AUTHORITY_CONTEXTS = ["RepositoryAuthorityContext", "InstallationAuthorityContext"]
ACTION_AUTHORITY_BASES = ["OrdinaryLiveRuntime", "BootstrapControlG0", "ContinuityMaintenance"]
ACTION_RESULT_OUTCOMES = ["committed", "no_op", "rejected", "stale", "conflict", "unavailable", "in_doubt"]
RESPONSE_ORIGINS = ["fresh", "replay"]
REPOSITORY_CAPACITY_KINDS = [
    "RepositoryOrdinaryMutation", "RepositoryAuthorityAdministration", "RepositoryEvidenceAcquisition",
    "RepositoryPlanningPublication", "RepositoryExternalEffect", "RepositoryPersistenceMaintenance",
]
INSTALLATION_CAPACITY_KINDS = [
    "InstallationAuthorityAdministration", "InstallationDistributionMutation",
    "InstallationGovernedReviewPublication", "InstallationExternalEffect",
    "InstallationWriterAdministration", "InstallationPersistenceMaintenance",
]
CMA_OBSERVATION_PUBLICATION_PURPOSES = [
    "TrustedTimeAcquisition", "RecoveryExternalRegistration", "RecoveryExternalStatus",
    "MaintenanceExecutorCurrentness", "ProspectiveContinuityCarrier",
]
CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES = [
    "MaintenanceExecutorCurrentness", "ProspectiveContinuityCarrier", "PlannedTurnoverHighWater",
    "RepositoryRecoveryAdmission", "InstallationRecoveryAdmission",
]
TRANSITION_GUARD_KINDS = [
    "RepositoryWorkAuthorityPolicyTransition", "RepositoryFirstWorkPublication",
    "RepositoryFloorOrTrustRootRotation", "InstallationPolicyBindingReplacement",
    "InstallationStructuralRootFloorReplacement", "TrustedTimePolicyStackRotation",
    "ExternalLogicalCarrierProfileRotation", "PlannedEpochTurnoverPreparation",
]
REPOSITORY_CONTINUITY_CLASSES = [
    "RepositoryOrdinaryMutationCapacityState", "RepositoryAuthorityAdministrationCapacityState",
    "RepositoryEvidenceAcquisitionCapacityState", "RepositoryPlanningPublicationCapacityState",
    "RepositoryExternalEffectCapacityState", "RepositoryPersistenceMaintenanceCapacityState",
    "RepositoryStoreGenerationCurrentness", "RepositoryGovernanceHead", "RepositoryAuthorityEpochState",
    "RepositoryTrustRootState", "RepositoryPrincipalBindingState", "RepositorySessionState",
    "RepositoryGrantState", "RepositoryDelegationState", "RepositoryMandateState",
    "RepositoryRevocationState", "RepositoryAuthorizationReceiptState", "RepositoryConsumptionCellState",
    "RepositoryContinuityState", "RepositoryTrustedTimeState", "RepositoryRecoveryCommitmentState",
    "RepositoryRecoveryAdmissionState", "RepositoryStepExecutionState", "RepositoryEffectIntentState",
    "RepositoryEvidenceState", "RepositoryGateSnapshot", "RepositoryPlanningState",
    "RepositoryCoordinationState", "RepositoryDesignDecisionState", "RepositoryContractState",
    "RepositoryWorkState", "RepositoryPersistenceRetentionState", "RepositoryMemoryState",
    "RepositoryIntakeState", "RepositoryResearchState",
]
INSTALLATION_CONTINUITY_CLASSES = [
    "InstallationAuthorityAdministrationCapacityState", "InstallationDistributionMutationCapacityState",
    "InstallationGovernedReviewPublicationCapacityState", "InstallationExternalEffectCapacityState",
    "InstallationWriterAdministrationCapacityState", "InstallationPersistenceMaintenanceCapacityState",
    "InstallationLocatorCurrentness", "InstallationStoreGenerationCurrentness", "InstallationGovernanceHead",
    "InstallationAuthorityEpochState", "InstallationTrustRootState", "InstallationPrincipalBindingState",
    "InstallationGrantState", "InstallationMandateState", "InstallationRevocationState",
    "InstallationAuthorizationReceiptState", "InstallationConsumptionCellState", "InstallationContinuityState",
    "InstallationRecoveryCommitmentState", "InstallationRecoveryAdmissionState",
    "InstallationWriterCohortState", "InstallationClientCompatibilityState",
    "InstallationDistributionTargetState", "InstallationDistributionTransactionState",
    "InstallationBinarySlotState", "InstallationResourceManifestState",
    "InstallationGovernedReviewPublicationState", "InstallationEffectIntentState",
    "InstallationEvidenceState", "InstallationPersistenceRetentionState",
]
BOOTSTRAP_TARGET_ROWS = [
    [1, "EnrollRecoveryCommitmentSelection", "admitted", "recovery_commitment_selection"],
    [2, "RotateRecoveryCommitmentSelection", "admitted", "recovery_commitment_selection"],
    [3, "RevokeRecoveryCommitmentSelection", "admitted", "recovery_commitment_selection"],
    [4, "FirstHumanBindingEnrollment", "excluded", "recursive_first_human_enrollment"],
    [5, "ReserveBootstrapMandateInteractionEffect", "excluded", "bootstrap_interaction_protocol"],
    [6, "PublishBootstrapMandateInteractionOutcome", "excluded", "bootstrap_interaction_protocol"],
    [7, "PublishBootstrapMandatePresentationObservation", "excluded", "bootstrap_interaction_protocol"],
    [8, "PublishBootstrapMandateResponseObservation", "excluded", "bootstrap_interaction_protocol"],
    [9, "ReconcileBootstrapMandateInteractionEffect", "excluded", "bootstrap_interaction_protocol"],
    [10, "IssueBootstrapMandate", "excluded", "self_authorizing_issuance"],
    [11, "WithdrawBootstrapMandateInteractionEffect", "excluded", "bootstrap_interaction_protocol"],
]
PRODUCED_RECORD_CLOSURE = [
    "AuthorityMandateOrConvergenceRefV1", "BootstrapMandateIssuanceBindingV1:exactly_one_if_newly_minted",
    "AuthorizationReceiptV1:primary", "ActionResultV1", "IdempotencyRecordV1",
    "AuthorityContinuityClosureV1:pre_cut_exact",
    "SuccessVisibleAuthorityContinuityStateV1:exactly_one",
    "AdmittedTransitionGuardV1:persisted_from_serialization_current_owner_facts",
    "BootstrapAuthoritySnapshotV1:successor_current_authority_carrier",
    "LinearizationCoverageWitnessV1:recoverable",
    "AuthorityContinuityPostCutConsequenceSetV1:complete_exact",
]
PRODUCED_SCHEMA_NAMES = [
    "AuthorityMandateV1", "BootstrapMandateIssuanceBindingV1", "AuthorizationReceiptV1",
    "ActionResultV1", "BootstrapAuthoritySnapshotV1", "AuthorityContinuityClosureV1", "SuccessVisibleAuthorityContinuityStateV1", "AdmittedTransitionGuardV1",
    "LinearizationCoverageWitnessV1", "AuthorityContinuityPostCutConsequenceSetV1",
]
SCHEMA_DEFINITIONS = [
    ("AuthorityMandateV1", ["authority_context_ref", "action", "subject", "action_revision", "consent_slot_binding_parameter", "responder_binding_ref", "responder_assurance_revision", "interaction_closure_ref", "authority_basis_commitment_ref", "valid_from_inclusive", "valid_until_exclusive", "maximum_uses", "delegation_depth_remaining"], [], ["maximum_uses_exactly_one", "delegation_forbidden", "same_context_only", "immutable"]),
    ("BootstrapMandateIssuanceBindingV1", ["mandate_ref", "target_action_commitment", "consent_slot_commitment", "interaction_closure_ref"], [], ["exists_exactly_once_only_for_new_mandate", "absent_for_convergence", "immutable"]),
    ("AuthorizationReceiptV1", ["receipt_id", "authority_context_ref", "action_request_ref", "authority_basis_ref", "authorization_decision", "primary", "action_result_ref"], ["authorized", "denied"], ["primary_exactly_one", "non_bearer", "retrospective_only", "immutable"]),
    ("ActionResultV1", ["result_id", "action_request_ref", "outcome", "response_origin", "before_refs", "after_refs", "receipt_refs", "produced_refs", "effect_attempt_refs", "safe_reason_code", "next_or_inspect_ref"], ACTION_RESULT_OUTCOMES, ["response_origin_not_outcome", "replay_returns_original_result", "immutable"]),
    ("IssueBootstrapMandateRequestV1", ["request_id", "authority_context_ref", "actor_binding_ref", "actor_session_ref", "responder_binding_ref", "presentation_observation_ref", "affirmative_response_observation_ref", "target", "target_subject", "target_revision", "consent_slot_binding_parameter", "idempotency_key", "supplied_mandates"], [], ["supplied_mandates_exactly_zero", "self_reference_free", "no_external_io"]),
    ("ConsentSlotBindingParameterV1", ["slot_protocol_commitment", "target_action_commitment", "slot_commitment"], [], ["fixed_before_issuance", "self_reference_free", "non_authorizing"]),
    ("ActionAuthorityBasisV1", ["basis_tag", "authority_context_ref", "basis_commitment"], ACTION_AUTHORITY_BASES, ["leaf_selects_exactly_one", "no_ranking", "no_fallback", "no_cross_donation"]),
    ("AuthorityContextV1", ["context_tag", "context_id", "stable_domain_identity", "protected_realm_if_installation", "store_generation", "authority_epoch", "trust_root_revision", "locator_binding_revision_if_installation"], AUTHORITY_CONTEXTS, ["exactly_one_domain", "no_cross_store_authority", "unknown_variant_refused"]),
    ("GovernedCapacityDebitV1", ["capacity_root_ref", "authority_context_kind", "authority_context_ref", "capacity_kind", "ordinal", "prior_spent", "resulting_spent"], REPOSITORY_CAPACITY_KINDS + INSTALLATION_CAPACITY_KINDS, ["quantity_exactly_one", "fresh_committed_only", "replay_zero_debit", "same_domain_only"]),
    ("AuthorityContinuityManifestV1", ["authority_context_kind", "protocol_version", "canonicalization_version", "obligations", "dispositions", "owner_contributions", "class_ids", "class_descriptors"], AUTHORITY_CONTEXTS, ["closed_class_set", "old_client_refusal", "candidate_only", "immutable"]),
    ("PrincipalBindingV1", ["binding_id", "principal_id", "authority_context_ref", "trust_root_revision", "assurance_revision", "validity", "human_capable"], [], ["nonzero_protocol_revisions", "finite_half_open_validity", "same_context_only", "immutable"]),
    ("SessionV1", ["session_id", "principal_binding_ref", "authority_context_ref", "store_generation", "authority_epoch", "request_commitment", "validity"], [], ["nonempty_bounded_ascii_commitment", "binding_context_generation_epoch_exact", "immutable"]),
    ("BootstrapGenesisGrantV1", ["grant_id", "authority_context_ref", "grantee_principal_ref", "authority_epoch", "trust_root_revision", "local_capacity_constraint", "terminal_scope", "delegable_scope", "valid_from_inclusive", "valid_until_exclusive"], ["NoLocalBoundedRoot"], ["exactly_one_structural_g0", "terminal_scope_bootstrap_control", "delegable_scope_ordinary_bounded", "terminal_and_delegable_scopes_disjoint", "bootstrap_control_nondelegable", "inert_before_context_genesis_activation"]),
    ("BootstrapMandateInteractionObservationJoinV1", ["interaction_closure_id", "authority_context_ref", "responder_binding_ref", "responder_current_authentication_ref", "presentation_observation_ref", "affirmative_response_observation_ref", "carrier_procedure_ref", "target_action_commitment"], [], ["presentation_and_affirmative_response_both_required", "same_authenticated_responder", "same_context_and_target", "observational_non_authorizing", "immutable"]),
    ("RevocationSetV1", ["authority_context_ref", "revocation_targets"], ["TrustRoot", "PrincipalBinding", "Session", "Grant", "Mandate"], ["finite_bounded_set", "same_context_only", "revoked_authority_never_revives", "immutable"]),
    ("BootstrapAuthoritySnapshotV1", ["authority_context", "authority_snapshot", "actor_binding", "actor_session", "responder_binding", "responder_session", "bootstrap_g0_candidate_paths", "revocations", "interaction_join", "current_carrier_procedure_ref", "target_action_projection", "current_target_head", "consent_slot_evaluation_facts", "continuity_transition_proof"], [], ["complete_store_loaded_evaluator_facts", "exact_current_generation", "fresh_authorization_only", "ambient_time_forbidden", "non_bearer"]),
    ("GovernedCapacityRootV1", ["capacity_root_id", "authority_context_kind", "authority_context_ref", "capacity_kind", "initial_maximum", "spent"], REPOSITORY_CAPACITY_KINDS + INSTALLATION_CAPACITY_KINDS, ["nonzero_bounded_initial_maximum", "spent_not_above_initial_maximum", "same_domain_only", "immutable_revision"]),
    ("SuccessVisibleAuthorityContinuityStateV1", ["protocol_version", "state_token_ref", "predecessor_state_token_ref", "authority_context_kind", "authority_context_ref", "store_generation", "store_publication_clock", "authority_epoch", "manifest_id", "closure_id", "store_allocation_commitment_ref", "guard_kind", "carrier_profile_status", "selected_trusted_time_stack_ref", "accepted_authority_time_floor", "lane_state_closure_root", "source_floor_root", "gap_companion_refs", "floor_provenance_refs", "external_revision_cell_refs", "cma_remaining_root", "cma_spent_root", "unresolved_effect_refs", "cut_sequence", "guard_admission_digest"], [], ["exactly_one_per_success", "sole_current_continuity_relation", "no_component_heads", "same_store_visible"]),
    ("AdmittedTransitionGuardV1", ["protocol_version", "guard_kind", "authority_context_kind", "authority_context_ref", "store_generation", "authority_epoch", "manifest_id", "closure_id", "predecessor_state_token_ref", "cut_sequence", "selected_trusted_time_stack_ref", "carrier_profile_status", "accepted_authority_time_floor", "lane_state_closure_root", "source_floor_root", "gap_companion_refs", "floor_provenance_refs", "external_revision_cell_refs", "cma_remaining_root", "cma_spent_root", "unresolved_effect_refs", "owner_term_facts", "owner_census_commitment", "owner_census_source_cut_commitment", "disclosure"], TRANSITION_GUARD_KINDS, ["fixed_nominal_owner_terms", "owner_facts_current_at_serialization", "bounded_fail_closed", "persisted_non_bearer", "same_store_visible"]),
    ("LinearizationCoverageWitnessV1", ["fence_subject_ref", "fence_carrier", "attempt_ref", "semantic_point_ref", "covered_closure_ref", "conservative_point_envelope_ref", "carrier_revision_ref"], ["SameStoreCommit", "ProtectedLocatorCAS", "ProtectedRepositoryGenerationCAS", "ProtectedSnapshot"], ["recoverable_exact_attempt_coverage", "non_aba", "missing_witness_blocks_success", "non_authorizing"]),
    ("AuthorityContinuityPostCutConsequenceSetV1", ["authority_continuity_closure_ref", "closure_id", "successor_state_token_ref", "action_request_commitment", "success_visible_continuity_state_ref", "selected_authority_consumption_refs", "phase_owned_semantic_mutation_ref", "primary_authorization_receipt_ref", "action_result_ref", "active_idempotency_mapping_ref", "linearization_coverage_witness_ref", "context_current_continuity_relation_ref"], [], ["static_profile_exact_complete_set", "same_store_atomic_visibility", "post_cut_facts_outside_own_closure_id", "no_runtime_optional_members", "no_separate_component_heads", "no_postcommit_repair"]),
    ("AuthorityContinuityClosureV1", ["protocol_version", "manifest_id", "authority_context_kind", "authority_context_ref", "predecessor", "store_successor_allocation", "semantic_cut", "class_entries", "graph_edges"], ["ContextGenesisPredecessor", "PriorClosurePredecessor"], ["fresh_store_allocated_non_aba_successor_token", "complete_typed_class_and_facet_closure", "finite_graph_endpoint_totality", "content_addressed_closure_id", "post_cut_consequences_excluded_from_own_closure_id", "immutable"]),
]
DOMAIN_SCHEMA_DESCRIPTOR = "maestro.vnext.stage2.authority.schema-descriptor.v1"
DOMAIN_SCHEMA_SUITE = "maestro.vnext.stage2.authority.schema-suite.v1"
DOMAIN_LITERALS = "maestro.vnext.stage2.authority.literals.v1"
DOMAIN_ACTION_SPEC = "maestro.vnext.stage2.authority.action-spec-v2.v1"
DOMAIN_CONTINUITY = "maestro.vnext.stage2.authority.continuity-manifest.v1"
DOMAIN_ROOT = "maestro.vnext.stage2.authority.root.v1"
STAGE0_EFFECT_HOME_DOMAIN = "maestro.vnext.stage0.effect-home.v1"
STAGE2_SEMANTIC_LITERAL_PATTERNS = list(dict.fromkeys([
    "EffectIntent", "EffectOrigin", "DispatchAttempt", "ReconciliationAttempt", "RemoteClassification",
    "EffectWithdrawal", "WithdrawEffectIntent", "RecoverReserved", "ControlHead", "ControlRevision", "WriterTerm",
    "PublishBootstrapMandateInteractionOutcome", *CMA_OBSERVATION_PUBLICATION_PURPOSES,
    *CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES, *TRANSITION_GUARD_KINDS,
]))
STAGE2_SEMANTIC_SOURCE_DECLARATIONS = {
    "src/domain/vnext/authority/bootstrap_catalog.rs": ("Authority", "candidate_contract_definition", "exact_stage2_bootstrap_target_literal"),
    "src/domain/vnext/authority/capacity.rs": ("Authority", "candidate_contract_definition", "exact_stage2_capacity_literal"),
    "src/domain/vnext/authority/closed.rs": ("Authority", "candidate_contract_definition", "exact_stage2_closed_sum_literal"),
    "src/domain/vnext/authority/continuity/catalog.rs": ("Authority", "candidate_contract_definition", "exact_stage2_continuity_effect_intent_class_literal"),
    "src/domain/vnext/authority/continuity/totality.rs": ("Authority", "candidate_contract_definition", "exact_stage2_continuity_owner_census_literal"),
    "src/domain/vnext/authority/mod.rs": ("Authority", "candidate_contract_definition", "exact_stage2_authority_facade_literal"),
    "src/domain/vnext/authority/transition.rs": ("Authority", "candidate_contract_definition", "exact_stage2_transition_guard_literal"),
    "tests/vnext_authority_capacity_transition.rs": ("Stage2Proof", "candidate_proof_reader", "exact_stage2_capacity_and_transition_proof"),
    "tests/vnext_authority_contracts.rs": ("Stage2Proof", "candidate_proof_reader", "exact_stage2_authority_contract_proof"),
    "tests/vnext_authority_continuity_totality.rs": ("Stage2Proof", "candidate_proof_reader", "exact_stage2_continuity_totality_proof"),
    "tests/vnext_authority_literals.rs": ("Stage2Proof", "candidate_proof_reader", "exact_stage2_literal_artifact_proof"),
    "tools/vnext_contracts/stage2/authority/build.py": ("Stage2Authority", "candidate_contract_definition", "exact_stage2_authority_builder_semantics"),
    "tools/vnext_contracts/stage2/authority/validate.py": ("Stage2Proof", "candidate_proof_reader", "independent_stage2_semantic_reconstruction"),
    "tools/vnext_contracts/stage2/authority/verify.rb": ("Stage2Proof", "candidate_proof_reader", "independent_stage2_ruby_reconstruction"),
}


class ValidationError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValidationError(message)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def json_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n").encode("ascii")


def cbor_head(major: int, value: int) -> bytes:
    require(isinstance(value, int) and not isinstance(value, bool) and 0 <= value <= 0xFFFFFFFFFFFFFFFF, "CBOR integer is not unsigned u64")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def encode(value: Any) -> bytes:
    if value is False:
        return b"\xf4"
    if value is True:
        return b"\xf5"
    if isinstance(value, int) and not isinstance(value, bool):
        return cbor_head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return cbor_head(3, len(raw)) + raw
    if isinstance(value, list):
        return cbor_head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        raw = bytes.fromhex(value["bytes"])
        return cbor_head(2, len(raw)) + raw
    raise ValidationError(f"unsupported canonical value: {value!r}")


def identity(envelope: list[Any]) -> tuple[str, bytes]:
    encoded = encode(envelope)
    return sha256_bytes(encoded), encoded


def slug(name: str) -> str:
    output = []
    for index, character in enumerate(name):
        if character.isupper() and index:
            output.append("-")
        output.append(character.lower())
    return "".join(output)


def tracked_stage0_tree_digest() -> str:
    process = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "contracts/vnext/stage0", "tools/vnext_contracts/stage0"],
        cwd=WORKSPACE, capture_output=True, check=False,
    )
    require(process.returncode == 0, "cannot enumerate Stage 0 tree")
    paths = sorted(
        relative
        for path in process.stdout.split(b"\0")
        if path
        for relative in [path.decode("utf-8")]
        if not relative.endswith(".pyc") and "/__pycache__/" not in relative
    )
    digest = hashlib.sha256()
    for relative in paths:
        path_bytes = relative.encode("utf-8")
        data = (WORKSPACE / relative).read_bytes()
        digest.update(len(path_bytes).to_bytes(8, "big"))
        digest.update(path_bytes)
        digest.update(len(data).to_bytes(8, "big"))
        digest.update(data)
    return digest.hexdigest()


def load(root: Path, relative: str) -> dict[str, Any]:
    try:
        value = json.loads((root / relative).read_text(encoding="ascii"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValidationError(f"invalid or missing Stage 2 artifact: {relative}") from error
    require(isinstance(value, dict), f"{relative} must contain one object")
    return value


def semantic_delta() -> dict[str, Any]:
    delta = json.loads(STAGE2_DELTA_PATH.read_text(encoding="ascii"))
    expected_rows = []
    for path, (owner, disposition, proof) in sorted(STAGE2_SEMANTIC_SOURCE_DECLARATIONS.items()):
        source = WORKSPACE / path
        contents = source.read_text(encoding="utf-8", errors="ignore")
        matched = [literal for literal in STAGE2_SEMANTIC_LITERAL_PATTERNS if literal in contents]
        require(bool(matched), f"Stage 2 semantic consumer has no literal: {path}")
        digest = sha256_bytes(source.read_bytes())
        expected_rows.append({
            "path": path, "resource_identity": f"sha256:{digest}", "worktree_sha256": digest,
            "matched_literals": matched, "owner": owner, "consumer_disposition": disposition, "proof": proof,
        })
    require(delta.get("consumer_rows") == expected_rows, "Stage 2 semantic-consumer delta rows drifted")
    encoded = encode([STAGE0_EFFECT_HOME_DOMAIN, delta["canonical_value"]])
    require(delta.get("identity") == f"sha256:{sha256_bytes(encoded)}", "Stage 2 semantic-consumer delta identity drifted")
    require(delta.get("consumer_count") == len(expected_rows), "Stage 2 semantic-consumer delta count drifted")
    require(delta.get("closure_status") == "complete_exact_source_overlay", "Stage 2 semantic-consumer delta is incomplete")
    return delta


def expected_documents() -> tuple[dict[str, dict[str, Any]], dict[str, bytes]]:
    stage0_tree_sha256 = tracked_stage0_tree_digest()
    descriptors = []
    descriptor_bytes: dict[str, bytes] = {}
    schema_ids: dict[str, str] = {}
    for tag, (name, fields, variants, invariants) in enumerate(SCHEMA_DEFINITIONS, 1):
        canonical = [tag, name, fields, [[index, variant] for index, variant in enumerate(variants, 1)], invariants]
        envelope = [DOMAIN_SCHEMA_DESCRIPTOR, canonical]
        descriptor_id, encoded = identity(envelope)
        cbor_path = f"descriptors/{slug(name)}.cbor"
        schema_ids[name] = descriptor_id
        descriptor_bytes[cbor_path] = encoded
        descriptors.append({
            "byte_length": len(encoded), "canonical_value": canonical, "cbor_path": cbor_path,
            "descriptor_id": descriptor_id, "fields": fields, "identity_envelope": envelope,
            "invariants": invariants, "schema_name": name, "tag": tag, "variants": variants,
        })
    suite_value = [[row["tag"], row["schema_name"], {"bytes": row["descriptor_id"]}] for row in descriptors]
    suite_envelope = [DOMAIN_SCHEMA_SUITE, suite_value]
    suite_id, suite_cbor = identity(suite_envelope)
    schemas = {
        "byte_length": len(suite_cbor), "descriptor_count": len(descriptors), "descriptors": descriptors,
        "identity_envelope": suite_envelope, "publication_state": PUBLICATION_STATE,
        "schema_version": "maestro.vnext.stage2.authority.schema-descriptors.v1", "suite_id": suite_id,
    }

    literals_value = [
        [[index, value] for index, value in enumerate(AUTHORITY_CONTEXTS, 1)],
        [[index, value] for index, value in enumerate(ACTION_AUTHORITY_BASES, 1)],
        [[index, value] for index, value in enumerate(ACTION_RESULT_OUTCOMES, 1)],
        [[index, value] for index, value in enumerate(RESPONSE_ORIGINS, 1)],
        [[index, value] for index, value in enumerate(REPOSITORY_CAPACITY_KINDS, 1)],
        [[index, value] for index, value in enumerate(INSTALLATION_CAPACITY_KINDS, 1)],
        [[index, value] for index, value in enumerate(CMA_OBSERVATION_PUBLICATION_PURPOSES, 1)],
        [[index, value] for index, value in enumerate(CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES, 1)],
        [[index, value] for index, value in enumerate(TRANSITION_GUARD_KINDS, 1)],
        [[index, value] for index, value in enumerate(REPOSITORY_CONTINUITY_CLASSES, 1)],
        [[index, value] for index, value in enumerate(INSTALLATION_CONTINUITY_CLASSES, 1)],
        BOOTSTRAP_TARGET_ROWS,
    ]
    literals_envelope = [DOMAIN_LITERALS, literals_value]
    literals_id, literals_cbor = identity(literals_envelope)
    literals = {
        "action_authority_bases": ACTION_AUTHORITY_BASES, "action_result_outcomes": ACTION_RESULT_OUTCOMES,
        "authority_contexts": AUTHORITY_CONTEXTS,
        "bootstrap_target_rows": [{"disposition": row[2], "leaf": row[1], "reason": row[3], "tag": row[0]} for row in BOOTSTRAP_TARGET_ROWS],
        "byte_length": len(literals_cbor), "canonical_value": literals_value,
        "cma_observation_publication_purposes": CMA_OBSERVATION_PUBLICATION_PURPOSES,
        "cma_effect_withdrawal_slot_families": CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES,
        "transition_guard_kinds": TRANSITION_GUARD_KINDS, "identity_envelope": literals_envelope,
        "installation_capacity_kinds": INSTALLATION_CAPACITY_KINDS,
        "installation_continuity_classes": INSTALLATION_CONTINUITY_CLASSES,
        "literals_id": literals_id, "publication_state": PUBLICATION_STATE,
        "repository_capacity_kinds": REPOSITORY_CAPACITY_KINDS,
        "repository_continuity_classes": REPOSITORY_CONTINUITY_CLASSES,
        "response_origins": RESPONSE_ORIGINS, "schema_version": "maestro.vnext.stage2.authority.literals.v1",
    }

    predecessor_catalog_id = PREDECESSOR_CATALOG_IDS[8][2]
    produced_schema_bindings = [[name, {"bytes": schema_ids[name]}] for name in PRODUCED_SCHEMA_NAMES]
    action_value = [
        1, "IssueBootstrapMandate", [9, {"bytes": PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID}],
        [9, {"bytes": predecessor_catalog_id}], {"bytes": schema_ids["IssueBootstrapMandateRequestV1"]},
        {"bytes": schema_ids["ActionAuthorityBasisV1"]}, "BootstrapControlG0", PRODUCED_RECORD_CLOSURE,
        produced_schema_bindings, ["newly_minted", 1, "converged", 0], ACTION_RESULT_OUTCOMES, RESPONSE_ORIGINS,
        ["same_store_atomic_semantic_point", "zero_external_io", "same_key_replay_zero_write",
         "different_key_convergence_fresh_authorization", "unknown_fields_fail_closed"],
    ]
    action_envelope = [DOMAIN_ACTION_SPEC, action_value]
    action_id, action_cbor = identity(action_envelope)
    action = {
        "action_authority_basis": "BootstrapControlG0", "action_spec_id": action_id,
        "byte_length": len(action_cbor), "canonical_value": action_value, "identity_envelope": action_envelope,
        "issuance_binding_cardinality": {"converged_existing_mandate": 0, "newly_minted_mandate": 1},
        "leaf": "IssueBootstrapMandate",
        "predecessor": {"catalog_09_manifest_id": predecessor_catalog_id, "catalog_number": 9,
                        "catalog_type": "ActionSpecV1", "descriptor_id": PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID},
        "produced_record_closure": PRODUCED_RECORD_CLOSURE, "publication_state": PUBLICATION_STATE,
        "produced_schema_bindings": [{"schema_name": name, "descriptor_id": schema_ids[name]} for name in PRODUCED_SCHEMA_NAMES],
        "schema_version": "maestro.vnext.stage2.authority.action-spec-v2.v1",
        "successor_scope": "IssueBootstrapMandate_only",
    }

    continuity_value = [
        {"bytes": schema_ids["AuthorityContinuityManifestV1"]},
        [7, {"bytes": PREDECESSOR_CATALOG_IDS[6][2]}, REPOSITORY_CONTINUITY_CLASSES],
        [8, {"bytes": PREDECESSOR_CATALOG_IDS[7][2]}, INSTALLATION_CONTINUITY_CLASSES],
        REPOSITORY_CAPACITY_KINDS, INSTALLATION_CAPACITY_KINDS,
        CMA_OBSERVATION_PUBLICATION_PURPOSES, CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES,
        TRANSITION_GUARD_KINDS, {"bytes": literals_id},
        ["old_client_refusal", "candidate_only", "no_runtime_activation", "no_predecessor_rewrite"],
    ]
    continuity_envelope = [DOMAIN_CONTINUITY, continuity_value]
    continuity_id, continuity_cbor = identity(continuity_envelope)
    continuity = {
        "byte_length": len(continuity_cbor),
        "cma_observation_publication_purposes": CMA_OBSERVATION_PUBLICATION_PURPOSES,
        "cma_effect_withdrawal_slot_families": CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES,
        "transition_guard_kinds": TRANSITION_GUARD_KINDS, "identity_envelope": continuity_envelope,
        "installation_capacity_kinds": INSTALLATION_CAPACITY_KINDS,
        "installation_classes": INSTALLATION_CONTINUITY_CLASSES,
        "installation_predecessor_catalog_id": PREDECESSOR_CATALOG_IDS[7][2], "manifest_id": continuity_id,
        "publication_state": PUBLICATION_STATE, "repository_capacity_kinds": REPOSITORY_CAPACITY_KINDS,
        "repository_classes": REPOSITORY_CONTINUITY_CLASSES,
        "repository_predecessor_catalog_id": PREDECESSOR_CATALOG_IDS[6][2],
        "schema_descriptor_id": schema_ids["AuthorityContinuityManifestV1"],
        "schema_version": "maestro.vnext.stage2.authority.continuity-manifest.v1",
    }

    documents = {"schemas": schemas, "literals": literals, "action": action, "continuity": continuity}
    files = {
        **descriptor_bytes, "schema-descriptors.v1.cbor": suite_cbor,
        "schema-descriptors.v1.json": json_bytes(schemas), "authority-literals.v1.cbor": literals_cbor,
        "authority-literals.v1.json": json_bytes(literals), "action-spec-v2.v1.cbor": action_cbor,
        "action-spec-v2.v1.json": json_bytes(action), "authority-continuity-manifest.v1.cbor": continuity_cbor,
        "authority-continuity-manifest.v1.json": json_bytes(continuity),
    }
    delta = semantic_delta()
    root_value = [
        stage0_tree_sha256,
        [[number, kind, {"bytes": value}] for number, kind, value in PREDECESSOR_CATALOG_IDS],
        [["schema_suite", {"bytes": suite_id}], ["authority_literals", {"bytes": literals_id}],
         ["action_spec_v2", {"bytes": action_id}], ["authority_continuity_manifest", {"bytes": continuity_id}],
         ["stage2_semantic_consumer_delta", delta["identity"]]],
        PUBLICATION_STATE,
    ]
    root_envelope = [DOMAIN_ROOT, root_value]
    root_id, root_cbor = identity(root_envelope)
    primary_paths = sorted(files)
    root = {
        "artifacts": [{"byte_length": len(files[path]), "path": path, "sha256": sha256_bytes(files[path])} for path in primary_paths],
        "byte_length": len(root_cbor),
        "component_ids": {"action_spec_v2": action_id, "authority_continuity_manifest": continuity_id,
                          "authority_literals": literals_id, "schema_suite": suite_id,
                          "stage2_semantic_consumer_delta": delta["identity"]},
        "identity_envelope": root_envelope, "predecessor_action_spec_descriptor_id": PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID,
        "predecessor_catalog_ids": [{"catalog_number": number, "catalog_type": kind, "manifest_id": value} for number, kind, value in PREDECESSOR_CATALOG_IDS],
        "publication_state": PUBLICATION_STATE, "root_id": root_id,
        "schema_version": "maestro.vnext.stage2.authority.root-manifest.v1",
        "stage": "stage2_authority_candidate", "stage0_tree_sha256": stage0_tree_sha256,
        "stage2_semantic_consumer_delta": {"identity": delta["identity"], "consumer_count": delta["consumer_count"],
                                           "consumer_digest": delta["consumer_digest"], "predecessor": delta["predecessor"]},
    }
    documents["manifest"] = root
    files["stage2-authority-manifest.v1.cbor"] = root_cbor
    files["stage2-authority-manifest.v1.json"] = json_bytes(root)
    return documents, files


def load_documents(root: Path) -> dict[str, dict[str, Any]]:
    return {
        "schemas": load(root, "schema-descriptors.v1.json"),
        "literals": load(root, "authority-literals.v1.json"),
        "action": load(root, "action-spec-v2.v1.json"),
        "continuity": load(root, "authority-continuity-manifest.v1.json"),
        "manifest": load(root, "stage2-authority-manifest.v1.json"),
    }


def validate_semantics(documents: dict[str, dict[str, Any]]) -> None:
    expected, _ = expected_documents()
    for name in ("schemas", "literals", "action", "continuity", "manifest"):
        require(documents.get(name) == expected[name], f"Stage 2 projection drifted: {name}")


def validate_physical(root: Path) -> None:
    _, expected_files = expected_documents()
    for relative, expected in expected_files.items():
        path = root / relative
        require(path.is_file(), f"Stage 2 component is missing: {relative}")
        require(path.read_bytes() == expected, f"Stage 2 component bytes drifted: {relative}")
    require(len(tracked_stage0_tree_digest()) == 64, "Stage 0 tree digest is not SHA-256")


def run_mutants(documents: dict[str, dict[str, Any]]) -> list[str]:
    mutations: list[tuple[str, Any]] = [
        ("third_authority_context", lambda value: value["literals"]["authority_contexts"].append("GlobalAuthorityContext")),
        ("fourth_authority_basis", lambda value: value["literals"]["action_authority_bases"].append("AmbientRole")),
        ("eighth_result_outcome", lambda value: value["literals"]["action_result_outcomes"].append("applied")),
        ("missing_observation_purpose", lambda value: value["literals"]["cma_observation_publication_purposes"].pop()),
        ("withdrawal_family_alias", lambda value: value["literals"]["cma_effect_withdrawal_slot_families"].__setitem__(2, "RecoveryExternalStatus")),
        ("missing_transition_guard_kind", lambda value: value["literals"]["transition_guard_kinds"].pop()),
        ("bootstrap_predecessor_order", lambda value: value["literals"]["bootstrap_target_rows"].__setitem__(6, copy.deepcopy(value["literals"]["bootstrap_target_rows"][8]))),
        ("action_spec_scope_widening", lambda value: value["action"].update({"successor_scope": "all_authority_actions"})),
        ("produced_record_omission", lambda value: value["action"]["produced_record_closure"].pop()),
        ("produced_schema_binding_substitution", lambda value: value["action"]["produced_schema_bindings"][0].update({"descriptor_id": "0" * 64})),
        ("missing_schema_descriptor", lambda value: value["schemas"]["descriptors"].pop()),
        ("component_identity_substitution", lambda value: value["manifest"]["component_ids"].update({"authority_literals": "0" * 64})),
        ("root_canonical_projection_substitution", lambda value: value["manifest"].update({"canonical_value": ["invented"]})),
        ("artifact_omission", lambda value: value["manifest"]["artifacts"].pop()),
        ("unknown_field", lambda value: value["manifest"].update({"unknown_projection": True})),
    ]
    rejected = []
    for name, mutate in mutations:
        mutant = copy.deepcopy(documents)
        mutate(mutant)
        try:
            validate_semantics(mutant)
        except ValidationError:
            rejected.append(name)
        else:
            raise ValidationError(f"semantic mutant was accepted: {name}")
    return rejected


def receipt(documents: dict[str, dict[str, Any]], mutants: list[str]) -> dict[str, Any]:
    return {
        "checks": ["candidate_only", "independent_semantic_reconstruction", "exact_projection_fields",
                   "exact_component_identities", "exact_artifact_closure", "exact_capacity_kinds",
                   "distinct_cma_observation_and_withdrawal_sets", "complete_transition_guard_kinds",
                   "exact_bootstrap_target_census", "action_spec_v2_single_leaf", "stage0_tree_freeze",
                   "action_spec_exact_12j_consequence_closure", "action_spec_produced_descriptor_bindings",
                   "stage2_semantic_consumer_delta"],
        "mutants_rejected": mutants, "publication_state": PUBLICATION_STATE,
        "result": "semantic_closure_validated", "root_id": documents["manifest"]["root_id"],
        "schema_version": "maestro.vnext.stage2.authority.semantic-validation-receipt.v1",
        "semantic_counts": {"action_authority_bases": 3, "action_result_outcomes": 7,
                            "action_produced_records": 11, "action_produced_schema_bindings": 10,
                            "authority_contexts": 2, "bootstrap_targets_admitted": 3,
                            "bootstrap_targets_excluded": 8, "bootstrap_targets_total": 11,
                            "cma_observation_publication_purposes": 5,
                            "cma_effect_withdrawal_slot_families": 5,
                            "transition_guard_kinds": 8, "installation_capacity_kinds": 6,
                            "installation_continuity_classes": 30, "repository_capacity_kinds": 6,
                            "repository_continuity_classes": 35, "response_origins": 2,
                            "schema_descriptors": 22},
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--mutants", action="store_true")
    parser.add_argument("--emit", action="store_true")
    args = parser.parse_args()
    try:
        documents = load_documents(args.root)
        validate_semantics(documents)
        validate_physical(args.root)
        mutants = run_mutants(documents)
        value = receipt(documents, mutants)
        if args.emit:
            print(json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True))
        else:
            require(load(args.root, "semantic-validation-receipt.v1.json") == value, "semantic validation receipt is stale")
            print("Stage 2 Authority semantic validation passed")
    except (ValidationError, OSError, KeyError, TypeError, ValueError) as error:
        print(f"Stage 2 Authority validation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
