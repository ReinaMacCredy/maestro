#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"

ROOT = File.expand_path("../../../..", __dir__)
CONTRACT_ROOT = ENV.fetch(
  "STAGE0_PROOF_MATRIX_ROOT",
  File.join(ROOT, "contracts/vnext/stage0/proof-matrix")
)
MANIFEST_PATH = File.join(CONTRACT_ROOT, "stage0-proof-manifest.v1.json")
CBOR_PATH = File.join(CONTRACT_ROOT, "stage0-proof-manifest.v1.cbor")
DOMAIN = "maestro.vnext.stage0-proof-manifest.v1"
VERIFIED_NON_PROMOTING = "verified_non_promoting"
U64_MAX = 0xffff_ffff_ffff_ffff
EXPECTED_GATES = %w[
  external_input_authorization
  decision_closure
  catalog_predecessor
  incorporated_catalog_checkpoints
  catalog_successor
  public_contracts
  public_identity
  submission_claim
  dispatch
  effect_home
  resource_release
  current_surface_consumer_census
  persistence_archive_golden_fixtures
  migration_rollback
  root_assembly_source_binding
].each_with_index.map { |name, index| [index + 1, name].freeze }.freeze

class Bytes
  attr_reader :value

  def initialize(hexadecimal)
    raise "canonical byte strings must be lowercase SHA-256 hex" unless hexadecimal.is_a?(String) && hexadecimal.match?(/\A[0-9a-f]{64}\z/)

    @value = [hexadecimal].pack("H*").freeze
  end

  def ==(other)
    other.is_a?(Bytes) && other.value == value
  end
end

def cbor_head(major, value)
  raise "deterministic CBOR integers and lengths must be unsigned u64" unless value.is_a?(Integer) && value.between?(0, U64_MAX)

  return [(major << 5) | value].pack("C") if value < 24
  return [(major << 5) | 24, value].pack("CC") if value <= 0xff
  return [(major << 5) | 25, value].pack("Cn") if value <= 0xffff
  return [(major << 5) | 26, value].pack("CN") if value <= 0xffff_ffff

  [(major << 5) | 27, value].pack("CQ>")
end

def encode(value)
  case value
  when Bytes
    cbor_head(2, value.value.bytesize) + value.value
  when String
    raise "canonical text must be ASCII" unless value.ascii_only?

    raw = value.b
    cbor_head(3, raw.bytesize) + raw
  when Integer
    cbor_head(0, value)
  when Array
    cbor_head(4, value.length) + value.map { |item| encode(item) }.join
  else
    raise "value outside the Stage0ProofManifest deterministic CBOR subset: #{value.inspect}"
  end
end

def parse_canonical(value)
  case value
  when Array
    value.map { |item| parse_canonical(item) }
  when Hash
    raise "only canonical SHA-256 byte wrappers are permitted" unless value.keys == ["bytes"]

    Bytes.new(value.fetch("bytes"))
  when String, Integer
    value
  else
    raise "unsupported canonical JSON value: #{value.inspect}"
  end
end

def ensure_exact(actual, expected, label)
  raise "#{label} mismatch" unless actual == expected
end

def sort_json(value)
  case value
  when Hash
    value.keys.sort.to_h { |key| [key, sort_json(value.fetch(key))] }
  when Array
    value.map { |item| sort_json(item) }
  else
    value
  end
end

def json_bytes(value)
  (JSON.generate(sort_json(value)) + "\n").encode(Encoding::US_ASCII)
end

def validate_artifact_path(path, label)
  valid = path.is_a?(String) && !path.empty? && path.ascii_only? && !path.start_with?("/") && !path.include?("\\")
  valid &&= path.split("/", -1).none? do |component|
    component.empty? || component == "." || component == ".."
  end
  raise "#{label} must be a canonical repository-relative path" unless valid
end

