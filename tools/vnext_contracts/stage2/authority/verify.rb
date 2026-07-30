#!/usr/bin/env ruby
# Independently reconstruct every Stage 2 Authority semantic projection and CBOR identity.

require "digest"
require "json"
require "open3"
require "optparse"

U64_MAX = 0xffffffffffffffff
TOOLS = File.expand_path(__dir__)
WORKSPACE = File.expand_path("../../../..", TOOLS)
DEFAULT_ROOT = File.join(WORKSPACE, "contracts/vnext/stage2/authority")
STAGE2_DELTA_PATH = File.join(
  WORKSPACE, "contracts/vnext/stage0/effect-home/stage2-semantic-consumer-delta-v1.json"
)
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
].freeze

AUTHORITY_CONTEXTS = %w[RepositoryAuthorityContext InstallationAuthorityContext].freeze
ACTION_AUTHORITY_BASES = %w[OrdinaryLiveRuntime BootstrapControlG0 ContinuityMaintenance].freeze
ACTION_RESULT_OUTCOMES = %w[committed no_op rejected stale conflict unavailable in_doubt].freeze
RESPONSE_ORIGINS = %w[fresh replay].freeze
REPOSITORY_CAPACITY_KINDS = %w[
  RepositoryOrdinaryMutation RepositoryAuthorityAdministration RepositoryEvidenceAcquisition
  RepositoryPlanningPublication RepositoryExternalEffect RepositoryPersistenceMaintenance
].freeze
INSTALLATION_CAPACITY_KINDS = %w[
  InstallationAuthorityAdministration InstallationDistributionMutation InstallationGovernedReviewPublication
  InstallationExternalEffect InstallationWriterAdministration InstallationPersistenceMaintenance
].freeze
CMA_OBSERVATION_PUBLICATION_PURPOSES = %w[
  TrustedTimeAcquisition RecoveryExternalRegistration RecoveryExternalStatus
  MaintenanceExecutorCurrentness ProspectiveContinuityCarrier
].freeze
CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES = %w[
  MaintenanceExecutorCurrentness ProspectiveContinuityCarrier PlannedTurnoverHighWater
  RepositoryRecoveryAdmission InstallationRecoveryAdmission
].freeze
TRANSITION_GUARD_KINDS = %w[
  RepositoryWorkAuthorityPolicyTransition RepositoryFirstWorkPublication
  RepositoryFloorOrTrustRootRotation InstallationPolicyBindingReplacement
  InstallationStructuralRootFloorReplacement TrustedTimePolicyStackRotation
  ExternalLogicalCarrierProfileRotation PlannedEpochTurnoverPreparation
].freeze
REPOSITORY_CONTINUITY_CLASSES = %w[
  RepositoryOrdinaryMutationCapacityState RepositoryAuthorityAdministrationCapacityState
  RepositoryEvidenceAcquisitionCapacityState RepositoryPlanningPublicationCapacityState
  RepositoryExternalEffectCapacityState RepositoryPersistenceMaintenanceCapacityState
  RepositoryStoreGenerationCurrentness RepositoryGovernanceHead RepositoryAuthorityEpochState
  RepositoryTrustRootState RepositoryPrincipalBindingState RepositorySessionState RepositoryGrantState
  RepositoryDelegationState RepositoryMandateState RepositoryRevocationState
  RepositoryAuthorizationReceiptState RepositoryConsumptionCellState RepositoryContinuityState
  RepositoryTrustedTimeState RepositoryRecoveryCommitmentState RepositoryRecoveryAdmissionState
  RepositoryStepExecutionState RepositoryEffectIntentState RepositoryEvidenceState RepositoryGateSnapshot
  RepositoryPlanningState RepositoryCoordinationState RepositoryDesignDecisionState RepositoryContractState
  RepositoryWorkState RepositoryPersistenceRetentionState RepositoryMemoryState RepositoryIntakeState
  RepositoryResearchState
].freeze
INSTALLATION_CONTINUITY_CLASSES = %w[
  InstallationAuthorityAdministrationCapacityState InstallationDistributionMutationCapacityState
  InstallationGovernedReviewPublicationCapacityState InstallationExternalEffectCapacityState
  InstallationWriterAdministrationCapacityState InstallationPersistenceMaintenanceCapacityState
  InstallationLocatorCurrentness InstallationStoreGenerationCurrentness InstallationGovernanceHead
  InstallationAuthorityEpochState InstallationTrustRootState InstallationPrincipalBindingState
  InstallationGrantState InstallationMandateState InstallationRevocationState
  InstallationAuthorizationReceiptState InstallationConsumptionCellState InstallationContinuityState
  InstallationRecoveryCommitmentState InstallationRecoveryAdmissionState InstallationWriterCohortState
  InstallationClientCompatibilityState InstallationDistributionTargetState
  InstallationDistributionTransactionState InstallationBinarySlotState InstallationResourceManifestState
  InstallationGovernedReviewPublicationState InstallationEffectIntentState InstallationEvidenceState
  InstallationPersistenceRetentionState
].freeze
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
].freeze
PRODUCED_RECORD_CLOSURE = [
  "AuthorityMandateOrConvergenceRefV1",
  "BootstrapMandateIssuanceBindingV1:exactly_one_if_newly_minted",
  "AuthorizationReceiptV1:primary", "ActionResultV1", "IdempotencyRecordV1",
  "AuthorityContinuityClosureV1:pre_cut_exact",
  "SuccessVisibleAuthorityContinuityStateV1:exactly_one",
  "AdmittedTransitionGuardV1:persisted_from_serialization_current_owner_facts",
  "BootstrapAuthoritySnapshotV1:successor_current_authority_carrier",
  "LinearizationCoverageWitnessV1:recoverable",
  "AuthorityContinuityPostCutConsequenceSetV1:complete_exact",
].freeze
PRODUCED_SCHEMA_NAMES = %w[
  AuthorityMandateV1 BootstrapMandateIssuanceBindingV1 AuthorizationReceiptV1 ActionResultV1 BootstrapAuthoritySnapshotV1
  AuthorityContinuityClosureV1
  SuccessVisibleAuthorityContinuityStateV1 AdmittedTransitionGuardV1
  LinearizationCoverageWitnessV1 AuthorityContinuityPostCutConsequenceSetV1
].freeze

