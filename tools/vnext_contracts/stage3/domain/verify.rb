#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"
require "open3"
require "optparse"

WORKSPACE = File.expand_path("../../../..", __dir__)
STAGE0_ENCODER_RECEIPT = "contracts/vnext/stage0/effect-home/encoder-receipt.json"
STAGE0_FINALIZATION_RECEIPT = "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json"
STAGE2_MANIFEST = "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json"
STAGE2_PROOF_RECEIPTS = %w[
  contracts/vnext/stage2/authority/python-encoder-receipt.v1.json
  contracts/vnext/stage2/authority/semantic-validation-receipt.v1.json
  contracts/vnext/stage2/authority/ruby-verification-receipt.v1.json
].freeze
DOMAIN = "maestro.vnext.stage3.domain-kernel.v1"
PUBLICATION_STATE = "inactive_candidate"
OWNERS = [
  ["Work", "work", %w[identity revision lifecycle submission requirement relation]],
  ["Contract", "contract", %w[revision generation semantic-publication-request current-root]],
  ["Step", "step", %w[identity revision binding dag lifecycle submission amendment]],
  ["Design", "design", %w[source-binding revision slot-manifest reconciliation]],
  ["Decision", "design", %w[revision alternative resolution materialization lineage batch]],
  ["Evidence", "evidence", %w[claim observation-reference submission-reference claim-subject no-lifecycle]],
  [
    "Repository Store", "repository",
    %w[publication-boundary authority-admission atomic-owner-joined-commit idempotent-replay stale-basis-refusal]
  ],
].freeze
ACTION_CATALOG = [
  "maestro.vnext.stage3.repository-action-catalog-closure.v1",
  "7a7a1311690fcdec194c0d9c9afbbe4e28f1332b567ed23451daf06ebca02970",
  "b7ef635dcd29af4fc41f20cd670b726e5627c2f7210344d058e7c188ace69647",
  1,
  [
    ["implemented", 1, "CreateDraftWork", "56ded201d62fbb94486581d13cc6a086b3e114ad889aa1a954841f7f646afc40", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 1],
    ["implemented", 2, "CancelWork", "b58d2fecb0f1b27146884f85847cb1b22575b32d8d6e92efe6608cf582420615", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 2],
    ["deferred_stage5", 3, "CompleteWork", "163de9814514910c9ca1d5b1f76ac982e0788bd8a81025eff5c551a0a923b5d2", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 3],
    ["catalogued_deferred_stage4_fail_closed", 4, "AbsorbWork", "4fb2d35f4bd7c2169bec6bc51af840f325c419c28de0041d60b69bc4691125ea", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 4],
    ["deferred_stage5", 5, "SubmitWorkCompletion", "7d8083d10f75348f805e89e8fcd5f27d81face12d2d8ae1047a01c53fbfb2803", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 5],
    ["deferred_stage5", 6, "RejectWorkCompletion", "d03d0a753eb0821b43f002de3eab1afb32a7ede75ff0a3588b0718292c0d7b3f", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 6],
    ["deferred_stage5", 7, "ReturnWorkForRepair", "2fbc3d51f0b750cb9a1292d404ada520c17fac6c9fd960350188d97f3b6acc0b", 1, "937dee23c1f157e7e4c224b3aacd856c9a5cbf939dc52800285515f8fcd381ee", 7],
    ["deferred_stage4", 8, "SubmitStep", "c5a7079af2dafa9acc956477b3004b5fb21dd688b4022677416164c4485f96d6", 2, "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e", 1],
    ["deferred_stage5", 9, "SatisfyStep", "a5887f9b87af5f6a5f466df222b3a76b2eca5762b33a5694b4ebcb61b3db127e", 2, "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e", 2],
    ["deferred_stage5", 10, "RejectStepSubmission", "b75460c72c4907e2893bb48559b2c19d99ecb4d27ab43a10adda4ee95dfbc62a", 2, "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e", 3],
    ["deferred_stage5", 11, "RecoverStepSubmission", "130cb3ecfd8146ba869de9a1198b3c3b6b67b2c06101cecf62a955db5b587e13", 2, "6d7cf7235682ed51152ba0fd64cf1610f98e890335db769333ebe3e042cd7e1e", 4],
    ["implemented", 12, "PublishInitialContract", "5c3bf7e45cc2e8348bb5a6ce403cf6d14f718c7ef4514ca01e60370174387234", 3, "e33f42c43c3fadf498db847773ed47e26a459453cc65f14dde9bb5d05cf356ab", 1],
    ["implemented", 13, "AmendContract", "65020299f9f323f4a098c2ff240cbbc984bfc5f6f761712c995e38486d2046f4", 3, "e33f42c43c3fadf498db847773ed47e26a459453cc65f14dde9bb5d05cf356ab", 2],
    ["implemented", 15, "AppendDesignRevision", "4235a07743d0fa3557f612b0d4dd499afcd02adadcc92facfcbc806645a99e83", 4, "85aad446bae62f47851f719f74296bd2576f30894b95ccb4b3b0c59790a80dc5", 2],
    ["implemented", 20, "ResolveDecision", "4e05d1c7d9314a843d43538c399ece7df7da52793062c7ca43805e8f763f75ac", 5, "a3d6c9c0dcd9b5e3447cf4dc45edf5d1b338c99dfc27a61df23966b7514ae9dc", 3],
    ["deferred_stage4", 23, "AcquireStepExecution", "8fe0e1c9141feb86e36badb1a861d49a94ea2224a8c1d0b7a859cd53b7f7a9a2", 6, "82d922e944dc4fe27d3101bc725e0caea82093e8dabe79ed5732ee5c8da91292", 1],
  ],
].freeze
STORE_SCHEMAS = [
  "maestro.vnext.stage3.repository-store-schema-closure.v1",
  [
    [1, "ActionRequest", "maestro.vnext.repository-action-request.v1", "c512faedbf87869f531be18baad9674652c5d83ab4ab76bc555330604e5791cd"],
    [2, "WorkRecord", "maestro.vnext.repository-work-record.v1", "03e178d89683e4c93528e1d6ed7c900d0da8957024e729b3661350b6c397cf37"],
    [3, "DesignStream", "maestro.vnext.repository-design-stream.v1", "8686ba5cc854ff7cb408e322817f173d6db93e99d492bea9bac3aa344b9f5537"],
    [4, "ContractRevision", "maestro.vnext.repository-contract-revision.v1", "221f327f2b29497599f943dd4c6fdea0d328916b7f2e7269c1cda6f81d4a12c0"],
    [5, "ContractGeneration", "maestro.vnext.repository-contract-generation.v1", "d5208061bd3aa95917b36ab2e55439365987286bc594e09ba3a327b3601a6cc7"],
    [6, "DesignFinalizationManifest", "maestro.vnext.repository-design-finalization-manifest.v1", "5ac8298b026b63f4548121ffcf054fa68c30314cb491e67f3150a3f929550226"],
    [7, "ContractRoot", "maestro.vnext.repository-contract-root.v1", "b6950c9de50712ae010468d1a1375ad2635967036972d07cdee0130286c42337"],
    [8, "Decision", "maestro.vnext.repository-decision.v1", "604471128e0c03afa1ac53f72f34c2287f1f975b98effbd2a78569e3490c7fb3"],
    [9, "StepGraph", "maestro.vnext.repository-step-graph.v1", "57bc1aca32b395c1fb43b6c960b38d5828aa71d3d63eac0f7cecf1887a1ed005"],
    [10, "StepState", "maestro.vnext.repository-step-state.v1", "90158ef1e261ea84260ad5fd98deb8461b0655d3216c4993d0d949df1cfc6596"],
    [11, "StepAmendmentAudit", "maestro.vnext.repository-step-amendment-audit.v1", "cf16f6af5205d093c58c63cc27fcd8c7314109ae2604e2baafe7473e74770bcf"],
    [12, "DecisionMaterializationAudit", "maestro.vnext.repository-decision-materialization-audit.v1", "19f3f998c8e408e621bc46aafb0716b4fb0d7b61b721b1c9d177addc4c9a88c4"],
    [13, "ExactEquivalenceReceipt", "maestro.vnext.repository-exact-equivalence-receipt.v1", "8108071f0749a887669d2537f9f7a95d7570e7588410a488e22c95dd73754fa8"],
    [14, "ComponentInvalidationReceipt", "maestro.vnext.repository-component-invalidation-receipt.v1", "ebc6630651d0379159b7369f70b4570440254418dae527043e64cf252128ad23"],
  ],
].freeze
WORK_STATES = %w[draft ready active awaiting_acceptance completed cancelled superseded].freeze
STEP_STATES = %w[open submitted satisfied cancelled superseded].freeze
DECISION_STATES = %w[open resolved withdrawn superseded].freeze
WORK_TRANSITIONS = [
  %w[publish draft ready], %w[start ready active], %w[submit active awaiting_acceptance],
  %w[accept awaiting_acceptance completed], %w[reject awaiting_acceptance active],
  %w[repair awaiting_acceptance active], %w[amend awaiting_acceptance active],
  ["cancel", "draft|ready|active|awaiting_acceptance", "cancelled"],
  ["supersede", "draft|ready|active|awaiting_acceptance", "superseded"],
].freeze
STEP_TRANSITIONS = [
  %w[submit open submitted], %w[satisfy submitted satisfied], %w[reject submitted open],
  %w[recover submitted open], ["cancel", "open|submitted", "cancelled"],
  ["supersede", "open|submitted", "superseded"],
].freeze
RELATIONS = [
  ["requirement", "before_execution|before_step|before_completion", "acyclic", "same_repository"],
  ["superseded_by", "lineage", "acyclic", "same_repository"],
  ["corrects", "lineage", "acyclic", "same_repository"],
  ["continues", "lineage", "acyclic", "same_repository"],
  ["reference", "informational", "cycles_allowed", "cross_repository_allowed"],
].freeze
AMENDMENTS = [
  ["retain_exact", "open_fresh_stage3", "satisfaction_carry_requires_stage5_canonical_evidence_gate_material", "no_lease_attempt_run_transfer"],
  ["replace", "successor_binding_required", "old_cancelled_or_superseded"],
  ["remove", "old_cancelled_or_superseded", "obligations_conserved"],
  ["add", "new_open_binding", "complete_required_dag"],
].freeze
INVARIANTS = %w[
  one_owner_per_concept all_mutations_require_typed_authorized_action_request
  current_generation_and_root_exact semantic_no_op_detected_before_authority
  terminal_work_rejects_design_and_decision_writes claim_binds_exactly_one_submission
  claim_subject_matches_full_submission_subject
  work_claim_subject_matches_exact_step_submission_closure
  submission_claim_cardinality_1_to_n_without_second_count_cap
  nonauthoritative_claim_carrier_refused
  step_binding_generation_scoped step_binding_commits_contract_generation
  contract_generation_identity_excludes_runtime_authority_and_is_predictable
  dag_complete_finite_acyclic
  decision_resolution_has_no_direct_contract_effect
  candidate_root_derived_only_from_typed_consequence_plan
  equal_root_detected_before_authority_and_requires_none
  exactly_equivalent_distinct_root_validated_before_authority_and_writes_nothing
  materialization_candidate_only_and_joined_only_by_contract_publication
  no_standalone_materialize_decision_action
  repository_store_is_only_publication_boundary
  closed_owner_handlers_have_no_generic_lifecycle_bypass
  repository_actions_use_exact_nominal_authority_leaves
  deferred_execution_evidence_and_gate_publication_surfaces_absent
  ordinary_grant_has_canonical_parent_delegation_reachability
  same_store_atomic_owner_joined_publication
  initial_contract_publication_roots_complete_step_dag_and_open_fresh_states
  contract_amendment_consumes_total_step_plan_and_publishes_all_dispositions
  stage3_satisfaction_carry_unavailable_until_canonical_evidence_gate_material
  authority_is_store_loaded_and_action_is_admitted_before_commit
  replay_returns_original_committed_result stale_store_basis_refused_before_commit
  failed_replayed_or_stale_publication_leaves_no_orphan_objects
].freeze
AUTHORITY_SUCCESSOR = [
  "maestro.vnext.stage3.authority-successor.v1",
  "additive_over_frozen_stage2_and_public_predecessors",
  [
    [23, "OrdinaryBoundedGrantV1", %w[complete_grant_definition exact_parent_and_delegation bounded_capacity_root immutable]],
    [24, "OrdinaryGrantDelegationV1", %w[exact_parent_child same_context same_capacity_root immutable]],
  ],
  [
    ["AllocateGovernedCapacitySlot", "BootstrapControlG0"],
    ["EstablishConsumptionCellRoot", "BootstrapControlG0"],
    ["IssueRootAttachedBoundedGrant", "BootstrapControlG0"],
    ["ReissueRootAttachedGrantOneToOne", "OrdinaryLiveRuntime"],
    ["RevokeGrant", "OrdinaryLiveRuntime"],
  ],
  %w[
    parentless_bounded_grant_refused unknown_or_cma_grant_action_basis_refused
    candidate_or_target_self_authorization_refused g0_issue_has_no_ordinary_capacity_debit
    reissue_and_revoke_require_separate_live_admin_grant
    reissue_and_revoke_spend_exactly_one_admin_capacity_unit
  ],
].freeze
SOURCE_PATHS = %w[
  contracts/vnext/catalogs/generated/catalog-09-action-spec.json
  contracts/vnext/public/setup_operation_compatibility.v1.json
  contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json
  src/lib.rs
  src/domain/mod.rs
  src/domain/vnext/authority/action_basis.rs
  src/domain/vnext/authority/bootstrap_catalog.rs
  src/domain/vnext/authority/capacity.rs
  src/domain/vnext/authority/closed.rs
  src/domain/vnext/authority/context.rs
  src/domain/vnext/authority/continuity.rs
  src/domain/vnext/authority/continuity/allocation.rs
  src/domain/vnext/authority/continuity/catalog.rs
  src/domain/vnext/authority/continuity/closure.rs
  src/domain/vnext/authority/continuity/state.rs
  src/domain/vnext/authority/continuity/totality.rs
  src/domain/vnext/authority/continuity/trusted_time.rs
  src/domain/vnext/authority/downstream_action_basis.rs
  src/domain/vnext/authority/evaluator.rs
  src/domain/vnext/authority/facade.rs
  src/domain/vnext/authority/facade/repository_admission.rs
  src/domain/vnext/authority/facade/repository_leaf_authority.rs
  src/domain/vnext/authority/facade_tests.rs
  src/domain/vnext/authority/grant.rs
  src/domain/vnext/authority/identity.rs
  src/domain/vnext/authority/mandate.rs
  src/domain/vnext/authority/mod.rs
  src/domain/vnext/authority/post_cut.rs
  src/domain/vnext/authority/principal.rs
  src/domain/vnext/authority/protected_diagnostic_envelope.rs
  src/domain/vnext/authority/protected_diagnostic_envelope_stage8_seed.rs
  src/domain/vnext/authority/publication.rs
  src/domain/vnext/authority/result.rs
  src/domain/vnext/authority/transition.rs
  src/domain/vnext/contract/assembly.rs
  src/domain/vnext/contract/component.rs
  src/domain/vnext/contract/component_kind.rs
  src/domain/vnext/contract/decision_closure.rs
  src/domain/vnext/contract/finalization.rs
  src/domain/vnext/contract/handoff.rs
  src/domain/vnext/contract/materialization.rs
  src/domain/vnext/contract/mod.rs
  src/domain/vnext/contract/proof.rs
  src/domain/vnext/contract/provenance.rs
  src/domain/vnext/contract/root.rs
  src/domain/vnext/contract/runtime.rs
  src/domain/vnext/evidence/submission_claim.rs
  src/domain/vnext/design/batch.rs
  src/domain/vnext/design/closure.rs
  src/domain/vnext/design/common.rs
  src/domain/vnext/design/decision.rs
  src/domain/vnext/design/materialization.rs
  src/domain/vnext/design/mod.rs
  src/domain/vnext/design/revision.rs
  src/domain/vnext/evidence/assessment.rs
  src/domain/vnext/evidence/claim.rs
  src/domain/vnext/evidence/diagnostics/mod.rs
  src/domain/vnext/evidence/erasure.rs
  src/domain/vnext/evidence/identity.rs
  src/domain/vnext/evidence/mod.rs
  src/domain/vnext/evidence/observation.rs
  src/domain/vnext/evidence/store.rs
  src/domain/vnext/identity/digest.rs
  src/domain/vnext/identity/manifest.rs
  src/domain/vnext/identity/mod.rs
  src/domain/vnext/identity/schema.rs
  src/domain/vnext/mod.rs
  src/domain/vnext/persistence/export.rs
  src/domain/vnext/persistence/generation.rs
  src/domain/vnext/persistence/idempotency.rs
  src/domain/vnext/persistence/metadata.rs
  src/domain/vnext/persistence/mod.rs
  src/domain/vnext/persistence/object.rs
  src/domain/vnext/persistence/protected_diagnostic.rs
  src/domain/vnext/persistence/protected_diagnostic_stage9_seed.rs
  src/domain/vnext/persistence/retention.rs
  src/domain/vnext/persistence/snapshot.rs
  src/domain/vnext/persistence/snapshot_blocks.rs
  src/domain/vnext/persistence/snapshot_export.rs
  src/domain/vnext/persistence/snapshot_restore.rs
  src/domain/vnext/persistence/snapshot_rows.rs
  src/domain/vnext/persistence/store.rs
  src/domain/vnext/persistence/tests/atomic_publication.rs
  src/domain/vnext/persistence/tests/canonical_store.rs
  src/domain/vnext/persistence/tests/mod.rs
  src/domain/vnext/persistence/tests/store_full_export.rs
  src/domain/vnext/persistence/tests/store_safety.rs
  src/domain/vnext/persistence/types.rs
  src/domain/vnext/repository/mod.rs
  src/domain/vnext/repository/tests.rs
  src/domain/vnext/step/amendment.rs
  src/domain/vnext/step/graph.rs
  src/domain/vnext/step/identity.rs
  src/domain/vnext/step/lifecycle.rs
  src/domain/vnext/step/mod.rs
  src/domain/vnext/step/revision.rs
  src/domain/vnext/step/submission.rs
  src/domain/vnext/work/identity.rs
  src/domain/vnext/work/lifecycle.rs
  src/domain/vnext/work/mod.rs
  src/domain/vnext/work/relation.rs
  src/domain/vnext/work/submission.rs
  src/foundation/mod.rs
  src/foundation/core/mod.rs
  src/foundation/core/deterministic_cbor.rs
  src/foundation/core/secure_fs.rs
  tests/vnext_work_identity.rs
  tests/vnext_work_lifecycle.rs tests/vnext_work_relations.rs
  tests/vnext_step_graph.rs
  tests/vnext_step_amendment.rs tests/vnext_step_amendment_application.rs
  tests/vnext_contract_step_publication.rs
  tests/vnext_design_revisions.rs tests/vnext_decision_kernel.rs
  tests/vnext_decision_closure.rs tests/vnext_decision_materialization_plan.rs
  tests/vnext_evidence_claims.rs tests/vnext_submission_claim_set.rs
  tests/vnext_stage3_contracts.rs
  tools/vnext_contracts/public/build_public_literals.py
  tools/vnext_contracts/catalogs/cbor_py.py
  tools/vnext_contracts/stage2/authority/build.py
  tools/vnext_contracts/stage2/authority/validate.py
  tools/vnext_contracts/stage2/authority/verify.rb
  tools/vnext_contracts/stage3/domain/build.py
  tools/vnext_contracts/stage3/domain/validate.py
  tools/vnext_contracts/stage3/domain/verify.rb
].freeze
RUST_SOURCE_ROOTS = %w[
  src/domain/vnext/authority
  src/domain/vnext/contract
  src/domain/vnext/design
  src/domain/vnext/evidence
  src/domain/vnext/identity
  src/domain/vnext/persistence
  src/domain/vnext/repository
  src/domain/vnext/step
  src/domain/vnext/work
].freeze

