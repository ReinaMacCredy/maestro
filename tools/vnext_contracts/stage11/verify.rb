#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "digest"
require "pathname"

root = Pathname.new(__dir__).join("../../..").cleanpath
fixture = JSON.parse(root.join("tests/fixtures/vnext/stage11/migration_cases.v1.json").read)
gates = JSON.parse(root.join("tests/fixtures/vnext/stage11/consumer_gates.v1.json").read)
instance_path = root.join("tests/fixtures/vnext/stage11/migration_instances.v1.jsonl")
instance_bytes = instance_path.binread
instance_records = instance_bytes.lines.map { |line| JSON.parse(line) }
instance_header = instance_records.shift

raise "wrong Stage-11 fixture schema" unless fixture.fetch("schema") == "maestro.vnext.stage11.migration-cases.v1"
raise "wrong consumer fixture schema" unless gates.fetch("schema") == "maestro.vnext.stage11.consumer-gates.v1"
raise "classification is not total" unless fixture.fetch("closed_dispositions").length == 5
raise "closed owner route was reopened" unless fixture.fetch("known_upstream_interface_gaps") == []
raise "production owner-route closure drifted" unless fixture.fetch("production_owner_routes") == %w[
  installation_consumer_snapshot_to_migration_closure
  durable_consumer_finality_receipt
  foundation_v2_aggregate_census_continuation
  installation_v2_pre_store_finality
]
raise "owner-bound adversarial cases drifted" unless %w[
  arbitrary_h3_digest_vector_is_unconstructible
  consumer_zero_without_authoritative_nonempty_census_is_refused
  raw_cutover_host_facts_are_unconstructible
].all? { |name| fixture.fetch("required_adversarial_cases").include?(name) }
raise "consumer closure is not three-stage" unless gates.fetch("gates").map { |gate| gate.fetch("stage") } == [
  "BeforeSemanticCurrentness",
  "ProtectedRetention",
  "PhysicalPruning"
]
raise "sealed reader became a bearer" unless gates.fetch("sealed_reader") == {
  "autoloading" => false,
  "bearer" => false,
  "admission" => "OpaqueSealedOnly"
}
raise "production owner-route contract drifted" unless gates.fetch("production_owner_routes") == {
  "consumer_snapshot" => "ConsumerClosureV1::evaluate_installation_snapshot",
  "durable_consumer_finality" => "ConsumerClosureDurableLinearizationV1",
  "aggregate_census" => "census_admitted_owner_roots_v2",
  "pre_store_finality" => "stage11_finality_v2::execute_pre_store"
}
expected_counts = {
  "e204" => 204,
  "c325" => 325,
  "skill_ledger" => 35
}
raise "instance header drifted" unless instance_header.fetch("row_counts") == expected_counts
raise "instance digest drifted" unless fixture.fetch("instance_fixture").fetch("sha256") == Digest::SHA256.hexdigest(instance_bytes)
grouped = instance_records.group_by { |record| record.fetch("family") }
raise "instance family drifted" unless grouped.keys.sort == expected_counts.keys.sort
expected_counts.each do |family, count|
  rows = grouped.fetch(family)
  raise "#{family} count drifted" unless rows.length == count
  raise "#{family} ordinal drifted" unless rows.map { |row| row.fetch("ordinal") } == (1..count).to_a
end
{
  "e204" => "embedded_resources.e204.v1.json",
  "c325" => "direct_consumers.c325.v1.json",
  "skill_ledger" => "v1_skill_ledger.v1.json"
}.each do |family, name|
  source = JSON.parse(root.join("contracts/vnext/public", name).read).fetch("rows")
  observed = grouped.fetch(family).map { |record| record.fetch("row") }
  raise "#{family} rows differ from frozen source" unless observed == source
end
physical_source = instance_header.fetch("source_files").fetch("physical")
raise "fabricated physical rows remain" if grouped.key?("physical")
raise "physical commitment count drifted" unless physical_source.fetch("historical_node_count") == 28_102
raise "physical commitment digest drifted" unless physical_source.fetch("sha256") == Digest::SHA256.file(
  root.join("contracts/vnext/public/physical_census.commitment.v1.json")
).hexdigest
raise "physical fixture posture drifted" unless physical_source.fetch("fixture_posture") ==
  "aggregate_commitment_only_no_fabricated_rows"
physical_commitment = JSON.parse(
  root.join("contracts/vnext/public/physical_census.commitment.v1.json").read
)
raise "physical grammar drifted" unless physical_commitment.fetch("identity_row_grammar") == [
  "type",
  "payload_length",
  "payload_sha256",
  "lexically_normalized_absolute_locator"
]
raise "physical directories became identity rows" unless physical_commitment.fetch(
  "directory_containers_included"
) == false
raise "physical historical rows were invented" unless physical_commitment.fetch(
  "literal_historical_rows_retained"
) == false

