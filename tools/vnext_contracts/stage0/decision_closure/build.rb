#!/usr/bin/env ruby
# frozen_string_literal: true

# Builds the Stage-0 external Decision authority closure. This is deliberately
# an importer of frozen v1 provenance, not a vNext Decision writer.

require "digest"
require "fiddle/import"
require "json"
require "pathname"
require "yaml"

EXPECTED = {
  "design" => "9d5bda2be6274351ff7afba7f396595d80f9d560622991de1c8214aae0b8fc1b",
  "decisions" => "18f14bce862e15be09c9d88155d62627582df50c7754e2e8e1d6f6bee8f7d522",
  "card" => "2cdf1f74843a6eca926ff3bc48e060654350e6a03b65342f8d7be48d111379b4"
}.freeze

SUCCESSOR_PACKET = {
  "packet" => "7f13c85b45799e39daedd30846b4a024d1f264134b46c3e3b3cdf720f8e5fb02",
  "packet_identity" => "fb33b048b59c66df9858558a2c80e59a478d101465761f902366c9a00751cbc5",
  "decision_manifest" => EXPECTED.fetch("decisions"),
  "raw_inventory" => "704c21bd7f1e6c39d5c4c488bba7e0c28d22fcb7af4059b72eb01a83715e0962",
  "external_closure" => "b58ba8af29e55004b6b34bd8a1b1767c91b23e482c16cfe1d0560655be4f66d6"
}.freeze

EXTERNAL_DOMAIN = "maestro.vnext.external-design-authority-closure.v1"
DECISION_DOMAIN = "maestro.vnext.decision-closure.v1"
MATERIALIZATION_DOMAIN = "maestro.vnext.decision-materialization.v1"

RATIONALE_ONLY = {
  "dec-use-domain-modeling-for-the-whole-flow-c0ae" => "methodology-only",
  "dec-canonical-authority-epoch-turnover-9823" => "parent-fork-only",
  "decset-maestro-vnext-capability-complete-remaining-architecture-2f1119" => "grouping-only"
}.freeze

IGNORED_UNILATERAL_CLAIMS = [
  ["dec-public-wire-contract-2f11", "dec-canonical-agent-control-plane-contract-25a5"],
  ["dec-branch-publication-carriers-2f11", "dec-canonical-carrier-specific-ed08"],
  ["dec-canonical-non-action-protected-90a9", "dec-canonical-trusted-host-protected-1fbc"]
].freeze

RECOGNIZED_EXTERNAL_COMPOSITE_HEADS = [
  "dec-canonical-branch-recovery-publication-e287",
  "dec-canonical-vnext-wire-envelope-and-a510"
].freeze

class Bytes
  attr_reader :value

  def initialize(value)
    @value = value.b
  end
end

def head(major, value)
  raise "CBOR unsigned u64 required" unless value.is_a?(Integer) && value.between?(0, 0xffff_ffff_ffff_ffff)

  if value < 24
    [(major << 5) | value].pack("C")
  elsif value <= 0xff
    [(major << 5) | 24, value].pack("CC")
  elsif value <= 0xffff
    [(major << 5) | 25, value].pack("Cn")
  elsif value <= 0xffff_ffff
    [(major << 5) | 26, value].pack("CN")
  else
    [(major << 5) | 27, value].pack("CQ>")
  end
end

def cbor(value)
  case value
  when false then "\xf4".b
  when true then "\xf5".b
  when Integer then head(0, value)
  when Bytes then head(2, value.value.bytesize) + value.value
  when String
    ascii = value.encode(Encoding::US_ASCII).b
    head(3, ascii.bytesize) + ascii
  when Array
    head(4, value.length) + value.map { |item| cbor(item) }.join
  else
    raise "unsupported deterministic CBOR value #{value.class}"
  end
end

def digest_value(domain, value)
  Digest::SHA256.digest(cbor([domain, value]))
end

def hex(value)
  value.unpack1("H*")
end

def json_bytes(value)
  { "bytes" => hex(value) }
end

module DescriptorCapture
  extend Fiddle::Importer
  dlload Fiddle::Handle::DEFAULT
  extern "int openat(int, const char*, int, ...)"