SCHEMA_DEFINITIONS = [
  ["AuthorityMandateV1", %w[authority_context_ref action subject action_revision consent_slot_binding_parameter responder_binding_ref responder_assurance_revision interaction_closure_ref authority_basis_commitment_ref valid_from_inclusive valid_until_exclusive maximum_uses delegation_depth_remaining], [], %w[maximum_uses_exactly_one delegation_forbidden same_context_only immutable]],
  ["BootstrapMandateIssuanceBindingV1", %w[mandate_ref target_action_commitment consent_slot_commitment interaction_closure_ref], [], %w[exists_exactly_once_only_for_new_mandate absent_for_convergence immutable]],
  ["AuthorizationReceiptV1", %w[receipt_id authority_context_ref action_request_ref authority_basis_ref authorization_decision primary action_result_ref], %w[authorized denied], %w[primary_exactly_one non_bearer retrospective_only immutable]],
  ["ActionResultV1", %w[result_id action_request_ref outcome response_origin before_refs after_refs receipt_refs produced_refs effect_attempt_refs safe_reason_code next_or_inspect_ref], ACTION_RESULT_OUTCOMES, %w[response_origin_not_outcome replay_returns_original_result immutable]],
  ["IssueBootstrapMandateRequestV1", %w[request_id authority_context_ref actor_binding_ref actor_session_ref responder_binding_ref presentation_observation_ref affirmative_response_observation_ref target target_subject target_revision consent_slot_binding_parameter idempotency_key supplied_mandates], [], %w[supplied_mandates_exactly_zero self_reference_free no_external_io]],
  ["ConsentSlotBindingParameterV1", %w[slot_protocol_commitment target_action_commitment slot_commitment], [], %w[fixed_before_issuance self_reference_free non_authorizing]],
  ["ActionAuthorityBasisV1", %w[basis_tag authority_context_ref basis_commitment], ACTION_AUTHORITY_BASES, %w[leaf_selects_exactly_one no_ranking no_fallback no_cross_donation]],
  ["AuthorityContextV1", %w[context_tag context_id stable_domain_identity protected_realm_if_installation store_generation authority_epoch trust_root_revision locator_binding_revision_if_installation], AUTHORITY_CONTEXTS, %w[exactly_one_domain no_cross_store_authority unknown_variant_refused]],
  ["GovernedCapacityDebitV1", %w[capacity_root_ref authority_context_kind authority_context_ref capacity_kind ordinal prior_spent resulting_spent], REPOSITORY_CAPACITY_KINDS + INSTALLATION_CAPACITY_KINDS, %w[quantity_exactly_one fresh_committed_only replay_zero_debit same_domain_only]],
  ["AuthorityContinuityManifestV1", %w[authority_context_kind protocol_version canonicalization_version obligations dispositions owner_contributions class_ids class_descriptors], AUTHORITY_CONTEXTS, %w[closed_class_set old_client_refusal candidate_only immutable]],
  ["PrincipalBindingV1", %w[binding_id principal_id authority_context_ref trust_root_revision assurance_revision validity human_capable], [], %w[nonzero_protocol_revisions finite_half_open_validity same_context_only immutable]],
  ["SessionV1", %w[session_id principal_binding_ref authority_context_ref store_generation authority_epoch request_commitment validity], [], %w[nonempty_bounded_ascii_commitment binding_context_generation_epoch_exact immutable]],
  ["BootstrapGenesisGrantV1", %w[grant_id authority_context_ref grantee_principal_ref authority_epoch trust_root_revision local_capacity_constraint terminal_scope delegable_scope valid_from_inclusive valid_until_exclusive], %w[NoLocalBoundedRoot], %w[exactly_one_structural_g0 terminal_scope_bootstrap_control delegable_scope_ordinary_bounded terminal_and_delegable_scopes_disjoint bootstrap_control_nondelegable inert_before_context_genesis_activation]],
  ["BootstrapMandateInteractionObservationJoinV1", %w[interaction_closure_id authority_context_ref responder_binding_ref responder_current_authentication_ref presentation_observation_ref affirmative_response_observation_ref carrier_procedure_ref target_action_commitment], [], %w[presentation_and_affirmative_response_both_required same_authenticated_responder same_context_and_target observational_non_authorizing immutable]],
  ["RevocationSetV1", %w[authority_context_ref revocation_targets], %w[TrustRoot PrincipalBinding Session Grant Mandate], %w[finite_bounded_set same_context_only revoked_authority_never_revives immutable]],
  ["BootstrapAuthoritySnapshotV1", %w[authority_context authority_snapshot actor_binding actor_session responder_binding responder_session bootstrap_g0_candidate_paths revocations interaction_join current_carrier_procedure_ref target_action_projection current_target_head consent_slot_evaluation_facts continuity_transition_proof], [], %w[complete_store_loaded_evaluator_facts exact_current_generation fresh_authorization_only ambient_time_forbidden non_bearer]],
  ["GovernedCapacityRootV1", %w[capacity_root_id authority_context_kind authority_context_ref capacity_kind initial_maximum spent], REPOSITORY_CAPACITY_KINDS + INSTALLATION_CAPACITY_KINDS, %w[nonzero_bounded_initial_maximum spent_not_above_initial_maximum same_domain_only immutable_revision]],
  ["SuccessVisibleAuthorityContinuityStateV1", %w[protocol_version state_token_ref predecessor_state_token_ref authority_context_kind authority_context_ref store_generation store_publication_clock authority_epoch manifest_id closure_id store_allocation_commitment_ref guard_kind carrier_profile_status selected_trusted_time_stack_ref accepted_authority_time_floor lane_state_closure_root source_floor_root gap_companion_refs floor_provenance_refs external_revision_cell_refs cma_remaining_root cma_spent_root unresolved_effect_refs cut_sequence guard_admission_digest], [], %w[exactly_one_per_success sole_current_continuity_relation no_component_heads same_store_visible]],
  ["AdmittedTransitionGuardV1", %w[protocol_version guard_kind authority_context_kind authority_context_ref store_generation authority_epoch manifest_id closure_id predecessor_state_token_ref cut_sequence selected_trusted_time_stack_ref carrier_profile_status accepted_authority_time_floor lane_state_closure_root source_floor_root gap_companion_refs floor_provenance_refs external_revision_cell_refs cma_remaining_root cma_spent_root unresolved_effect_refs owner_term_facts owner_census_commitment owner_census_source_cut_commitment disclosure], TRANSITION_GUARD_KINDS, %w[fixed_nominal_owner_terms owner_facts_current_at_serialization bounded_fail_closed persisted_non_bearer same_store_visible]],
  ["LinearizationCoverageWitnessV1", %w[fence_subject_ref fence_carrier attempt_ref semantic_point_ref covered_closure_ref conservative_point_envelope_ref carrier_revision_ref], %w[SameStoreCommit ProtectedLocatorCAS ProtectedRepositoryGenerationCAS ProtectedSnapshot], %w[recoverable_exact_attempt_coverage non_aba missing_witness_blocks_success non_authorizing]],
  ["AuthorityContinuityPostCutConsequenceSetV1", %w[authority_continuity_closure_ref closure_id successor_state_token_ref action_request_commitment success_visible_continuity_state_ref selected_authority_consumption_refs phase_owned_semantic_mutation_ref primary_authorization_receipt_ref action_result_ref active_idempotency_mapping_ref linearization_coverage_witness_ref context_current_continuity_relation_ref], [], %w[static_profile_exact_complete_set same_store_atomic_visibility post_cut_facts_outside_own_closure_id no_runtime_optional_members no_separate_component_heads no_postcommit_repair]],
  ["AuthorityContinuityClosureV1", %w[protocol_version manifest_id authority_context_kind authority_context_ref predecessor store_successor_allocation semantic_cut class_entries graph_edges], %w[ContextGenesisPredecessor PriorClosurePredecessor], %w[fresh_store_allocated_non_aba_successor_token complete_typed_class_and_facet_closure finite_graph_endpoint_totality content_addressed_closure_id post_cut_consequences_excluded_from_own_closure_id immutable]],
].freeze

