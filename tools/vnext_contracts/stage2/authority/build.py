#!/usr/bin/env python3
"""Build the additive, inactive Stage 2 Authority contract literals."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[3]
OUTPUT = WORKSPACE / "contracts/vnext/stage2/authority"
CATALOGS = WORKSPACE / "contracts/vnext/catalogs/generated"
sys.dont_write_bytecode = True
sys.path.insert(0, str(WORKSPACE / "tools/vnext_contracts/catalogs"))
import cbor_py  # noqa: E402


PUBLICATION_STATE = "inactive_candidate"
STAGE2_SEMANTIC_DELTA = (
    WORKSPACE
    / "contracts/vnext/stage0/effect-home/stage2-semantic-consumer-delta-v1.json"
)
GOVERNANCE_FLOOR_SOURCE = "src/domain/vnext/authority/governance_floor.rs"
GOVERNANCE_FLOOR_REQUIRED_LITERALS = (
    "RepositoryGovernanceFloorSnapshotV1",
    "maestro.vnext.repository-governance-floor-snapshot.v1",
    "maestro.vnext.repository-governance-head-class-8.v1",
)
GOVERNANCE_FLOOR_REQUIRED_SOURCE_FRAGMENTS = (
    "pub(super) struct RepositoryGovernanceFloorSnapshotV1 {",
    "let snapshot = RepositoryGovernanceFloorSnapshotV1::decode_object(direct_object)?;",
    (
        "let history = validate_history(*direct_root, &by_id)?; "
        "let class_root = hash_value(&CborValue::Array(vec![ "
        'CborValue::text("maestro.vnext.repository-governance-head-class-8.v1")?,'
    ),
    (
        "let commitment = current_view_commitment( view, head, generation, &snapshot, "
        "*direct_root, class_root,"
    ),
)
GOVERNANCE_FLOOR_SOURCE_MUTANTS = (
    (
        b"pub(super) struct RepositoryGovernanceFloorSnapshotV1 {",
        b"pub(super) struct RepositoryGovernanceFloorSnapshotMutantV1 {",
    ),
    (b"decode_object(direct_object)?", b"decode_object(mutant_object)?"),
    (
        b"maestro.vnext.repository-governance-head-class-8.v1",
        b"maestro.vnext.repository-governance-head-class-mutant.v1",
    ),
    (b"class_root,\n        authority,", b"[0; 32],\n        authority,"),
)
PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID = (
    "bf2075863bfa3ec7e5269560464182264e78fbeec6dff8197d5dae7bf278a0b4"
)
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
ACTION_AUTHORITY_BASES = [
    "OrdinaryLiveRuntime",
    "BootstrapControlG0",
    "ContinuityMaintenance",
]
ACTION_RESULT_OUTCOMES = [
    "committed",
    "no_op",
    "rejected",
    "stale",
    "conflict",
    "unavailable",
    "in_doubt",
]
RESPONSE_ORIGINS = ["fresh", "replay"]
REPOSITORY_CAPACITY_KINDS = [
    "RepositoryOrdinaryMutation",
    "RepositoryAuthorityAdministration",
    "RepositoryEvidenceAcquisition",
    "RepositoryPlanningPublication",
    "RepositoryExternalEffect",
    "RepositoryPersistenceMaintenance",
]
INSTALLATION_CAPACITY_KINDS = [
    "InstallationAuthorityAdministration",
    "InstallationDistributionMutation",
    "InstallationGovernedReviewPublication",
    "InstallationExternalEffect",
    "InstallationWriterAdministration",
    "InstallationPersistenceMaintenance",
]
CMA_OBSERVATION_PUBLICATION_PURPOSES = [
    "TrustedTimeAcquisition",
    "RecoveryExternalRegistration",
    "RecoveryExternalStatus",
    "MaintenanceExecutorCurrentness",
    "ProspectiveContinuityCarrier",
]
CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES = [
    "MaintenanceExecutorCurrentness",
    "ProspectiveContinuityCarrier",
    "PlannedTurnoverHighWater",
    "RepositoryRecoveryAdmission",
    "InstallationRecoveryAdmission",
]
TRANSITION_GUARD_KINDS = [
    "RepositoryWorkAuthorityPolicyTransition",
    "RepositoryFirstWorkPublication",
    "RepositoryFloorOrTrustRootRotation",
    "InstallationPolicyBindingReplacement",
    "InstallationStructuralRootFloorReplacement",
    "TrustedTimePolicyStackRotation",
    "ExternalLogicalCarrierProfileRotation",
    "PlannedEpochTurnoverPreparation",
]
REPOSITORY_CONTINUITY_CLASSES = [
    "RepositoryOrdinaryMutationCapacityState",
    "RepositoryAuthorityAdministrationCapacityState",
    "RepositoryEvidenceAcquisitionCapacityState",
    "RepositoryPlanningPublicationCapacityState",
    "RepositoryExternalEffectCapacityState",
    "RepositoryPersistenceMaintenanceCapacityState",
    "RepositoryStoreGenerationCurrentness",
    "RepositoryGovernanceHead",
    "RepositoryAuthorityEpochState",
    "RepositoryTrustRootState",
    "RepositoryPrincipalBindingState",
    "RepositorySessionState",
    "RepositoryGrantState",
    "RepositoryDelegationState",
    "RepositoryMandateState",
    "RepositoryRevocationState",
    "RepositoryAuthorizationReceiptState",
    "RepositoryConsumptionCellState",
    "RepositoryContinuityState",
    "RepositoryTrustedTimeState",
    "RepositoryRecoveryCommitmentState",
    "RepositoryRecoveryAdmissionState",
    "RepositoryStepExecutionState",
    "RepositoryEffectIntentState",
    "RepositoryEvidenceState",
    "RepositoryGateSnapshot",
    "RepositoryPlanningState",
    "RepositoryCoordinationState",
    "RepositoryDesignDecisionState",
    "RepositoryContractState",
    "RepositoryWorkState",
    "RepositoryPersistenceRetentionState",
    "RepositoryMemoryState",
    "RepositoryIntakeState",
    "RepositoryResearchState",
]
INSTALLATION_CONTINUITY_CLASSES = [
    "InstallationAuthorityAdministrationCapacityState",
    "InstallationDistributionMutationCapacityState",
    "InstallationGovernedReviewPublicationCapacityState",
    "InstallationExternalEffectCapacityState",
    "InstallationWriterAdministrationCapacityState",
    "InstallationPersistenceMaintenanceCapacityState",
    "InstallationLocatorCurrentness",
    "InstallationStoreGenerationCurrentness",
    "InstallationGovernanceHead",
    "InstallationAuthorityEpochState",
    "InstallationTrustRootState",
    "InstallationPrincipalBindingState",
    "InstallationGrantState",
    "InstallationMandateState",
    "InstallationRevocationState",
    "InstallationAuthorizationReceiptState",
    "InstallationConsumptionCellState",
    "InstallationContinuityState",
    "InstallationRecoveryCommitmentState",
    "InstallationRecoveryAdmissionState",
    "InstallationWriterCohortState",
    "InstallationClientCompatibilityState",
    "InstallationDistributionTargetState",
    "InstallationDistributionTransactionState",
    "InstallationBinarySlotState",
    "InstallationResourceManifestState",
    "InstallationGovernedReviewPublicationState",
    "InstallationEffectIntentState",
    "InstallationEvidenceState",
    "InstallationPersistenceRetentionState",
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
    "AuthorityMandateOrConvergenceRefV1",
    "BootstrapMandateIssuanceBindingV1:exactly_one_if_newly_minted",
    "AuthorizationReceiptV1:primary",
    "ActionResultV1",
    "IdempotencyRecordV1",
    "AuthorityContinuityClosureV1:pre_cut_exact",
    "SuccessVisibleAuthorityContinuityStateV1:exactly_one",
    "AdmittedTransitionGuardV1:persisted_from_serialization_current_owner_facts",
    "BootstrapAuthoritySnapshotV1:successor_current_authority_carrier",
    "LinearizationCoverageWitnessV1:recoverable",
    "AuthorityContinuityPostCutConsequenceSetV1:complete_exact",
]
PRODUCED_SCHEMA_NAMES = [
    "AuthorityMandateV1",
    "BootstrapMandateIssuanceBindingV1",
    "AuthorizationReceiptV1",
    "ActionResultV1",
    "BootstrapAuthoritySnapshotV1",
    "AuthorityContinuityClosureV1",
    "SuccessVisibleAuthorityContinuityStateV1",
    "AdmittedTransitionGuardV1",
    "LinearizationCoverageWitnessV1",
    "AuthorityContinuityPostCutConsequenceSetV1",
]

SCHEMA_DEFINITIONS = [
    (
        "AuthorityMandateV1",
        [
            "authority_context_ref", "action", "subject", "action_revision",
            "consent_slot_binding_parameter", "responder_binding_ref",
            "responder_assurance_revision", "interaction_closure_ref",
            "authority_basis_commitment_ref", "valid_from_inclusive",
            "valid_until_exclusive", "maximum_uses", "delegation_depth_remaining",
        ],
        [],
        ["maximum_uses_exactly_one", "delegation_forbidden", "same_context_only", "immutable"],
    ),
    (
        "BootstrapMandateIssuanceBindingV1",
        [
            "mandate_ref", "target_action_commitment", "consent_slot_commitment",
            "interaction_closure_ref",
        ],
        [],
        ["exists_exactly_once_only_for_new_mandate", "absent_for_convergence", "immutable"],
    ),
    (
        "AuthorizationReceiptV1",
        [
            "receipt_id", "authority_context_ref", "action_request_ref",
            "authority_basis_ref", "authorization_decision", "primary",
            "action_result_ref",
        ],
        ["authorized", "denied"],
        ["primary_exactly_one", "non_bearer", "retrospective_only", "immutable"],
    ),
    (
        "ActionResultV1",
        [
            "result_id", "action_request_ref", "outcome", "response_origin",
            "before_refs", "after_refs", "receipt_refs", "produced_refs",
            "effect_attempt_refs", "safe_reason_code", "next_or_inspect_ref",
        ],
        ACTION_RESULT_OUTCOMES,
        ["response_origin_not_outcome", "replay_returns_original_result", "immutable"],
    ),
    (
        "IssueBootstrapMandateRequestV1",
        [
            "request_id", "authority_context_ref", "actor_binding_ref", "actor_session_ref",
            "responder_binding_ref", "presentation_observation_ref",
            "affirmative_response_observation_ref", "target", "target_subject",
            "target_revision", "consent_slot_binding_parameter", "idempotency_key",
            "supplied_mandates",
        ],
        [],
        ["supplied_mandates_exactly_zero", "self_reference_free", "no_external_io"],
    ),
    (
        "ConsentSlotBindingParameterV1",
        ["slot_protocol_commitment", "target_action_commitment", "slot_commitment"],
        [],
        ["fixed_before_issuance", "self_reference_free", "non_authorizing"],
    ),
    (
        "ActionAuthorityBasisV1",
        ["basis_tag", "authority_context_ref", "basis_commitment"],
        ACTION_AUTHORITY_BASES,
        ["leaf_selects_exactly_one", "no_ranking", "no_fallback", "no_cross_donation"],
    ),
    (
        "AuthorityContextV1",
        [
            "context_tag", "context_id", "stable_domain_identity",
            "protected_realm_if_installation", "store_generation", "authority_epoch",
            "trust_root_revision", "locator_binding_revision_if_installation",
        ],
        AUTHORITY_CONTEXTS,
        ["exactly_one_domain", "no_cross_store_authority", "unknown_variant_refused"],
    ),
    (
        "GovernedCapacityDebitV1",
        [
            "capacity_root_ref", "authority_context_kind", "authority_context_ref",
            "capacity_kind", "ordinal", "prior_spent", "resulting_spent",
        ],
        REPOSITORY_CAPACITY_KINDS + INSTALLATION_CAPACITY_KINDS,
        ["quantity_exactly_one", "fresh_committed_only", "replay_zero_debit", "same_domain_only"],
    ),
    (
        "AuthorityContinuityManifestV1",
        [
            "authority_context_kind", "protocol_version", "canonicalization_version",
            "obligations", "dispositions", "owner_contributions", "class_ids",
            "class_descriptors",
        ],
        AUTHORITY_CONTEXTS,
        ["closed_class_set", "old_client_refusal", "candidate_only", "immutable"],
    ),
    (
        "PrincipalBindingV1",
        [
            "binding_id", "principal_id", "authority_context_ref", "trust_root_revision",
            "assurance_revision", "validity", "human_capable",
        ],
        [],
        ["nonzero_protocol_revisions", "finite_half_open_validity", "same_context_only", "immutable"],
    ),
    (
        "SessionV1",
        [
            "session_id", "principal_binding_ref", "authority_context_ref", "store_generation",
            "authority_epoch", "request_commitment", "validity",
        ],
        [],
        ["nonempty_bounded_ascii_commitment", "binding_context_generation_epoch_exact", "immutable"],
    ),
    (
        "BootstrapGenesisGrantV1",
        [
            "grant_id", "authority_context_ref", "grantee_principal_ref", "authority_epoch",
            "trust_root_revision", "local_capacity_constraint", "terminal_scope", "delegable_scope",
            "valid_from_inclusive", "valid_until_exclusive",
        ],
        ["NoLocalBoundedRoot"],
        [
            "exactly_one_structural_g0", "terminal_scope_bootstrap_control",
            "delegable_scope_ordinary_bounded", "terminal_and_delegable_scopes_disjoint",
            "bootstrap_control_nondelegable", "inert_before_context_genesis_activation",
        ],
    ),
    (
        "BootstrapMandateInteractionObservationJoinV1",
        [
            "interaction_closure_id", "authority_context_ref", "responder_binding_ref",
            "responder_current_authentication_ref", "presentation_observation_ref",
            "affirmative_response_observation_ref", "carrier_procedure_ref",
            "target_action_commitment",
        ],
        [],
        [
            "presentation_and_affirmative_response_both_required", "same_authenticated_responder",
            "same_context_and_target", "observational_non_authorizing", "immutable",
        ],
    ),
    (
        "RevocationSetV1",
        ["authority_context_ref", "revocation_targets"],
        ["TrustRoot", "PrincipalBinding", "Session", "Grant", "Mandate"],
        ["finite_bounded_set", "same_context_only", "revoked_authority_never_revives", "immutable"],
    ),
    (
        "BootstrapAuthoritySnapshotV1",
        [
            "authority_context", "authority_snapshot", "actor_binding", "actor_session",
            "responder_binding", "responder_session", "bootstrap_g0_candidate_paths",
            "revocations", "interaction_join", "current_carrier_procedure_ref",
            "target_action_projection", "current_target_head", "consent_slot_evaluation_facts",
            "continuity_transition_proof",
        ],
        [],
        [
            "complete_store_loaded_evaluator_facts", "exact_current_generation",
            "fresh_authorization_only", "ambient_time_forbidden", "non_bearer",
        ],
    ),
    (
        "GovernedCapacityRootV1",
        [
            "capacity_root_id", "authority_context_kind", "authority_context_ref", "capacity_kind",
            "initial_maximum", "spent",
        ],
        REPOSITORY_CAPACITY_KINDS + INSTALLATION_CAPACITY_KINDS,
        ["nonzero_bounded_initial_maximum", "spent_not_above_initial_maximum", "same_domain_only", "immutable_revision"],
    ),
    (
        "SuccessVisibleAuthorityContinuityStateV1",
        [
            "protocol_version", "state_token_ref", "predecessor_state_token_ref",
            "authority_context_kind", "authority_context_ref", "store_generation",
            "store_publication_clock", "authority_epoch", "manifest_id", "closure_id",
            "store_allocation_commitment_ref", "guard_kind", "carrier_profile_status",
            "selected_trusted_time_stack_ref", "accepted_authority_time_floor",
            "lane_state_closure_root", "source_floor_root", "gap_companion_refs",
            "floor_provenance_refs", "external_revision_cell_refs", "cma_remaining_root",
            "cma_spent_root", "unresolved_effect_refs", "cut_sequence",
            "guard_admission_digest",
        ],
        [],
        ["exactly_one_per_success", "sole_current_continuity_relation", "no_component_heads", "same_store_visible"],
    ),
    (
        "AdmittedTransitionGuardV1",
        [
            "protocol_version", "guard_kind", "authority_context_kind", "authority_context_ref",
            "store_generation", "authority_epoch", "manifest_id", "closure_id",
            "predecessor_state_token_ref", "cut_sequence", "selected_trusted_time_stack_ref",
            "carrier_profile_status", "accepted_authority_time_floor", "lane_state_closure_root",
            "source_floor_root", "gap_companion_refs", "floor_provenance_refs",
            "external_revision_cell_refs", "cma_remaining_root", "cma_spent_root",
            "unresolved_effect_refs", "owner_term_facts", "owner_census_commitment",
            "owner_census_source_cut_commitment", "disclosure",
        ],
        TRANSITION_GUARD_KINDS,
        [
            "fixed_nominal_owner_terms", "owner_facts_current_at_serialization",
            "bounded_fail_closed", "persisted_non_bearer", "same_store_visible",
        ],
    ),
    (
        "LinearizationCoverageWitnessV1",
        [
            "fence_subject_ref", "fence_carrier", "attempt_ref", "semantic_point_ref",
            "covered_closure_ref", "conservative_point_envelope_ref", "carrier_revision_ref",
        ],
        ["SameStoreCommit", "ProtectedLocatorCAS", "ProtectedRepositoryGenerationCAS", "ProtectedSnapshot"],
        ["recoverable_exact_attempt_coverage", "non_aba", "missing_witness_blocks_success", "non_authorizing"],
    ),
    (
        "AuthorityContinuityPostCutConsequenceSetV1",
        [
            "authority_continuity_closure_ref", "closure_id", "successor_state_token_ref",
            "action_request_commitment", "success_visible_continuity_state_ref",
            "selected_authority_consumption_refs", "phase_owned_semantic_mutation_ref",
            "primary_authorization_receipt_ref", "action_result_ref",
            "active_idempotency_mapping_ref", "linearization_coverage_witness_ref",
            "context_current_continuity_relation_ref",
        ],
        [],
        [
            "static_profile_exact_complete_set", "same_store_atomic_visibility",
            "post_cut_facts_outside_own_closure_id", "no_runtime_optional_members",
            "no_separate_component_heads", "no_postcommit_repair",
        ],
    ),
    (
        "AuthorityContinuityClosureV1",
        [
            "protocol_version", "manifest_id", "authority_context_kind", "authority_context_ref",
            "predecessor", "store_successor_allocation", "semantic_cut", "class_entries",
            "graph_edges",
        ],
        ["ContextGenesisPredecessor", "PriorClosurePredecessor"],
        [
            "fresh_store_allocated_non_aba_successor_token", "complete_typed_class_and_facet_closure",
            "finite_graph_endpoint_totality", "content_addressed_closure_id",
            "post_cut_consequences_excluded_from_own_closure_id", "immutable",
        ],
    ),
]

DOMAIN_SCHEMA_DESCRIPTOR = "maestro.vnext.stage2.authority.schema-descriptor.v1"
DOMAIN_SCHEMA_SUITE = "maestro.vnext.stage2.authority.schema-suite.v1"
DOMAIN_LITERALS = "maestro.vnext.stage2.authority.literals.v1"
DOMAIN_ACTION_SPEC = "maestro.vnext.stage2.authority.action-spec-v2.v1"
DOMAIN_CONTINUITY = "maestro.vnext.stage2.authority.continuity-manifest.v1"
DOMAIN_ROOT = "maestro.vnext.stage2.authority.root.v1"
STAGE0_EFFECT_HOME_DOMAIN = "maestro.vnext.stage0.effect-home.v1"


class BuildError(RuntimeError):
    pass


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def json_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n").encode("ascii")


def write_json(root: Path, relative: str, value: Any) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(json_bytes(value))


def write_bytes(root: Path, relative: str, value: bytes) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value)


def identity(envelope: list[Any]) -> tuple[str, bytes]:
    encoded = cbor_py.encode(envelope)
    return sha256_bytes(encoded), encoded


def slug(name: str) -> str:
    result = []
    for index, character in enumerate(name):
        if character.isupper() and index:
            result.append("-")
        result.append(character.lower())
    return "".join(result)


def tracked_stage0_tree_digest() -> str:
    process = subprocess.run(
        [
            "git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "contracts/vnext/stage0",
            "tools/vnext_contracts/stage0",
        ],
        cwd=WORKSPACE,
        capture_output=True,
        check=False,
    )
    if process.returncode != 0:
        raise BuildError("cannot enumerate tracked Stage 0 files")
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


def verify_governance_floor_source(source_bytes: bytes | None = None) -> None:
    if source_bytes is None:
        source_bytes = (WORKSPACE / GOVERNANCE_FLOOR_SOURCE).read_bytes()
    contents = source_bytes.decode("utf-8", errors="ignore")
    missing = [
        literal
        for literal in GOVERNANCE_FLOOR_REQUIRED_LITERALS
        if literal not in contents
    ]
    if missing:
        raise BuildError(
            "Stage 2 governance-floor source is missing exact persisted semantics: "
            + ", ".join(missing)
        )
    normalized = " ".join(contents.split())
    missing_fragments = [
        fragment
        for fragment in GOVERNANCE_FLOOR_REQUIRED_SOURCE_FRAGMENTS
        if fragment not in normalized
    ]
    if missing_fragments:
        raise BuildError(
            "Stage 2 governance-floor source is missing causal persistence/current-head binding"
        )


def self_test_governance_floor_source() -> None:
    source = (WORKSPACE / GOVERNANCE_FLOOR_SOURCE).read_bytes()
    verify_governance_floor_source(source)
    for target, replacement in GOVERNANCE_FLOOR_SOURCE_MUTANTS:
        if target not in source:
            raise BuildError(f"governance-floor mutant target is absent: {target!r}")
        mutated = source.replace(target, replacement)
        try:
            verify_governance_floor_source(mutated)
        except BuildError:
            continue
        raise BuildError(f"governance-floor causal mutant was accepted: {target!r}")


def load_catalog(number: int) -> dict[str, Any]:
    names = {
        1: "catalog-01-observation.json", 2: "catalog-02-effect.json",
        3: "catalog-03-repository-capacity.json", 4: "catalog-04-installation-capacity.json",
        5: "catalog-05-ceremony.json", 6: "catalog-06-action-leaf.json",
        7: "catalog-07-repository-continuity.json", 8: "catalog-08-installation-continuity.json",
        9: "catalog-09-action-spec.json",
    }
    return json.loads((CATALOGS / names[number]).read_text(encoding="ascii"))


def verify_frozen_inputs() -> None:
    verify_governance_floor_source()
    digest = tracked_stage0_tree_digest()
    if len(digest) != 64:
        raise BuildError("Stage 0 tree digest is not SHA-256")
    for number, kind, expected_id in PREDECESSOR_CATALOG_IDS:
        catalog = load_catalog(number)
        if catalog.get("catalog_type") != kind or catalog.get("manifest_id") != expected_id:
            raise BuildError(f"predecessor catalog {number} identity or kind drifted")
    expected_rows = {
        3: REPOSITORY_CAPACITY_KINDS,
        4: INSTALLATION_CAPACITY_KINDS,
        7: REPOSITORY_CONTINUITY_CLASSES,
        8: INSTALLATION_CONTINUITY_CLASSES,
    }
    for number, expected in expected_rows.items():
        actual = [row["value"][1] for row in load_catalog(number)["descriptors"]]
        if actual != expected:
            raise BuildError(f"predecessor catalog {number} semantic rows drifted")
    action_spec = load_catalog(9)
    rows = [row for row in action_spec["descriptors"] if row["value"][1] == "IssueBootstrapMandate"]
    if len(rows) != 1 or rows[0]["descriptor_id"] != PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID:
        raise BuildError("IssueBootstrapMandate predecessor descriptor drifted")
    delta = json.loads(STAGE2_SEMANTIC_DELTA.read_text(encoding="ascii"))
    encoded = cbor_py.encode([STAGE0_EFFECT_HOME_DOMAIN, delta["canonical_value"]])
    if delta.get("identity") != f"sha256:{sha256_bytes(encoded)}":
        raise BuildError("Stage 2 semantic-consumer delta identity drifted")
    if delta.get("closure_status") != "complete_exact_source_overlay":
        raise BuildError("Stage 2 semantic-consumer delta is incomplete")


def schema_artifacts(root: Path) -> tuple[dict[str, str], str]:
    descriptors = []
    ids: dict[str, str] = {}
    for tag, (name, fields, variants, invariants) in enumerate(SCHEMA_DEFINITIONS, 1):
        canonical = [tag, name, fields, [[index, value] for index, value in enumerate(variants, 1)], invariants]
        envelope = [DOMAIN_SCHEMA_DESCRIPTOR, canonical]
        descriptor_id, encoded = identity(envelope)
        cbor_path = f"descriptors/{slug(name)}.cbor"
        write_bytes(root, cbor_path, encoded)
        ids[name] = descriptor_id
        descriptors.append(
            {
                "byte_length": len(encoded),
                "canonical_value": canonical,
                "cbor_path": cbor_path,
                "descriptor_id": descriptor_id,
                "fields": fields,
                "identity_envelope": envelope,
                "invariants": invariants,
                "schema_name": name,
                "tag": tag,
                "variants": variants,
            }
        )
    suite_value = [[row["tag"], row["schema_name"], {"bytes": row["descriptor_id"]}] for row in descriptors]
    suite_envelope = [DOMAIN_SCHEMA_SUITE, suite_value]
    suite_id, suite_cbor = identity(suite_envelope)
    write_bytes(root, "schema-descriptors.v1.cbor", suite_cbor)
    write_json(
        root,
        "schema-descriptors.v1.json",
        {
            "byte_length": len(suite_cbor),
            "descriptor_count": len(descriptors),
            "descriptors": descriptors,
            "identity_envelope": suite_envelope,
            "publication_state": PUBLICATION_STATE,
            "schema_version": "maestro.vnext.stage2.authority.schema-descriptors.v1",
            "suite_id": suite_id,
        },
    )
    return ids, suite_id


def literals_artifact(root: Path) -> str:
    canonical = [
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
    envelope = [DOMAIN_LITERALS, canonical]
    literals_id, encoded = identity(envelope)
    write_bytes(root, "authority-literals.v1.cbor", encoded)
    write_json(
        root,
        "authority-literals.v1.json",
        {
            "action_authority_bases": ACTION_AUTHORITY_BASES,
            "action_result_outcomes": ACTION_RESULT_OUTCOMES,
            "authority_contexts": AUTHORITY_CONTEXTS,
            "bootstrap_target_rows": [
                {"disposition": row[2], "leaf": row[1], "reason": row[3], "tag": row[0]}
                for row in BOOTSTRAP_TARGET_ROWS
            ],
            "byte_length": len(encoded),
            "canonical_value": canonical,
            "cma_observation_publication_purposes": CMA_OBSERVATION_PUBLICATION_PURPOSES,
            "cma_effect_withdrawal_slot_families": CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES,
            "transition_guard_kinds": TRANSITION_GUARD_KINDS,
            "identity_envelope": envelope,
            "installation_capacity_kinds": INSTALLATION_CAPACITY_KINDS,
            "installation_continuity_classes": INSTALLATION_CONTINUITY_CLASSES,
            "literals_id": literals_id,
            "publication_state": PUBLICATION_STATE,
            "repository_capacity_kinds": REPOSITORY_CAPACITY_KINDS,
            "repository_continuity_classes": REPOSITORY_CONTINUITY_CLASSES,
            "response_origins": RESPONSE_ORIGINS,
            "schema_version": "maestro.vnext.stage2.authority.literals.v1",
        },
    )
    return literals_id


def action_spec_artifact(root: Path, schema_ids: dict[str, str]) -> str:
    predecessor_catalog_id = PREDECESSOR_CATALOG_IDS[8][2]
    produced_schema_bindings = [
        [name, {"bytes": schema_ids[name]}] for name in PRODUCED_SCHEMA_NAMES
    ]
    canonical = [
        1,
        "IssueBootstrapMandate",
        [9, {"bytes": PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID}],
        [9, {"bytes": predecessor_catalog_id}],
        {"bytes": schema_ids["IssueBootstrapMandateRequestV1"]},
        {"bytes": schema_ids["ActionAuthorityBasisV1"]},
        "BootstrapControlG0",
        PRODUCED_RECORD_CLOSURE,
        produced_schema_bindings,
        ["newly_minted", 1, "converged", 0],
        ACTION_RESULT_OUTCOMES,
        RESPONSE_ORIGINS,
        [
            "same_store_atomic_semantic_point", "zero_external_io", "same_key_replay_zero_write",
            "different_key_convergence_fresh_authorization", "unknown_fields_fail_closed",
        ],
    ]
    envelope = [DOMAIN_ACTION_SPEC, canonical]
    action_spec_id, encoded = identity(envelope)
    write_bytes(root, "action-spec-v2.v1.cbor", encoded)
    write_json(
        root,
        "action-spec-v2.v1.json",
        {
            "action_authority_basis": "BootstrapControlG0",
            "action_spec_id": action_spec_id,
            "byte_length": len(encoded),
            "canonical_value": canonical,
            "identity_envelope": envelope,
            "issuance_binding_cardinality": {
                "converged_existing_mandate": 0,
                "newly_minted_mandate": 1,
            },
            "leaf": "IssueBootstrapMandate",
            "predecessor": {
                "catalog_09_manifest_id": predecessor_catalog_id,
                "catalog_number": 9,
                "catalog_type": "ActionSpecV1",
                "descriptor_id": PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID,
            },
            "produced_record_closure": PRODUCED_RECORD_CLOSURE,
            "produced_schema_bindings": [
                {"schema_name": name, "descriptor_id": schema_ids[name]}
                for name in PRODUCED_SCHEMA_NAMES
            ],
            "publication_state": PUBLICATION_STATE,
            "schema_version": "maestro.vnext.stage2.authority.action-spec-v2.v1",
            "successor_scope": "IssueBootstrapMandate_only",
        },
    )
    return action_spec_id


def continuity_artifact(root: Path, schema_ids: dict[str, str], literals_id: str) -> str:
    canonical = [
        {"bytes": schema_ids["AuthorityContinuityManifestV1"]},
        [7, {"bytes": PREDECESSOR_CATALOG_IDS[6][2]}, REPOSITORY_CONTINUITY_CLASSES],
        [8, {"bytes": PREDECESSOR_CATALOG_IDS[7][2]}, INSTALLATION_CONTINUITY_CLASSES],
        REPOSITORY_CAPACITY_KINDS,
        INSTALLATION_CAPACITY_KINDS,
        CMA_OBSERVATION_PUBLICATION_PURPOSES,
        CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES,
        TRANSITION_GUARD_KINDS,
        {"bytes": literals_id},
        ["old_client_refusal", "candidate_only", "no_runtime_activation", "no_predecessor_rewrite"],
    ]
    envelope = [DOMAIN_CONTINUITY, canonical]
    manifest_id, encoded = identity(envelope)
    write_bytes(root, "authority-continuity-manifest.v1.cbor", encoded)
    write_json(
        root,
        "authority-continuity-manifest.v1.json",
        {
            "byte_length": len(encoded),
            "cma_observation_publication_purposes": CMA_OBSERVATION_PUBLICATION_PURPOSES,
            "cma_effect_withdrawal_slot_families": CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES,
            "transition_guard_kinds": TRANSITION_GUARD_KINDS,
            "identity_envelope": envelope,
            "installation_capacity_kinds": INSTALLATION_CAPACITY_KINDS,
            "installation_classes": INSTALLATION_CONTINUITY_CLASSES,
            "installation_predecessor_catalog_id": PREDECESSOR_CATALOG_IDS[7][2],
            "manifest_id": manifest_id,
            "publication_state": PUBLICATION_STATE,
            "repository_capacity_kinds": REPOSITORY_CAPACITY_KINDS,
            "repository_classes": REPOSITORY_CONTINUITY_CLASSES,
            "repository_predecessor_catalog_id": PREDECESSOR_CATALOG_IDS[6][2],
            "schema_descriptor_id": schema_ids["AuthorityContinuityManifestV1"],
            "schema_version": "maestro.vnext.stage2.authority.continuity-manifest.v1",
        },
    )
    return manifest_id


def artifact_row(root: Path, relative: str) -> dict[str, Any]:
    path = root / relative
    return {"byte_length": path.stat().st_size, "path": relative, "sha256": sha256_file(path)}


def root_artifact(
    root: Path,
    suite_id: str,
    literals_id: str,
    action_spec_id: str,
    continuity_id: str,
) -> str:
    semantic_delta = json.loads(STAGE2_SEMANTIC_DELTA.read_text(encoding="ascii"))
    stage0_tree_sha256 = tracked_stage0_tree_digest()
    canonical = [
        stage0_tree_sha256,
        [[number, kind, {"bytes": value}] for number, kind, value in PREDECESSOR_CATALOG_IDS],
        [
            ["schema_suite", {"bytes": suite_id}],
            ["authority_literals", {"bytes": literals_id}],
            ["action_spec_v2", {"bytes": action_spec_id}],
            ["authority_continuity_manifest", {"bytes": continuity_id}],
            ["stage2_semantic_consumer_delta", semantic_delta["identity"]],
        ],
        PUBLICATION_STATE,
    ]
    envelope = [DOMAIN_ROOT, canonical]
    root_id, encoded = identity(envelope)
    write_bytes(root, "stage2-authority-manifest.v1.cbor", encoded)
    primary_paths = [
        "action-spec-v2.v1.cbor", "action-spec-v2.v1.json",
        "authority-continuity-manifest.v1.cbor", "authority-continuity-manifest.v1.json",
        "authority-literals.v1.cbor", "authority-literals.v1.json",
        "schema-descriptors.v1.cbor", "schema-descriptors.v1.json",
    ] + [f"descriptors/{slug(name)}.cbor" for name, _, _, _ in SCHEMA_DEFINITIONS]
    write_json(
        root,
        "stage2-authority-manifest.v1.json",
        {
            "artifacts": [artifact_row(root, path) for path in sorted(primary_paths)],
            "byte_length": len(encoded),
            "component_ids": {
                "action_spec_v2": action_spec_id,
                "authority_continuity_manifest": continuity_id,
                "authority_literals": literals_id,
                "schema_suite": suite_id,
                "stage2_semantic_consumer_delta": semantic_delta["identity"],
            },
            "identity_envelope": envelope,
            "predecessor_action_spec_descriptor_id": PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID,
            "predecessor_catalog_ids": [
                {"catalog_number": number, "catalog_type": kind, "manifest_id": value}
                for number, kind, value in PREDECESSOR_CATALOG_IDS
            ],
            "publication_state": PUBLICATION_STATE,
            "root_id": root_id,
            "schema_version": "maestro.vnext.stage2.authority.root-manifest.v1",
            "stage": "stage2_authority_candidate",
            "stage0_tree_sha256": stage0_tree_sha256,
            "stage2_semantic_consumer_delta": {
                "identity": semantic_delta["identity"],
                "consumer_count": semantic_delta["consumer_count"],
                "consumer_digest": semantic_delta["consumer_digest"],
                "predecessor": semantic_delta["predecessor"],
            },
        },
    )
    return root_id


def run_receipt(command: list[str]) -> dict[str, Any]:
    process = subprocess.run(command, cwd=WORKSPACE, capture_output=True, text=True, check=False)
    if process.returncode != 0:
        detail = process.stderr.strip() or process.stdout.strip() or "no diagnostic"
        raise BuildError(f"receipt producer failed ({' '.join(command)}): {detail}")
    try:
        value = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise BuildError(f"receipt producer emitted invalid JSON: {' '.join(command)}") from error
    if not isinstance(value, dict):
        raise BuildError("receipt producer must emit one JSON object")
    return value


def build_to(root: Path) -> None:
    verify_frozen_inputs()
    root.mkdir(parents=True, exist_ok=True)
    schema_ids, suite_id = schema_artifacts(root)
    literals_id = literals_artifact(root)
    action_spec_id = action_spec_artifact(root, schema_ids)
    continuity_id = continuity_artifact(root, schema_ids, literals_id)
    root_id = root_artifact(root, suite_id, literals_id, action_spec_id, continuity_id)
    cbor_files = sorted(path.relative_to(root).as_posix() for path in root.rglob("*.cbor"))
    write_json(
        root,
        "python-encoder-receipt.v1.json",
        {
            "artifacts": [artifact_row(root, path) for path in cbor_files],
            "encoder": "python_stdlib_existing_cbor_py",
            "publication_state": PUBLICATION_STATE,
            "result": "canonical_cbor_emitted",
            "root_id": root_id,
            "schema_version": "maestro.vnext.stage2.authority.python-encoder-receipt.v1",
        },
    )
    ruby_receipt = run_receipt(["ruby", str(TOOLS / "verify.rb"), "--root", str(root), "--emit"])
    write_json(root, "ruby-verification-receipt.v1.json", ruby_receipt)
    validation_receipt = run_receipt(
        [sys.executable, str(TOOLS / "validate.py"), "--root", str(root), "--mutants", "--emit"]
    )
    write_json(root, "semantic-validation-receipt.v1.json", validation_receipt)


def compare_trees(expected: Path, actual: Path) -> None:
    expected_paths = sorted(path.relative_to(expected).as_posix() for path in expected.rglob("*") if path.is_file())
    actual_paths = sorted(path.relative_to(actual).as_posix() for path in actual.rglob("*") if path.is_file())
    if expected_paths != actual_paths:
        raise BuildError("Stage 2 Authority output file set is stale")
    for relative in expected_paths:
        if (expected / relative).read_bytes() != (actual / relative).read_bytes():
            raise BuildError(f"Stage 2 Authority artifact is stale: {relative}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--source-only", action="store_true")
    parser.add_argument("--self-test-source-only", action="store_true")
    args = parser.parse_args()
    try:
        if args.source_only:
            verify_governance_floor_source()
            print("Stage 2 Authority builder source semantics valid")
            return 0
        if args.self_test_source_only:
            self_test_governance_floor_source()
            print("Stage 2 Authority builder source-only mutants rejected")
            return 0
        with tempfile.TemporaryDirectory(prefix="maestro-stage2-authority-") as temporary:
            generated = Path(temporary) / "authority"
            build_to(generated)
            if args.check:
                compare_trees(OUTPUT, generated)
            else:
                if OUTPUT.exists():
                    shutil.rmtree(OUTPUT)
                shutil.copytree(generated, OUTPUT)
    except (BuildError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"Stage 2 Authority build failed: {error}", file=sys.stderr)
        return 1
    print("Stage 2 Authority contracts are reproducible" if args.check else "Stage 2 Authority contracts generated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
