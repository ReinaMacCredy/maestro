#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"

ROOT = File.expand_path("../../../..", __dir__)
OUT = File.join(ROOT, "contracts/vnext/stage0/resource-release")
RESOURCE_COUNT = 377
DELTA_ENTRY_COUNT = 530

SCHEMA_IDS = {
  "ResourceDescriptorV1" => "78cc56e71ae16fa2539429601fb08e37970d32569d0fddfd12c2129b6344bcc9",
  "BundleManifestHeaderV1" => "ab811246b97d67ed2414723b046cfa29734cd7e7060114592ec6bd74d6cf8f63",
  "BundleManifestV1" => "f2d7bb5d5b5ba81fed67b3d1e25c89285c893aa57491f59212d34c7fb51c5dd2",
  "ReleaseBundleMembershipV1" => "d99e73adb9ffb8db858424033eb9afabc9ceb3f17870ee3be14d8176daf56e77",
  "EmbeddedReleaseHeaderV1" => "2eb8a479550b066cd39e472f6db303d43df256b7ca621e397c435bc2f5ac24a5",
  "EmbeddedReleaseBundleV1" => "0f64450315ba74ce5206d378e71a8d6c63041631d34a59b905af0e2f55a5721f",
  "ReleaseResourceCensusEntryV1" => "82c01e900d537186647b5258745c45777842d5952fcfa62a90723473189c878a",
  "ReleaseResourceCensusHeaderV1" => "7f27f443f927fee5ace98650d80a7c0566a03a236cf49b206c971c7917a6a08a",
  "ReleaseResourceCensusV1" => "6b43ddca6f7c18f9693de17d8915eee8a5a51df42b341ac862aac86a7dd108de"
}.freeze

MANIFEST_IDENTITY_PROTOCOL_SHA256 = "807c478cdd7b84fa44c7bb27827f972dfe05e25b0d2339285dfe311b81cfc077"
OWNER_PROTOCOL_ID = "a21d3d2c1eb16604331c1d206df86ae2fa3263b012dd0de12cf0bb83d19074ca"
DEFAULT_PROFILE_IDS = [
  "12bbaf6404b4943b1f8d3ef85ed12c3e2bf2b97b037fd0ad3f71876634f4909a",
  "6cf1432e99a82e54e4698789bb2aa58a79f7a3a0a28abe5b849064ec1a6e1545",
  "069552018c8211f81eedb347a9427c5b0ade70cb86da25de7d491742e673c043",
  "00c33d207de36dcf7a65a3ab60956a55b19cf7cc556fcb2bedfec07fcc6aaa24"
].freeze

RESOURCE_DESCRIPTOR_DOMAIN = "maestro.vnext.resource.descriptor.v1"
BUNDLE_MANIFEST_DOMAIN = "maestro.vnext.bundle.manifest.v1"
RELEASE_CENSUS_ROW_DOMAIN = "maestro.vnext.release-resource-census-row.descriptor.v1"
RELEASE_CENSUS_MANIFEST_DOMAIN = "maestro.vnext.release-resource-census.manifest.v1"
RELEASE_MEMBERSHIP_DOMAIN = "maestro.vnext.release-bundle-membership.descriptor.v1"
EMBEDDED_RELEASE_MANIFEST_DOMAIN = "maestro.vnext.embedded-release-bundle.manifest.v1"

CONTENT_ENCODING_TAGS = { "OpaqueBytes" => 1, "Utf8Text" => 2 }.freeze
RESOURCE_KIND_TAGS = {
  "Executable" => 1,
  "Signature" => 2,
  "BillOfMaterials" => 3,
  "AgentInstruction" => 4,
  "OrchestrationDefinition" => 5,
  "PublicContract" => 6,
  "AdapterArtifact" => 7,
  "ExternalPattern" => 8,
  "MigrationArtifact" => 9,
  "License" => 10,
  "ProvenanceManifest" => 11
}.freeze
BUNDLE_KIND_TAGS = {
  "Release" => 1,
  "AgentBootstrap" => 2,
  "Capability" => 3,
  "Orchestration" => 4,
  "SharedContract" => 5,
  "Adapter" => 6,
  "ExternalPattern" => 7,
  "Migration" => 8
}.freeze
PROVENANCE_KIND_TAGS = { "FirstParty" => 1, "ThirdParty" => 2 }.freeze
DISPOSITION_TAGS = {
  "Retain" => 1,
  "Rewrite" => 2,
  "Replace" => 3,
  "MigrationOnly" => 4,
  "Remove" => 5
}.freeze
RESOURCE_OWNER_TAGS = {
  "Distribution" => 1,
  "AgentBootstrap" => 2,
  "Capability" => 3,
  "Orchestration" => 4,
  "SharedContract" => 5,
  "Adapter" => 6,
  "Design" => 7,
  "Migration" => 8,
  "ContractClosure" => 9,
  "Submission" => 10
}.freeze
READER_OWNER_TAGS = RESOURCE_OWNER_TAGS.merge("Execution" => 11, "Integration" => 12).freeze
DIRECT_CONSUMER_KIND_TAGS = {
  "Build" => 1,
  "Runtime" => 2,
  "Install" => 3,
  "Migration" => 4,
  "Proof" => 5,
  "Documentation" => 6,
  "RemovalReader" => 7
}.freeze

BUNDLE_SPECS = [
  { receipt: "bundle-001", base: "bundle-001-migration.v1", tag: 1, kind: "Migration", group: "Migration:default", slug: "migration" },
  { receipt: "bundle-002", base: "bundle-002-external-pattern-neutral.v1", tag: 2, kind: "ExternalPattern", group: "ExternalPattern:first-party-neutral-baseline", slug: "external-pattern-neutral" },
  { receipt: "bundle-003", base: "bundle-003-external-pattern-vendor.v1", tag: 3, kind: "ExternalPattern", group: "ExternalPattern:third-party-awesome-design-md", slug: "external-pattern-vendor" },
  { receipt: "bundle-004", base: "bundle-004-shared-contract.v1", tag: 4, kind: "SharedContract", group: "SharedContract:default", slug: "shared-contract" },
  { receipt: "bundle-005", base: "bundle-005-orchestration.v1", tag: 5, kind: "Orchestration", group: "Orchestration:default", slug: "orchestration" },
  { receipt: "bundle-006", base: "bundle-006-capability.v1", tag: 6, kind: "Capability", group: "Capability:default", slug: "capability" },
  { receipt: "bundle-007", base: "bundle-007-adapter.v1", tag: 7, kind: "Adapter", group: "Adapter:default", slug: "adapter" },
  { receipt: "bundle-008", base: "bundle-008-agent-bootstrap.v1", tag: 8, kind: "AgentBootstrap", group: "AgentBootstrap:default", slug: "agent-bootstrap" }
].freeze

GENERATED_OUTPUT_PATHS = [
  "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.cbor",
  "contracts/vnext/stage0/resource-release/expected-delta-successor.v1.json",
  "contracts/vnext/stage0/resource-release/resource-release.v1.json"
].freeze
PROOF_STABLE_INVENTORY_VALIDATION_KEYS = %w[
  authoritative_source_count
  direct_reader_edge_count
  external_pattern_bundle_group_count
  family_count
  generated_reference_producer_count
  historical_e204_count
  inventory_sha256
  legacy_tui_migration_census_only_count
  legacy_tui_runtime_reachable_count
  legacy_tui_source_count
  legacy_tui_typescript_project_only_count
  resource_count
  unclassified_paths
].sort.freeze