DOMAIN_SCHEMA_DESCRIPTOR = "maestro.vnext.stage2.authority.schema-descriptor.v1"
DOMAIN_SCHEMA_SUITE = "maestro.vnext.stage2.authority.schema-suite.v1"
DOMAIN_LITERALS = "maestro.vnext.stage2.authority.literals.v1"
DOMAIN_ACTION_SPEC = "maestro.vnext.stage2.authority.action-spec-v2.v1"
DOMAIN_CONTINUITY = "maestro.vnext.stage2.authority.continuity-manifest.v1"
DOMAIN_ROOT = "maestro.vnext.stage2.authority.root.v1"
STAGE0_EFFECT_HOME_DOMAIN = "maestro.vnext.stage0.effect-home.v1"
STAGE2_SEMANTIC_LITERAL_PATTERNS = [
  "EffectIntent", "EffectOrigin", "DispatchAttempt", "ReconciliationAttempt", "RemoteClassification",
  "EffectWithdrawal", "WithdrawEffectIntent", "RecoverReserved", "ControlHead", "ControlRevision", "WriterTerm",
  "PublishBootstrapMandateInteractionOutcome", *CMA_OBSERVATION_PUBLICATION_PURPOSES,
  *CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES, *TRANSITION_GUARD_KINDS,
  "RepositoryGovernanceFloorSnapshotV1",
  "maestro.vnext.repository-governance-floor-snapshot.v1",
  "maestro.vnext.repository-governance-head-class-8.v1",
].uniq.freeze
STAGE2_SEMANTIC_SOURCE_DECLARATIONS = {
  "src/domain/vnext/authority/action_basis.rs" => ["Authority", "candidate_contract_definition", "exact_stage4_execution_basis_partition"],
  "src/domain/vnext/authority/bootstrap_catalog.rs" => ["Authority", "candidate_contract_definition", "exact_stage2_bootstrap_target_literal"],
  "src/domain/vnext/authority/capacity.rs" => ["Authority", "candidate_contract_definition", "exact_stage2_capacity_literal"],
  "src/domain/vnext/authority/closed.rs" => ["Authority", "candidate_contract_definition", "exact_stage2_closed_sum_literal"],
  "src/domain/vnext/authority/continuity/catalog.rs" => ["Authority", "candidate_contract_definition", "exact_stage2_continuity_effect_intent_class_literal"],
  "src/domain/vnext/authority/continuity/totality.rs" => ["Authority", "candidate_contract_definition", "exact_stage2_continuity_owner_census_literal"],
  "src/domain/vnext/authority/governance_floor.rs" => ["Authority", "candidate_contract_definition", "exact_internal_append_only_authority_schema_tag_25"],
  "src/domain/vnext/authority/mod.rs" => ["Authority", "candidate_contract_definition", "exact_stage2_authority_facade_literal"],
  "src/domain/vnext/authority/publication.rs" => ["Authority", "candidate_contract_definition", "exact_internal_authority_schema_registry_prefix_and_tag_25"],
  "src/domain/vnext/authority/facade/repository_admission.rs" => ["Authority", "candidate_contract_definition", "exact_stage4_execution_authority_admission"],
  "src/domain/vnext/authority/facade/repository_leaf_authority.rs" => ["Authority", "candidate_contract_definition", "exact_stage4_execution_authority_closed_union"],
  "src/domain/vnext/authority/transition.rs" => ["Authority", "candidate_contract_definition", "exact_stage2_transition_guard_literal"],
  "tests/vnext_authority_capacity_transition.rs" => ["Stage2Proof", "candidate_proof_reader", "exact_stage2_capacity_and_transition_proof"],
  "tests/vnext_authority_contracts.rs" => ["Stage2Proof", "candidate_proof_reader", "exact_stage2_authority_contract_proof"],
  "tests/vnext_authority_continuity_totality.rs" => ["Stage2Proof", "candidate_proof_reader", "exact_stage2_continuity_totality_proof"],
  "tests/vnext_authority_literals.rs" => ["Stage2Proof", "candidate_proof_reader", "exact_stage2_literal_artifact_proof"],
  "tools/vnext_contracts/stage2/authority/build.py" => ["Stage2Authority", "candidate_contract_definition", "exact_stage2_authority_builder_semantics"],
  "tools/vnext_contracts/stage2/authority/validate.py" => ["Stage2Proof", "candidate_proof_reader", "independent_stage2_semantic_reconstruction"],
  "tools/vnext_contracts/stage2/authority/verify.rb" => ["Stage2Proof", "candidate_proof_reader", "independent_stage2_ruby_reconstruction"],
}.freeze
STAGE2_REQUIRED_LITERALS_BY_SOURCE = {
  "src/domain/vnext/authority/governance_floor.rs" => [
    "RepositoryGovernanceFloorSnapshotV1",
    "maestro.vnext.repository-governance-floor-snapshot.v1",
    "maestro.vnext.repository-governance-head-class-8.v1",
  ].freeze,
}.freeze
GOVERNANCE_FLOOR_REQUIRED_SOURCE_FRAGMENTS = [
  "pub(super) struct RepositoryGovernanceFloorSnapshotV1 {",
  "let snapshot = RepositoryGovernanceFloorSnapshotV1::decode_object(direct_object)?;",
  [
    "let history = validate_history(*direct_root, &by_id)?;",
    "let class_root = hash_value(&CborValue::Array(vec![",
    'CborValue::text("maestro.vnext.repository-governance-head-class-8.v1")?,',
  ].join(" "),
  [
    "let commitment = current_view_commitment(",
    "view, head, generation, &snapshot, *direct_root, class_root,",
  ].join(" "),
].freeze
GOVERNANCE_FLOOR_SOURCE_MUTANTS = [
  [
    "pub(super) struct RepositoryGovernanceFloorSnapshotV1 {",
    "pub(super) struct RepositoryGovernanceFloorSnapshotMutantV1 {",
  ],
  ["decode_object(direct_object)?", "decode_object(mutant_object)?"],
  [
    "maestro.vnext.repository-governance-head-class-8.v1",
    "maestro.vnext.repository-governance-head-class-mutant.v1",
  ],
  ["class_root,\n        authority,", "[0; 32],\n        authority,"],
].freeze

