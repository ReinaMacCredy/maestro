#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "etc"
require "json"
require "open3"
require "optparse"
require "pathname"

WORKSPACE = File.expand_path("../../../..", __dir__)
DOMAIN = "maestro.vnext.stage4.execution-effects.v1"
PUBLICATION_STATE = "inactive_candidate"
DISPATCH_ATTEMPT = "Dispatch" + "AttemptV1"
RECONCILIATION_ATTEMPT = "Reconciliation" + "AttemptV1"
EFFECT_INTENT = "Effect" + "IntentV1"
EFFECT_CONTROL_HEAD = "Effect" + "Intent" + "Control" + "HeadV1"
WITHDRAWAL_SCHEMA = "Effect" + "Intent" + "WithdrawalV1"
PREDECESSOR_RECEIPTS = {
  "stage0_effect_home" => %w[
    contracts/vnext/stage0/effect-home/encoder-receipt.json
    contracts/vnext/stage0/effect-home/finalization-receipt.v1.json
  ],
  "stage0_dispatch_cutover" => %w[
    contracts/vnext/stage0/dispatch-cutover/build-receipt.v1.json
    contracts/vnext/stage0/dispatch-cutover/validation-receipt.v1.json
  ],
  "stage2_authority" => %w[
    contracts/vnext/stage2/authority/python-encoder-receipt.v1.json
    contracts/vnext/stage2/authority/semantic-validation-receipt.v1.json
    contracts/vnext/stage2/authority/ruby-verification-receipt.v1.json
  ],
  "stage3_domain" => %w[
    contracts/vnext/stage3/domain/python-encoder-receipt.v1.json
    contracts/vnext/stage3/domain/semantic-validation-receipt.v1.json
    contracts/vnext/stage3/domain/ruby-verification-receipt.v1.json
  ],
}.freeze
PREDECESSOR_MANIFESTS = {
  "stage0_effect_home" => "contracts/vnext/stage0/effect-home/finalization-receipt.v1.json",
  "stage0_dispatch_cutover" => "contracts/vnext/stage0/dispatch-cutover/validation-receipt.v1.json",
  "stage2_authority" => "contracts/vnext/stage2/authority/stage2-authority-manifest.v1.json",
  "stage3_domain" => "contracts/vnext/stage3/domain/domain-kernel.v1.json",
}.freeze
PREDECESSOR_COMMANDS = [
  %w[python3 tools/vnext_contracts/stage0/effect_home/build.py --check],
  %w[python3 tools/vnext_contracts/stage0/effect_home/validate.py --mutants],
  %w[python3 tools/vnext_contracts/stage0/dispatch_cutover/build.py --check],
  %w[python3 tools/vnext_contracts/stage0/dispatch_cutover/validate.py --mutant-suite --no-write],
  %w[python3 tools/vnext_contracts/stage2/authority/build.py --check],
  %w[python3 tools/vnext_contracts/stage3/domain/build.py --check],
].freeze
CATALOG_PATHS = %w[
  contracts/vnext/catalogs/generated/inventory.json
  contracts/vnext/catalogs/generated/catalog-profile-grammar-v1.json
  contracts/vnext/catalogs/generated/catalog-02-effect.json
  contracts/vnext/catalogs/generated/catalog-06-action-leaf.json
  contracts/vnext/catalogs/generated/catalog-09-action-spec.json
].freeze
EXPECTED_CATALOGS = {
  "catalog-profile-grammar-v1.json" => ["b7ef635dcd29af4fc41f20cd670b726e5627c2f7210344d058e7c188ace69647", 156],
  "catalog-02-effect.json" => ["d28f8e573ddb450c427e628df121dbd516d0e5b05c03caf18d2757782dfd259d", 23],
  "catalog-06-action-leaf.json" => ["b2f538d76795db0338448cc8cb837419157c1bebdc8bcc7d7b42fd961790d454", 145],
  "catalog-09-action-spec.json" => ["7a7a1311690fcdec194c0d9c9afbbe4e28f1332b567ed23451daf06ebca02970", 145],
}.freeze
DISPATCH_PATH = "contracts/vnext/stage0/dispatch-cutover/dispatch-attempt-state.v1.json"
WITHDRAWAL_PATH = "contracts/vnext/stage0/effect-home/effect-withdrawal-v1.json"
COMPILATION_ANCESTORS = %w[
  Cargo.toml
  Cargo.lock
  build.rs
  src/lib.rs
  src/domain/mod.rs
  src/domain/vnext/mod.rs
  src/foundation/mod.rs
  src/foundation/core/mod.rs
  src/foundation/core/deterministic_cbor.rs
].freeze
AUTHORITY_EXTENSION_SOURCES = %w[
  src/domain/vnext/authority/action_basis.rs
  src/domain/vnext/authority/continuity/trusted_time.rs
  src/domain/vnext/authority/downstream_action_basis.rs
  src/domain/vnext/authority/facade.rs
  src/domain/vnext/authority/facade/repository_admission.rs
  src/domain/vnext/authority/facade/repository_leaf_authority.rs
  src/domain/vnext/authority/governance_attestation.rs
  src/domain/vnext/authority/governance_attestation_stage7_seed.rs
  src/domain/vnext/authority/materialization.rs
  src/domain/vnext/authority/mod.rs
  src/domain/vnext/installation/durable_finality.rs
  src/domain/vnext/installation/durable_finality_stage9_seed.rs
  src/domain/vnext/installation/durable_finality_stage11_seed.rs
  src/domain/vnext/persistence/protected_locator_lease.rs
  src/domain/vnext/persistence/protected_locator_stage9_seed.rs
  src/foundation/core/secure_fs.rs
  src/foundation/core/aggregate_census.rs
  src/foundation/core/aggregate_census_stage11_seed.rs
  src/foundation/core/descriptor_census_platform.rs
  src/foundation/core/descriptor_census_platform_stage11_seed.rs
].freeze
FOCAL_STEP_EVIDENCE_SOURCES = %w[
  src/domain/vnext/evidence/mod.rs
  src/domain/vnext/evidence/submission_claim.rs
  src/domain/vnext/evidence/claim.rs
  src/domain/vnext/step/lifecycle.rs
  src/domain/vnext/step/submission.rs
].freeze
TOOL_SOURCES = %w[
  tests/vnext_stage4_contracts.rs
  tools/vnext_contracts/catalogs/cbor_py.py
  tools/vnext_contracts/stage4/execution/build.py
  tools/vnext_contracts/stage4/execution/test_behavior_census.py
  tools/vnext_contracts/stage4/execution/validate.py
  tools/vnext_contracts/stage4/execution/verify.rb
].freeze
BEHAVIOR_COMMANDS = [
  %w[cargo test --lib domain::vnext::execution:: -- --nocapture],
  %w[cargo test --lib domain::vnext::authority::facade::repository_admission::ancestry_tests -- --nocapture],
  %w[cargo test --lib domain::vnext::authority::continuity::trusted_time::tests -- --nocapture],
    %w[cargo test --test vnext_stage4_contracts stage4_public_effect_facade_exports_are_complete -- --nocapture],
    %w[cargo test --test vnext_stage4_contracts runtime_withdrawal_catalog_matches_all_sixty_frozen_rows_and_twenty_one_denials -- --nocapture],
    %w[cargo test --test vnext_effect_home_literals stage0_effect_home_artifacts_are_reproducible_and_reject_mutants -- --nocapture],
  ].freeze