AUDIT_FIELDS = {
  "current-archive-manifest.v1.json" => "current_archive_manifest",
  "current-consumer-census.v1.json" => "current_consumer_census",
  "current-persistence-manifest.v1.json" => "current_persistence_manifest",
  "current-surface-manifest.v1.json" => "current_surface_manifest",
  "golden-fixture-manifest.v1.json" => "golden_fixture_manifest",
  "migration-rollback-requirements.v1.json" => "migration_rollback_requirements"
}.freeze

IDENTITY_KIND_TAGS = {
  "Schema" => 1,
  "Manifest" => 2,
  "Resource" => 3,
  "Bundle" => 4,
  "Census" => 5,
  "Release" => 6,
  "RootInput" => 7,
  "HandoffInput" => 8
}.freeze
DELTA_DISPOSITION_TAGS = { "Introduce" => 1, "Preserve" => 2, "Rotate" => 3, "Retire" => 4 }.freeze

def contract!(condition, message)
  raise message unless condition
end

def unsigned!(value, minimum, maximum, name)
  contract!(value.is_a?(Integer) && value.between?(minimum, maximum), "#{name} is outside #{minimum}..#{maximum}")
  value
end

def ascii!(value, minimum, maximum, name)
  contract!(value.is_a?(String) && value.ascii_only?, "#{name} must be ASCII text")
  contract!(value.bytesize.between?(minimum, maximum), "#{name} length is outside #{minimum}..#{maximum}")
  value
end

def bare_hash!(value, name)
  contract!(value.is_a?(String) && value.match?(/\A[0-9a-f]{64}\z/), "#{name} is not a canonical SHA-256")
  value
end

def prefixed_hash!(value, name)
  contract!(value.is_a?(String) && value.match?(/\Asha256:[0-9a-f]{64}\z/), "#{name} is not a prefixed canonical SHA-256")
  value.delete_prefix("sha256:")
end

def bytes32(value)
  { "bytes" => value }
end

def bytes32_hash!(value, name)
  contract!(value.is_a?(Hash) && value.keys == ["bytes"], "#{name} is not an exact bytes32 value")
  bare_hash!(value["bytes"], name)
end

def optional_bytes32!(value, name)
  contract!(value.is_a?(Array) && !value.empty?, "#{name} is not an exact optional")
  if value[0] == 0
    contract!(value == [0], "#{name} absent optional is not exact")
    nil
  else
    contract!(value[0] == 1 && value.length == 2, "#{name} present optional is not exact")
    bytes32_hash!(value[1], name)
  end
end

def strict_tags!(values, name)
  contract!(
    !values.empty? && values.all? { |value| value.is_a?(Integer) && value.positive? } &&
      values == values.sort && values.uniq.length == values.length,
    "#{name} tags are not positive, strictly sorted, and unique"
  )
end

def tag_for!(mapping, name, field)
  contract!(name.is_a?(String) && mapping.key?(name), "#{field} is not a frozen closed-enum value")
  mapping.fetch(name)
end

def tag_value!(mapping, value, field)
  contract!(value.is_a?(Integer) && mapping.value?(value), "#{field} is not a frozen closed-enum tag")
  value
end

def contains_nil?(value)
  case value
  when nil then true
  when Array then value.any? { |item| contains_nil?(item) }
  when Hash then value.values.any? { |item| contains_nil?(item) }
  else false
  end
end

def head(major, value)
  unsigned!(value, 0, 0xffffffffffffffff, "CBOR integer/length")
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
    contract!(value.ascii_only?, "CBOR text must be ASCII")
    raw = value.b
    head(3, raw.bytesize) + raw
  when Array
    head(4, value.length) + value.map { |item| encode(item) }.join
  when Hash
    raw = [bytes32_hash!(value, "CBOR bytes32")].pack("H*")
    head(2, raw.bytesize) + raw
  else
    raise "unsupported deterministic-CBOR value #{value.inspect}"
  end
end

def digest(value)
  Digest::SHA256.hexdigest(encode(value))
end

def artifact_path(name)
  File.join(OUT, name)
end

def load_json(name)
  path = artifact_path(name)
  contract!(File.file?(path), "missing Resource/Release artifact #{name}")
  value = JSON.parse(File.read(path))
  contract!(value.is_a?(Hash), "#{name} is not a JSON object")
  value
end

def python_bracket_delta(source)
  depth = 0
  quote = nil
  escaped = false
  source.each_char do |character|
    if quote
      if escaped
        escaped = false
      elsif character == "\\"
        escaped = true
      elsif character == quote
        quote = nil
      end
      next
    end
    break if character == "#"
    if character == '"' || character == "'"
      quote = character
    elsif "([{".include?(character)
      depth += 1
    elsif ")]}".include?(character)
      depth -= 1
    end
  end
  depth
end

def python_assignment_literal!(source, symbol, literal, label)
  pattern = /\A#{Regexp.escape(symbol)}\s*=\s*(.*)\z/
  matches = source.lines.each_with_index.filter_map do |line, index|
    match = line.chomp.match(pattern)
    [index, match[1]] if match
  end
  contract!(matches.length == 1, "#{label} top-level assignment is missing or duplicate")
  index, first_rhs = matches.first
  scope = first_rhs.dup
  depth = python_bracket_delta(first_rhs)
  while depth.positive? && index + 1 < source.lines.length
    index += 1
    line = source.lines[index]
    scope << line
    depth += python_bracket_delta(line)
  end
  contract!(depth.zero?, "#{label} top-level assignment is not balanced")
  quoted_literals = [JSON.generate(literal), "'#{literal.gsub("'", "\\\\'")}'"]
  contract!(quoted_literals.any? { |quoted| scope.include?(quoted) }, "#{label} does not bind the exact path literal")
end

def stage0_raw!(document, schema, label)
  contract!(document["schema"] == schema, "#{label} schema drifted")
  contract!(document["identity_protocol"] == "Stage0CanonicalCommitmentV1", "#{label} falsely claims ManifestIdentityV1")
  contract!(document["identity_scope"] == "canonical_commitment_envelope_only", "#{label} identity scope drifted")
  contract!(!document.key?("manifest_identity_envelope"), "#{label} contains a false ManifestIdentity envelope")
  envelope = document["canonical_commitment_envelope"]
  contract!(envelope.is_a?(Array) && envelope.length == 2, "#{label} is not an exact two-slot commitment")
  contract!(envelope == [schema, document["canonical_value"]], "#{label} commitment/value mismatch")
  contract!(!contains_nil?(envelope), "#{label} commitment contains nil")
  raw = encode(envelope)
  identity = Digest::SHA256.hexdigest(raw)
  contract!(prefixed_hash!(document["identity"], "#{label} identity") == identity, "#{label} identity drifted")
  contract!(document["canonical_cbor_sha256"] == identity, "#{label} canonical CBOR SHA-256 drifted")
  contract!(document["canonical_cbor_byte_length"] == raw.bytesize, "#{label} canonical CBOR length drifted")
  contract!(document["canonical_cbor_hex"] == raw.unpack1("H*"), "#{label} canonical CBOR hex drifted")
  contract!(document["candidate_only"] == true && document["runtime_activation"] == false, "#{label} candidate state drifted")
  raw
end

