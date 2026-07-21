#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "fileutils"
require "json"
require "open3"
require "optparse"

WORKSPACE = File.expand_path("../../../..", __dir__)
DOMAIN = "maestro.vnext.stage5.evidence-gates.v1"
SOURCE_PATHS = %w[
  Cargo.toml Cargo.lock build.rs src/lib.rs src/domain/mod.rs src/domain/vnext/mod.rs
  contracts/vnext/catalogs/generated/catalog-01-observation.json
  src/domain/vnext/authority/action_basis.rs src/domain/vnext/authority/facade.rs
  src/domain/vnext/authority/facade/repository_admission.rs
  src/domain/vnext/authority/facade/repository_leaf_authority.rs src/domain/vnext/authority/mod.rs
  src/domain/vnext/authority/result.rs
  src/domain/vnext/contract/runtime.rs
  src/domain/vnext/evidence/assessment.rs src/domain/vnext/evidence/claim.rs
  src/domain/vnext/evidence/erasure.rs src/domain/vnext/evidence/identity.rs
  src/domain/vnext/evidence/mod.rs src/domain/vnext/evidence/observation.rs
  src/domain/vnext/evidence/submission_claim.rs src/domain/vnext/execution/store.rs
  src/domain/vnext/execution/runtime.rs
  src/domain/vnext/evidence/store.rs src/domain/vnext/gate/mod.rs
  src/domain/vnext/persistence/mod.rs src/domain/vnext/persistence/idempotency.rs
  src/domain/vnext/persistence/metadata.rs
  src/domain/vnext/persistence/store.rs
  src/domain/vnext/persistence/tests/atomic_publication.rs
  src/domain/vnext/repository/mod.rs
  src/domain/vnext/repository/tests.rs
  src/domain/vnext/work/lifecycle.rs src/domain/vnext/work/mod.rs
  src/domain/vnext/work/submission.rs src/foundation/core/secure_fs.rs
  tests/vnext_evidence_claims.rs tests/vnext_submission_claim_set.rs
  tests/vnext_stage5_contracts.rs tests/vnext_stage5_evidence_gates.rs
  tests/vnext_work_lifecycle.rs
  tools/vnext_contracts/catalogs/cbor_py.py
  tools/vnext_contracts/proof_engine/__init__.py
  tools/vnext_contracts/proof_engine/README.md
  tools/vnext_contracts/proof_engine/engine.py
  tools/vnext_contracts/proof_engine/test_engine.py
  tools/vnext_contracts/stage5/evidence_gates/behavior.py
  tools/vnext_contracts/stage5/evidence_gates/build.py
  tools/vnext_contracts/stage5/evidence_gates/consensus.py
  tools/vnext_contracts/stage5/evidence_gates/harness.py
  tools/vnext_contracts/stage5/evidence_gates/predecessor.py
  tools/vnext_contracts/stage5/evidence_gates/validate.py
  tools/vnext_contracts/stage5/evidence_gates/verify.rb
  tools/vnext_contracts/stage5/evidence_gates/seal.py
  tools/vnext_contracts/stage5/evidence_gates/test_consensus.py
  tools/vnext_contracts/stage5/evidence_gates/test_seal.py
  tools/vnext_contracts/stage5/evidence_gates/test_toolchain.py
  tools/vnext_contracts/stage5/evidence_gates/toolchain.py
].freeze
PREDECESSOR_PATHS = %w[
  contracts/vnext/stage4/execution/execution-effects.v1.json
  contracts/vnext/stage4/execution/execution-effects.v1.cbor
  contracts/vnext/stage4/execution/behavioral-proof-receipt.v1.json
  contracts/vnext/stage4/execution/python-encoder-receipt.v1.json
  contracts/vnext/stage4/execution/semantic-validation-receipt.v1.json
  contracts/vnext/stage4/execution/ruby-verification-receipt.v1.json
].freeze
RESULTS = [[1, "Pass"], [2, "Fail"], [3, "Indeterminate"], [4, "Error"]].freeze
OBSERVATION_CONTRACT_TABLE_IDENTITY = "sha256:a5f0e9137c091972802cb7084d86070a930091f0570cefcc7df445074478a676"
INPUT_CLASSES = [[1, "Evidence"], [2, "Authority"], [3, "Mixed"], [4, "Composite"]].freeze
OPERATORS = [[1, "Leaf"], [2, "All"], [3, "Any"], [4, "Quorum"], [5, "Veto"], [6, "DenyOverrides"]].freeze
ACQUISITION_MODES = [
  [1, "EffectFree", "zero_run"],
  [2, "RunMediated", "exact_execution_attempt_owner"],
  [3, "DeclaredDerivation", "source_observation_closure"]
].freeze
INVALIDATION_REASONS = [
  [1, "WorkGenerationAdvanced"], [2, "StepRevisionAdvanced"],
  [3, "GateSnapshotChanged"], [4, "EvaluatorChanged"], [5, "InputTombstoned"],
  [6, "InputCorrected"], [7, "FreshnessExpired"], [8, "IntegrityFailure"],
  [9, "AuthorizationReceiptRevoked"]
].freeze
INVARIANTS = %w[
  observation_kind_exact_43_dense_closed observation_catalog_binds_producer_action_routes_and_cma
  observation_payload_schemas_are_exact_typed_and_kind_specific
  observation_scope_binds_exact_work_step_submission_and_generation
  observation_secret_scan_redaction_and_retention_are_typed_and_authenticated
  secret_scan_is_deterministically_recomputed_from_exact_payload_bytes
  observation_is_immutable_non_bearer
  observation_publication_requires_typed_action_authority_and_atomic_store_index
  stored_evidence_records_require_canonical_identity_consistent_decoding
  payload_identity_distinct_from_observation_identity effect_free_acquisition_has_zero_run
  effecting_acquisition_binds_exact_run_and_attempt_owner acquisition_identity_is_unique_per_store
  declared_derivation_equals_lineage claim_binds_exactly_one_submission
  claim_publication_resolves_exact_observation_records submission_claim_set_has_exact_three_field_carrier
  assessment_evaluates_exactly_one_gate_node
  assessment_scope_store_generation_and_evidence_cut_are_exact
  assessment_support_binds_pairwise_independent_contributors_and_sources
  assessment_uses_trusted_time_freshness_and_pinned_trust_root
  empirical_authority_and_composite_inputs_are_nominally_distinct
  gate_snapshot_is_complete_content_addressed_and_acyclic gate_snapshot_has_no_detached_nodes
  gate_leaf_cannot_accept_a_proposed_result gate_composite_evaluation_is_pure_and_pinned
  closed_semantic_leaf_evaluators_produce_pass_or_fail_from_exact_inputs
  only_pass_derives_satisfaction fail_indeterminate_and_error_block
  equally_applicable_conflict_is_indeterminate applicability_has_no_newest_selector
  invalidation_requires_typed_authority_and_exact_evidence_cut
  assessment_and_invalidation_publication_require_complete_store_derived_cut
  security_erasure_derives_complete_narrow_invalidation_closure
  security_erasure_transitively_invalidates_composite_dependents
  security_erasure_publishes_in_doubt_intent_before_physical_absence
  security_erasure_receipt_requires_verified_physical_absence_and_exact_resume
  security_erasure_revokes_every_secret_bearing_sealed_export_under_one_durable_barrier
  security_erasure_restores_exact_insert_only_schema_before_publication_commit
  security_erasure_finalization_survives_authority_head_advance
  physical_erasure_never_resolves_while_hard_link_or_crash_debt_remains
  atomic_publication_builders_reduce_supersets_to_the_exact_generation_closure
  raw_atomic_publication_rejects_every_object_outside_the_exact_generation_closure
  idempotency_results_remain_durable_replay_horizons
  work_completion_atomically_commits_current_claim_gate_and_submission_evidence
  work_completion_requires_repository_derived_current_satisfied_submission_closure
  persisted_invalidation_rejoins_exact_authorized_action_and_effect_intent
  stage3_claim_and_work_submission_v1_bytes_remain_exact
  scheduling_and_admission_assessments_are_outside_evidence
].freeze
EXPECTED_RUNS = [
  ["assessment-kernel", "maestro", %w[
    domain::vnext::evidence::assessment::tests::all_same_result_assessments_remain_applicable_without_newest_selection
    domain::vnext::evidence::assessment::tests::applicability_binds_the_complete_historical_time_basis
    domain::vnext::evidence::assessment::tests::claim_assessment_requires_exact_resolved_observations
    domain::vnext::evidence::assessment::tests::closed_presence_rules_cannot_self_attest_gate_satisfaction
    domain::vnext::evidence::assessment::tests::composite_assessment_consumes_exact_child_resolutions
    domain::vnext::evidence::assessment::tests::closed_semantic_evaluator_can_pass_fail_and_derive_satisfaction
    domain::vnext::evidence::assessment::tests::conflict_invalidation_and_expiry_never_prefer_pass
    domain::vnext::evidence::assessment::tests::foreign_work_and_contract_claims_are_rejected_before_evaluation
    domain::vnext::evidence::assessment::tests::leaf_assessment_uses_pinned_evaluator_and_conservative_freshness
    domain::vnext::evidence::assessment::tests::quorum_requires_pairwise_contributor_and_source_independence
    domain::vnext::evidence::assessment::tests::security_erasure_is_authorized_and_couples_all_invalidations
    domain::vnext::evidence::assessment::tests::step_claim_subject_helper_remains_generation_scoped
    domain::vnext::evidence::assessment::tests::step_scope_and_mixed_authority_inputs_are_exact
    domain::vnext::evidence::assessment::tests::stored_assessment_decoder_rejects_self_consistent_duplicate_inputs
    domain::vnext::evidence::assessment::tests::trusted_time_and_store_domain_fail_closed
  ]],
  ["submission-evidence-join", "maestro", %w[
    domain::vnext::execution::store::tests::competing_step_submissions_have_one_atomic_winner
    domain::vnext::execution::store::tests::step_submission_and_renewal_race_has_one_atomic_winner
    domain::vnext::execution::store::tests::step_submission_and_takeover_boundary_proves_both_atomic_linearizations
    domain::vnext::execution::store::tests::step_submission_one_and_many_claims_are_atomic_restart_decodable_and_idempotent
    domain::vnext::execution::store::tests::step_submission_rejects_empty_and_wrong_fence_claim_sets_before_publication
  ]],
  ["authorized-evidence-store", "maestro", %w[
    domain::vnext::evidence::store::tests::authorized_store_cut_and_security_erasure_are_restart_safe
    domain::vnext::persistence::store::tests::controlled_copy_census_fails_closed_on_a_renamed_export_carrier
    domain::vnext::persistence::store::tests::controlled_copy_census_includes_an_orphan_pre_receipt_export
    domain::vnext::persistence::store::tests::controlled_copy_erasure_recovery_accepts_only_monotonic_disappearance
    domain::vnext::persistence::store::tests::failed_sealer_cleanup_cannot_unlink_a_waiting_sealers_committed_carrier
    domain::vnext::persistence::store::tests::hard_link_race_blocks_controlled_copy_absence_receipt_after_restart
    domain::vnext::persistence::tests::atomic_publication::historical_idempotency_result_is_a_durable_replay_horizon_after_head_advance
    domain::vnext::persistence::idempotency::tests::atomic_publication_rejects_unreachable_supplied_objects
    domain::vnext::persistence::idempotency::tests::generation_closure_rejects_a_missing_referenced_object
    domain::vnext::persistence::idempotency::tests::publication_builder_reduces_a_superset_to_the_exact_generation_closure
    domain::vnext::repository::tests::work_completion_atomically_persists_claim_gate_and_submission_proof
    domain::vnext::repository::tests::work_completion_requires_and_commits_the_exact_current_satisfied_step_submission_closure
    foundation::core::secure_fs::tests::digest_addressed_removal_recovers_after_payload_unlink_and_marker_crashes
    foundation::core::secure_fs::tests::digest_addressed_removal_recovers_after_the_quarantine_rename
    foundation::core::secure_fs::tests::crash_residual_temp_blocks_absence_until_digest_bound_cleanup
    foundation::core::secure_fs::tests::hard_link_after_sentinel_check_never_publishes_resolution
    foundation::core::secure_fs::tests::hard_link_race_leaves_durable_removal_debt_across_restart
  ]],
  ["work-completion-boundary", "vnext_work_lifecycle", %w[
    pure_lifecycle_appends_revision_facts_and_refuses_unverified_completion
  ]],
  ["claim-contracts", "vnext_evidence_claims", %w[
    authoritative_claim_set_has_no_second_claim_count_cap
    authoritative_claim_set_is_derived_only_from_claims_bound_to_one_submission
    claim_identity_and_record_bind_one_exact_submission_deterministically
    stage3_claim_and_work_submission_v1_vectors_remain_exact
    zero_or_missing_claim_identity_material_is_rejected_before_publication
  ]],
  ["submission-claim-carrier", "vnext_submission_claim_set", %w[
    freezes_one_and_many_claim_vectors
    freezes_the_schema_identity_and_rejects_shape_mutants
    reference_encoder_matches_the_rust_encoder
    rejects_every_malformed_set_product
  ]],
  ["evidence-gate-contracts", "vnext_stage5_evidence_gates", %w[
    claim_publication_requires_exact_resolved_observation_records
    composite_gate_grammars_are_fail_closed_and_order_independent
    gate_snapshot_is_canonical_closed_and_root_reachable
    observation_kind_runtime_matches_all_frozen_catalog_semantics
    payload_manifest_requires_current_authenticated_zero_secret_scan
    observation_publication_route_rejects_wrong_action_route_and_profile
    observations_bind_effect_free_and_exact_derivation_provenance
    pure_composite_evaluator_refuses_leaf_self_attestation
  ]]
].freeze
EXPECTED_TESTS = EXPECTED_RUNS.sum { |row| row.fetch(2).length }
EXPECTED_BEHAVIOR_MANIFEST_IDENTITY = "sha256:a45a1774976a2ad7d3e9cf9702ea78bb5bbae33a9deca7a06d5127c451477f12"