class SourceSemanticError < StandardError; end

def head(major, value)
  raise "canonical integers and lengths are unsigned u64" unless value.is_a?(Integer) && value.between?(0, U64_MAX)
  return [(major << 5) | value].pack("C") if value < 24
  return [(major << 5) | 24, value].pack("CC") if value <= 0xff
  return [(major << 5) | 25, value].pack("Cn") if value <= 0xffff
  return [(major << 5) | 26, value].pack("CN") if value <= 0xffffffff

  [(major << 5) | 27, value].pack("CQ>")
end

def encode(value)
  case value
  when false then "\xf4".b
  when true then "\xf5".b
  when Integer then head(0, value)
  when String
    raw = value.encode(Encoding::US_ASCII).b
    head(3, raw.bytesize) + raw
  when Array then head(4, value.length) + value.map { |item| encode(item) }.join
  when Hash
    raise "only canonical raw-byte wrappers are allowed" unless value.keys == ["bytes"]
    hexadecimal = value.fetch("bytes")
    raise "raw bytes must be lowercase hexadecimal" unless hexadecimal.match?(/\A[0-9a-f]*\z/) && hexadecimal.length.even?
    raw = [hexadecimal].pack("H*")
    head(2, raw.bytesize) + raw
  else raise "unsupported canonical value: #{value.inspect}"
  end