def verify_stage0!(json_name, cbor_name, schema)
  document = load_json(json_name)
  raw = stage0_raw!(document, schema, json_name)
  contract!(File.binread(artifact_path(cbor_name)) == raw, "#{cbor_name} differs from independently encoded bytes")
  [document, raw]
end

def verify_manifest!(json_name, cbor_name, schema:, identity_name:, domain:, manifest_schema_id:, descriptor_schema_id:)
  document = load_json(json_name)
  contract!(document["schema"] == schema, "#{json_name} wrapper schema drifted")
  contract!(document["identity_protocol"] == "ManifestIdentityV1", "#{json_name} identity protocol drifted")
  contract!(!document.key?("canonical_commitment_envelope"), "#{json_name} contains a false Stage0 commitment envelope")
  contract!(document["candidate_only"] == true && document["runtime_activation"] == false, "#{json_name} candidate state drifted")
  envelope = document["manifest_identity_envelope"]
  contract!(envelope.is_a?(Array) && envelope.length == 5, "#{json_name} does not have exactly five identity slots")
  contract!(!contains_nil?(envelope), "#{json_name} ManifestIdentity contains nil")
  contract!(envelope[0] == domain, "#{json_name} ManifestIdentity domain drifted")
  contract!(envelope[1] == bytes32(manifest_schema_id), "#{json_name} manifest SchemaId drifted")
  contract!(envelope[2] == bytes32(descriptor_schema_id), "#{json_name} descriptor SchemaId drifted")
  contract!(document["canonical_value"] == envelope[3, 2], "#{json_name} canonical value differs from header/rows")
  raw = encode(envelope)
  identity = Digest::SHA256.hexdigest(raw)
  contract!(bare_hash!(document[identity_name], "#{json_name} #{identity_name}") == identity, "#{json_name} named identity drifted")
  contract!(prefixed_hash!(document["identity"], "#{json_name} identity") == identity, "#{json_name} rendered identity drifted")
  contract!(document["canonical_cbor_sha256"] == identity, "#{json_name} canonical CBOR SHA-256 drifted")
  contract!(document["canonical_cbor_byte_length"] == raw.bytesize, "#{json_name} canonical CBOR length drifted")
  contract!(document["canonical_cbor_hex"] == raw.unpack1("H*"), "#{json_name} canonical CBOR hex drifted")
  contract!(File.binread(artifact_path(cbor_name)) == raw, "#{cbor_name} differs from independently encoded bytes")
  [document, raw]
end

def validate_owner!(value, name)
  contract!(value.is_a?(Array) && value.length == 2, "#{name} is not an exact OwnerRefV1")
  [unsigned!(value[0], 1, 20, "#{name} owner tag"), bytes32_hash!(value[1], "#{name} owner profile")]
end

def validate_manifest_core!(core, generated_schema:, descriptor_schema:, header_schema:, manifest_schema:, dependencies:, row_count:, max_row_tag:, label:)
  contract!(core.is_a?(Array) && core.length == 16 && core[0, 3] == [1, 1, 1], "#{label} ManifestHeaderCoreV1 shape drifted")
  expected_schema_ids = [generated_schema, descriptor_schema, header_schema, manifest_schema].map { |identity| bytes32(identity) }
  contract!(core[3, 4] == expected_schema_ids, "#{label} ManifestHeaderCoreV1 SchemaIds drifted")
  expected_dependencies = dependencies.map { |tag, identity| [tag, bytes32(identity)] }
  contract!(core[7] == expected_dependencies, "#{label} ManifestHeaderCoreV1 dependencies drifted")
  strict_tags!(dependencies.map(&:first), "#{label} dependency") unless dependencies.empty?
  contract!(core[8] == bytes32(MANIFEST_IDENTITY_PROTOCOL_SHA256), "#{label} ManifestIdentity protocol commitment drifted")
  contract!(core[9] == bytes32(OWNER_PROTOCOL_ID), "#{label} owner protocol commitment drifted")
  contract!(core[10] == row_count && core[11] == max_row_tag, "#{label} ManifestHeaderCoreV1 row coordinates drifted")
  contract!(core[12, 4] == DEFAULT_PROFILE_IDS.map { |identity| bytes32(identity) }, "#{label} frozen profile commitments drifted")
end

def validate_resource_value!(value, label)
  contract!(value.is_a?(Array) && value.length == 24, "#{label} is not an exact 24-field ResourceDescriptorV1")
  contract!(!contains_nil?(value), "#{label} ResourceDescriptorV1 contains nil")
  resource_tag = unsigned!(value[0], 1, 4096, "#{label} Resource tag")
  ascii!(value[1], 1, 512, "#{label} stable Resource key")
  bytes32_hash!(value[2], "#{label} content SHA-256")
  unsigned!(value[3], 0, 0xffffffffffffffff, "#{label} content length")
  tag_value!(CONTENT_ENCODING_TAGS, value[4], "#{label} content encoding")
  ascii!(value[5], 1, 128, "#{label} media type")
  tag_value!(RESOURCE_KIND_TAGS, value[6], "#{label} Resource kind")
  validate_owner!(value[7], "#{label} semantic owner")
  tag_value!(BUNDLE_KIND_TAGS, value[8], "#{label} required Bundle kind")
  tag_value!(PROVENANCE_KIND_TAGS, value[9], "#{label} provenance kind")
  bytes32_hash!(value[10], "#{label} provenance commitment")
  optional_bytes32!(value[11], "#{label} license commitment")
  dependencies = value[12]
  contract!(dependencies.is_a?(Array) && dependencies.length <= 4095, "#{label} Resource dependencies drifted")
  dependency_tags = dependencies.map do |row|
    contract!(row.is_a?(Array) && row.length == 2, "#{label} Resource dependency row drifted")
    bytes32_hash!(row[1], "#{label} dependency ResourceId")
    unsigned!(row[0], 1, 4096, "#{label} dependency Resource tag")
  end
  strict_tags!(dependency_tags, "#{label} Resource dependency") unless dependency_tags.empty?
  contract!(dependency_tags.all? { |tag| tag < resource_tag }, "#{label} Resource dependency is not strictly backward")
  bytes32_hash!(value[13], "#{label} compatibility profile")
  optional_bytes32!(value[14], "#{label} generator commitment")
  value[15, 7].each_with_index { |item, index| bytes32_hash!(item, "#{label} policy profile #{index + 15}") }
  tag_value!(DISPOSITION_TAGS, value[22], "#{label} disposition")
  bytes32_hash!(value[23], "#{label} proof profile")
  value
end

def receipt_row(document, raw)
  {
    "identity_protocol" => document.fetch("identity_protocol"),
    "identity" => document.fetch("identity"),
    "canonical_cbor_sha256" => Digest::SHA256.hexdigest(raw),
    "canonical_cbor_byte_length" => raw.bytesize
  }
end

descriptor_set, descriptor_cbor = verify_stage0!(
  "resource-descriptors.v1.json",
  "resource-descriptors.v1.cbor",
  "maestro.vnext.stage0.resource-descriptor-set.v1"
)
contract!(descriptor_set["descriptor_domain"] == RESOURCE_DESCRIPTOR_DOMAIN, "Resource descriptor domain drifted")
contract!(descriptor_set["descriptor_schema_id"] == SCHEMA_IDS.fetch("ResourceDescriptorV1"), "Resource descriptor SchemaId drifted")
records = descriptor_set["resources"]
contract!(descriptor_set["resource_count"] == RESOURCE_COUNT && records.is_a?(Array) && records.length == RESOURCE_COUNT, "Resource descriptor set is not exact #{RESOURCE_COUNT}")