def head(major, value)
  raise "CBOR unsigned value exceeds u64" unless value.between?(0, 0xffffffffffffffff)

  if value < 24
    [(major << 5) | value].pack("C")
  elsif value <= 0xff
    [(major << 5) | 24, value].pack("CC")
  elsif value <= 0xffff
    [(major << 5) | 25, value].pack("Cn")
  elsif value <= 0xffffffff
    [(major << 5) | 26, value].pack("CN")
  else
    [(major << 5) | 27, value].pack("CQ>")
  end
end

def encode(value)
  case value
  when Integer
    head(0, value)
  when String
    raw = value.encode(Encoding::US_ASCII).b
    head(3, raw.bytesize) + raw
  when Array
    head(4, value.length) + value.map { |item| encode(item) }.join
  when Hash
    raise "only a canonical bytes wrapper is accepted" unless value.keys == ["bytes"]

    raw = [value.fetch("bytes")].pack("H*")
    head(2, raw.bytesize) + raw
  else
    raise "value is outside the Stage 5 canonical CBOR subset: #{value.inspect}"
  end
end

def file_row(relative)
  bytes = File.binread(File.join(WORKSPACE, relative))
  [relative, bytes.bytesize, Digest::SHA256.hexdigest(bytes)]
end

def canonicalize(value)
  case value
  when Hash
    value.keys.sort.to_h { |key| [key, canonicalize(value.fetch(key))] }
  when Array
    value.map { |item| canonicalize(item) }
  else
    value
  end