end

def identity(envelope)
  encoded = encode(envelope)
  [Digest::SHA256.hexdigest(encoded), encoded]
end

def load_json(root, relative)
  value = JSON.parse(File.read(File.join(root, relative), encoding: Encoding::US_ASCII))
  raise "#{relative} must contain one object" unless value.is_a?(Hash)
  value
end

def slug(name)
  name.gsub(/([A-Z])/, '-\\1').sub(/\A-/, "").downcase
end

def tracked_stage0_tree_digest
  stdout, _stderr, status = Open3.capture3(
    "git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--",
    "contracts/vnext/stage0", "tools/vnext_contracts/stage0", chdir: WORKSPACE
  )
  raise "cannot enumerate Stage 0 tree" unless status.success?
  digest = Digest::SHA256.new
  stdout.split("\0").reject(&:empty?).reject do |relative|
    relative.end_with?(".pyc") || relative.include?("/__pycache__/")
  end.sort.each do |relative|
    path_bytes = relative.encode(Encoding::UTF_8).b
    data = File.binread(File.join(WORKSPACE, relative))
    digest.update([path_bytes.bytesize].pack("Q>"))
    digest.update(path_bytes)
    digest.update([data.bytesize].pack("Q>"))
    digest.update(data)
  end
  digest.hexdigest
end

def semantic_source_rows(source_overrides = {})
  raise "Stage 2 semantic source override is undeclared" unless (source_overrides.keys - STAGE2_SEMANTIC_SOURCE_DECLARATIONS.keys).empty?
  STAGE2_SEMANTIC_SOURCE_DECLARATIONS.sort.map do |path, (owner, disposition, proof)|
    bytes = source_overrides.fetch(path) { File.binread(File.join(WORKSPACE, path)) }
    contents = bytes.force_encoding(Encoding::UTF_8).scrub
    matched = STAGE2_SEMANTIC_LITERAL_PATTERNS.select { |literal| contents.include?(literal) }
    raise "Stage 2 semantic consumer has no literal: #{path}" if matched.empty?
    missing = STAGE2_REQUIRED_LITERALS_BY_SOURCE.fetch(path, []).reject { |literal| contents.include?(literal) }
    unless missing.empty?
      raise SourceSemanticError, "Stage 2 semantic consumer is missing exact literals: #{path}: #{missing.join(', ')}"
    end
    if path == "src/domain/vnext/authority/governance_floor.rs"
      normalized = contents.split.join(" ")
      unless GOVERNANCE_FLOOR_REQUIRED_SOURCE_FRAGMENTS.all? { |fragment| normalized.include?(fragment) }
        raise SourceSemanticError, "Stage 2 governance-floor source is missing causal persistence/current-head binding"
      end
    end
    digest = Digest::SHA256.hexdigest(bytes)
    {
      "path" => path, "resource_identity" => "sha256:#{digest}", "worktree_sha256" => digest,
      "matched_literals" => matched, "owner" => owner, "consumer_disposition" => disposition,
      "proof" => proof,
    }
  end
end

def semantic_source_identity(rows)
  canonical_rows = rows.map do |row|
    [
      row.fetch("path"),
      row.fetch("resource_identity"),
      row.fetch("worktree_sha256"),
      row.fetch("matched_literals"),
      row.fetch("owner"),
      row.fetch("consumer_disposition"),
      row.fetch("proof"),
    ]
  end
  digest, = identity(["maestro.vnext.stage2.authority.semantic-source-closure.v1", canonical_rows])
  "sha256:#{digest}"