def head(major, value)
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
    raise "unsupported Stage 3 canonical map: #{value.inspect}" unless value.keys == ["bytes"]

    raw = [value.fetch("bytes")].pack("H*")
    head(2, raw.bytesize) + raw
  else raise "unsupported Stage 3 canonical value: #{value.inspect}"
  end
end

def source_rows
  SOURCE_PATHS.map do |relative|
    bytes = File.binread(File.join(WORKSPACE, relative))
    [relative, bytes.bytesize, Digest::SHA256.hexdigest(bytes)]
  end
end

def validate_source_closure
  actual = RUST_SOURCE_ROOTS.flat_map do |relative|
    Dir.glob(File.join(WORKSPACE, relative, "**", "*.rs")).map do |path|
      path.delete_prefix("#{WORKSPACE}/")
    end
  end.sort
  declared = SOURCE_PATHS.select do |relative|
    relative.end_with?(".rs") && RUST_SOURCE_ROOTS.any? { |root| relative.start_with?("#{root}/") }
  end.sort
  unless declared == actual
    missing = actual - declared
    unexpected = declared - actual
    raise "Stage 3 transitive Rust source closure drifted: missing=#{missing}, unexpected=#{unexpected}"
  end
  %w[
    src/lib.rs
    src/domain/mod.rs
    src/domain/vnext/mod.rs
    src/foundation/mod.rs
    src/foundation/core/mod.rs
    src/foundation/core/deterministic_cbor.rs
    src/foundation/core/secure_fs.rs
  ].each do |relative|
    raise "Stage 3 transitive semantic dependency is undeclared: #{relative}" unless SOURCE_PATHS.include?(relative)
  end