end

def canonical_json(value)
  JSON.generate(canonicalize(value)) + "\n"
end

def behavior_manifest_identity
  rows = EXPECTED_RUNS.flat_map do |_, target, tests|
    tests.map { |test| [target, test] }
  end
  raise "Stage 5 behavior manifest is not an exact unique target/test closure" unless rows.length == EXPECTED_TESTS && rows.uniq.length == EXPECTED_TESTS

  "sha256:#{Digest::SHA256.hexdigest(canonical_json(rows))}"
end

def observation_rows(catalog)
  unless catalog.fetch("schema_version") == "maestro.vnext.catalog.literal.v1" &&
         catalog.fetch("publication_state") == "inactive_candidate" &&
         catalog.fetch("catalog_tag") == 1 &&
         catalog.fetch("catalog_slug") == "observation" &&
         catalog.fetch("catalog_type") == "ObservationKindV1"
    raise "Observation catalog header identity differs"
  end

  schemas = catalog.fetch("schemas")
  raise "Observation catalog schema closure differs" unless schemas.keys.sort == %w[descriptor header manifest]

  schemas.each_value do |schema|
    encoded = encode(schema.fetch("identity_envelope"))
    unless encoded.unpack1("H*") == schema.fetch("cbor_hex") &&
           encoded.bytesize == schema.fetch("byte_length") &&
           Digest::SHA256.hexdigest(encoded) == schema.fetch("schema_id")
      raise "Observation catalog schema identity differs"
    end
  end

  descriptors = catalog.fetch("descriptors")
  raise "ObservationKindV1 catalog is not the exact dense 43-row closure" unless descriptors.length == 43

  expected_cma = {
    29 => [[45], [1], [[1, 1]]], 30 => [[45], [2], [[1, 2]]],
    31 => [[45], [7], [[4, 7]]], 32 => [[45], [8], [[5, 8]]],
    33 => [[45], [4, 6], [[2, 4], [3, 6]]], 34 => [[45], [3], [[2, 3]]],
    35 => [[45], [5], [[3, 5]]], 36 => [[45], [9], [[5, 9]]],
    37 => [[45], [10], [[5, 10]]]
  }.freeze
  descriptors.each_with_index do |descriptor, offset|
    tag = offset + 1
    value = descriptor.fetch("value")
    encoded = encode(descriptor.fetch("identity_envelope"))
    expected_relations = if expected_cma.key?(tag)
                           expected_cma.fetch(tag)
                         elsif tag == 17
                           [[43], [], []]
                         elsif tag == 18
                           [[44], [], []]
                         else
                           [[39], [], []]
                         end
    unless value.fetch(0) == tag &&
           descriptor.fetch("identity_envelope").fetch(2) == value &&
           encoded.unpack1("H*") == descriptor.fetch("cbor_hex") &&
           encoded.bytesize == descriptor.fetch("byte_length") &&
           Digest::SHA256.hexdigest(encoded) == descriptor.fetch("descriptor_id") &&
           value[3, 3] == expected_relations
      raise "Observation descriptor identity or producer relation differs"
    end
  end

  owner = catalog.fetch("primary_owner_relation")
  owner_encoded = encode(owner.fetch("identity_envelope"))
  expected_owner_rows = descriptors.map do |descriptor|
    value = descriptor.fetch("value")
    [value.fetch(0), *value.fetch(2)]
  end
  unless owner.fetch("rows") == expected_owner_rows &&
         owner.fetch("identity_envelope").fetch(1) == expected_owner_rows &&
         owner_encoded.unpack1("H*") == owner.fetch("cbor_hex") &&
         owner_encoded.bytesize == owner.fetch("byte_length") &&
         Digest::SHA256.hexdigest(owner_encoded) == owner.fetch("relation_id")
    raise "Observation primary-owner relation differs"
  end

  header = catalog.fetch("manifest_header")
  unless header[0, 3] == [1, 1, 1] &&
         header.fetch(3) == { "bytes" => catalog.fetch("grammar_id") } &&
         header.fetch(4) == [] &&
         header.fetch(5) == { "bytes" => owner.fetch("relation_id") } &&
         header[6, 2] == [43, 43] &&
         header.fetch(10) == 1
    raise "Observation manifest header grammar or ownership binding differs"
  end
  expected_rows = descriptors.map do |descriptor|
    value = descriptor.fetch("value")
    [value.fetch(0), { "bytes" => descriptor.fetch("descriptor_id") }, value]
  end
  manifest_encoded = encode(catalog.fetch("manifest_identity_envelope"))
  unless catalog.fetch("manifest_rows") == expected_rows &&
         catalog.fetch("manifest_identity_envelope").fetch(3) == header &&
         catalog.fetch("manifest_identity_envelope").fetch(4) == expected_rows &&
         manifest_encoded.unpack1("H*") == catalog.fetch("cbor_hex") &&
         manifest_encoded.bytesize == catalog.fetch("byte_length") &&
         Digest::SHA256.hexdigest(manifest_encoded) == catalog.fetch("manifest_id")
    raise "Observation manifest canonical bytes or identity differs"
  end
  expected_rows