end

def self_test_semantic_sources
  semantic_source_rows
  path = "src/domain/vnext/authority/governance_floor.rs"
  source = File.binread(File.join(WORKSPACE, path))
  GOVERNANCE_FLOOR_SOURCE_MUTANTS.each do |target, replacement|
    raise "governance-floor mutant target is absent: #{target.inspect}" unless source.include?(target)
    mutated = source.gsub(target, replacement)
    begin
      semantic_source_rows(path => mutated)
    rescue SourceSemanticError
      next
    end
    raise "governance-floor causal mutant was accepted: #{target.inspect}"
  end
end

def semantic_delta
  delta = JSON.parse(File.read(STAGE2_DELTA_PATH, encoding: Encoding::US_ASCII))
  rows = semantic_source_rows
  raise "Stage 2 semantic-consumer delta rows drifted" unless delta.fetch("consumer_rows") == rows
  delta_id, = identity([STAGE0_EFFECT_HOME_DOMAIN, delta.fetch("canonical_value")])
  raise "Stage 2 semantic-consumer delta identity drifted" unless delta.fetch("identity") == "sha256:#{delta_id}"
  raise "Stage 2 semantic-consumer delta count drifted" unless delta.fetch("consumer_count") == rows.length
  raise "Stage 2 semantic-consumer delta is incomplete" unless delta.fetch("closure_status") == "complete_exact_source_overlay"
  delta
end

def tagged(values)
  values.each_with_index.map { |value, index| [index + 1, value] }
end