end

O_DIRECTORY = RUBY_PLATFORM.include?("darwin") ? 0x10_0000 : 0o200000
O_CLOEXEC = RUBY_PLATFORM.include?("darwin") ? 0x100_0000 : 0o2000000

def descriptor_identity(value)
  [
    value.dev, value.ino, value.mode, value.uid, value.gid, value.nlink,
    value.size, value.mtime.to_i, value.mtime.nsec, value.ctime.to_i, value.ctime.nsec
  ]
end

def descriptor_object_identity(value)
  [value.dev, value.ino, value.mode, value.uid, value.gid]
end

def openat_io(parent, name, flags)
  descriptor = DescriptorCapture.openat(parent.fileno, name, flags)
  raise SystemCallError.new("openat #{name}", Fiddle.last_error) if descriptor.negative?

  IO.for_fd(descriptor, autoclose: true)
end

def descriptor_chain(path)
  absolute = path.expand_path
  if absolute.to_s == "/var" || absolute.to_s.start_with?("/var/")
    absolute = Pathname.new("/private").join(absolute.relative_path_from(Pathname.new("/")))
  elsif absolute.to_s == "/tmp" || absolute.to_s.start_with?("/tmp/")
    absolute = Pathname.new("/private/tmp").join(
      absolute.relative_path_from(Pathname.new("/tmp"))
    )
  end
  components = absolute.each_filename.to_a
  raise "descriptor input path is not absolute: #{path}" unless absolute.absolute?
  raise "descriptor input path has no leaf: #{path}" if components.empty?

  directories = [File.open("/", File::RDONLY)]
  components[0...-1].each do |component|
    directory = openat_io(
      directories.last,
      component,
      File::RDONLY | File::NOFOLLOW | O_DIRECTORY | O_CLOEXEC
    )
    metadata = directory.stat
    mode = metadata.mode & 0o7777
    unsafe_writable = (mode & 0o022) != 0 && (mode & 0o1000).zero?
    raise "unsafe descriptor-captured ancestor: #{path}" unless
      metadata.directory? && [0, Process.euid].include?(metadata.uid) && !unsafe_writable
    directories << directory
  end
  [directories, components.last, components]
rescue StandardError
  directories&.reverse_each(&:close)
  raise
end

def capture_regular(path)
  directories, leaf, components = descriptor_chain(path)
  parent = directories.last
  io = openat_io(parent, leaf, File::RDONLY | File::NOFOLLOW | O_CLOEXEC)
  begin
    descriptor_before = io.stat
    raise "input is not a regular file: #{path}" unless descriptor_before.file?
    named_before = openat_io(parent, leaf, File::RDONLY | File::NOFOLLOW | O_CLOEXEC)
    begin
      raise "descriptor-captured input name mismatch: #{path}" unless
        descriptor_identity(named_before.stat) == descriptor_identity(descriptor_before)
    ensure
      named_before.close
    end
    bytes = io.read.b
    descriptor_after = io.stat
    named_after = openat_io(parent, leaf, File::RDONLY | File::NOFOLLOW | O_CLOEXEC)
    begin
      raise "descriptor-captured input changed: #{path}" unless
        descriptor_identity(descriptor_before) == descriptor_identity(descriptor_after) &&
          descriptor_identity(descriptor_before) == descriptor_identity(named_after.stat) &&
          bytes.bytesize == descriptor_before.size
    ensure
      named_after.close
    end
    directories.each_cons(2).with_index do |(retained_parent, retained_child), index|
      observed = openat_io(
        retained_parent,
        components[index],
        File::RDONLY | File::NOFOLLOW | O_DIRECTORY | O_CLOEXEC
      )
      begin
        raise "descriptor ancestor changed during read: #{path}" unless
          descriptor_object_identity(observed.stat) ==
            descriptor_object_identity(retained_child.stat)
      ensure
        observed.close
      end
    end
    bytes
  ensure
    io.close
    directories.reverse_each(&:close)
  end
end