end

def load_object(relative)
  value = JSON.parse(File.read(File.join(WORKSPACE, relative), encoding: Encoding::US_ASCII))
  raise "predecessor artifact must contain one object: #{relative}" unless value.is_a?(Hash)

  value
end

def proof_receipt_rows(paths)
  paths.map do |relative|
    bytes = File.binread(File.join(WORKSPACE, relative))
    { "byte_length" => bytes.bytesize, "path" => relative, "sha256" => Digest::SHA256.hexdigest(bytes) }
  end
end

def predecessor_chain_binding
  stage0_encoder = load_object(STAGE0_ENCODER_RECEIPT)
  stage0_finalization = load_object(STAGE0_FINALIZATION_RECEIPT)
  valid_stage0 = stage0_encoder["schema_version"] == "maestro.vnext.stage0.effect-home-encoder-receipt.v1" &&
                 stage0_finalization["schema_version"] == "maestro.vnext.stage0.effect-home-finalization-receipt.v1" &&
                 stage0_finalization["finalization_state"] == "final" &&
                 stage0_finalization["candidate_only"] == true &&
                 stage0_finalization["runtime_activation"] == false
  raise "Stage 0 proof receipt is not a final inactive-candidate certification" unless valid_stage0

  stage0_encoder_sha256 = Digest::SHA256.hexdigest(File.binread(File.join(WORKSPACE, STAGE0_ENCODER_RECEIPT)))
  unless stage0_finalization["encoder_receipt_sha256"] == stage0_encoder_sha256
    raise "Stage 0 finalization receipt does not bind the exact encoder receipt"
  end
  stage0_semantic_root = stage0_finalization["identity"]
  unless stage0_semantic_root.is_a?(String) && stage0_semantic_root.start_with?("sha256:")
    raise "Stage 0 finalization receipt has no semantic root"
  end

  stage2_manifest = load_object(STAGE2_MANIFEST)
  stage2_root_id = stage2_manifest["root_id"]
  stage0_tree_sha256 = stage2_manifest["stage0_tree_sha256"]
  valid_roots = stage2_manifest["schema_version"] == "maestro.vnext.stage2.authority.root-manifest.v1" &&
                stage2_root_id.is_a?(String) && stage2_root_id.length == 64 &&
                stage0_tree_sha256.is_a?(String) && stage0_tree_sha256.length == 64
  raise "Stage 2 manifest has no exact Stage 0 tree and Stage 2 semantic roots" unless valid_roots

  STAGE2_PROOF_RECEIPTS.each do |relative|
    raise "Stage 2 proof receipt does not bind the exact root: #{relative}" unless load_object(relative)["root_id"] == stage2_root_id
  end

  {
    "mode" => "full_chain",
    "stage0" => {
      "proof_receipts" => proof_receipt_rows([STAGE0_ENCODER_RECEIPT, STAGE0_FINALIZATION_RECEIPT]),
      "semantic_root" => stage0_semantic_root,
      "source_tree_root" => "sha256:#{stage0_tree_sha256}",
    },
    "stage2" => {
      "proof_receipts" => proof_receipt_rows(STAGE2_PROOF_RECEIPTS),
      "semantic_root" => "sha256:#{stage2_root_id}",
    },
  }