def expected_documents(root)
  stage0_tree_sha256 = tracked_stage0_tree_digest
  schema_ids = {}
  descriptor_paths = []
  descriptors = SCHEMA_DEFINITIONS.each_with_index.map do |(name, fields, variants, invariants), index|
    tag = index + 1
    canonical = [tag, name, fields, tagged(variants), invariants]
    envelope = [DOMAIN_SCHEMA_DESCRIPTOR, canonical]
    descriptor_id, encoded = identity(envelope)
    cbor_path = "descriptors/#{slug(name)}.cbor"
    raise "descriptor CBOR drifted: #{cbor_path}" unless File.binread(File.join(root, cbor_path)) == encoded
    schema_ids[name] = descriptor_id
    descriptor_paths << cbor_path
    {
      "byte_length" => encoded.bytesize, "canonical_value" => canonical, "cbor_path" => cbor_path,
      "descriptor_id" => descriptor_id, "fields" => fields, "identity_envelope" => envelope,
      "invariants" => invariants, "schema_name" => name, "tag" => tag, "variants" => variants,
    }
  end
  suite_value = descriptors.map { |row| [row.fetch("tag"), row.fetch("schema_name"), { "bytes" => row.fetch("descriptor_id") }] }
  suite_envelope = [DOMAIN_SCHEMA_SUITE, suite_value]
  suite_id, suite_cbor = identity(suite_envelope)
  schemas = {
    "byte_length" => suite_cbor.bytesize, "descriptor_count" => descriptors.length,
    "descriptors" => descriptors, "identity_envelope" => suite_envelope,
    "publication_state" => PUBLICATION_STATE,
    "schema_version" => "maestro.vnext.stage2.authority.schema-descriptors.v1", "suite_id" => suite_id,
  }

  literals_value = [
    tagged(AUTHORITY_CONTEXTS), tagged(ACTION_AUTHORITY_BASES), tagged(ACTION_RESULT_OUTCOMES),
    tagged(RESPONSE_ORIGINS), tagged(REPOSITORY_CAPACITY_KINDS), tagged(INSTALLATION_CAPACITY_KINDS),
    tagged(CMA_OBSERVATION_PUBLICATION_PURPOSES), tagged(CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES),
    tagged(TRANSITION_GUARD_KINDS), tagged(REPOSITORY_CONTINUITY_CLASSES),
    tagged(INSTALLATION_CONTINUITY_CLASSES), BOOTSTRAP_TARGET_ROWS,
  ]
  literals_envelope = [DOMAIN_LITERALS, literals_value]
  literals_id, literals_cbor = identity(literals_envelope)
  literals = {
    "action_authority_bases" => ACTION_AUTHORITY_BASES, "action_result_outcomes" => ACTION_RESULT_OUTCOMES,
    "authority_contexts" => AUTHORITY_CONTEXTS,
    "bootstrap_target_rows" => BOOTSTRAP_TARGET_ROWS.map { |tag, leaf, disposition, reason| { "disposition" => disposition, "leaf" => leaf, "reason" => reason, "tag" => tag } },
    "byte_length" => literals_cbor.bytesize, "canonical_value" => literals_value,
    "cma_observation_publication_purposes" => CMA_OBSERVATION_PUBLICATION_PURPOSES,
    "cma_effect_withdrawal_slot_families" => CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES,
    "transition_guard_kinds" => TRANSITION_GUARD_KINDS, "identity_envelope" => literals_envelope,
    "installation_capacity_kinds" => INSTALLATION_CAPACITY_KINDS,
    "installation_continuity_classes" => INSTALLATION_CONTINUITY_CLASSES,
    "literals_id" => literals_id, "publication_state" => PUBLICATION_STATE,
    "repository_capacity_kinds" => REPOSITORY_CAPACITY_KINDS,
    "repository_continuity_classes" => REPOSITORY_CONTINUITY_CLASSES,
    "response_origins" => RESPONSE_ORIGINS,
    "schema_version" => "maestro.vnext.stage2.authority.literals.v1",
  }

  predecessor_catalog_id = PREDECESSOR_CATALOG_IDS.fetch(8).fetch(2)
  produced_schema_bindings = PRODUCED_SCHEMA_NAMES.map { |name| [name, { "bytes" => schema_ids.fetch(name) }] }
  action_value = [
    1, "IssueBootstrapMandate", [9, { "bytes" => PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID }],
    [9, { "bytes" => predecessor_catalog_id }], { "bytes" => schema_ids.fetch("IssueBootstrapMandateRequestV1") },
    { "bytes" => schema_ids.fetch("ActionAuthorityBasisV1") }, "BootstrapControlG0",
    PRODUCED_RECORD_CLOSURE, produced_schema_bindings, ["newly_minted", 1, "converged", 0], ACTION_RESULT_OUTCOMES,
    RESPONSE_ORIGINS, %w[same_store_atomic_semantic_point zero_external_io same_key_replay_zero_write different_key_convergence_fresh_authorization unknown_fields_fail_closed],
  ]
  action_envelope = [DOMAIN_ACTION_SPEC, action_value]
  action_id, action_cbor = identity(action_envelope)
  action = {
    "action_authority_basis" => "BootstrapControlG0", "action_spec_id" => action_id,
    "byte_length" => action_cbor.bytesize, "canonical_value" => action_value,
    "identity_envelope" => action_envelope,
    "issuance_binding_cardinality" => { "converged_existing_mandate" => 0, "newly_minted_mandate" => 1 },
    "leaf" => "IssueBootstrapMandate",
    "predecessor" => { "catalog_09_manifest_id" => predecessor_catalog_id, "catalog_number" => 9,
                       "catalog_type" => "ActionSpecV1", "descriptor_id" => PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID },
    "produced_record_closure" => PRODUCED_RECORD_CLOSURE, "publication_state" => PUBLICATION_STATE,
    "produced_schema_bindings" => PRODUCED_SCHEMA_NAMES.map { |name| { "schema_name" => name, "descriptor_id" => schema_ids.fetch(name) } },
    "schema_version" => "maestro.vnext.stage2.authority.action-spec-v2.v1",
    "successor_scope" => "IssueBootstrapMandate_only",
  }

  continuity_value = [
    { "bytes" => schema_ids.fetch("AuthorityContinuityManifestV1") },
    [7, { "bytes" => PREDECESSOR_CATALOG_IDS.fetch(6).fetch(2) }, REPOSITORY_CONTINUITY_CLASSES],
    [8, { "bytes" => PREDECESSOR_CATALOG_IDS.fetch(7).fetch(2) }, INSTALLATION_CONTINUITY_CLASSES],
    REPOSITORY_CAPACITY_KINDS, INSTALLATION_CAPACITY_KINDS, CMA_OBSERVATION_PUBLICATION_PURPOSES,
    CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES, TRANSITION_GUARD_KINDS, { "bytes" => literals_id },
    %w[old_client_refusal candidate_only no_runtime_activation no_predecessor_rewrite],
  ]
  continuity_envelope = [DOMAIN_CONTINUITY, continuity_value]
  continuity_id, continuity_cbor = identity(continuity_envelope)
  continuity = {
    "byte_length" => continuity_cbor.bytesize,
    "cma_observation_publication_purposes" => CMA_OBSERVATION_PUBLICATION_PURPOSES,
    "cma_effect_withdrawal_slot_families" => CMA_EFFECT_WITHDRAWAL_SLOT_FAMILIES,
    "transition_guard_kinds" => TRANSITION_GUARD_KINDS, "identity_envelope" => continuity_envelope,
    "installation_capacity_kinds" => INSTALLATION_CAPACITY_KINDS,
    "installation_classes" => INSTALLATION_CONTINUITY_CLASSES,
    "installation_predecessor_catalog_id" => PREDECESSOR_CATALOG_IDS.fetch(7).fetch(2),
    "manifest_id" => continuity_id, "publication_state" => PUBLICATION_STATE,
    "repository_capacity_kinds" => REPOSITORY_CAPACITY_KINDS,
    "repository_classes" => REPOSITORY_CONTINUITY_CLASSES,
    "repository_predecessor_catalog_id" => PREDECESSOR_CATALOG_IDS.fetch(6).fetch(2),
    "schema_descriptor_id" => schema_ids.fetch("AuthorityContinuityManifestV1"),
    "schema_version" => "maestro.vnext.stage2.authority.continuity-manifest.v1",
  }

  delta = semantic_delta
  root_value = [
    stage0_tree_sha256,
    PREDECESSOR_CATALOG_IDS.map { |number, kind, value| [number, kind, { "bytes" => value }] },
    [["schema_suite", { "bytes" => suite_id }], ["authority_literals", { "bytes" => literals_id }],
     ["action_spec_v2", { "bytes" => action_id }], ["authority_continuity_manifest", { "bytes" => continuity_id }],
     ["stage2_semantic_consumer_delta", delta.fetch("identity")]],
    PUBLICATION_STATE,
  ]
  root_envelope = [DOMAIN_ROOT, root_value]
  root_id, root_cbor = identity(root_envelope)
  primary_paths = [
    "action-spec-v2.v1.cbor", "action-spec-v2.v1.json",
    "authority-continuity-manifest.v1.cbor", "authority-continuity-manifest.v1.json",
    "authority-literals.v1.cbor", "authority-literals.v1.json",
    "schema-descriptors.v1.cbor", "schema-descriptors.v1.json", *descriptor_paths,
  ].sort
  manifest = {
    "artifacts" => primary_paths.map do |path|
      bytes = File.binread(File.join(root, path))
      { "byte_length" => bytes.bytesize, "path" => path, "sha256" => Digest::SHA256.hexdigest(bytes) }
    end,
    "byte_length" => root_cbor.bytesize,
    "component_ids" => { "action_spec_v2" => action_id, "authority_continuity_manifest" => continuity_id,
                         "authority_literals" => literals_id, "schema_suite" => suite_id,
                         "stage2_semantic_consumer_delta" => delta.fetch("identity") },
    "identity_envelope" => root_envelope,
    "predecessor_action_spec_descriptor_id" => PREDECESSOR_ACTION_SPEC_DESCRIPTOR_ID,
    "predecessor_catalog_ids" => PREDECESSOR_CATALOG_IDS.map { |number, kind, value| { "catalog_number" => number, "catalog_type" => kind, "manifest_id" => value } },
    "publication_state" => PUBLICATION_STATE, "root_id" => root_id,
    "schema_version" => "maestro.vnext.stage2.authority.root-manifest.v1",
    "stage" => "stage2_authority_candidate", "stage0_tree_sha256" => stage0_tree_sha256,
    "stage2_semantic_consumer_delta" => { "identity" => delta.fetch("identity"),
      "consumer_count" => delta.fetch("consumer_count"), "consumer_digest" => delta.fetch("consumer_digest"),
      "predecessor" => delta.fetch("predecessor") },
  }
  documents = { "schema-descriptors.v1.json" => schemas, "authority-literals.v1.json" => literals,
                "action-spec-v2.v1.json" => action, "authority-continuity-manifest.v1.json" => continuity,
                "stage2-authority-manifest.v1.json" => manifest }
  cbor = { "schema-descriptors.v1.cbor" => suite_cbor, "authority-literals.v1.cbor" => literals_cbor,
           "action-spec-v2.v1.cbor" => action_cbor, "authority-continuity-manifest.v1.cbor" => continuity_cbor,
           "stage2-authority-manifest.v1.cbor" => root_cbor }
  [documents, cbor, descriptor_paths, root_id]