MUTANT_COMMANDS = [
  %w[cargo test --test vnext_stage4_contracts stage4_regenerated_ -- --nocapture],
  %w[cargo test --test vnext_stage4_contracts stage4_proof_rejects_ -- --nocapture],
  %w[cargo test --test vnext_stage4_contracts independent_execution_artifact_rejects_semantic_and_shape_mutants -- --nocapture],
].freeze
  BEHAVIOR_EXPECTED_PASSED = [70, 7, 1, 1, 1, 1].freeze
MUTANT_EXPECTED_PASSED = [10, 6, 1].freeze
SANITIZED_ENVIRONMENT_KEYS = %w[
  CARGO_BUILD_TARGET CARGO_ENCODED_RUSTFLAGS CARGO_HOME CARGO_INCREMENTAL CARGO_TARGET_DIR CC CFLAGS HOME LDFLAGS
  MACOSX_DEPLOYMENT_TARGET PATH RUSTC RUSTC_WORKSPACE_WRAPPER RUSTC_WRAPPER RUSTDOC RUSTFLAGS
  RUSTUP_HOME RUSTUP_TOOLCHAIN
].freeze
UNSET_BUILD_OVERRIDE_KEYS = %w[
  CARGO_BUILD_TARGET CARGO_ENCODED_RUSTFLAGS CARGO_TARGET_DIR RUSTC_WORKSPACE_WRAPPER RUSTC_WRAPPER
  RUSTDOC RUSTFLAGS
].freeze
EXECUTION_MODEL = [
  [
    "ExecutionAttemptV1", "closed_union",
    [
      ["StepAttemptV1", "exact_step_binding_and_live_step_lease_fence", "step_execution_runs_only"],
      [DISPATCH_ATTEMPT, "one_durable_effect_intent_and_dispatch_fence", "dispatch_runs_only"],
      [RECONCILIATION_ATTEMPT, "one_durable_effect_intent_and_fresh_action_request", "reconciliation_runs_only"],
    ],
    "one_run_exactly_one_owner", "no_fourth_owner",
  ],
  [
    "StepLeaseV1", "step_only", "one_to_one_with_step_attempt", "exact_generation_step_binding",
    "immutable_contiguous_hash_linked_terms",
    "takeover_requires_exact_owner_receipt_binding_predecessor_term_fences_and_trusted_time_cut",
    "stage4_consumes_owner_issued_takeover_safety_stage5_evidence_owns_production_issuance_and_decode",
    "missing_owner_evidence_fails_closed",
    %w[submitted yielded failed cancelled timed_out lost fenced],
    "terminal_never_reopens",
  ],
  [
    "RunV1", "finite_and_owned",
    [
      ["reserved", %w[active definitely_not_started cancelled timed_out lost fenced]],
      ["active", %w[succeeded failed cancelled timed_out lost fenced]],
    ],
      "terminal_never_reopens", "run_success_is_not_step_or_remote_success",
      "definitely_not_started_requires_non_self_attested_run_boundary_receipt",
      "store_issues_no_start_receipt_from_current_authority_time_and_pinned_boundary_observer",
      "timed_out_only_at_or_after_exact_deadline",
  ],
  [
    EFFECT_INTENT, "stable_across_step_amendment_removal_and_recovery",
    "home_qualified_stable_subject_semantic_uniqueness_independent_of_provider_envelope",
    "committed_with_dispatch_reservation_before_io",
    "one_current_control_head", "durable_uncertainty",
  ],
  [
    EFFECT_CONTROL_HEAD, "sole_mutable_selector", %w[None Reserved Sealed],
    %w[prepared dispatching pending in_doubt confirmed_applied confirmed_not_applied partially_applied conflicted cancelled],
    "ten_legal_seventeen_denied_products", "same_store_atomic_expected_old_publication",
  ],
  [
    RECONCILIATION_ATTEMPT, "fresh_authorized_action_request_and_use_fence", "same_durable_intent",
    "one_action_admission_and_capacity_debit_at_begin", "read_and_terminal_reuse_exact_attempt_authority",
      "no_dispatch", "no_lease", "no_stale_step_mutation", "unknown_may_refine_but_never_infer",
      "read_release_is_ephemeral_non_clone_consumed_once_and_deadline_guarded",
      "replayed_begin_never_reconstructs_read_release",
  ],
  [
    "ExecutionAuthorityV1", "closed_union",
    [
      ["Ordinary", "ordinary_live_runtime_only"],
      ["BootstrapG0", "exact_nondelegable_genesis_grant"],
      ["ContinuityMaintenance", "exact_branch_phase_slot_and_executor"],
    ],
    "continuity_slot_binds_purpose_request_subject_epoch_and_job_applicability",
    "exact_leaf_selects_one_basis", "no_cross_basis_donation",
  ],
  [
    "ActiveStoreDomainParityV1", %w[RepositoryDomain InstallationDomain],
    "same_atomic_control_product", %w[RepositoryExternalEffect InstallationExternalEffect],
    "stable_home_and_generation_bound", "cross_domain_refused",
  ],
  [
    "ProtectedCeremonyEffectStoreV1", %w[NoStoreProtectedCas PreStoreProtectedCas], 11,
    %w[Initiate RecoverReserved ResolveResult Withdraw], "durable_expected_old_carrier",
    "exact_managed_root_and_unique_database_leaf_anchor",
    "one_winner_exact_replay", "zero_provider_io", "request_requires_opaque_owner_issued_authority",
      "carrier_owns_no_authority", "external_owner_authority_retains_secret_carrier_persists_commitment_only",
      "managed_root_database_and_rollback_journal_custody_reverified_on_every_operation",
      "rollback_journal_create_is_exclusive_nofollow_and_open_descriptor_verified_before_commit",
      "sqlite_connection_identity_uses_documented_file_controls_only",
  ],
  [
    "ProviderApplicationReleaseV1", "ephemeral_non_clone", "winner_only",
      "never_persisted_or_reconstructed", "sealed_capability_has_no_public_accessor",
      "consuming_single_use_gateway", "exact_run_boundary_and_deadline_binding",
      "serialized_store_gateway_loads_current_control_head_and_current_authority_time_at_io_boundary",
      "writer_handoff_linearizes_before_or_after_external_io_never_during_release_validation",
      "fresh_current_trusted_time_must_be_strictly_before_deadline", "deadline_refusal_performs_zero_provider_io",
    "terminal_and_withdrawal_require_disposition",
  ],
  [
    "WriterHandoffAndHealthV1", "store_issued_same_home_fence", "old_writer_fenced",
    "one_head_writer_winner", %w[Healthy RecoveryRequired IntegrityBlocked],
    "unhealthy_blocks_behavior", "integrity_blocked_blocks_handoff",
  ],
  [
    "retry_policy", "no_retry_engine", "no_fresh_key_after_uncertainty",
    "safe_redispatch_is_typed_action_on_same_intent_only_when_conclusive",
    "in_doubt_survives_crash_restore_and_step_disposition",
    "definitely_not_started_requires_boundary_observation_not_caller_assertion",
  ],
  [
    "StepSubmissionV1", "step_owned_submission_stores_exact_claim_set_digest_only",
    ["ClaimV1", "SubmissionClaimSetV1", "evidence_owned_immutable_participant"],
    "execution_validates_binding_and_atomic_preconditions_without_owning_claim_semantics",
    "persistence_commits_submission_claimset_claims_and_step_closure_in_one_generation",
    %w[one_claim n_claims], %w[submit_vs_submit submit_vs_renew submit_vs_takeover],
    "loser_zero_write_zero_debit",
  ],
].freeze
INVARIANTS = %w[
  execution_attempt_union_is_exactly_three_owners
  every_run_has_exactly_one_execution_attempt_owner
  step_lease_is_step_attempt_only_and_one_to_one
  step_takeover_requires_exact_non_self_attested_owner_safety_proof
  stage4_takeover_safety_is_consumer_only_and_production_issuance_decode_belongs_to_stage5_evidence
  missing_stage5_takeover_evidence_fails_closed_without_self_attestation
  dispatch_and_reconciliation_provenance_never_donates_step_authority
  effect_intent_and_dispatch_reservation_commit_before_external_io
  dispatch_attempt_has_exactly_four_typed_outcomes
  sealed_or_ambiguous_crossing_is_durably_in_doubt
  reconciliation_requires_fresh_current_authorization
  one_reconciliation_attempt_spends_exactly_one_action_capacity
  reconciliation_never_dispatches_or_mutates_stale_step_state
  execution_authority_is_closed_three_branch_and_rejects_basis_donation
  continuity_maintenance_slot_is_nontransferable_across_purpose_subject_request_epoch_or_applicability
  effect_semantic_uniqueness_is_stable_across_provider_key_or_envelope_changes
  active_store_effects_have_repository_and_installation_domain_parity
  provider_release_is_ephemeral_winner_only_and_never_reconstructed
  effect_intent_control_head_is_the_only_current_selector
  store_publication_is_atomic_expected_old_and_zero_io
  protected_ceremony_matrix_is_eleven_by_four_durable_expected_old_cas
  protected_ceremony_database_leaf_identity_is_anchored_and_aba_replacement_is_refused
  protected_ceremony_carrier_is_non_authorizing_and_publicly_read_only
  protected_ceremony_request_requires_opaque_owner_issued_authority
  protected_ceremony_requests_have_owner_bound_canonical_bytes_for_post_crash_exact_replay
  protected_ceremony_reads_and_commit_acknowledgements_bind_the_sqlite_connection_descriptor_to_the_anchored_leaf
  protected_ceremony_revalidates_managed_root_database_and_rollback_journal_custody_on_every_operation
  protected_ceremony_uses_only_documented_sqlite_file_controls_and_public_connection_paths
  writer_handoff_uses_store_issued_same_home_fence_and_one_head_winner
  recovery_required_and_integrity_blocked_products_fail_closed
    terminal_attempt_closes_runs_and_clears_live_dispatch_atomically
    definitely_not_started_requires_an_opaque_run_term_boundary_and_time_receipt
    run_no_start_receipt_is_store_issued_from_current_authority_time_and_pinned_boundary_observation
    run_timeout_is_refused_before_the_exact_deadline
    provider_and_reconciliation_io_releases_are_consuming_non_clone_capabilities_without_raw_accessors
    provider_and_reconciliation_adapters_execute_inside_one_serialized_current_store_view
    provider_and_reconciliation_io_are_refused_at_or_after_deadline_before_adapter_invocation
    writer_handoff_and_external_io_have_one_serial_order
    replayed_reconciliation_begin_never_reconstructs_external_read_authority
    persistence_head_cas_loss_projects_to_execution_stale_expected_state
  withdrawal_has_exactly_sixty_legal_cells_and_twenty_one_denied_products
  withdrawal_performs_no_provider_io_and_creates_no_intent_attempt_or_run
  uncertainty_survives_crash_restore_amendment_removal_and_supersession
  no_blind_retry_no_fresh_key_retry_and_no_retry_engine
  step_submission_contains_only_the_exact_claim_set_digest_and_no_embedded_claim_records
  evidence_owns_claim_and_claim_set_semantics_step_owns_submission_and_execution_cannot_reauthor_them
  one_and_n_claim_participants_commit_with_step_closure_in_one_atomic_generation
  submit_vs_submit_submit_vs_renew_and_submit_vs_takeover_races_have_loser_zero_write_zero_debit
  stage5_gate_and_non_submission_evidence_implementation_is_outside_stage4_source_closure
  all_runtime_mutations_use_frozen_nominal_action_or_ceremony_owners
].freeze
REQUIRED_SOURCE_GROUPS = [
  ["ExecutionAttemptV1", "StepAttemptV1", DISPATCH_ATTEMPT, RECONCILIATION_ATTEMPT],
  %w[ExecutionAuthorityV1 BootstrapExecutionAuthorityV1 ContinuityMaintenanceExecutionAuthorityV1 job_applicability_commitment],
  %w[StepLeaseV1 LeaseTermV1 TakeoverSafetyV1 owner_receipt_commitment],
    %w[RunV1 RunNoStartReceiptV1 RunExecutionTimeReceiptV1 PinnedExecutionBoundaryObserverV1 issue_run_no_start_receipt],
  [EFFECT_INTENT, EFFECT_CONTROL_HEAD],
    %w[ProtectedCeremonyOwnerAuthorityV1 ProtectedCeremonyAuthorityV1 ProtectedCeremonyCarrierAnchorV1 ProtectedCeremonyEffectStoreV1 owner_basis_commitment decode_request canonical_bytes verify_connection_leaf verify_live_connection verify_rollback_journal_custody protected_ceremony_vfs_open SQLITE_OPEN_EXCLUSIVE SQLITE_FCNTL_JOURNAL_POINTER SQLITE_FCNTL_HAS_MOVED sqlite3_db_filename TransactionBehavior::Immediate],
    %w[ProviderApplicationReleaseV1 ReconciliationReadReleaseV1 RunExecutionTimeReceiptV1 execute_provider_once execute_reconciliation_read_once current_repository_authority_time with_serialized_active_view map_store_error HeadCasMismatch StaleExternalIoRelease HandoffWriter IntegrityBlocked],
  %w[StoreRoleV1::Repository StoreRoleV1::Installation],
  %w[publish transaction],
].freeze
FOCAL_STEP_EVIDENCE_REQUIRED_SOURCE_GROUPS = [
  %w[StepSubmissionV1 claim_set_digest],
  %w[SubmissionClaimSetV1 digest],
  %w[ClaimV1 EvidenceClaimPublicationV1],
].freeze