resources = records.each_with_index.map do |record, offset|
  tag = offset + 1
  label = "Resource #{tag}"
  contract!(record.is_a?(Hash), "#{label} record is not an object")
  value = validate_resource_value!(record["value"], label)
  contract!(value[0] == tag && record["inventory_ordinal"] == tag, "#{label} tag/order drifted")
  contract!(value[12] == [], "#{label} has a non-builder backward dependency")
  contract!(record["stable_resource_key"] == value[1], "#{label} stable key drifted")
  ascii!(record["stable_locator"], 1, 4096, "#{label} stable locator")
  contract!(record["content_sha256"] == bytes32_hash!(value[2], "#{label} content SHA-256"), "#{label} content SHA-256 metadata drifted")
  contract!(record["content_byte_length"] == value[3], "#{label} content length metadata drifted")
  contract!(record["content_encoding"] == CONTENT_ENCODING_TAGS.key(value[4]), "#{label} content encoding metadata drifted")
  contract!(record["media_type"] == value[5], "#{label} media type metadata drifted")
  contract!(record["resource_kind"] == RESOURCE_KIND_TAGS.key(value[6]), "#{label} Resource kind metadata drifted")
  contract!(record["semantic_owner"] == RESOURCE_OWNER_TAGS.key(value[7][0]), "#{label} semantic owner metadata drifted")
  contract!(record["required_bundle_kind"] == BUNDLE_KIND_TAGS.key(value[8]), "#{label} Bundle kind metadata drifted")
  contract!(record["provenance_kind"] == PROVENANCE_KIND_TAGS.key(value[9]), "#{label} provenance metadata drifted")
  contract!(record["disposition"] == DISPOSITION_TAGS.key(value[22]), "#{label} disposition metadata drifted")
  bare_hash!(record["inventory_candidate_id"], "#{label} inventory candidate identity")
  contract!(record["reader_evidence"].is_a?(Array), "#{label} reader evidence is missing")
  expected_profiles = {
    "owner" => bytes32_hash!(value[7][1], "#{label} owner profile"),
    "provenance" => bytes32_hash!(value[10], "#{label} provenance profile"),
    "license" => optional_bytes32!(value[11], "#{label} license profile"),
    "compatibility" => bytes32_hash!(value[13], "#{label} compatibility profile"),
    "generator" => optional_bytes32!(value[14], "#{label} generator profile"),
    "target" => bytes32_hash!(value[15], "#{label} target profile"),
    "custody" => bytes32_hash!(value[16], "#{label} custody profile"),
    "migration" => bytes32_hash!(value[17], "#{label} migration profile"),
    "rollback" => bytes32_hash!(value[18], "#{label} rollback profile"),
    "uninstall" => bytes32_hash!(value[19], "#{label} uninstall profile"),
    "retention" => bytes32_hash!(value[20], "#{label} retention profile"),
    "removal" => bytes32_hash!(value[21], "#{label} removal profile"),
    "proof" => bytes32_hash!(value[23], "#{label} proof profile")
  }
  contract!(record["profiles"] == expected_profiles, "#{label} profile coordinates drifted")
  envelope = [RESOURCE_DESCRIPTOR_DOMAIN, bytes32(SCHEMA_IDS.fetch("ResourceDescriptorV1")), value]
  contract!(record["identity_envelope"] == envelope, "#{label} exact three-slot identity envelope drifted")
  raw = encode(envelope)
  resource_id = Digest::SHA256.hexdigest(raw)
  contract!(bare_hash!(record["resource_id"], "#{label} ResourceId") == resource_id, "#{label} ResourceId drifted")
  contract!(record["cbor_hex"] == raw.unpack1("H*") && record["byte_length"] == raw.bytesize, "#{label} canonical Resource bytes drifted")
  {
    tag: tag,
    id: resource_id,
    value: value,
    record: record,
    key: record["stable_resource_key"],
    locator: record["stable_locator"],
    group: record["target_bundle_group"],
    bundle_kind: record["required_bundle_kind"],
    disposition: record["disposition"]
  }
end

descriptor_pairs = resources.map { |resource| [resource[:tag], bytes32(resource[:id])] }
contract!(descriptor_set["canonical_value"] == [1, descriptor_pairs], "Resource descriptor-set exact tag/ResourceId commitment drifted")
resources_by_id = resources.to_h { |resource| [resource[:id], resource] }

bundle_pairs = BUNDLE_SPECS.map do |spec|
  document, raw = verify_manifest!(
    "#{spec[:base]}.json",
    "#{spec[:base]}.cbor",
    schema: "maestro.vnext.bundle.manifest.v1",
    identity_name: "bundle_id",
    domain: BUNDLE_MANIFEST_DOMAIN,
    manifest_schema_id: SCHEMA_IDS.fetch("BundleManifestV1"),
    descriptor_schema_id: SCHEMA_IDS.fetch("ResourceDescriptorV1")
  )
  [spec, document, raw]
end
bundle_ids = bundle_pairs.map { |_spec, document, _raw| bare_hash!(document["bundle_id"], "BundleId") }

bundle_memberships = []
bundle_pairs.each_with_index do |(spec, document, _raw), index|
  label = "Bundle #{spec[:tag]}"
  contract!(document["bundle_tag"] == spec[:tag] && document["bundle_kind"] == spec[:kind], "#{label} tag/kind topology drifted")
  contract!(document["stable_bundle_group"] == spec[:group], "#{label} concrete group drifted")
  header, rows = document["canonical_value"]
  contract!(header.is_a?(Array) && header.length == 14 && rows.is_a?(Array) && !rows.empty?, "#{label} header/rows shape drifted")
  expected_dependency_indexes = { 4 => [3], 5 => [4] }.fetch(index, [])
  dependencies = expected_dependency_indexes.map { |dependency_index| [BUNDLE_SPECS[dependency_index][:tag], bundle_ids[dependency_index]] }
  expected_dependency_ids = dependencies.map(&:last)
  contract!(document["dependency_bundle_ids"] == expected_dependency_ids, "#{label} strict-backward dependency IDs drifted")
  contract!(header[1] == spec[:tag], "#{label} header tag drifted")
  contract!(header[2] == "maestro.vnext.bundle.#{spec[:slug]}", "#{label} stable key drifted")
  contract!(header[3] == BUNDLE_KIND_TAGS.fetch(spec[:kind]) && header[4] == "1", "#{label} header kind/version drifted")
  bytes32_hash!(header[5], "#{label} compatibility profile")
  contract!(header[6] == dependencies.map { |tag, identity| [tag, bytes32(identity)] }, "#{label} dependency rows drifted")
  bytes32_hash!(header[7], "#{label} provenance commitment")
  license = optional_bytes32!(header[8], "#{label} license commitment")
  contract!((spec[:tag] == 3) == !license.nil?, "#{label} third-party license optional drifted")
  bytes32_hash!(header[9], "#{label} package policy")
  expected_targets = %w[Adapter AgentBootstrap].include?(spec[:kind]) ? [[2, 2]] : [[1, 1]]
  contract!(header[10] == expected_targets, "#{label} supported target classes drifted")
  header[11, 3].each_with_index { |item, position| bytes32_hash!(item, "#{label} policy #{position + 11}") }
  row_tags = []
  row_ids = []
  rows.each do |row|
    contract!(row.is_a?(Array) && row.length == 3, "#{label} Resource row shape drifted")
    row_tag = unsigned!(row[0], 1, 4096, "#{label} Resource tag")
    row_id = bytes32_hash!(row[1], "#{label} ResourceId")
    resource = resources_by_id[row_id]
    contract!(resource && resource[:tag] == row_tag, "#{label} Resource tag/ID pair is outside the exact set")
    contract!(resource[:group] == spec[:group] && resource[:bundle_kind] == spec[:kind], "#{label} owns a Resource from another concrete group")
    contract!(row[2] == resource[:value], "#{label} embedded Resource descriptor drifted")
    validate_resource_value!(row[2], "#{label} Resource #{row_tag}")
    row_tags << row_tag
    row_ids << row_id
  end
  strict_tags!(row_tags, "#{label} Resource")
  contract!(document["resource_ids"] == row_ids, "#{label} ResourceId metadata drifted")
  validate_manifest_core!(
    header[0],
    generated_schema: SCHEMA_IDS.fetch("ResourceDescriptorV1"),
    descriptor_schema: SCHEMA_IDS.fetch("ResourceDescriptorV1"),
    header_schema: SCHEMA_IDS.fetch("BundleManifestHeaderV1"),
    manifest_schema: SCHEMA_IDS.fetch("BundleManifestV1"),
    dependencies: dependencies,
    row_count: rows.length,
    max_row_tag: row_tags.max,
    label: label
  )
  bundle_memberships << row_ids