end

options = { root: DEFAULT_ROOT, emit: false, source_only: false, self_test_source_only: false }
OptionParser.new do |parser|
  parser.on("--root PATH") { |path| options[:root] = File.expand_path(path) }
  parser.on("--emit") { options[:emit] = true }
  parser.on("--source-only") { options[:source_only] = true }
  parser.on("--self-test-source-only") { options[:self_test_source_only] = true }
end.parse!

if options.fetch(:source_only)
  rows = semantic_source_rows
  puts JSON.generate({ "consumer_count" => rows.length, "source_identity" => semantic_source_identity(rows) })
  exit 0
end
if options.fetch(:self_test_source_only)
  self_test_semantic_sources
  puts "Stage 2 Authority source-only mutants rejected"
  exit 0
end

root = options.fetch(:root)
documents, cbor_files, descriptor_paths, root_id = expected_documents(root)
documents.each do |relative, expected|
  raise "independent semantic projection mismatch: #{relative}" unless load_json(root, relative) == expected
end
cbor_files.each do |relative, expected|
  raise "independent CBOR mismatch: #{relative}" unless File.binread(File.join(root, relative)) == expected
end
checked = (descriptor_paths + cbor_files.keys).sort
receipt = {
  "checked_cbor_files" => checked, "encoder" => "independent_ruby_stdlib_semantic_reconstruction",
  "publication_state" => PUBLICATION_STATE, "result" => "all_semantics_cbor_and_identities_equal",
  "root_id" => root_id,
  "schema_version" => "maestro.vnext.stage2.authority.ruby-verification-receipt.v1",
  "verifier" => "tools/vnext_contracts/stage2/authority/verify.rb",
}

if options.fetch(:emit)
  puts JSON.generate(receipt)
else
  raise "Ruby verification receipt is stale" unless load_json(root, "ruby-verification-receipt.v1.json") == receipt
  puts "Stage 2 Authority Ruby semantic verification passed"
end