def sha(bytes)
  Digest::SHA256.hexdigest(bytes)
end

def load_object(relative)
  value = JSON.parse(File.read(File.join(WORKSPACE, relative), encoding: Encoding::US_ASCII))
  raise "artifact must contain one object: #{relative}" unless value.is_a?(Hash)

  value
end

def payload(document)
  document.fetch("canonical_value", document.fetch("value", document))
end

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
  else raise "unsupported Stage 4 canonical value: #{value.inspect}"
  end
end

def receipt_rows(paths)
  paths.map do |relative|
    bytes = File.binread(File.join(WORKSPACE, relative))
    { "byte_length" => bytes.bytesize, "path" => relative, "sha256" => sha(bytes) }
  end
end

def executable_path(name)
  ENV.fetch("PATH", "").split(File::PATH_SEPARATOR).each do |directory|
    candidate = File.join(directory, name)
    return File.expand_path(candidate) if File.file?(candidate) && File.executable?(candidate)
  end
  raise "required Stage 4 proof executable is unavailable: #{name}"
end

def tool_descriptor(name)
  invocation = executable_path(name)
  resolved = File.realpath(invocation)
  bytes = File.binread(resolved)
  {
    "invocation_path" => invocation,
    "resolved_path" => resolved,
    "sha256" => sha(bytes),
    "byte_length" => bytes.bytesize,
  }