end

def run_behavior(cargo, rustc)
  raise "Stage 5 behavior manifest identity differs" unless behavior_manifest_identity == EXPECTED_BEHAVIOR_MANIFEST_IDENTITY

  target = ENV.fetch("CARGO_TARGET_DIR")
  environment = {
    "CARGO_HOME" => ENV.fetch("CARGO_HOME"), "CARGO_INCREMENTAL" => "0",
    "CARGO_NET_OFFLINE" => "true", "CARGO_TARGET_DIR" => target,
    "CC" => ENV.fetch("CC"), "CXX" => ENV.fetch("CXX"), "AR" => ENV.fetch("AR"),
    "RANLIB" => ENV.fetch("RANLIB"), "HOME" => ENV.fetch("HOME"),
    "LANG" => "C", "LC_ALL" => "C", "PATH" => ENV.fetch("PATH"),
    "MAESTRO_VERSION" => ENV.fetch("MAESTRO_VERSION"),
    "RUSTC" => rustc, "TEMP" => ENV.fetch("TEMP"),
    "SDKROOT" => ENV.fetch("SDKROOT"), "TMP" => ENV.fetch("TMP"),
    "TMPDIR" => ENV.fetch("TMPDIR"), "TZ" => "UTC"
  }
  compile_command = %w[
    test --frozen --offline --no-run --message-format=json --lib
    --test vnext_evidence_claims --test vnext_submission_claim_set
    --test vnext_stage5_evidence_gates --test vnext_work_lifecycle
  ]
  stdout, stderr, status = Open3.capture3(
    environment, cargo, *compile_command, chdir: WORKSPACE
  )
  raise stderr[-8000..] unless status.success?

  executables = {}
  stdout.each_line do |line|
    begin
      message = JSON.parse(line)
    rescue JSON::ParserError
      next
    end
    next unless message["reason"] == "compiler-artifact" && message.dig("profile", "test")

    name = message.dig("target", "name")
    executable = message["executable"]
    next unless executable && EXPECTED_RUNS.any? { |row| row.fetch(1) == name }

    path = File.realpath(executable)
    raise "compiled test target #{name} is ambiguous" if executables.key?(name) && executables[name] != path

    executables[name] = path
  end
  expected_targets = EXPECTED_RUNS.map { |row| row.fetch(1) }.uniq.sort
  raise "compiled test target closure differs" unless executables.keys.sort == expected_targets

  receipts = EXPECTED_RUNS.map do |label, target, test_names|
    tests = test_names.map do |test_name|
      args = [test_name, "--exact", "--nocapture"]
      run_stdout, run_stderr, run_status = Open3.capture3(
        environment, executables.fetch(target), *args, chdir: WORKSPACE
      )
      output = run_stdout + run_stderr
      passed = exact_test_passed_count(output)
      raise output[-8000..] unless run_status.success? && passed == 1
      {
        "command" => [target, *args],
        "name" => test_name,
        "result" => "pass"
      }
    end
    {
      "binary_sha256" => Digest::SHA256.file(executables.fetch(target)).hexdigest,
      "label" => label,
      "passed" => test_names.length,
      "tests" => tests
    }
  end
  target = EXPECTED_RUNS.fetch(0).fetch(1)
  exact_name = EXPECTED_RUNS.fetch(0).fetch(2).fetch(0)
  substituted_name = "#{exact_name}_same_count_substitution_mutant"
  args = [substituted_name, "--exact", "--nocapture"]
  run_stdout, run_stderr, run_status = Open3.capture3(
    environment, executables.fetch(target), *args, chdir: WORKSPACE
  )
  passed = exact_test_passed_count(run_stdout + run_stderr)
  raise "same-count exact-test substitution was not rejected" unless run_status.success? && passed.zero?

  receipts << {
    "binary_sha256" => Digest::SHA256.file(executables.fetch(target)).hexdigest,
    "command" => [target, *args],
    "label" => "same-count-substitution-mutant",
    "passed" => 0,
    "rejected" => true,
    "result" => "rejected",
    "substituted_for" => exact_name
  }
  receipts