end

flattened_bundle_resources = bundle_memberships.flatten
contract!(flattened_bundle_resources.uniq.length == RESOURCE_COUNT, "Bundle membership contains duplicate Resources")
contract!(flattened_bundle_resources.sort == resources_by_id.keys.sort, "Bundle/Resource membership is not exact-set equal")

census, census_cbor = verify_manifest!(
  "release-resource-census.v1.json",
  "release-resource-census.v1.cbor",
  schema: "maestro.vnext.release-resource-census.manifest.v1",
  identity_name: "census_id",
  domain: RELEASE_CENSUS_MANIFEST_DOMAIN,
  manifest_schema_id: SCHEMA_IDS.fetch("ReleaseResourceCensusV1"),
  descriptor_schema_id: SCHEMA_IDS.fetch("ReleaseResourceCensusEntryV1")
)
census_id = bare_hash!(census["census_id"], "CensusId")
census_header, census_rows = census["canonical_value"]
contract!(census_header.is_a?(Array) && census_header.length == 12 && census_rows.is_a?(Array), "ReleaseResourceCensus header/rows shape drifted")
contract!(census_header[1, 3] == ["maestro-vnext-candidate", "1", "macos-arm64"], "ReleaseResourceCensus release coordinates drifted")
expected_bundle_rows = BUNDLE_SPECS.each_with_index.map { |spec, index| [spec[:tag], bytes32(bundle_ids[index])] }
contract!(census_header[4] == expected_bundle_rows, "ReleaseResourceCensus Bundle pairs drifted")
contract!(census["resource_ids"] == resources.map { |resource| resource[:id] }, "ReleaseResourceCensus Resource order drifted")
contract!(census["bundle_ids"] == bundle_ids, "ReleaseResourceCensus Bundle order drifted")
%w[source_inventory_digest consumer_inventory_digest build_graph_digest].each_with_index do |field, index|
  identity = bare_hash!(census[field], "ReleaseResourceCensus #{field}")
  contract!(census_header[index + 8] == bytes32(identity), "ReleaseResourceCensus #{field} header pointer drifted")
end
contract!(census_header[11] == 1, "ReleaseResourceCensus totality guard drifted")
direct_consumers = census["direct_consumers"]
contract!(direct_consumers.is_a?(Array), "ReleaseResourceCensus direct-consumer records are missing")
contract!(census_rows.length == RESOURCE_COUNT + direct_consumers.length, "ReleaseResourceCensus row partition drifted")
contract!(census_rows.map(&:first) == (1..census_rows.length).to_a, "ReleaseResourceCensus row tags drifted")

bundle_by_resource_id = {}
bundle_memberships.each_with_index do |ids, bundle_index|
  ids.each { |identity| bundle_by_resource_id[identity] = bundle_ids[bundle_index] }
end

census_rows.first(RESOURCE_COUNT).each_with_index do |row, index|
  resource = resources[index]
  entry_tag = index + 1
  contract!(row.is_a?(Array) && row.length == 3 && row[0] == entry_tag, "Census Resource row #{entry_tag} shape drifted")
  expected_value = [
    resource[:locator],
    resource[:value][7],
    bytes32(resource[:id]),
    bytes32(bundle_by_resource_id.fetch(resource[:id])),
    resource[:value][10],
    resource[:value][15],
    resource[:value][22],
    resource[:value][17],
    resource[:value][23],
    resource[:value][21]
  ]
  entry = [entry_tag, [1, expected_value], [0]]
  contract!(row[2] == entry, "Census Resource row #{entry_tag} locator/owner/Bundle coordinates drifted")
  descriptor_id = digest([RELEASE_CENSUS_ROW_DOMAIN, bytes32(SCHEMA_IDS.fetch("ReleaseResourceCensusEntryV1")), entry])
  contract!(row[1] == bytes32(descriptor_id), "Census Resource row #{entry_tag} DescriptorId drifted")
end