end

def bound_environment_value(key, environment)
  value = environment.fetch(key, "<unset>")
  return value unless key == "PATH" && value != "<unset>"

  value.split(File::PATH_SEPARATOR).map do |component|
    component.match?(%r{/\.codex/tmp/arg0/codex-arg0[^/]*\z}) ? "<codex-transient-arg0>" : component
  end.join(File::PATH_SEPARATOR)
end

def command_environment
  home = Etc.getpwuid.dir
  {
    "CARGO_HOME" => File.join(home, ".cargo"),
    "CARGO_INCREMENTAL" => "0",
    "HOME" => home,
    "LANG" => "C",
    "LC_ALL" => "C",
    "PATH" => "/usr/bin:/bin:/usr/sbin:/sbin",
    "PYTHONDONTWRITEBYTECODE" => "1",
    "RUSTC" => tool_descriptor("rustc").fetch("invocation_path"),
    "RUSTUP_HOME" => File.join(home, ".rustup"),
  }
end

def canonical_json(value)
  normalized = case value
               when Hash
                 value.keys.sort.to_h { |key| [key, canonical_json_value(value.fetch(key))] }
               else
                 canonical_json_value(value)
               end
  JSON.generate(normalized)
end

def canonical_json_value(value)
  case value
  when Hash
    value.keys.sort.to_h { |key| [key, canonical_json_value(value.fetch(key))] }
  when Array
    value.map { |item| canonical_json_value(item) }
  else
    value
  end