end

def exact_test_passed_count(output)
  match = output.match(/test result: ok\. (\d+) passed; 0 failed/)
  match ? match[1].to_i : -1
end

options = {}
OptionParser.new do |parser|
  parser.on("--artifact PATH") { |value| options[:artifact] = value }
  parser.on("--artifact-cbor PATH") { |value| options[:artifact_cbor] = value }
  parser.on("--output-root PATH") { |value| options[:output_root] = value }
  parser.on("--cargo PATH") { |value| options[:cargo] = value }
  parser.on("--rustc PATH") { |value| options[:rustc] = value }
  parser.on("--self-test-output-parser") { options[:self_test_output_parser] = true }
end.parse!
if options[:self_test_output_parser]
  raise "exact-test output parser rejected one passing test" unless exact_test_passed_count(
    "test result: ok. 1 passed; 0 failed"
  ) == 1
  raise "exact-test output parser admitted malformed output" unless exact_test_passed_count(
    "test result: FAILED. 1 passed; 1 failed"
  ) == -1
  raise "independent Ruby behavior manifest identity differs" unless behavior_manifest_identity == EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
  puts({
    "behavior_manifest_identity" => EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
    "exact_test_output_parser" => "pass"
  }.to_json)
  exit 0
end
%i[artifact artifact_cbor output_root cargo rustc].each { |key| raise "missing --#{key.to_s.tr('_', '-')}" unless options[key] }

