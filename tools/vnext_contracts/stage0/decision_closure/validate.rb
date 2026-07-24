#!/usr/bin/env ruby
# frozen_string_literal: true

# Independent Ruby encoder and semantic validator for Stage-0 Decision closure.

require "digest"
require "json"

EXTERNAL_DOMAIN = "maestro.vnext.external-design-authority-closure.v1"
DECISION_DOMAIN = "maestro.vnext.decision-closure.v1"
SUCCESSOR_DECISION_STORE = "18f14bce862e15be09c9d88155d62627582df50c7754e2e8e1d6f6bee8f7d522"
SUCCESSOR_HEADS = {
  "dec-canonical-authority-materialization-df3b" => ["locked", "0d7c406f68f04fdf47ce00d56e8189b54159f164323c9511504790b941f715d0", "624f81c44b1a6459bc13472df05f547276d694e0f38c7216bb8df732aa3418cf"],
  "dec-canonical-execution-h3-verified-0939" => ["locked", "b5935c389182a7f3ec6447fb2a13dcb70e912108b399d0b1d25fee5f132186a7", "a98f1fdb95fcb3f2604936f50e9aa6661ad75bd51469d576e49239c5a6138307"],
  "dec-canonical-foundation-descriptor-a128" => ["locked", "17fb79ef9bc74cf3838d869bf5fb3b0ae0e9ae017670ca7cb207aeb8105c234e", "59fc4db26ec24f2f2ddc2df5cd70462f767e5d7e2d81644edc11a61c7fb7b26c"],
  "dec-canonical-installation-consumer-c1fe" => ["locked", "aaba56a8f34fb293a68f26743fbf4ef879d9f5a399a4eb45da74eed70a509e53", "5f35840fed183b406baab4cf9044ab05e3677f7061798c7949e80a868d2cd466"],
  "dec-canonical-non-action-protected-90a9" => ["locked", "8c6be56db78d8695b4e85e09fc4217257fee0b2dce0f5b5be8ef10230f24c20e", "7f0ea93dddef6354183b48cec27f6dee47f802688956bf552e3cb64ecca88f81"],
  "dec-canonical-trusted-host-protected-1fbc" => ["locked", "e572dc28e0c811c81207558e64b0372f757a873122b7f537f6354af819f118d8", "e6e84dea058097be48312ef98154958246763bac7f38d877ea04aee4af030d99"]
}.freeze

class Bytes
  attr_reader :value
  def initialize(hex)
    @value = [hex].pack("H*")
  end
end

def head(major, value)
  raise "unsigned u64 required" unless value.is_a?(Integer) && value.between?(0, 0xffffffffffffffff)
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
  when Bytes then head(2, value.value.bytesize) + value.value
  when String
    raw = value.encode(Encoding::US_ASCII).b
    head(3, raw.bytesize) + raw
  when Array then head(4, value.length) + value.map { |item| encode(item) }.join
  else raise "unsupported deterministic CBOR value #{value.class}"
  end
end

def optional(value)
  value.nil? ? [0] : [1, value]
end

def raw_bytes(record)
  Bytes.new(record.fetch("raw_record_bytes").fetch("bytes"))
end

def external_record(record)
  [record.fetch("id"), record.fetch("terminal_status"), Bytes.new(record.fetch("raw_record_sha256")), Bytes.new(record.fetch("raw_body_sha256")), record.fetch("raw_supersedes"), record.fetch("raw_superseded_by"), record.fetch("external_authoring_disposition"), optional(record["normalized_successor"]), record.fetch("consequence_classification"), optional(record["rationale_disposition"]), record.fetch("materialization_ids").map { |item| Bytes.new(item) }, record.fetch("derived_effect_status"), raw_bytes(record)]
end

def decision_record(record)
  [record.fetch("id"), record.fetch("terminal_status"), Bytes.new(record.fetch("raw_record_sha256")), Bytes.new(record.fetch("raw_body_sha256")), record.fetch("raw_supersedes"), record.fetch("raw_superseded_by"), record.fetch("external_authoring_disposition"), optional(record["normalized_successor"]), record.fetch("consequence_classification"), optional(record["rationale_disposition"]), record.fetch("materialization_ids").map { |item| Bytes.new(item) }, record.fetch("derived_effect_status")]