end

def command_result_digest(payload)
  sha(canonical_json(payload))
end

def test_binary_receipt_matches(binary)
  return false unless binary.is_a?(Hash) && binary["path"].is_a?(String)

  path = Pathname.new(binary["path"])
  path = Pathname.new(WORKSPACE).join(path) unless path.absolute?
  bytes = File.binread(File.realpath(path))
  binary["byte_length"] == bytes.bytesize && binary["sha256"] == sha(bytes)
rescue Errno::ENOENT, Errno::EACCES, ArgumentError
  false
end

def execute_commands(commands, label)
  commands.map do |command|
    executable = tool_descriptor(command.first)
    stdout, stderr, status = Open3.capture3(
      command_environment,
      executable.fetch("invocation_path"),
      *command.drop(1),
      chdir: WORKSPACE,
    )
    unless status.success?
      detail = stderr.strip.empty? ? stdout.strip : stderr.strip
      raise "#{label} failed: #{command.join(' ')}: #{detail}"
    end
    {
      "command" => command,
      "executable" => executable,
      "exit_code" => 0,
      "result" => "pass",
      "stdout_sha256" => sha(stdout),
      "stderr_sha256" => sha(stderr),
    }
  end
end

def execute_test_commands(commands, expected_passed, label)
  raise "#{label} expectation cardinality drifted" unless commands.length == expected_passed.length

  commands.zip(expected_passed).map do |command, expected|
    executable = tool_descriptor(command.first)
    stdout, stderr, status = Open3.capture3(
      command_environment,
      executable.fetch("invocation_path"),
      *command.drop(1),
      chdir: WORKSPACE,
    )
    output = "#{stdout}\n#{stderr}"
    raise "#{label} failed: #{command.join(' ')}: #{output.strip}" unless status.success?

    outcomes = output.scan(/^test (.+) \.\.\. (ok|ignored)$/)
    names = outcomes.select { |(_, result)| result == "ok" }.map(&:first).sort
    ignored = outcomes.select { |(_, result)| result == "ignored" }.map(&:first).sort
    summaries = output.scan(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;/)
    unless summaries == [[expected.to_s, "0", "0"]] && names.length == expected && ignored.empty?
      raise "#{label} did not execute exactly #{expected} passing non-ignored tests"
    end
    binaries = output.scan(/Running [^\n]*\(([^)]+)\)/).flatten
    raise "#{label} did not expose exactly one compiled test binary" unless binaries.length == 1

    binary_path = Pathname.new(binaries.first)
    binary_path = Pathname.new(WORKSPACE).join(binary_path) unless binary_path.absolute?
    binary_path = Pathname.new(File.realpath(binary_path))
    bytes = File.binread(binary_path)
    workspace_path = Pathname.new(WORKSPACE)
    display_path = begin
      binary_path.relative_path_from(workspace_path).to_s
    rescue ArgumentError
      binary_path.to_s
    end
    outcome = {
      "command" => command,
      "ignored" => 0,
      "passed" => expected,
      "test_binary" => {
        "byte_length" => bytes.bytesize,
        "path" => display_path,
        "sha256" => sha(bytes),
      },
      "test_names" => names,
    }
    outcome.merge(
      "executable" => executable,
      "exit_code" => 0,
      "normalized_output_sha256" => command_result_digest(outcome),
      "result" => "pass",
    )
  end
end

def verify_recorded_test_receipts(receipts, commands, expected_passed, label)
  raise "#{label} receipt cardinality drifted" unless receipts.is_a?(Array) && receipts.length == commands.length

  receipts.zip(commands, expected_passed).each do |row, command, expected|
    raise "#{label} command binding drifted" unless row.is_a?(Hash) && row["command"] == command

    names = row["test_names"]
    binary = row["test_binary"]
    outcome = {
      "command" => command,
      "ignored" => 0,
      "passed" => expected,
      "test_binary" => binary,
      "test_names" => names,
    }
    valid = row["executable"] == tool_descriptor(command.first) &&
            row["exit_code"] == 0 && row["result"] == "pass" && row["ignored"] == 0 &&
            row["passed"] == expected && names.is_a?(Array) && names.length == expected &&
            names.uniq.length == expected && test_binary_receipt_matches(binary) &&
            row["normalized_output_sha256"] == command_result_digest(outcome)
    raise "#{label} exact test census receipt drifted" unless valid
  end
  receipts
end

def predecessor_command_receipts
  return $predecessor_command_receipts if defined?($predecessor_command_receipts) && $predecessor_command_receipts

  $predecessor_command_receipts = execute_commands(
    PREDECESSOR_COMMANDS,
    "Stage 4 predecessor validation",
  )
end