def require_bytes(path, expected)
  bytes = capture_regular(path)
  actual = Digest::SHA256.hexdigest(bytes)
  raise "frozen input drifted: #{path} expected #{expected}, got #{actual}" unless actual == expected

  bytes
end

def explicit_input(name)
  path = Pathname.new(ENV.fetch(name)).expand_path
  raise "#{name} is a symlink" if path.lstat.symlink?

  path
end

def packet_rows(bytes, columns, path)
  bytes.force_encoding(Encoding::UTF_8).lines(chomp: true).map do |line|
    fields = line.split("\t", -1)
    raise "invalid packet row width in #{path}" unless fields.length == columns

    fields
  end
end

def verify_successor_packet(packet_root, parsed, raw_by_id)
  raise "replacement packet root is a symlink" if packet_root.lstat.symlink?
  raise "replacement packet root is not a directory" unless packet_root.directory?

  packet_path = packet_root.join("replacement-build-approval-packet.v1.json")
  manifest_path = packet_root.join("successor-decision-store-manifest.v1.txt")
  inventory_path = packet_root.join("raw-decision-inventory.v1.txt")
  closure_path = packet_root.join("external-design-authority-closure.v1.txt")
  packet_bytes = require_bytes(packet_path, SUCCESSOR_PACKET.fetch("packet"))
  manifest_bytes = require_bytes(manifest_path, SUCCESSOR_PACKET.fetch("decision_manifest"))
  inventory_bytes = require_bytes(inventory_path, SUCCESSOR_PACKET.fetch("raw_inventory"))
  closure_bytes = require_bytes(closure_path, SUCCESSOR_PACKET.fetch("external_closure"))
  packet = JSON.parse(packet_bytes.force_encoding(Encoding::UTF_8))
  raise "replacement packet identity drifted" unless packet.fetch("packet_sha256") == SUCCESSOR_PACKET.fetch("packet_identity")
  raise "replacement packet candidate root was prematurely populated" unless packet.dig("sections", "identity_state", "text").include?("candidate_contract_root=absent-before-stage-0")
  raise "replacement packet Decision counts drifted" unless packet.fetch("decision_counts") == {
    "locked" => 117, "open" => 0, "superseded" => 96, "total" => 213
  }

  manifest = packet_rows(manifest_bytes, 4, manifest_path).to_h { |id, status, record_sha, body_sha| [id, [status, record_sha, body_sha]] }
  inventory = packet_rows(inventory_bytes, 4, inventory_path).to_h { |id, status, supersedes, superseded_by| [id, [status, supersedes, superseded_by]] }
  raise "replacement packet manifest count drifted" unless manifest.length == 213
  raise "replacement packet inventory count drifted" unless inventory.length == 213
  raise "replacement packet Decision ids disagree" unless manifest.keys.sort == inventory.keys.sort
  raise "replacement packet raw Decision ids disagree" unless manifest.keys.sort == raw_by_id.keys.sort

  parsed.each do |record|
    id = record.fetch("id")
    status, record_sha, body_sha = manifest.fetch(id)
    supersedes, superseded_by = directions(record)
    inventory_status, inventory_supersedes, inventory_superseded_by = inventory.fetch(id)
    raise "replacement packet status mismatch for #{id}" unless status == record.fetch("status") && inventory_status == status
    raise "replacement packet raw record mismatch for #{id}" unless Digest::SHA256.hexdigest(raw_by_id.fetch(id)) == record_sha
    body = record.fetch("extra", {}).fetch("decision", "").b
    raise "replacement packet body mismatch for #{id}" unless Digest::SHA256.hexdigest(body) == body_sha
    raise "replacement packet supersedes mismatch for #{id}" unless supersedes.join(",") == inventory_supersedes
    raise "replacement packet superseded-by mismatch for #{id}" unless superseded_by.join(",") == inventory_superseded_by
  end
  reconstructed_manifest = parsed.sort_by { |record| record.fetch("id") }.map do |record|
    id = record.fetch("id")
    body = record.fetch("extra", {}).fetch("decision", "").b
    [
      id,
      record.fetch("status"),
      Digest::SHA256.hexdigest(raw_by_id.fetch(id)),
      Digest::SHA256.hexdigest(body)
    ].join("\t")
  end.join("\n") + "\n"
  raise "replacement packet manifest is not the exact all-Decision reconstruction" unless
    Digest::SHA256.hexdigest(reconstructed_manifest) == SUCCESSOR_PACKET.fetch("decision_manifest") &&
      manifest_bytes == reconstructed_manifest

  closure_lines = closure_bytes.force_encoding(Encoding::UTF_8).lines(chomp: true)
  node_ids = closure_lines.filter_map { |line| line.split("\t", -1)[1] if line.start_with?("N\t") }
  ignored = closure_lines.filter_map do |line|
    fields = line.split("\t", -1)
    [fields[1], fields[2]] if fields[0] == "E" && fields[4] == "ignored_unilateral_claim"
  end
  raise "replacement packet closure node mismatch" unless node_ids.sort == manifest.keys.sort
  raise "replacement packet ignored unilateral claims drifted" unless ignored == IGNORED_UNILATERAL_CLAIMS
