#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"
require "pathname"

ROOT = Pathname.new(__dir__).join("../../../..").cleanpath
CONTRACT = Pathname.new(ENV.fetch("STAGE0_DISPATCH_CUTOVER_ROOT", ROOT.join("contracts/vnext/stage0/dispatch-cutover").to_s))

ARTIFACTS = {
  "dispatch" => "dispatch-attempt-state.v1",
  "expected_delta" => "expected-delta-manifest.v1",
  "migration_candidate" => "migration-cutover-successor.v1"
}.freeze

DELTA_NAMES = [
  "7138_public_contract",
  "d116_bounded_recovery",
  "h2_causal_join",
  "h3_cancellation_label",
  "efa0_core_catalogs",
  "c868_behavioral_suite",
  "release_binding",
  "writer_compatibility"
].freeze

def fail_validation(message)
  warn(message)
  exit(1)
end

def head(major, value)
  prefix = major << 5
  return [prefix | value].pack("C") if value < 24
  return [prefix | 24, value].pack("CC") if value <= 0xff
  return [prefix | 25, value].pack("Cn") if value <= 0xffff
  return [prefix | 26, value].pack("CN") if value <= 0xffff_ffff
  return [prefix | 27, value].pack("CQ>") if value <= 0xffff_ffff_ffff_ffff

  raise "unsigned integer exceeds u64"
end

def encode(value)
  case value
  when true then "\xf5".b
  when false then "\xf4".b
  when Integer
    raise "negative integer" if value.negative?
    head(0, value)
  when String
    raise "non-ASCII text" unless value.ascii_only?
    raw = value.b
    head(3, raw.bytesize) + raw
  when Array
    head(4, value.length) + value.map { |item| encode(item) }.join
  when Hash
    raise "unsupported map" unless value.keys == ["bytes"]
    raw = [value.fetch("bytes")].pack("H*")
    raise "bytes32 required" unless raw.bytesize == 32
    head(2, raw.bytesize) + raw
  else
    raise "outside deterministic CBOR subset: #{value.inspect}"
  end
end

def ensure_exact(actual, expected, label)
  fail_validation("#{label} mismatch") unless actual == expected
end

def validate_dispatch(document)
  value = document.fetch("canonical_value")
  ensure_exact(value[0, 2], [1, "maestro.vnext.dispatch-attempt-state.v1"], "dispatch header")
  ensure_exact(value[2], [[1, "reserved_unsealed", 0, 0], [2, "sealed_in_flight", 1, 0], [3, "terminal", 0, 1]], "dispatch states")
  ensure_exact(value[3], [[1, "pre_seal_locally_rejected", 0, [1]], [2, "sealed_dispatch_terminal", 1, [2, 3, 4]]], "dispatch terminal union")
  ensure_exact(value[4], [[1, "locally_rejected", 1], [2, "definitely_not_sent", 2], [3, "response_received", 2], [4, "ambiguous_transport", 2]], "dispatch outcomes")
  ensure_exact(value[5], [[1, [0], 3, [1, 1]], [1, [0], 2, [0]], [2, [0], 3, [1, 2]]], "dispatch transition matrix")
  fields = value[6]
  ensure_exact(fields.map { |row| row[0] }, (1..14).to_a, "dispatch binding tags")
  ensure_exact(fields.map { |row| row[1] }, %w[attempt_id attempt_revision effect_intent_home_id effect_intent_use_fence_id application_envelope_id provider_operation_contract_id provider_scope_id provider_key_id credential_id authority_basis_id dispatch_fence_id material_stamp_id run_set_revision_id accounting_basis_id], "dispatch binding fields")
  ensure_exact(value[7], [1, "seal_id", "seal_is_exact_binding_snapshot", fields], "dispatch seal carry")
  ensure_exact(value[8].map { |row| row[0] }, (1..14).to_a, "dispatch invariant tags")
  ensure_exact(value[9], [1, 1, "successful_live_seal_cas_caller_only", false], "dispatch race descriptor")
  ensure_exact(value[10], [1, 0, false, false, false, false, %w[bounded_handle reconcile]], "dispatch recovery descriptor")
  ensure_exact(document["runtime_activated"], false, "dispatch runtime status")
  ensure_exact(document["outcome_count"], 4, "dispatch outcome count")
  ensure_exact(document["legal_transition_count"], 3, "dispatch transition count")
end

def validate_delta(document)
  value = document.fetch("canonical_value")
  ensure_exact(value[0, 2], [1, "maestro.vnext.migration-cutover-expected-delta.v1"], "delta header")
  rows = value[2]
  ensure_exact(rows.length, 8, "delta count")
  ensure_exact(rows.map { |row| row[0] }, (1..8).to_a, "delta tags")
  ensure_exact(rows.map { |row| row[1] }, DELTA_NAMES, "delta names")
  rows.each do |row|
    ensure_exact(row[3], [0], "unresolved delta identity")
    ensure_exact(row[4], true, "blocking delta")
  end
  ensure_exact(rows[5][2].length, 3, "c868 predecessor evidence")
  fail_validation("c868 delta lost 38/62/61 semantics") unless rows[5][5].include?("38_62_61")
  ensure_exact(document["successor_ids"], Array.new(8), "delta successor IDs")
  ensure_exact(document["publication_status"], "blocked_unresolved_dependencies", "delta status")