def predecessor_binding
  stage0 = load_object(PREDECESSOR_MANIFESTS.fetch("stage0_effect_home"))
  dispatch = load_object(PREDECESSOR_MANIFESTS.fetch("stage0_dispatch_cutover"))
  stage2 = load_object(PREDECESSOR_MANIFESTS.fetch("stage2_authority"))
  stage3 = load_object(PREDECESSOR_MANIFESTS.fetch("stage3_domain"))
  unless stage0["finalization_state"] == "final" && stage0["candidate_only"] == true && stage0["runtime_activation"] == false
    raise "Stage 0 effect-home predecessor is not final, candidate-only, and inactive"
  end
  raise "Stage 0 dispatch predecessor is not passing and inactive" unless dispatch["status"] == "pass" && dispatch["runtime_activated"] == false
  raise "Stage 2 or Stage 3 predecessor is not inactive" unless stage2["publication_state"] == PUBLICATION_STATE && stage3["publication_state"] == PUBLICATION_STATE

  %w[stage3_domain].each do |group|
    PREDECESSOR_RECEIPTS.fetch(group).each do |relative|
      raise "predecessor receipt skipped the full chain: #{relative}" unless load_object(relative)["validation_mode"] == "full_chain"
    end
  end
  roots = {
    "stage0_effect_home" => stage0.fetch("identity"),
    "stage0_dispatch_cutover" => "sha256:#{sha(File.binread(File.join(WORKSPACE, PREDECESSOR_MANIFESTS.fetch('stage0_dispatch_cutover'))))}",
    "stage2_authority" => "sha256:#{stage2.fetch('root_id')}",
    "stage3_domain" => stage3.fetch("identity"),
  }
  unless roots.values.all? { |root| root.is_a?(String) && root.match?(/\Asha256:[0-9a-f]{64}\z/) }
    raise "Stage 4 predecessor chain has a missing semantic root"
  end
  {
    "command_receipts" => predecessor_command_receipts,
    "mode" => "full_chain",
    "roots" => roots,
    "proof_receipts" => PREDECESSOR_RECEIPTS.to_h { |group, paths| [group, receipt_rows(paths)] },
  }
end

def predecessor_canonical
  binding = predecessor_binding
  [
    "full_chain",
    %w[stage0_effect_home stage0_dispatch_cutover stage2_authority stage3_domain].map do |group|
      [
        group,
        binding.fetch("roots").fetch(group),
        binding.fetch("proof_receipts").fetch(group).map { |row| [row.fetch("path"), row.fetch("byte_length"), row.fetch("sha256")] },
      ]
    end,
    binding.fetch("command_receipts").map do |row|
      executable = row.fetch("executable")
      [
        row.fetch("command"),
        [
          executable.fetch("invocation_path"), executable.fetch("resolved_path"),
          executable.fetch("byte_length"), executable.fetch("sha256"),
        ],
        row.fetch("exit_code"), row.fetch("result"),
        row.fetch("stdout_sha256"), row.fetch("stderr_sha256"),
      ]
    end,
  ]
end

def catalog_binding
  inventory = load_object(CATALOG_PATHS.first)
  counts = inventory.fetch("semantic_counts")
  actual_counts = %w[actions ceremonies effect_origins effect_routes execution_attempt_owners grammar_symbols].map { |key| counts[key] }
  raise "frozen catalog counts are not 145/11/23/139/3/156" unless actual_counts == [145, 11, 23, 139, 3, 156]

  inventory_rows = inventory.fetch("artifacts").to_h { |row| [row.fetch("path"), row] }
  rows = CATALOG_PATHS.drop(1).map do |relative|
    name = File.basename(relative)
    identity, count = EXPECTED_CATALOGS.fetch(name)
    document = load_object(relative)
    actual_identity = name.start_with?("catalog-profile") ? document.dig("catalog_profile_grammar", "catalog_profile_grammar_id") : document["manifest_id"]
    bytes = File.binread(File.join(WORKSPACE, relative))
    row = inventory_rows.fetch(name)
    valid = actual_identity == identity && row["identity"] == identity && row["row_count"] == count && row["sha256"] == sha(bytes)
    raise "frozen catalog drifted: #{name}" unless valid

    [name, identity, count, sha(bytes)]
  end
  ["frozen_public_catalogs", [145, 11, 23, 139, 3, 156], rows]
end

def dispatch_binding
  value = payload(load_object(DISPATCH_PATH))
  outcomes = [[1, "locally_rejected", 1], [2, "definitely_not_sent", 2], [3, "response_received", 2], [4, "ambiguous_transport", 2]]
  raise "dispatch outcome payload closure drifted" unless value.is_a?(Array) && value.length >= 8 && value[4] == outcomes

  bytes = File.binread(File.join(WORKSPACE, DISPATCH_PATH))
  ["dispatch_attempt", outcomes, bytes.bytesize, sha(bytes)]
end

def withdrawal_binding
  value = payload(load_object(WITHDRAWAL_PATH))
  valid = value.is_a?(Array) && value.length == 6 && value[0, 2] == [WITHDRAWAL_SCHEMA, 1] &&
          value[2].length == 60 && value[4].length == 21 && value[2].map(&:first).uniq.sort == %w[confirmed_not_applied prepared]
  raise "withdrawal closure drifted from exact 60 legal and 21 denied products" unless valid

  bytes = File.binread(File.join(WORKSPACE, WITHDRAWAL_PATH))
  ["withdrawal", 60, 21, value[3], value[4], value[5], bytes.bytesize, sha(bytes)]
end

def rust_sources(root)
  Dir.glob(File.join(WORKSPACE, root, "**", "*.rs")).map { |path| path.delete_prefix("#{WORKSPACE}/") }.sort
end