consumer_edges = []
recorded_resource_ids = []
census_rows.drop(RESOURCE_COUNT).each_with_index do |row, index|
  record = direct_consumers[index]
  entry_tag = RESOURCE_COUNT + 1 + index
  contract!(record.is_a?(Hash), "direct-consumer record #{index + 1} is not an object")
  contract!(row.is_a?(Array) && row.length == 3 && row[0] == entry_tag, "direct-consumer row #{index + 1} shape drifted")
  entry = row[2]
  contract!(entry.is_a?(Array) && entry.length == 3 && entry[0] == entry_tag && entry[1] == [0], "direct-consumer row #{index + 1} branch drifted")
  contract!(entry[2].is_a?(Array) && entry[2].length == 2 && entry[2][0] == 1, "direct-consumer row #{index + 1} optional drifted")
  value = entry[2][1]
  contract!(value.is_a?(Array) && value.length == 9 && !contains_nil?(value), "direct-consumer row #{index + 1} value drifted")
  ascii!(value[0], 1, 4096, "direct-consumer locator")
  owner_tag, owner_profile = validate_owner!(value[1], "direct-consumer owner")
  tag_value!(DIRECT_CONSUMER_KIND_TAGS, value[2], "direct-consumer kind")
  pairs = value[3]
  contract!(pairs.is_a?(Array) && !pairs.empty?, "direct-consumer Resource pairs are empty")
  pair_tags = []
  pair_records = pairs.map do |pair|
    contract!(pair.is_a?(Array) && pair.length == 2, "direct-consumer Resource pair shape drifted")
    resource_tag = unsigned!(pair[0], 1, 4096, "direct-consumer Resource tag")
    resource_id = bytes32_hash!(pair[1], "direct-consumer ResourceId")
    resource = resources_by_id[resource_id]
    contract!(resource && resource[:tag] == resource_tag, "direct-consumer Resource tag/ID pair drifted")
    contract!(resource[:disposition] != "Remove", "Remove Resource has a direct-consumer edge")
    pair_tags << resource_tag
    recorded_resource_ids << resource_id
    { "resource_tag" => resource_tag, "resource_id" => resource_id }
  end
  strict_tags!(pair_tags, "direct-consumer Resource")
  value[4, 5].each_with_index do |item, position|
    position == 1 ? tag_value!(DISPOSITION_TAGS, item, "direct-consumer disposition") : bytes32_hash!(item, "direct-consumer profile #{position}")
  end
  contract!(record["locator"] == value[0] && record["owner_tag"] == owner_tag, "direct-consumer locator/owner tag metadata drifted")
  owner_name = record["owner"]
  contract!(tag_for!(READER_OWNER_TAGS, owner_name, "direct-consumer owner") == owner_tag, "direct-consumer owner metadata drifted")
  contract!(owner_profile == digest([1, "consumer-owner", owner_name]), "direct-consumer owner profile drifted")
  contract!(tag_for!(DIRECT_CONSUMER_KIND_TAGS, record["consumer_kind"], "direct-consumer kind") == value[2], "direct-consumer kind metadata drifted")
  contract!(tag_for!(DISPOSITION_TAGS, record["disposition"], "direct-consumer disposition") == value[5], "direct-consumer disposition metadata drifted")
  contract!(record["resource_pairs"] == pair_records, "direct-consumer Resource pair metadata drifted")
  contract!(record["provenance_commitment_id"] == bytes32_hash!(value[4], "direct-consumer provenance"), "direct-consumer provenance metadata drifted")
  contract!(record["migration_profile_id"] == bytes32_hash!(value[6], "direct-consumer migration"), "direct-consumer migration metadata drifted")
  contract!(record["proof_profile_id"] == bytes32_hash!(value[7], "direct-consumer proof"), "direct-consumer proof metadata drifted")
  contract!(record["removal_profile_id"] == bytes32_hash!(value[8], "direct-consumer removal"), "direct-consumer removal metadata drifted")
  bare_hash!(record["reader_content_sha256"], "direct-consumer reader content SHA-256")
  ascii!(record["reader_role"], 1, 128, "direct-consumer reader role")
  consumer_key = digest(value)
  pairs.each { |pair| consumer_edges << [consumer_key, bytes32_hash!(pair[1], "direct-consumer ResourceId")] }
  descriptor_id = digest([RELEASE_CENSUS_ROW_DOMAIN, bytes32(SCHEMA_IDS.fetch("ReleaseResourceCensusEntryV1")), entry])
  contract!(row[1] == bytes32(descriptor_id), "direct-consumer row #{index + 1} DescriptorId drifted")
end

non_remove_ids = resources.reject { |resource| resource[:disposition] == "Remove" }.map { |resource| resource[:id] }
remove_ids = resources.select { |resource| resource[:disposition] == "Remove" }.map { |resource| resource[:id] }
contract!(recorded_resource_ids.uniq.sort == non_remove_ids.sort, "direct-consumer rows do not cover the exact non-Remove Resource set")
contract!((recorded_resource_ids & remove_ids).empty?, "Remove Resource consumer-zero invariant drifted")
contract!(census["consumer_edges"] == consumer_edges.sort, "ReleaseResourceCensus direct-consumer edge IDs drifted")
contract!(census_header[5, 3] == [RESOURCE_COUNT, direct_consumers.length, consumer_edges.length], "ReleaseResourceCensus exact counts drifted")
validate_manifest_core!(
  census_header[0],
  generated_schema: SCHEMA_IDS.fetch("ReleaseResourceCensusEntryV1"),
  descriptor_schema: SCHEMA_IDS.fetch("ReleaseResourceCensusEntryV1"),
  header_schema: SCHEMA_IDS.fetch("ReleaseResourceCensusHeaderV1"),
  manifest_schema: SCHEMA_IDS.fetch("ReleaseResourceCensusV1"),
  dependencies: BUNDLE_SPECS.each_with_index.map { |spec, index| [spec[:tag], bundle_ids[index]] },
  row_count: census_rows.length,
  max_row_tag: census_rows.length,
  label: "ReleaseResourceCensus"
)

release, release_cbor = verify_manifest!(
  "embedded-release-bundle.v1.json",
  "embedded-release-bundle.v1.cbor",
  schema: "maestro.vnext.embedded-release-bundle.manifest.v1",
  identity_name: "release_id",
  domain: EMBEDDED_RELEASE_MANIFEST_DOMAIN,
  manifest_schema_id: SCHEMA_IDS.fetch("EmbeddedReleaseBundleV1"),
  descriptor_schema_id: SCHEMA_IDS.fetch("ReleaseBundleMembershipV1")
)
release_id = bare_hash!(release["release_id"], "ReleaseId")
contract!(release["sole_release_root"] == true, "EmbeddedReleaseBundle is not the sole Release root")
contract!((%w[state runtime release_state] & release.keys).empty?, "EmbeddedReleaseBundle contains synthetic runtime state")
contract!(release["bundle_ids"] == bundle_ids && release["census_id"] == census_id, "EmbeddedReleaseBundle Bundle/Census pointers drifted")
release_header, release_rows = release["canonical_value"]
contract!(release_header.is_a?(Array) && release_header.length == 13 && release_rows.is_a?(Array) && release_rows.length == 8, "EmbeddedReleaseBundle header/rows shape drifted")
contract!(release_header[1, 4] == ["maestro-vnext-candidate", BUNDLE_KIND_TAGS.fetch("Release"), "1", "macos-arm64"], "EmbeddedReleaseBundle sole Release coordinates drifted")
release_header[5, 3].each_with_index { |item, index| bytes32_hash!(item, "EmbeddedReleaseBundle root identity #{index}") }
contract!(release_header[8] == bytes32(census_id), "EmbeddedReleaseBundle Census pointer drifted")
release_header[9, 4].each_with_index { |item, index| bytes32_hash!(item, "EmbeddedReleaseBundle policy #{index}") }

release_rows.each_with_index do |row, index|
  spec = BUNDLE_SPECS[index]
  dependencies = { 4 => [4], 5 => [5] }.fetch(index, [])
  membership = [spec[:tag], BUNDLE_KIND_TAGS.fetch(spec[:kind]), bytes32(bundle_ids[index]), dependencies.map { |tag| [tag] }]
  contract!(row.is_a?(Array) && row.length == 3 && row[0] == spec[:tag] && row[2] == membership, "EmbeddedReleaseBundle membership #{spec[:tag]} drifted")
  descriptor_id = digest([RELEASE_MEMBERSHIP_DOMAIN, bytes32(SCHEMA_IDS.fetch("ReleaseBundleMembershipV1")), membership])
  contract!(row[1] == bytes32(descriptor_id), "EmbeddedReleaseBundle membership #{spec[:tag]} DescriptorId drifted")
end
release_dependencies = BUNDLE_SPECS.each_with_index.map { |spec, index| [spec[:tag], bundle_ids[index]] } + [[9, census_id]]
validate_manifest_core!(
  release_header[0],
  generated_schema: SCHEMA_IDS.fetch("ReleaseBundleMembershipV1"),
  descriptor_schema: SCHEMA_IDS.fetch("ReleaseBundleMembershipV1"),
  header_schema: SCHEMA_IDS.fetch("EmbeddedReleaseHeaderV1"),
  manifest_schema: SCHEMA_IDS.fetch("EmbeddedReleaseBundleV1"),
  dependencies: release_dependencies,
  row_count: 8,
  max_row_tag: 8,
  label: "EmbeddedReleaseBundle"
)