def validate_artifacts(artifacts, label)
  raise "#{label} must be an array" unless artifacts.is_a?(Array)

  paths = artifacts.map.with_index do |pair, index|
    pair_label = "#{label}[#{index}]"
    raise "#{pair_label} must be [path, SHA-256 bytes]" unless pair.is_a?(Array) && pair.length == 2

    path, sha256 = pair
    validate_artifact_path(path, pair_label)
    raise "#{pair_label} must bind SHA-256 bytes" unless sha256.is_a?(Bytes) && sha256.value.bytesize == 32

    path
  end
  raise "#{label} must be sorted by unique path" unless paths == paths.sort && paths.uniq.length == paths.length
end

def validate_counts(counts, label)
  raise "#{label} must be an array" unless counts.is_a?(Array)

  names = counts.map.with_index do |pair, index|
    pair_label = "#{label}[#{index}]"
    raise "#{pair_label} must be [name, unsigned]" unless pair.is_a?(Array) && pair.length == 2

    name, count = pair
    raise "#{pair_label} name must be non-empty ASCII" unless name.is_a?(String) && !name.empty? && name.ascii_only?
    raise "#{pair_label} count must be unsigned u64" unless count.is_a?(Integer) && count.between?(0, U64_MAX)

    name
  end
  raise "#{label} must be sorted by unique name" unless names == names.sort && names.uniq.length == names.length
end

def projected_artifacts(rows, label)
  raise "#{label} must be an array" unless rows.is_a?(Array)

  rows.map.with_index do |row, index|
    row_label = "#{label}[#{index}]"
    raise "#{row_label} must contain only path and sha256" unless row.is_a?(Hash) && row.keys.sort == %w[path sha256]

    [row.fetch("path"), Bytes.new(row.fetch("sha256"))]
  end
end

def projected_counts(rows, label)
  raise "#{label} must be an array" unless rows.is_a?(Array)

  rows.map.with_index do |row, index|
    row_label = "#{label}[#{index}]"
    raise "#{row_label} must contain only name and value" unless row.is_a?(Hash) && row.keys.sort == %w[name value]

    [row.fetch("name"), row.fetch("value")]
  end
end

def validate_projection(projection, canonical, index)
  label = "gate projection #{index}"
  ensure_exact(projected_artifacts(projection.fetch("source_artifacts"), "#{label} sources"), canonical[2], "#{label} sources")
  ensure_exact(projected_artifacts(projection.fetch("validator_artifacts"), "#{label} validators"), canonical[3], "#{label} validators")
  ensure_exact(projected_artifacts(projection.fetch("input_artifacts"), "#{label} inputs"), canonical[4], "#{label} inputs")
  ensure_exact(projection.fetch("result_sha256"), canonical[7].value.unpack1("H*"), "#{label} result SHA-256")
  ensure_exact(projected_counts(projection.fetch("semantic_counts"), "#{label} semantic counts"), canonical[8], "#{label} semantic counts")

  return if canonical[0] == 1

  result_document = {
    "assertions" => projection.fetch("assertions", {}),
    "gate" => canonical[1],
    "input_artifacts" => projection.fetch("input_artifacts"),
    "result" => "passed",
    "semantic_counts" => projection.fetch("semantic_counts"),
    "source_artifacts" => projection.fetch("source_artifacts"),
    "validator_artifacts" => projection.fetch("validator_artifacts")
  }
  ensure_exact(
    projection.fetch("result_sha256"),
    Digest::SHA256.hexdigest(json_bytes(result_document)),
    "#{label} independently reproduced result SHA-256"
  )
end