def execution_sources
  paths = rust_sources("src/domain/vnext/execution")
  raise "live Execution source root is absent" unless paths.include?("src/domain/vnext/execution/mod.rs")

  paths
end

def persistence_sources
  paths = rust_sources("src/domain/vnext/persistence")
  raise "persistence source root is absent" unless paths.include?("src/domain/vnext/persistence/mod.rs")

  paths
end

def contract_ownership_sources
  paths = rust_sources("src/domain/vnext/contract")
  raise "Contract ownership source root is absent" unless paths.include?("src/domain/vnext/contract/mod.rs")

  paths
end

def source_paths
  paths = (
    CATALOG_PATHS + [DISPATCH_PATH, WITHDRAWAL_PATH] + PREDECESSOR_MANIFESTS.values +
    PREDECESSOR_RECEIPTS.values.flatten + COMPILATION_ANCESTORS + AUTHORITY_EXTENSION_SOURCES +
    FOCAL_STEP_EVIDENCE_SOURCES + contract_ownership_sources + execution_sources +
    persistence_sources + [
      "src/domain/vnext/installation/consumer_snapshot.rs",
      "src/domain/vnext/installation/consumer_snapshot_stage11_seed.rs",
      "src/domain/vnext/installation/mod.rs",
      "src/domain/vnext/integration/consumer_closure.rs"
    ] + TOOL_SOURCES
  ).uniq.sort
  leaked = paths.any? do |path|
    path.start_with?("src/domain/vnext/gate/") ||
      (path.start_with?("src/domain/vnext/evidence/") && !FOCAL_STEP_EVIDENCE_SOURCES.include?(path))
  end
  raise "Stage 5 Gate or non-submission Evidence source leaked into Stage 4 closure" if leaked

  paths
end

def verify_runtime_source
  text = execution_sources.map { |path| File.read(File.join(WORKSPACE, path), encoding: Encoding::UTF_8) }.join("\n")
  REQUIRED_SOURCE_GROUPS.each do |group|
    raise "live Execution source lacks runtime semantics: #{group}" unless group.all? { |marker| text.include?(marker) }
  end
  definition = "pub struct SubmissionClaimSetV1"
  definition_owners = rust_sources("src/domain/vnext").select do |path|
    File.read(File.join(WORKSPACE, path), encoding: Encoding::UTF_8).include?(definition)
  end
  unless definition_owners == ["src/domain/vnext/evidence/submission_claim.rs"]
    raise "SubmissionClaimSetV1 must have exactly one Evidence-owned definition"
  end
  contract_text = contract_ownership_sources.map do |path|
    File.read(File.join(WORKSPACE, path), encoding: Encoding::UTF_8)
  end.join("\n")
  raise "Contract cannot define or re-export SubmissionClaimSetV1" if contract_text.include?("SubmissionClaimSetV1")
  focal_text = FOCAL_STEP_EVIDENCE_SOURCES.map do |path|
    File.read(File.join(WORKSPACE, path), encoding: Encoding::UTF_8)
  end.join("\n")
  FOCAL_STEP_EVIDENCE_REQUIRED_SOURCE_GROUPS.each do |group|
    unless group.all? { |marker| focal_text.include?(marker) }
      raise "live Step/Evidence submission participants are incomplete: #{group}"
    end
  end
  raise "candidate-only execution literals cannot satisfy Stage 4" if text.include?("Stage 4 is the only future implementation owner")
  %w[from_owner_receipt from_canonical_value].each do |constructor|
    marker = "#[cfg(test)]\n    pub(crate) fn #{constructor}("
    unless text.scan(marker).length == 1
      raise "Stage 4 takeover safety must remain consumer-only until Stage 5 owner Evidence"
    end
  end

  persistence = persistence_sources.map { |path| File.read(File.join(WORKSPACE, path), encoding: Encoding::UTF_8) }.join("\n")
  bound = ["crate::domain::vnext::persistence", "super::super::persistence"].any? { |marker| text.include?(marker) }
  raise "Execution does not bind the canonical persistence owner" unless bound
  raise "persistence source lacks atomic publication semantics" unless %w[transaction publish].all? { |marker| persistence.include?(marker) }
end

def source_rows
  source_paths.map do |relative|
    bytes = File.binread(File.join(WORKSPACE, relative))
    [relative, bytes.bytesize, sha(bytes)]
  end
end

def toolchain_binding
  environment = command_environment
  descriptors = %w[cargo rustc python3 ruby].map do |name|
    descriptor = tool_descriptor(name)
    [
      name, descriptor.fetch("invocation_path"), descriptor.fetch("resolved_path"),
      descriptor.fetch("byte_length"), descriptor.fetch("sha256"),
    ]
  end
  cargo_home = Pathname.new(environment.fetch("CARGO_HOME", File.join(Dir.home, ".cargo")))
  config_rows = [
    Pathname.new(WORKSPACE).join(".cargo/config.toml"),
    Pathname.new(WORKSPACE).join(".cargo/config"),
    cargo_home.join("config.toml"),
    cargo_home.join("config"),
  ].select(&:file?).map do |path|
    bytes = File.binread(path)
    [File.realpath(path), bytes.bytesize, sha(bytes)]
  end
  commands = [
    [tool_descriptor("rustc").fetch("invocation_path"), "-vV"],
    [tool_descriptor("cargo").fetch("invocation_path"), "-Vv"],
    [tool_descriptor("rustc").fetch("invocation_path"), "--print", "cfg"],
  ]
  results = commands.map { |command| Open3.capture3(environment, *command, chdir: WORKSPACE) }
  raise "Stage 4 could not bind the active Rust toolchain and target cfg" unless results.all? { |(_, _, status)| status.success? }

  [
    "proof_toolchain_environment_and_target_v2",
    descriptors,
    SANITIZED_ENVIRONMENT_KEYS.map { |key| [key, bound_environment_value(key, environment)] },
    config_rows,
    results[0][0].strip.lines.map(&:chomp),
    results[1][0].strip.lines.map(&:chomp),
    results[2][0].strip.lines.map(&:chomp).reject(&:empty?).sort,
  ]