artifact_bytes = File.binread(File.realpath(options.fetch(:artifact)))
artifact = JSON.parse(artifact_bytes)
raise "Stage 5 domain differs" unless artifact.fetch("schema_version") == DOMAIN
raise "Stage 5 publication state differs" unless artifact.fetch("publication_state") == "inactive_candidate"

catalog = JSON.parse(
  File.read(File.join(WORKSPACE, "contracts/vnext/catalogs/generated/catalog-01-observation.json"), encoding: Encoding::US_ASCII)
)
observations = observation_rows(catalog)

sources = SOURCE_PATHS.sort.map { |path| file_row(path) }
predecessors = PREDECESSOR_PATHS.map { |path| file_row(path) }
raise "source closure differs" unless artifact.fetch("source_closure") == sources
raise "predecessor closure differs" unless artifact.fetch("predecessors") == predecessors
raise "Observation closure differs" unless artifact.fetch("observation_kinds") == observations
unless artifact.fetch("observation_contract_table_identity") == OBSERVATION_CONTRACT_TABLE_IDENTITY
  raise "Observation runtime contract table differs"
end
raise "Stage 5 behavior manifest identity differs" unless artifact.fetch("behavior_manifest_identity") == EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
raise "Gate result closure differs" unless artifact.dig("protocol", "gate_results") == RESULTS
raise "Gate input closure differs" unless artifact.dig("protocol", "gate_input_classes") == INPUT_CLASSES
raise "Gate operator closure differs" unless artifact.dig("protocol", "gate_operators") == OPERATORS
raise "acquisition closure differs" unless artifact.dig("protocol", "acquisition_modes") == ACQUISITION_MODES
raise "invalidation closure differs" unless artifact.fetch("invalidation_reasons") == INVALIDATION_REASONS
raise "invariant closure differs" unless artifact.fetch("invariants") == INVARIANTS