end

def validate_action_catalog
  setup = JSON.parse(File.read(File.join(WORKSPACE, "contracts/vnext/public/setup_operation_compatibility.v1.json")))
  generated_path = File.join(WORKSPACE, "contracts/vnext/catalogs/generated/catalog-09-action-spec.json")
  generated_bytes = File.binread(generated_path)
  generated = JSON.parse(generated_bytes)
  bindings = setup.fetch("catalog_bindings")
  identities_match = generated.fetch("manifest_id") == ACTION_CATALOG[1] &&
                     generated.fetch("grammar_id") == ACTION_CATALOG[2] &&
                     bindings.fetch("action_spec_manifest_id") == ACTION_CATALOG[1] &&
                     bindings.fetch("catalog_profile_grammar_id") == ACTION_CATALOG[2] &&
                     bindings.fetch("action_spec_file_sha256") == Digest::SHA256.hexdigest(generated_bytes)
  raise "the frozen ActionSpec manifest or grammar identity drifted" unless identities_match

  manifest_envelope = generated.fetch("manifest_identity_envelope")
  manifest_bytes = encode(manifest_envelope)
  manifest_matches = generated.fetch("cbor_hex") == manifest_bytes.unpack1("H*") &&
                     generated.fetch("byte_length") == manifest_bytes.bytesize &&
                     Digest::SHA256.hexdigest(manifest_bytes) == ACTION_CATALOG[1] &&
                     manifest_envelope[3] == generated.fetch("manifest_header") &&
                     manifest_envelope[4] == generated.fetch("manifest_rows") &&
                     generated.fetch("manifest_header")[1] == ACTION_CATALOG[3] &&
                     generated.fetch("manifest_header")[3].fetch("bytes") == ACTION_CATALOG[2]
  raise "the frozen ActionSpec manifest envelope is not self-authenticating" unless manifest_matches

  setup_by_tag = setup.fetch("action_rows").to_h { |row| [row.fetch("catalog_tag"), row] }
  generated_by_tag = generated.fetch("descriptors").to_h { |row| [row.fetch("value")[0], row] }
  manifest_by_tag = generated.fetch("manifest_rows").to_h { |row| [row[0], row] }
  ACTION_CATALOG[4].each do |_, tag, name, descriptor_id, owner_tag, owner_id, local_tag|
    setup_row = setup_by_tag.fetch(tag)
    generated_row = generated_by_tag.fetch(tag)
    manifest_row = manifest_by_tag.fetch(tag)
    expected = [name, descriptor_id, owner_tag, owner_id, local_tag]
    actual_setup = [setup_row.fetch("name"), setup_row.fetch("descriptor_id"), setup_row.fetch("primary_owner_tag"), setup_row.fetch("primary_owner_descriptor_id"), setup_row.fetch("family_local_tag")]
    value = generated_row.fetch("value")
    actual_generated = [value[1], generated_row.fetch("descriptor_id"), value[2][0], value[2][1].fetch("bytes"), value[4]]
    descriptor_envelope = generated_row.fetch("identity_envelope")
    descriptor_bytes = encode(descriptor_envelope)
    descriptor_matches = actual_setup == expected &&
                         actual_generated == expected &&
                         value[3] == setup_row.fetch("family_tag") &&
                         descriptor_envelope[2] == value &&
                         generated_row.fetch("cbor_hex") == descriptor_bytes.unpack1("H*") &&
                         generated_row.fetch("byte_length") == descriptor_bytes.bytesize &&
                         Digest::SHA256.hexdigest(descriptor_bytes) == descriptor_id &&
                         manifest_row == [tag, { "bytes" => descriptor_id }, value]
    raise "frozen ActionSpec row drifted for #{tag}:#{name}" unless descriptor_matches
  end