end

def raw_records(raw)
  starts = raw.enum_for(:scan, /^- schema_version: maestro\.card\.v1$/).map { Regexp.last_match.begin(0) }
  raise "decision source contains no records" if starts.empty?

  starts.each_with_index.map do |start, index|
    finish = starts.fetch(index + 1, raw.bytesize)
    aggregate = raw.byteslice(start, finish - start)
    lines = aggregate.lines
    first = lines.shift
    raise "Decision aggregate record lacks list prefix" unless first == "- schema_version: maestro.card.v1\n"
    standalone = +"schema_version: maestro.card.v1\n"
    lines.each do |line|
      if line == "\n"
        standalone << line
      elsif line.start_with?("  ")
        standalone << line.byteslice(2, line.bytesize - 2)
      else
        raise "Decision aggregate record has non-reversible indentation"
      end
    end
    standalone = standalone.sub(/\n+\z/, "\n")
    raise "standalone Decision record lacks one final LF" unless standalone.end_with?("\n") && !standalone.end_with?("\n\n")
    standalone.b
  end
end

def record_id(raw)
  match = raw.match(/^id: ([a-z0-9-]+)$/)
  raise "raw Decision record lacks id" unless match

  match[1]
end

def directions(record)
  extra = record.fetch("extra", {})
  [Array(extra["supersedes"]), Array(extra["superseded_by"])]
end

def terminal_head(id, by_id)
  visited = []
  current = id
  loop do
    raise "raw lineage cycle at #{current}" if visited.include?(current)

    visited << current
    successors = directions(by_id.fetch(current)).last
    return current if successors.empty?
    raise "raw lineage fan-out at #{current}" unless successors.length == 1

    current = successors.fetch(0)
  end
end

def direct_successor(id, by_id)
  directions(by_id.fetch(id)).last.fetch(0, nil)
end

def external_disposition(id, record, by_id, forward)
  supersedes, superseded_by = directions(record)
  incoming = forward.fetch(id, [])
  known_unilateral = IGNORED_UNILATERAL_CLAIMS.any? { |source, target| source == id || target == id }
  composite = supersedes.length > 1 || incoming.any? { |source| directions(by_id.fetch(source)).first.length > 1 }

  if known_unilateral
    ["unilateral_raw_claim", nil]
  elsif composite
    ["composite_external_authoring", nil]
  elsif superseded_by.length == 1
    successor = superseded_by.fetch(0)
    reciprocal = directions(by_id.fetch(successor)).first
    if reciprocal == [id]
      ["one_to_one", successor]
    else
      ["invalid_raw_lineage", nil]
    end
  elsif superseded_by.empty? && supersedes.empty?
    ["none", nil]
  elsif superseded_by.empty?
    ["external_head", nil]
  else
    ["invalid_raw_lineage", nil]
  end
end