def validate_gate(value, tag, name)
  label = "gate #{tag} (#{name})"
  raise "#{label} canonical value must contain exactly nine fields" unless value.is_a?(Array) && value.length == 9

  ensure_exact(value[0], tag, "#{label} tag")
  ensure_exact(value[1], name, "#{label} name")
  validate_artifacts(value[2], "#{label} sources")
  validate_artifacts(value[3], "#{label} validators")
  validate_artifacts(value[4], "#{label} inputs")
  raise "#{label} must bind at least one validator" if value[3].empty?
  ensure_exact(value[5], 1, "#{label} pass result tag")
  raise "#{label} result class must be lower snake case ASCII" unless value[6].is_a?(String) && value[6].match?(/\A[a-z_]+\z/)
  raise "#{label} result must bind SHA-256 bytes" unless value[7].is_a?(Bytes) && value[7].value.bytesize == 32
  validate_counts(value[8], "#{label} semantic counts")

  return unless tag == 1

  ensure_exact(value[2], [], "#{label} sources")
  ensure_exact(value[4], [], "#{label} inputs")
  ensure_exact(value[8], [], "#{label} semantic counts")
  ensure_exact(value[6], VERIFIED_NON_PROMOTING, "#{label} result class")
  ensure_exact(value[7].value, Digest::SHA256.digest(VERIFIED_NON_PROMOTING.b), "#{label} result hash")
end

document = JSON.parse(File.read(MANIFEST_PATH, encoding: Encoding::UTF_8))
ensure_exact(document.fetch("schema"), DOMAIN, "manifest schema")
ensure_exact(document.fetch("candidate_only"), true, "candidate-only marker")
ensure_exact(document.fetch("runtime_activation"), false, "runtime activation marker")
ensure_exact(document.fetch("gate_count"), EXPECTED_GATES.length, "gate count")

gates = document.fetch("gates")
raise "manifest gates must be an array" unless gates.is_a?(Array)
ensure_exact(gates.length, EXPECTED_GATES.length, "manifest gate length")

canonical = parse_canonical(document.fetch("canonical_value"))
raise "canonical manifest must be [1, gates]" unless canonical.is_a?(Array) && canonical.length == 2
ensure_exact(canonical[0], 1, "canonical manifest version")
raise "canonical gate set must be an array" unless canonical[1].is_a?(Array)
ensure_exact(canonical[1].length, EXPECTED_GATES.length, "canonical gate count")

EXPECTED_GATES.each_with_index do |(tag, name), index|
  projection = gates.fetch(index)
  raise "gate projection #{index} must be an object" unless projection.is_a?(Hash)
  ensure_exact(projection.fetch("tag"), tag, "gate projection #{index} tag")
  ensure_exact(projection.fetch("name"), name, "gate projection #{index} name")
  ensure_exact(projection.fetch("result"), "passed", "gate projection #{index} result")
  ensure_exact(projection.fetch("result_class"), canonical[1][index][6], "gate projection #{index} result class")
  ensure_exact(projection["result_tag"], 1, "gate projection #{index} result tag") if projection.key?("result_tag")
  validate_gate(canonical[1][index], tag, name)
  validate_projection(projection, canonical[1][index], index)
end

encoded = encode(canonical)
ensure_exact(File.binread(CBOR_PATH), encoded, "canonical CBOR file")
canonical_sha256 = Digest::SHA256.hexdigest(encoded)
ensure_exact(document.fetch("canonical_cbor_sha256"), canonical_sha256, "canonical CBOR SHA-256")
if document.key?("canonical_cbor_byte_length")
  ensure_exact(document.fetch("canonical_cbor_byte_length"), encoded.bytesize, "canonical CBOR byte length")
end

identity_sha256 = Digest::SHA256.hexdigest(encode([DOMAIN, canonical]))
ensure_exact(document.fetch("identity"), "sha256:#{identity_sha256}", "manifest identity")

puts JSON.generate(
  "schema" => "maestro.vnext.stage0-proof-manifest-ruby-verification.v1",
  "status" => "pass",
  "encoder" => "ruby-independent",
  "identity" => document.fetch("identity"),
  "canonical_cbor_sha256" => canonical_sha256,
  "canonical_cbor_byte_length" => encoded.bytesize,
  "gate_count" => gates.length,
  "semantic_validation" => "pass"
)