end

def validate_store_schemas
  STORE_SCHEMAS[1].each_with_index do |(ordinal, _, domain, schema_id), index|
    expected = Digest::SHA256.hexdigest(encode(["maestro.vnext.repository-runtime-schema.v1", domain]))
    raise "Repository Store schema identity drifted for #{domain}" unless ordinal == index + 1 && schema_id == expected
  end
end

def validate_predecessor_chain
  commands = [
    ["python3", "tools/vnext_contracts/stage0/effect_home/build.py", "--check"],
    ["python3", "tools/vnext_contracts/stage0/effect_home/validate.py"],
    ["python3", "tools/vnext_contracts/stage2/authority/build.py", "--check"],
    ["python3", "tools/vnext_contracts/stage2/authority/validate.py"],
    ["ruby", "tools/vnext_contracts/stage2/authority/verify.rb"],
  ]
  commands.each do |command|
    stdout, stderr, status = Open3.capture3(*command, chdir: WORKSPACE)
    next if status.success?

    detail = stderr.strip.empty? ? stdout.strip : stderr.strip
    raise "Stage 3 predecessor validation failed: #{command.join(' ')}: #{detail}"
  end
end

root = File.join(WORKSPACE, "contracts/vnext/stage3/domain")
catalog_only = false
artifact_only = false
OptionParser.new do |options|
  options.on("--root ROOT") { |value| root = value }
  options.on("--catalog-only") { catalog_only = true }
  options.on("--artifact-only") { artifact_only = true }