delta, delta_cbor = verify_stage0!(
  "expected-delta-successor.v1.json",
  "expected-delta-successor.v1.cbor",
  "maestro.vnext.migration-cutover-expected-delta-successor.v1"
)
expected_kind_counts = {
  "Schema" => 117,
  "Manifest" => 26,
  "Resource" => RESOURCE_COUNT,
  "Bundle" => 8,
  "Census" => 1,
  "Release" => 1
}
entries = delta["entries"]
contract!(entries.is_a?(Array) && entries.length == DELTA_ENTRY_COUNT, "expected delta exact entry closure drifted")
entry_keys = entries.map { |row| [row["identity_kind"], row["logical_key"]] }
contract!(entry_keys.uniq.length == entries.length, "expected delta identity-kind/logical-key rows are not unique")
contract!(entry_keys == entry_keys.sort_by { |kind, key| [IDENTITY_KIND_TAGS.fetch(kind), key] }, "expected delta rows are not canonically ordered")
canonical_entries = entries.each_with_index.map do |row, index|
  kind = row["identity_kind"]
  disposition = row["disposition"]
  contract!(IDENTITY_KIND_TAGS.key?(kind) && IDENTITY_KIND_TAGS.fetch(kind) <= 6, "expected delta identity kind drifted")
  contract!(DELTA_DISPOSITION_TAGS.key?(disposition), "expected delta disposition drifted")
  logical_key = ascii!(row["logical_key"], 1, 4096, "expected delta logical key")
  source_artifact = ascii!(row["source_artifact"], 1, 4096, "expected delta source artifact")
  predecessor = row["predecessor_identity"]
  successor = prefixed_hash!(row["successor_identity"], "expected delta successor")
  predecessor_id = predecessor.nil? ? nil : prefixed_hash!(predecessor, "expected delta predecessor")
  case disposition
  when "Introduce"
    contract!(predecessor_id.nil?, "Introduce delta row has a predecessor")
  when "Preserve"
    contract!(!predecessor_id.nil? && predecessor_id == successor, "Preserve delta row changes identity")
  when "Rotate"
    contract!(!predecessor_id.nil? && predecessor_id != successor, "Rotate delta row preserves identity")
  when "Retire"
    contract!(!predecessor_id.nil?, "Retire delta row lacks a predecessor")
  end
  [
    index + 1,
    IDENTITY_KIND_TAGS.fetch(kind),
    logical_key,
    predecessor_id ? [1, bytes32(predecessor_id)] : [0],
    bytes32(successor),
    DELTA_DISPOSITION_TAGS.fetch(disposition),
    source_artifact,
    bytes32(bare_hash!(row["source_artifact_sha256"], "expected delta source SHA-256"))
  ]
end
actual_kind_counts = expected_kind_counts.to_h { |kind, _count| [kind, entries.count { |row| row["identity_kind"] == kind }] }
contract!(actual_kind_counts == expected_kind_counts && delta["exact_identity_kind_counts"] == expected_kind_counts, "expected delta identity-kind counts drifted")

resource_delta = entries.select { |row| row["identity_kind"] == "Resource" }.to_h { |row| [row["logical_key"], row] }
expected_resource_delta = resources.to_h { |resource| ["resource:#{resource[:key]}", resource[:id]] }
contract!(resource_delta.keys.sort == expected_resource_delta.keys.sort, "expected delta Resource exact set drifted")
expected_resource_delta.each do |key, identity|
  row = resource_delta.fetch(key)
  contract!(row["predecessor_identity"].nil? && row["disposition"] == "Introduce" && prefixed_hash!(row["successor_identity"], "delta ResourceId") == identity, "expected delta Resource binding drifted: #{key}")
end
bundle_delta = entries.select { |row| row["identity_kind"] == "Bundle" }.to_h { |row| [row["logical_key"], row] }
expected_bundle_delta = BUNDLE_SPECS.each_with_index.to_h { |spec, index| ["bundle:#{spec[:group]}", bundle_ids[index]] }
contract!(bundle_delta.keys.sort == expected_bundle_delta.keys.sort, "expected delta Bundle exact set drifted")
expected_bundle_delta.each do |key, identity|
  row = bundle_delta.fetch(key)
  contract!(row["predecessor_identity"].nil? && row["disposition"] == "Introduce" && prefixed_hash!(row["successor_identity"], "delta BundleId") == identity, "expected delta Bundle binding drifted: #{key}")
end
census_delta = entries.select { |row| row["identity_kind"] == "Census" }
release_delta = entries.select { |row| row["identity_kind"] == "Release" }
contract!(census_delta.length == 1 && census_delta[0]["logical_key"] == "census:release-resources" && census_delta[0]["predecessor_identity"].nil? && prefixed_hash!(census_delta[0]["successor_identity"], "delta CensusId") == census_id, "expected delta Census binding drifted")
contract!(release_delta.length == 1 && release_delta[0]["logical_key"] == "release:embedded-candidate" && release_delta[0]["predecessor_identity"].nil? && prefixed_hash!(release_delta[0]["successor_identity"], "delta ReleaseId") == release_id, "expected delta Release binding drifted")

expected_obligation_coordinates = [
  ["RootInput", "candidate-root"],
  ["RootInput", "candidate-finalization"],
  ["HandoffInput", "candidate-handoff"]
]
expected_obligations = expected_obligation_coordinates.map do |kind, key|
  {
    "identity_kind" => kind,
    "logical_key" => key,
    "predecessor_identity" => nil,
    "successor_identity" => nil,
    "disposition" => "Introduce",
    "depends_on_release_identity" => "sha256:#{release_id}",
    "status" => "pending_downstream_stage0_producer",
    "owner" => "candidate-root-worker"
  }
end
contract!(delta["downstream_obligations"] == expected_obligations, "expected delta downstream obligations drifted")
canonical_obligations = expected_obligation_coordinates.map do |kind, key|
  [IDENTITY_KIND_TAGS.fetch(kind), key, [0], [0], DELTA_DISPOSITION_TAGS.fetch("Introduce"), bytes32(release_id), "pending_downstream_stage0_producer", "candidate-root-worker"]
end
expected_delta_value = [1, "maestro.vnext.exact-identity-delta.v4", canonical_entries, canonical_obligations, [0], [0]]
contract!(delta["canonical_value"] == expected_delta_value, "expected delta canonical value drifted")
contract!(delta["publication_status"] == "resolved_through_release_downstream_obligations_pending", "expected delta publication status drifted")
contract!(delta["resolved_entry_count"] == DELTA_ENTRY_COUNT && delta["blocked_dependency_count"] == 3 && delta["unresolved_obligation_count"] == 3, "expected delta totality counts drifted")
contract!(delta["post_root_delta_identity"].nil? && delta["post_root_union_identity"].nil? && delta["post_root_identity_feedback_into_resource_bundle_census_release"] == false, "expected delta post-root boundary drifted")
delta_id = prefixed_hash!(delta["identity"], "expected delta commitment")

closure, closure_cbor = verify_stage0!(
  "resource-release.v1.json",
  "resource-release.v1.cbor",
  "maestro.vnext.stage0.resource-release.v1"
)
contract!(closure["source_publication"] == false && closure["runtime_registration"] == false && closure["installation"] == false, "Resource/Release closure falsely claims publication, registration, or installation")
contract!((%w[manifest_identity_envelope state runtime release_state] & closure.keys).empty?, "Resource/Release closure contains a false Manifest/runtime/state claim")
contract!(closure["resource_descriptor_set_identity"] == descriptor_set["identity"], "Resource/Release descriptor-set pointer drifted")
contract!(closure["resource_count"] == RESOURCE_COUNT && closure["resources"] == records, "Resource/Release Resource exact set drifted")
expected_bundle_records = bundle_pairs.map do |spec, document, _raw|
  document.merge("artifact_path" => "contracts/vnext/stage0/resource-release/#{spec[:base]}.json")