semantic_value = [
  DOMAIN, "inactive_candidate", 5, catalog.fetch("manifest_id"),
  OBSERVATION_CONTRACT_TABLE_IDENTITY, observations, RESULTS,
  INPUT_CLASSES, OPERATORS, ACQUISITION_MODES, INVALIDATION_REASONS, INVARIANTS,
  sources, predecessors, EXPECTED_TESTS, EXPECTED_BEHAVIOR_MANIFEST_IDENTITY
]
encoded = encode(semantic_value)
artifact_id = Digest::SHA256.hexdigest(encoded)
raise "published canonical CBOR differs" unless File.binread(File.realpath(options.fetch(:artifact_cbor))) == encoded
raise "canonical CBOR differs" unless artifact.fetch("cbor_hex") == encoded.unpack1("H*")
raise "canonical byte length differs" unless artifact.fetch("byte_length") == encoded.bytesize
raise "artifact identity differs" unless artifact.fetch("artifact_id") == artifact_id

behavior_runs = run_behavior(File.realpath(options.fetch(:cargo)), File.realpath(options.fetch(:rustc)))
passed = behavior_runs.sum { |run| run.fetch("passed") }
receipt_value = {
  "artifact_id" => artifact_id,
  "artifact_sha256" => Digest::SHA256.hexdigest(artifact_bytes),
  "behavior_manifest_identity" => EXPECTED_BEHAVIOR_MANIFEST_IDENTITY,
  "behavior_passed" => passed,
  "behavior_runs" => behavior_runs,
  "publication_state" => "inactive_candidate",
  "schema_version" => "maestro.vnext.stage5.ruby-verification-receipt.v1",
  "source_closure_sha256" => Digest::SHA256.hexdigest(canonical_json(sources)),
  "verifier_sha256" => file_row("tools/vnext_contracts/stage5/evidence_gates/verify.rb")[2]
}
receipt = receipt_value.merge(
  "receipt_identity" => "sha256:#{Digest::SHA256.hexdigest(canonical_json(receipt_value))}"
)
FileUtils.mkdir_p(options.fetch(:output_root))
File.write(
  File.join(options.fetch(:output_root), "ruby-verification-receipt.v1.json"),
  JSON.pretty_generate(receipt, quirks_mode: true) + "\n",
  mode: "wb"
)