end

def materialization(item)
  [Bytes.new(item.fetch("id")), item.fetch("artifact_id"), item.fetch("component_kind_tag"), 0, item.fetch("decision_sources").map { |source| [source.fetch("id"), Bytes.new(source.fetch("body_sha256"))] }]
end

def canonical_value(document, external)
  lineage = document.fetch("lineage")
  records = document.fetch("records").map { |item| external ? external_record(item) : decision_record(item) }
  ignored = lineage.fetch("ignored_unilateral_claims").map { |item| [item.fetch("source"), item.fetch("claimed_predecessor")] }
  composites = lineage.fetch("composite_external_heads").map { |item| [item.fetch("id"), item.fetch("raw_supersedes")] }
  value = [1, records, document.fetch("materializations").map { |item| materialization(item) }, ignored, composites]
  value << lineage.fetch("recognized_external_composite_heads") if external
  value
end

def validate(document, external)
  raise "closure not closed" unless document.fetch("closure_state") == "closed"
  decisions_sha256 = document.fetch("source_provenance_excluded_from_identity").fetch("decisions_sha256")
  summary = case decisions_sha256
  when "1f97e67b156d5a17d13b94ff955ad17efeb3bb71a4b74b1aec14e20dac1100dd"
    { "total" => 207, "locked" => 112, "superseded" => 95, "open" => 0, "material" => 204, "rationale_only" => 3, "unresolved_mappings" => 0, "pending_component_slots" => 109, "normalized_one_to_one_edges" => 23 }
  when SUCCESSOR_DECISION_STORE
    { "total" => 213, "locked" => 117, "superseded" => 96, "open" => 0, "material" => 210, "rationale_only" => 3, "unresolved_mappings" => 0, "pending_component_slots" => 114, "normalized_one_to_one_edges" => 24 }
  else
    raise "unknown Decision-store provenance"
  end
  raise "summary drift" unless document.fetch("summary") == summary
  materialization_base = { "kind" => "initial_external_design_closure", "decision_closure_id" => external ? document.fetch("decision_closure_reference") : document.fetch("identity") }
  root_assembly = { "state" => "pending_exact_component_resolution", "resolved_component_ids" => [], "materialization_base" => materialization_base, "candidate_root_after" => nil, "finalization_manifest_id" => nil }
  raise "fabricated or incomplete root resolution" unless document.fetch("root_assembly") == root_assembly
  records = document.fetch("records")
  ids = records.map { |item| item.fetch("id") }
  raise "records unsorted or duplicate" unless ids == ids.sort && ids.uniq.length == summary.fetch("total")
  index = records.to_h { |record| [record.fetch("id"), record] }
  if decisions_sha256 == SUCCESSOR_DECISION_STORE
    manifest_bytes = records.map do |record|
      [
        record.fetch("id"),
        record.fetch("terminal_status"),
        record.fetch("raw_record_sha256"),
        record.fetch("raw_body_sha256")
      ].join("\t")
    end.join("\n") + "\n"
    raise "all-Decision successor manifest reconstruction mismatch" unless
      Digest::SHA256.hexdigest(manifest_bytes) == SUCCESSOR_DECISION_STORE
    SUCCESSOR_HEADS.each do |decision_id, expected|
      record = index.fetch(decision_id) { raise "missing successor Decision head: #{decision_id}" }
      actual = [record.fetch("terminal_status"), record.fetch("raw_record_sha256"), record.fetch("raw_body_sha256")]
      raise "substituted successor Decision head: #{decision_id}" unless actual == expected
    end
    ignored = document.fetch("lineage").fetch("ignored_unilateral_claims").map do |item|
      [item.fetch("source"), item.fetch("claimed_predecessor")]
    end
    unless ignored.include?(["dec-canonical-non-action-protected-90a9", "dec-canonical-trusted-host-protected-1fbc"])
      raise "missing protected-diagnostic unilateral-claim refusal"
    end
  end
  records.each do |record|
    seen = []
    current = record.fetch("id")
    until current.nil?
      raise "normalized successor cycle" if seen.include?(current)

      seen << current
      current = index.fetch(current)["normalized_successor"]
    end
  end
  materialization_rows = document.fetch("materializations")
  raise "duplicate materialization" unless materialization_rows.map { |item| item.fetch("id") }.uniq.length == materialization_rows.length
  used = []
  records.each do |record|
    raise "raw record mismatch" if external && Digest::SHA256.hexdigest(raw_bytes(record).value) != record.fetch("raw_record_sha256")
    raise "invalid terminal status" unless %w[locked superseded].include?(record.fetch("terminal_status"))
    raise "raw lineage omission" if record.fetch("terminal_status") == "superseded" && record.fetch("raw_superseded_by").empty?
    raise "composite promotion" if record.fetch("external_authoring_disposition") == "composite_external_authoring" && !record["normalized_successor"].nil?
    raise "unilateral repair" if record.fetch("external_authoring_disposition") == "unilateral_raw_claim" && !record["normalized_successor"].nil?
    if record.fetch("consequence_classification") == "rationale_only"
      raise "missing rationale disposition" if record["rationale_disposition"].nil? || !record.fetch("materialization_ids").empty?
    else
      raise "missing materialization" if record.fetch("materialization_ids").empty?
      raise "effect-live coverage missing" unless %w[unapplied superseded_but_effect_live].include?(record.fetch("derived_effect_status"))
    end
    successor = record["normalized_successor"]
    raise "unknown successor" if !successor.nil? && !index.key?(successor)
  end
  materialization_rows.each do |item|
    raise "materialization base drift" unless item.fetch("materialization_base") == materialization_base
    raise "materialization must remain pending exact root resolution" unless item.fetch("binding_state") == "required_component_slot_pending" && !item.key?("before_root_id") && %w[exact_component_id after_root_id finalization_manifest_id].all? { |key| item[key].nil? }
    sources = item.fetch("decision_sources")
    source_ids = sources.map { |source| source.fetch("id") }
    raise "duplicate materialization source" unless source_ids == source_ids.sort && source_ids.uniq.length == source_ids.length
    sources.each do |source|
      record = index.fetch(source.fetch("id"))
      raise "stale materialization" unless source.fetch("body_sha256") == record.fetch("raw_body_sha256")
      raise "nonreciprocal materialization" unless record.fetch("materialization_ids").include?(item.fetch("id"))
      used << source.fetch("id")
    end
  end
  expected = records.select { |record| record.fetch("consequence_classification") == "material" }.map { |record| record.fetch("id") }
  raise "incomplete materialization closure" unless used.sort == expected.sort
  value = canonical_value(document, external)
  encoded = encode(value)
  domain = external ? EXTERNAL_DOMAIN : DECISION_DOMAIN
  identity = Digest::SHA256.hexdigest(encode([domain, value]))
  raise "identity mismatch" unless document.fetch("identity") == "sha256:#{identity}"
  raise "CBOR hash mismatch" unless document.fetch("canonical_cbor_sha256") == Digest::SHA256.hexdigest(encoded)
  [identity, encoded]
end

root = ENV.fetch("STAGE0_DECISION_CLOSURE_ROOT", File.expand_path("../../../../contracts/vnext/stage0/decision-closure", __dir__))
external = JSON.parse(File.read(File.join(root, "external-design-authority-closure.v1.json"), encoding: Encoding::US_ASCII))
decision = JSON.parse(File.read(File.join(root, "decision-closure.v1.json"), encoding: Encoding::US_ASCII))
external_id, external_cbor = validate(external, true)
decision_id, decision_cbor = validate(decision, false)
raise "external CBOR file mismatch" unless external_cbor == File.binread(File.join(root, "external-design-authority-closure.v1.cbor"))
raise "Decision CBOR file mismatch" unless decision_cbor == File.binread(File.join(root, "decision-closure.v1.cbor"))
puts JSON.generate({ "external_closure_id" => "sha256:#{external_id}", "decision_closure_id" => "sha256:#{decision_id}", "encoder" => "ruby", "semantic_validation" => "pass" })