end

def validate_migration(document, delta_id)
  value = document.fetch("canonical_value")
  ensure_exact(value[0, 2], [1, "maestro.vnext.migration-cutover-successor-candidate.v1"], "migration header")
  ensure_exact(value[2], [[1, "schemas", 12], [2, "invariants", 23], [3, "predecessors", 10], [4, "components", 50], [5, "finality_schema_ids", 3], [6, "finality_edge_rows", 11], [7, "read_write_cohorts", 4], [8, "read_write_rows_per_cohort", 46], [9, "c868_schemas", 38], [10, "c868_suite_components", 62], [11, "c868_runtime_edges", 61]], "migration counts")
  predecessors = value[3]
  ensure_exact(predecessors.length, 10, "migration predecessor count")
  ensure_exact(predecessors.map { |row| row[0] }, (1..10).to_a, "migration predecessor tags")
  ensure_exact(predecessors.map { |row| row[2]["bytes"] }.uniq.length, 10, "migration predecessor uniqueness")
  evidence = value[4]
  ensure_exact(evidence[2].length, 3, "predecessor finality schema count")
  current = value[5]
  ensure_exact(current[0], ["successor_manifest_id", [0]], "successor manifest blocker")
  ensure_exact(current[1], ["finality_schema_ids", [[0], [0], [0]]], "successor finality schema blockers")
  current.each { |row| fail_validation("fabricated current identity") unless row[1] == [0] || row[1] == [[0], [0], [0]] }
  ensure_exact(value[6].length, 15, "association binding field count")
  ensure_exact(value[7], [[1, "repository", [0]], [2, "installation", [1, "exact_release_id"]]], "Release matrix")
  ensure_exact(value[8][0], [1, "active_store", ["distribution_receipt", "distribution_commit_record"], ["atomic", "migration_cutover_association", "owning_head"]], "ActiveStore finality")
  ensure_exact(value[8][1], [2, "pre_store", ["sealed_ceremony_attempt"], ["atomic", "migration_cutover_association", "candidate_seal", "protected_expected_old_cas"]], "PreStore finality")
  ensure_exact(value[9].length, 11, "currentness refusal matrix")
  ensure_exact(value[9].map { |row| row[0] }, (1..11).to_a, "currentness refusal tags")
  policies = value[10].to_h { |row| [row[1], row[2]] }
  ensure_exact(policies["association_is_typed_atomic_participant"], true, "association participant")
  ensure_exact(policies["association_consumed_exactly_once"], true, "association single use")
  ensure_exact(policies["filename_or_sidecar_inference"], false, "sidecar inference")
  ensure_exact(policies["old_reader_admission"], false, "old reader refusal")
  ensure_exact(policies["h2_causal_join_promotes_evidence"], false, "H2 nonpromotion")
  ensure_exact(policies["h3_cancel_label_promotes_evidence"], false, "H3 nonpromotion")
  ensure_exact(policies["partial_finality_is_current"], false, "partial finality")
  ensure_exact(value[11], [[1, 46], [2, 46], [3, 46], [4, 46]], "read/write cohorts")
  ensure_exact(value[12][0, 3], [38, 62, 61], "c868 semantics")
  ensure_exact(value[12][5, 2], [[0], [0]], "rotated c868 identities")
  ensure_exact(value[13]["bytes"], delta_id, "expected delta binding")
  ensure_exact(value[14].map { |row| row[1] }, DELTA_NAMES, "migration blockers")
  ensure_exact(document["successor_manifest_id"], nil, "successor ManifestId")
  ensure_exact(document["current_finality_schema_ids"], [nil, nil, nil], "current finality SchemaIds")
  ensure_exact(document["publication_status"], "blocked_unresolved_dependencies", "migration status")
  ensure_exact(document["runtime_activated"], false, "migration runtime status")
end

documents = {}
ARTIFACTS.each do |label, stem|
  json_path = CONTRACT.join("#{stem}.json")
  cbor_path = CONTRACT.join("#{stem}.cbor")
  document = JSON.parse(json_path.read)
  envelope = [document.fetch("identity_domain"), document.fetch("canonical_value")]
  encoded = encode(envelope)
  ensure_exact(cbor_path.binread, encoded, "#{label} CBOR")
  digest = Digest::SHA256.hexdigest(encoded)
  ensure_exact(document.fetch("candidate_literal_id"), digest, "#{label} identity")
  ensure_exact(document.fetch("cbor_sha256"), digest, "#{label} CBOR digest")
  ensure_exact(document.fetch("byte_length"), encoded.bytesize, "#{label} byte length")
  documents[label] = document
end

validate_dispatch(documents.fetch("dispatch"))
validate_delta(documents.fetch("expected_delta"))
validate_migration(documents.fetch("migration_candidate"), documents.fetch("expected_delta").fetch("candidate_literal_id"))

puts JSON.generate({
  "status" => "pass",
  "encoder" => "ruby-independent",
  "artifact_ids" => documents.transform_values { |document| document.fetch("candidate_literal_id") },
  "semantic_validation" => "pass",
  "blocked_dependencies" => 8
})