end
contract!(closure["bundle_count"] == 8 && closure["bundles"] == expected_bundle_records, "Resource/Release Bundle exact set drifted")
contract!(closure["release_resource_census"] == census, "Resource/Release embedded Census drifted")
contract!(closure["embedded_release_bundle"] == release, "Resource/Release embedded Release drifted")
contract!(closure["expected_delta"] == delta && closure["resolved_expected_delta_commitment_id"] == delta["identity"], "Resource/Release expected-delta pointer drifted")
contract!(closure["downstream_delta_obligations"] == expected_obligations, "Resource/Release downstream obligations drifted")
surface = load_json("current-surface-manifest.v1.json")
{
  "current-surface" => surface,
  "resource-release" => closure
}.each do |label, document|
  validation = document["inventory_validation"]
  contract!(validation.is_a?(Hash), "#{label} inventory-validation projection is not an object")
  contract!(validation.keys.sort == PROOF_STABLE_INVENTORY_VALIDATION_KEYS, "#{label} proof-stable inventory-validation key set drifted")
end

audit_ids = AUDIT_FIELDS.keys.sort.map do |name|
  field = AUDIT_FIELDS.fetch(name)
  audit = load_json(name)
  contract!(closure[field] == audit, "Resource/Release embedded audit drifted: #{name}")
  stage0_raw!(audit, audit["schema"], name)
  [name, bytes32(prefixed_hash!(audit["identity"], "#{name} identity"))]
end

bindings = closure["downstream_generated_output_bindings"]
contract!(bindings.is_a?(Array) && bindings.length == 3, "Resource/Release requires exactly three generated-output bindings")
contract!(bindings.map { |binding| binding["logical_path"] } == GENERATED_OUTPUT_PATHS, "generated-output path/order drifted")
expected_reader_locators = [
  [
    "tools/vnext_contracts/stage0/effect_home/build.py#DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS",
    "tools/vnext_contracts/stage0/effect_home/validate.py#DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS"
  ],
  [
    "tools/vnext_contracts/stage0/effect_home/build.py#DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS",
    "tools/vnext_contracts/stage0/effect_home/validate.py#DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS",
    "tools/vnext_contracts/stage0/candidate_root/build.py#RESOURCE_SUCCESSOR_DELTA"
  ],
  [
    "tools/vnext_contracts/stage0/effect_home/build.py#DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS",
    "tools/vnext_contracts/stage0/effect_home/validate.py#DOWNSTREAM_GENERATED_SEMANTIC_OBLIGATIONS",
    "tools/vnext_contracts/stage0/candidate_root/build.py#RESOURCE_RELEASE"
  ]
]

generated_output_receipts = {}
binding_ids = bindings.each_with_index.map do |binding, index|
  path = GENERATED_OUTPUT_PATHS[index]
  absolute = File.join(ROOT, path)
  contract!(File.file?(absolute), "generated output is missing: #{path}")
  producer_id = index < 2 ? delta_id : release_id
  encoding = path.end_with?(".cbor") ? "CanonicalCbor" : "CanonicalJson"
  exact_content_sha = index < 2 ? Digest::SHA256.file(absolute).hexdigest : nil
  content_binding = exact_content_sha ? "ExactRenderedBytes" : "ExternalByteReceiptAfterRender"
  readers = binding["readers"]
  contract!(readers.is_a?(Array) && readers.length == expected_reader_locators[index].length, "generated-output reader count drifted: #{path}")
  contract!(readers.map { |reader| reader["reader_locator"] } == expected_reader_locators[index], "generated-output reader locators drifted: #{path}")
  reader_coordinates = readers.map do |reader|
    locator = reader["reader_locator"]
    source_locator, symbol = locator.split("#", 2)
    reader_sha = bare_hash!(reader["reader_content_sha256"], "generated-output reader SHA-256")
    contract!(reader["evidence_kind"] == "python_ast_exact_string_constant" && reader["literal"] == path, "generated-output reader evidence drifted: #{locator}")
    source_path = File.join(ROOT, source_locator)
    contract!(File.file?(source_path) && Digest::SHA256.file(source_path).hexdigest == reader_sha, "generated-output reader bytes drifted: #{locator}")
    source = File.read(source_path)
    python_assignment_literal!(source, symbol, path, "generated-output reader #{locator}")
    [locator, bytes32(reader_sha)]
  end
  canonical = [
    1,
    path,
    bytes32(producer_id),
    encoding,
    content_binding,
    exact_content_sha ? [1, bytes32(exact_content_sha)] : [0],
    reader_coordinates
  ]
  binding_id = digest(canonical)
  expected_binding = {
    "binding_id" => binding_id,
    "logical_path" => path,
    "producer_identity" => "sha256:#{producer_id}",
    "encoding" => encoding,
    "content_binding" => content_binding,
    "exact_content_sha256" => exact_content_sha,
    "readers" => readers,
    "removal_obligations" => [],
    "canonical_value" => canonical
  }
  contract!(binding == expected_binding, "generated-output binding drifted: #{path}")
  generated_output_receipts[path] = {
    "binding_id" => binding_id,
    "sha256" => Digest::SHA256.file(absolute).hexdigest,
    "byte_length" => File.size(absolute)
  }
  binding_id
end

expected_closure_value = [
  1,
  bytes32(prefixed_hash!(descriptor_set["identity"], "descriptor-set identity")),
  BUNDLE_SPECS.each_with_index.map { |spec, index| [spec[:tag], bytes32(bundle_ids[index])] },
  bytes32(census_id),
  bytes32(release_id),
  bytes32(delta_id),
  audit_ids,
  binding_ids.map { |identity| bytes32(identity) },
  [0],
  [0],
  false
]
contract!(closure["canonical_value"] == expected_closure_value, "Resource/Release canonical commitment value drifted")
contract!(closure["declared_successor_slot_count"] == 8 && closure["resolved_successor_slot_count"] == 8 && closure["blocked_successor_slot_count"] == 0 && closure["null_successor_identity_count"] == 0, "Resource/Release successor-slot totality drifted")
contract!(closure["post_root_delta_identity"].nil? && closure["post_root_union_identity"].nil? && closure["post_root_identity_feedback_into_resource_bundle_census_release"] == false, "Resource/Release post-root boundary drifted")

artifact_receipts = {
  "resource-descriptors.v1" => receipt_row(descriptor_set, descriptor_cbor)
}
bundle_pairs.each do |spec, document, raw|
  artifact_receipts[spec[:receipt]] = receipt_row(document, raw)
end
artifact_receipts["release-resource-census.v1"] = receipt_row(census, census_cbor)
artifact_receipts["embedded-release-bundle.v1"] = receipt_row(release, release_cbor)
artifact_receipts["expected-delta-successor.v1"] = receipt_row(delta, delta_cbor)
artifact_receipts["resource-release.v1"] = receipt_row(closure, closure_cbor)

puts JSON.generate(
  "status" => "pass",
  "encoder" => "ruby-independent",
  "artifacts" => artifact_receipts,
  "generated_output_byte_receipts" => generated_output_receipts
)
