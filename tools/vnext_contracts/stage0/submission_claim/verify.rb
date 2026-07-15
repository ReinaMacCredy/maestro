#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"

repo = File.expand_path("../../../..", __dir__)
artifact_path = File.join(repo, "contracts/vnext/stage0/submission-claim/submission-claim-set.v1.json")
receipt_path = File.join(repo, "contracts/vnext/stage0/submission-claim/encoder-receipt.v1.json")
artifact_raw = File.binread(artifact_path)
artifact = JSON.parse(artifact_raw)
receipt = JSON.parse(File.read(receipt_path, encoding: "UTF-8"))
domain = [artifact.fetch("domain_hex")].pack("H*")

def cbor_head(major, value)
  prefix = major << 5
  return [prefix | value].pack("C") if value < 24
  return [prefix | 24, value].pack("CC") if value <= 0xff
  return [prefix | 25, value].pack("Cn") if value <= 0xffff
  return [prefix | 26, value].pack("CN") if value <= 0xffffffff

  [prefix | 27, value].pack("CQ>")
end

def cbor(value)
  case value
  when Integer
    cbor_head(0, value)
  when String
    raw = value.b
    raise "non-ASCII schema text" unless raw.ascii_only?

    cbor_head(3, raw.bytesize) + raw
  when Array
    cbor_head(4, value.length) + value.map { |item| cbor(item) }.join
  else
    raise "unsupported schema value"
  end
end

def lp(raw)
  [raw.bytesize].pack("Q>") + raw
end

digests = artifact.fetch("vectors").map do |vector|
  entries = vector.fetch("entries")
  input = domain + lp(vector.fetch("submission_id").b) + [entries.length].pack("Q>")
  entries.each do |entry|
    input += lp(entry.fetch("claim_id").b)
    input += [entry.fetch("normalized_proposition_hash")].pack("H*")
    input += [entry.fetch("claim_record_hash")].pack("H*")
  end
  raise "digest-input mismatch" unless input.unpack1("H*") == vector.fetch("canonical_digest_input_hex")

  Digest::SHA256.hexdigest(input)
end

raise "artifact hash mismatch" unless Digest::SHA256.hexdigest(artifact_raw) == receipt.fetch("artifact_sha256")
raise "independent vector digest mismatch" unless digests == receipt.fetch("vector_digests")
descriptor_cbor = cbor(artifact.fetch("schema_descriptor"))
identity_input_cbor = cbor(["maestro.vnext.schema.v1", artifact.fetch("schema_descriptor")])
raise "descriptor CBOR mismatch" unless descriptor_cbor.unpack1("H*") == artifact.fetch("schema_descriptor_cbor_hex")
raise "descriptor CBOR hash mismatch" unless Digest::SHA256.hexdigest(descriptor_cbor) == artifact.fetch("schema_descriptor_cbor_sha256")
raise "schema identity input mismatch" unless identity_input_cbor.unpack1("H*") == artifact.fetch("schema_identity_input_cbor_hex")
raise "schema identity input hash mismatch" unless Digest::SHA256.hexdigest(identity_input_cbor) == artifact.fetch("schema_identity_input_cbor_sha256")
independent_schema_id = "sha256:" + Digest::SHA256.hexdigest(identity_input_cbor)
raise "independent schema identity mismatch" unless independent_schema_id == receipt.fetch("schema_id")
raise "semantic mutant closure mismatch" unless artifact.fetch("semantic_mutants_rejected").length == 10

puts JSON.generate(
  schema: "maestro.vnext.submission-claim-set-independent-verification.v1",
  encoder: "ruby-stdlib",
  artifact_sha256: receipt.fetch("artifact_sha256"),
  schema_id: receipt.fetch("schema_id"),
  vector_digests: digests,
  result: "verified"
)