runtime_sources = root.join("src/domain/migration/runtime").glob("*.rs").sort
owned = runtime_sources.map(&:read).join("\n")
production_owned = runtime_sources.map(&:read).join("\n").sub(
  /\n#\[cfg\(test\)\]\nmod cohort_observation_tests \{.*\z/m,
  "",
)
consumer = root.join("src/domain/migration/runtime/consumer.rs").read
consumer_snapshot = root.join("src/domain/installation/consumer_snapshot.rs").read
installation = root.join("src/domain/installation/mod.rs").read
foundation = root.join("src/foundation/core/mod.rs").read
%w[OldProtocol MixedProtocol UnknownProtocol ReleaseMismatch].each do |reason|
  raise "missing refusal #{reason}" unless owned.include?(reason)
end
%w[
  AuthoritativeConsumerCensusV1
  test_only_from_stage4_publication
  ActiveStoreFinalityV1
  PreStoreFinalityV1
].each do |token|
  raise "missing owner binding #{token}" unless owned.include?(token)
end
raise "Migration does not consume the Installation-owned consumer snapshot" unless
  consumer.include?("evaluate_installation_snapshot") &&
  consumer.include?("InstallationMigrationConsumerSnapshotV1") &&
  consumer.include?("snapshot.into_parts()")
raise "Installation durable consumer finality route drifted" unless
  consumer_snapshot.include?("ConsumerClosureDurableLinearizationV1") &&
  consumer_snapshot.include?("bind_migration_census") &&
  consumer_snapshot.include?("durable_effect.commit")
raise "Installation V2 PreStore finality route drifted" unless
  installation.include?("pub(in crate::domain) mod stage11_finality_v2") &&
  installation.include?("execute_pre_store") &&
  installation.include?("ProtectedLocatorLeaseV2")
raise "Foundation V2 aggregate-census owner route drifted" unless
  foundation.include?("pub(crate) mod stage11_aggregate_census") &&
  foundation.include?("census_from_stage11_owner_v2") &&
  foundation.include?("consume_for_stage11")
raise "arbitrary-digest H3 constructor remains" if owned.include?("NativeCancellationCausalJoinV1::new")
raise "incomplete H3 adapter escaped test-only scope" unless owned.include?(
  "#[cfg(test)]\n    pub fn test_only_from_stage4_publication"
)
raise "H3 publication is reusable" unless owned.include?("Stage4PublicationReused")
raise "H3 row binding is absent" unless owned.include?("CancellationJoinRowMismatch")
raise "frozen H3 native-cancelled member is not consumed exactly" unless %w[
  VerifiedH3WithdrawalPublicationUseV1
  H3NativeCancelledMigrationMemberV1
  consume_native_cancelled_member_for_migration
  H3NativeCancelledSourceMemberV1::new
  H3NativeCancelledTargetMemberV1::new
  H3NativeCancelledClassificationV1::new
  H3MigrationFinalityV1::ActiveStore
  H3MigrationFinalityV1::PreStore
  H3CarrierCountMismatch
  H3MemberCoverageMismatch
  H3MemberDuplicate
  H3MemberContradiction
  H3VerifiedMigrationAssociationUseV1
  _consumed_members
].all? { |token| owned.include?(token) }
raise "obsolete H3 carrier-only consumption remains" if
  owned.include?(".consume_for_migration(") ||
  owned.include?("ConsumedH3WithdrawalPublicationV1")
raise "H3 member identity grammar drifted" unless owned.include?(
  "maestro.execution.h3-native-cancelled-member.v1\\0"
)
raise "zero identity guard is absent" unless owned.include?("ZeroDigest") && owned.include?("ZeroIdentity")
raise "rollback deletes production bytes" if
  production_owned.include?("remove_file") || production_owned.include?("remove_dir")
operations_facade = root.join("src/operations/migration/mod.rs").read
raise "Foundation census is not production-wired" if
  operations_facade.include?("#[cfg(test)]\nmod census;")
census_source = root.join("src/operations/migration/census.rs").read
raise "Foundation V2 aggregate-census entry route drifted" unless census_source.include?(
  "census_admitted_owner_roots_v2"
)
raise "Stage 11 does not consume only the V2 Foundation continuation" unless %w[
  MigrationClassificationContinuationV2
  continuation.consume_for_stage11()
  Stage11CensusContinuationV2
  ProtectedLocatorLeaseV2
  stage11_finality_v2
].all? { |token| census_source.include?(token) }
[
  "DeclaredRootScanV1",
  "recensus_declared_roots",
  "PathBuf",
  "for_admitted_root_set",
  "descriptor_census_platform::census"
].each do |token|
  raise "retired V1 physical census input escaped Foundation ownership: #{token}" if
    census_source.include?(token)
end

puts({ schema: "maestro.vnext.stage11.independent-verification.v1", status: "ok" }.to_json)