def evidence_for(id, record, design, by_id)
  return "design-full-id" if design.include?(id)

  suffix = id.split("-").last
  return "design-unambiguous-suffix" if design.match?(/\b#{Regexp.escape(suffix)}\b/)

  successor = direct_successor(id, by_id)
  return "locked-record" unless successor

  body = by_id.fetch(successor).fetch("extra", {}).fetch("decision", "")
  body_hash = Digest::SHA256.hexdigest(record.fetch("extra", {}).fetch("decision", "").b)
  return "successor-exact-id" if body.include?(id)
  return "successor-body-hash" if body.include?(body_hash)
  return "successor-incorporation" if body.match?(/incorporat|preserve.*superseded|supersede.*preserve/im)

  raise "unmapped external Decision consequence: #{id} has no explicit design or successor absorption evidence"
end

def external_record_value(record)
  [
    record.fetch("id"),
    record.fetch("terminal_status"),
    Bytes.new([record.fetch("raw_record_sha256")].pack("H*")),
    Bytes.new([record.fetch("raw_body_sha256")].pack("H*")),
    record.fetch("raw_supersedes"),
    record.fetch("raw_superseded_by"),
    record.fetch("external_authoring_disposition"),
    optional_text(record["normalized_successor"]),
    record.fetch("consequence_classification"),
    optional_text(record["rationale_disposition"]),
    record.fetch("materialization_ids").map { |value| Bytes.new([value].pack("H*")) },
    record.fetch("derived_effect_status"),
    Bytes.new([record.fetch("raw_record_bytes").fetch("bytes")].pack("H*"))
  ]
end

def decision_record_value(record)
  [
    record.fetch("id"),
    record.fetch("terminal_status"),
    Bytes.new([record.fetch("raw_record_sha256")].pack("H*")),
    Bytes.new([record.fetch("raw_body_sha256")].pack("H*")),
    record.fetch("raw_supersedes"),
    record.fetch("raw_superseded_by"),
    record.fetch("external_authoring_disposition"),
    optional_text(record["normalized_successor"]),
    record.fetch("consequence_classification"),
    optional_text(record["rationale_disposition"]),
    record.fetch("materialization_ids").map { |value| Bytes.new([value].pack("H*")) },
    record.fetch("derived_effect_status")
  ]
end

def optional_text(value)
  value.nil? ? [0] : [1, value]
end

def materialization_value(materialization)
  [
    Bytes.new([materialization.fetch("id")].pack("H*")),
    materialization.fetch("artifact_id"),
    materialization.fetch("component_kind_tag"),
    0,
    materialization.fetch("decision_sources").map do |source|
      [source.fetch("id"), Bytes.new([source.fetch("body_sha256")].pack("H*"))]
    end
  ]
end

def write_json(path, value)
  File.write(path, JSON.generate(value) + "\n", mode: "w", encoding: Encoding::US_ASCII)
end

def build(repo, output)
  source = repo.join(".maestro/cards/maestro-whole-flow-architecture-refoundation")
  design_path = source.join("design.md")
  decisions_path = explicit_input("STAGE0_SUCCESSOR_DECISIONS_YAML")
  card_path = explicit_input("STAGE0_SUCCESSOR_CARD_YAML")
  design_bytes = require_bytes(design_path, EXPECTED.fetch("design"))
  source_bytes = capture_regular(decisions_path)
  card_bytes = require_bytes(card_path, EXPECTED.fetch("card"))
  hashes = {
    "design" => Digest::SHA256.hexdigest(design_bytes),
    "decisions" => EXPECTED.fetch("decisions"),
    "card" => Digest::SHA256.hexdigest(card_bytes)
  }

  design = design_bytes.force_encoding(Encoding::UTF_8)
  parsed = YAML.load(source_bytes, permitted_classes: [Time], aliases: false)
  raw = raw_records(source_bytes)
  raise "record count drifted" unless parsed.length == 213 && raw.length == 213

  by_id = parsed.to_h { |record| [record.fetch("id"), record] }
  raise "duplicate decision id" unless by_id.length == parsed.length
  raw_by_id = raw.to_h { |bytes| [record_id(bytes), bytes] }
  raise "raw/parsed Decision ids disagree" unless raw_by_id.keys.sort == by_id.keys.sort
  packet_root = explicit_input("STAGE0_SUCCESSOR_PACKET_ROOT").realpath
  verify_successor_packet(packet_root, parsed, raw_by_id)

  statuses = parsed.group_by { |record| record.fetch("status") }.transform_values(&:length)
  raise "terminal status count drifted" unless statuses == { "locked" => 117, "superseded" => 96 }

  forward = Hash.new { |hash, key| hash[key] = [] }
  parsed.each { |record| directions(record).first.each { |predecessor| forward[predecessor] << record.fetch("id") } }
  actual_composites = parsed.filter_map do |record|
    supersedes = directions(record).first
    [record.fetch("id"), supersedes] if supersedes.length > 1
  end
  actual_ignored = IGNORED_UNILATERAL_CLAIMS.select do |source_id, target_id|
    directions(by_id.fetch(source_id)).first.include?(target_id) &&
      !directions(by_id.fetch(target_id)).last.include?(source_id)
  end
  raise "known unilateral raw claims drifted" unless actual_ignored == IGNORED_UNILATERAL_CLAIMS
  raise "recognized composite head drifted" unless RECOGNIZED_EXTERNAL_COMPOSITE_HEADS.all? { |id| actual_composites.map(&:first).include?(id) }

  terminal_by_id = parsed.to_h { |record| [record.fetch("id"), terminal_head(record.fetch("id"), by_id)] }
  material_ids_by_head = {}
  records = parsed.sort_by { |record| record.fetch("id") }.map do |source_record|
    id = source_record.fetch("id")
    raw_record = raw_by_id.fetch(id)
    body = source_record.fetch("extra", {}).fetch("decision", "").b
    classification = RATIONALE_ONLY.key?(id) ? "rationale_only" : "material"
    head_id = terminal_by_id.fetch(id)
    materialization_ids = if classification == "material"
      material_ids_by_head[head_id] ||= digest_value(
        MATERIALIZATION_DOMAIN,
        [1, "maestro.vnext.candidate-contract.normative-inputs.v1/#{head_id}", 12]
      )
      [hex(material_ids_by_head.fetch(head_id))]
    else
      []
    end
    disposition, normalized_successor = external_disposition(id, source_record, by_id, forward)
    effect_status = if classification == "rationale_only"
      "no_contract_effect"
    elsif source_record.fetch("status") == "locked"
      "unapplied"
    else
      "superseded_but_effect_live"
    end
    {
      "id" => id,
      "terminal_status" => source_record.fetch("status"),
      "raw_record_sha256" => Digest::SHA256.hexdigest(raw_record),
      "raw_body_sha256" => Digest::SHA256.hexdigest(body),
      "raw_record_bytes" => json_bytes(raw_record),
      "raw_supersedes" => directions(source_record).first,
      "raw_superseded_by" => directions(source_record).last,
      "external_authoring_disposition" => disposition,
      "normalized_successor" => normalized_successor,
      "consequence_classification" => classification,
      "rationale_disposition" => RATIONALE_ONLY[id],
      "materialization_ids" => materialization_ids,
      "derived_effect_status" => effect_status,
      "external_absorption_evidence" => evidence_for(id, source_record, design, by_id),
      "effective_external_head" => head_id
    }
  end

  materializations = material_ids_by_head.sort_by { |head_id, _| head_id }.map do |head_id, materialization_id|
    sources = records.filter_map do |record|
      next unless record.fetch("consequence_classification") == "material" && record.fetch("effective_external_head") == head_id

      { "id" => record.fetch("id"), "body_sha256" => record.fetch("raw_body_sha256") }
    end
    {
      "id" => hex(materialization_id),
      "artifact_id" => "maestro.vnext.candidate-contract.normative-inputs.v1/#{head_id}",
      "component_kind_tag" => 12,
      "binding_state" => "required_component_slot_pending",
      "exact_component_id" => nil,
      "after_root_id" => nil,
      "finalization_manifest_id" => nil,
      "decision_sources" => sources.sort_by { |source| source.fetch("id") }
    }
  end

  material_count = records.count { |record| record.fetch("consequence_classification") == "material" }
  rationale_count = records.length - material_count
  raise "required rationale-only disposition missing" unless RATIONALE_ONLY.keys.all? do |id|
    records.find { |record| record.fetch("id") == id }.fetch("consequence_classification") == "rationale_only"
  end
  raise "materialization closure incomplete" unless material_count == materializations.sum { |item| item.fetch("decision_sources").length }

  lineage = {
    "ignored_unilateral_claims" => actual_ignored.map { |source_id, target_id| { "source" => source_id, "claimed_predecessor" => target_id } },
    "composite_external_heads" => actual_composites.map { |id, predecessors| { "id" => id, "raw_supersedes" => predecessors } }.sort_by { |item| item.fetch("id") },
    "recognized_external_composite_heads" => RECOGNIZED_EXTERNAL_COMPOSITE_HEADS.sort
  }
  external_value = [1, records.map { |record| external_record_value(record) }, materializations.map { |item| materialization_value(item) }, lineage.fetch("ignored_unilateral_claims").map { |item| [item.fetch("source"), item.fetch("claimed_predecessor")] }, lineage.fetch("composite_external_heads").map { |item| [item.fetch("id"), item.fetch("raw_supersedes")] }, lineage.fetch("recognized_external_composite_heads")]
  decision_value = [1, records.map { |record| decision_record_value(record) }, materializations.map { |item| materialization_value(item) }, lineage.fetch("ignored_unilateral_claims").map { |item| [item.fetch("source"), item.fetch("claimed_predecessor")] }, lineage.fetch("composite_external_heads").map { |item| [item.fetch("id"), item.fetch("raw_supersedes")] }]
    external_cbor = cbor(external_value)
    decision_cbor = cbor(decision_value)
    external_id = digest_value(EXTERNAL_DOMAIN, external_value)
    decision_id = digest_value(DECISION_DOMAIN, decision_value)
    materialization_base = {
      "kind" => "initial_external_design_closure",
      "decision_closure_id" => "sha256:#{hex(decision_id)}"
    }
    materializations.each { |item| item["materialization_base"] = materialization_base }

  common = {
    "version" => 1,
    "closure_state" => "closed",
    "records" => records,
    "materializations" => materializations,
    "lineage" => lineage,
    "summary" => {
      "total" => records.length,
      "locked" => statuses.fetch("locked"),
      "superseded" => statuses.fetch("superseded"),
      "open" => 0,
      "material" => material_count,
      "rationale_only" => rationale_count,
      "unresolved_mappings" => 0,
      "pending_component_slots" => materializations.length,
      "normalized_one_to_one_edges" => records.count { |record| !record.fetch("normalized_successor").nil? }
    },
      "root_assembly" => {
        "state" => "pending_exact_component_resolution",
        "resolved_component_ids" => [],
        "materialization_base" => materialization_base,
        "candidate_root_after" => nil,
      "finalization_manifest_id" => nil
    },
    "source_provenance_excluded_from_identity" => {
      "design_sha256" => hashes.fetch("design"),
      "decisions_sha256" => hashes.fetch("decisions"),
      "card_sha256" => hashes.fetch("card")
    }
  }
    external = common.merge(
      "schema" => "maestro.vnext.external-design-authority-closure.v1",
      "identity_domain" => EXTERNAL_DOMAIN,
      "identity" => "sha256:#{hex(external_id)}",
      "canonical_cbor_sha256" => Digest::SHA256.hexdigest(external_cbor),
      "decision_closure_reference" => "sha256:#{hex(decision_id)}"
  )
  decision_records = records.map do |record|
    record.reject { |key, _| %w[raw_record_bytes external_absorption_evidence effective_external_head].include?(key) }
  end
  decision = common.merge(
    "records" => decision_records,
    "schema" => "maestro.vnext.decision-closure.v1",
    "identity_domain" => DECISION_DOMAIN,
    "identity" => "sha256:#{hex(decision_id)}",
    "canonical_cbor_sha256" => Digest::SHA256.hexdigest(decision_cbor),
    "external_authority_reference_excluded_from_identity" => "sha256:#{hex(external_id)}"
  )

  output.mkpath
  write_json(output.join("external-design-authority-closure.v1.json"), external)
  File.binwrite(output.join("external-design-authority-closure.v1.cbor"), external_cbor)
  write_json(output.join("decision-closure.v1.json"), decision)
  File.binwrite(output.join("decision-closure.v1.cbor"), decision_cbor)
  write_json(output.join("root-binding-requirements.v1.json"), {
    "schema" => "maestro.vnext.decision-root-binding-requirements.v1",
      "decision_closure_id" => decision.fetch("identity"),
      "state" => "pending_exact_component_resolution",
      "materialization_base" => materialization_base,
      "required_component_slots" => materializations.map do |item|
        {
          "materialization_id" => item.fetch("id"),
          "component_kind_tag" => item.fetch("component_kind_tag"),
          "artifact_id" => item.fetch("artifact_id"),
          "materialization_base" => item.fetch("materialization_base"),
          "decision_source_ids" => item.fetch("decision_sources").map { |source| source.fetch("id") }
        }
    end,
    "resolution_requires" => [
      "every required slot resolves to one exact ContractComponentIdV1",
      "every source body hash remains identical",
        "every binding records MaterializationBaseV1 and the exact resulting CandidateContractRootIdV1",
      "one exact DesignFinalizationManifestV1 binds the resolved root",
      "no pending slot may be treated as applied, published, or executable"
    ]
  })
  puts JSON.generate({ "external_closure_id" => external.fetch("identity"), "decision_closure_id" => decision.fetch("identity"), "material" => material_count, "rationale_only" => rationale_count, "unresolved_mappings" => 0 })
end

if ENV["STAGE0_DESCRIPTOR_CAPTURE_TEST"] == "1"
  path = Pathname.new(ENV.fetch("STAGE0_DESCRIPTOR_CAPTURE_FIXTURE"))
  bytes = capture_regular(path)
  puts JSON.generate({
    "schema" => "maestro.vnext.stage0-descriptor-capture-test.v1",
    "sha256" => Digest::SHA256.hexdigest(bytes),
    "byte_length" => bytes.bytesize
  })
  exit
end

if ENV["STAGE0_RAW_RECORD_RECONSTRUCTION_TEST"] == "1"
  fixture = File.binread(ENV.fetch("STAGE0_RAW_RECORD_FIXTURE"))
  raise "fixture is not one standalone card with one final LF" unless
    fixture.start_with?("schema_version: maestro.card.v1\n") &&
      fixture.end_with?("\n") &&
      !fixture.end_with?("\n\n")
  aggregate = fixture.lines.each_with_index.map do |line, index|
    index.zero? ? "- #{line}" : (line == "\n" ? line : "  #{line}")
  end.join
  reconstructed = raw_records(aggregate).fetch(0)
  raise "live-shape standalone record reconstruction drifted" unless reconstructed == fixture
  expected_hash = Digest::SHA256.hexdigest(fixture)
  raise "live-shape standalone record hash drifted" unless
    Digest::SHA256.hexdigest(reconstructed) == expected_hash
  {
    "one_byte" => aggregate.sub("maestro.card.v1", "maestro.card.v2"),
    "indent" => aggregate.sub(/^  id:/, " id:"),
    "final_lf" => aggregate.delete_suffix("\n")
  }.each do |name, mutant|
    accepted = false
    begin
      candidate = raw_records(mutant)
      accepted = candidate == [fixture]
    rescue RuntimeError
      accepted = false
    end
    raise "#{name} mutant was accepted" if accepted
  end
  puts JSON.generate({
    "schema" => "maestro.vnext.stage0-standalone-decision-record-reconstruction-test.v1",
    "fixture_sha256" => expected_hash,
    "mutants_rejected" => %w[one_byte indent final_lf]
  })
  exit
end

workspace = Pathname.new(__dir__).join("../../../../").realpath
source_repo = Pathname.new(ENV.fetch("MAESTRO_AUTHORITATIVE_SOURCE", workspace.to_s)).realpath
output = workspace.join("contracts/vnext/stage0/decision-closure")
build(source_repo, output)