end

root = File.join(WORKSPACE, "contracts/vnext/stage4/execution")
artifact_only = false
source_only = false
skip_mutants = false
parent_certification_identity = nil
OptionParser.new do |options|
  options.on("--root ROOT") { |value| root = value }
  options.on("--artifact-only") { artifact_only = true }
  options.on("--source-only") { source_only = true }
  options.on("--skip-mutants") { skip_mutants = true }
  options.on("--parent-certification-identity IDENTITY") { |value| parent_certification_identity = value }
end.parse!

verify_runtime_source
if source_only
  puts "Stage 4 runtime source semantics valid"
  exit 0
end
if skip_mutants
  unless parent_certification_identity&.match?(/\Asha256:[0-9a-f]{64}\z/)
    raise "nested Stage 4 verification requires an exact parent certification identity"
  end
  validation_mode = "nested_subset"
else
  raise "full-chain Stage 4 verification cannot claim a parent identity" unless parent_certification_identity.nil?

  validation_mode = "full_chain"
end
expected = [
  DOMAIN, 1, PUBLICATION_STATE, predecessor_canonical, catalog_binding, EXECUTION_MODEL,
  dispatch_binding, withdrawal_binding, INVARIANTS, source_rows, toolchain_binding,
]
manifest = JSON.parse(File.read(File.join(root, "execution-effects.v1.json"), encoding: Encoding::US_ASCII))
raise "Stage 4 manifest fields drifted" unless manifest.keys.sort == %w[canonical_value identity publication_state schema_version]
raise "Stage 4 semantic projection drifted" unless manifest.fetch("canonical_value") == expected
encoded = encode(expected)
raise "Stage 4 CBOR drifted" unless File.binread(File.join(root, "execution-effects.v1.cbor")) == encoded
identity = "sha256:#{sha(encoded)}"
raise "Stage 4 identity drifted" unless manifest.fetch("identity") == identity
raise "Stage 4 schema or publication state drifted" unless manifest.fetch("schema_version") == DOMAIN && manifest.fetch("publication_state") == PUBLICATION_STATE
encoder_receipt = JSON.parse(File.read(File.join(root, "python-encoder-receipt.v1.json"), encoding: Encoding::US_ASCII))
unless encoder_receipt["identity"] == identity && encoder_receipt["validation_mode"] == validation_mode
  raise "Stage 4 encoder receipt identity or validation mode drifted"
end
if skip_mutants
  unless encoder_receipt["parent_certification_identity"] == parent_certification_identity
    raise "nested Stage 4 encoder receipt lost its parent certification identity"
  end
elsif encoder_receipt.key?("parent_certification_identity")
  raise "full-chain Stage 4 encoder receipt claimed a parent certification identity"
end
if artifact_only
  puts identity
  exit 0
end
behavior = JSON.parse(File.read(File.join(root, "behavioral-proof-receipt.v1.json"), encoding: Encoding::US_ASCII))
command_receipts = verify_recorded_test_receipts(
  behavior["command_receipts"], BEHAVIOR_COMMANDS, BEHAVIOR_EXPECTED_PASSED,
  "recorded Stage 4 behavior",
)
mutant_command_receipts = if skip_mutants
                            []
                          else
                            verify_recorded_test_receipts(
                              behavior["mutant_command_receipts"], MUTANT_COMMANDS,
                              MUTANT_EXPECTED_PASSED, "recorded Stage 4 mutants",
                            )
                          end
expected_behavior = {
  "command_receipts" => command_receipts,
  "commands" => BEHAVIOR_COMMANDS,
  "identity" => identity,
  "mutant_command_receipts" => mutant_command_receipts,
  "mutant_commands" => MUTANT_COMMANDS,
  "mutant_validation" => skip_mutants ? "nested_skip" : "executed",
  "result" => "pass",
  "schema_version" => "#{DOMAIN}.behavioral-proof-receipt.v1",
  "validation_mode" => validation_mode,
  "validator" => "compiled-rust-execution-contracts",
}
expected_behavior["parent_certification_identity"] = parent_certification_identity if skip_mutants
raise "Stage 4 behavioral proof receipt drifted" unless behavior == expected_behavior
receipt = {
  "identity" => identity,
  "predecessor_chain" => predecessor_binding,
  "schema_version" => "#{DOMAIN}.ruby-verification-receipt.v1",
  "validation_mode" => validation_mode,
  "validator" => "independent-ruby-reconstruction",
}
  if skip_mutants
    receipt["parent_certification_identity"] = parent_certification_identity
  else
    fresh_behavior = execute_test_commands(
      BEHAVIOR_COMMANDS, BEHAVIOR_EXPECTED_PASSED, "independent Ruby Stage 4 behavior",
    )
    fresh_mutants = execute_test_commands(
      MUTANT_COMMANDS, MUTANT_EXPECTED_PASSED, "independent Ruby Stage 4 mutants",
    )
    unless fresh_behavior == command_receipts && fresh_mutants == mutant_command_receipts
      raise "independent Ruby reexecution diverged from the certified test census or binary"
    end
    receipt["behavioral_reexecution"] = {
      "command_receipts" => fresh_behavior,
      "mutant_command_receipts" => fresh_mutants,
    }
end
File.write(File.join(root, "ruby-verification-receipt.v1.json"), JSON.pretty_generate(receipt) + "\n", encoding: Encoding::US_ASCII)
puts identity