end.parse!
if catalog_only
  validate_action_catalog
  puts "Stage 3 frozen ActionSpec catalog validated"
  exit 0
end
validate_predecessor_chain unless artifact_only
validate_action_catalog
validate_store_schemas
validate_source_closure
manifest = JSON.parse(File.read(File.join(root, "domain-kernel.v1.json"), encoding: Encoding::US_ASCII))
expected = [
  DOMAIN, 1, PUBLICATION_STATE, OWNERS, [WORK_STATES, STEP_STATES, DECISION_STATES],
  [WORK_TRANSITIONS, STEP_TRANSITIONS], RELATIONS, AMENDMENTS, INVARIANTS, ACTION_CATALOG, STORE_SCHEMAS, source_rows,
  AUTHORITY_SUCCESSOR,
]
raise "Stage 3 manifest fields drifted" unless manifest.keys.sort == %w[canonical_value identity publication_state schema_version]
raise "Stage 3 semantic projection drifted" unless manifest.fetch("canonical_value") == expected
encoded = encode(expected)
raise "Stage 3 CBOR drifted" unless File.binread(File.join(root, "domain-kernel.v1.cbor")) == encoded
identity = "sha256:#{Digest::SHA256.hexdigest(encoded)}"
raise "Stage 3 identity drifted" unless manifest.fetch("identity") == identity
raise "Stage 3 schema drifted" unless manifest.fetch("schema_version") == DOMAIN
raise "Stage 3 publication state drifted" unless manifest.fetch("publication_state") == PUBLICATION_STATE
if artifact_only
  puts identity
  exit 0
end
receipt = {
  "identity" => identity,
  "predecessor_chain" => predecessor_chain_binding,
  "schema_version" => "maestro.vnext.stage3.domain-kernel.ruby-verification-receipt.v1",
  "validation_mode" => "full_chain",
  "validator" => "independent-ruby-reconstruction",
}
File.write(File.join(root, "ruby-verification-receipt.v1.json"), JSON.pretty_generate(receipt) + "\n", encoding: Encoding::US_ASCII)
puts identity
