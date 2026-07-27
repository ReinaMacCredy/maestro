#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"
require "open3"

def load_object(path)
  raw = File.binread(path)
  raise "noncanonical JSON: #{path}" unless raw.end_with?("\n") && !raw.include?("\r") && !raw.start_with?("\xEF\xBB\xBF".b)
  value = JSON.parse(raw)
  raise "object required: #{path}" unless value.is_a?(Hash)
  value
end

def identity(path)
  "sha256:#{Digest::SHA256.hexdigest(File.binread(path))}"
end

def execute(argv, cwd)
  _stdout, _stderr, status = Open3.capture3(*argv, chdir: cwd)
  status.exitstatus
end

snapshot_path, ledger_path, readback_path, source_path, output_path = ARGV.map { |value| File.expand_path(value) }
snapshot = load_object(snapshot_path)
ledger = load_object(ledger_path)
readback = load_object(readback_path)
raise "snapshot schema differs" unless snapshot["schema_version"] == "maestro.external.vnext-final-cumulative-closure-snapshot.v1"
raise "ledger schema differs" unless ledger["schema_version"] == "maestro.external.vnext-final-proof-ledger.v1"
raise "readback schema differs" unless readback["schema_version"] == "maestro.external.vnext-stage12-semantic-readback-plan.v1"
proof_ids = ledger.fetch("proofs").map { |proof| proof.fetch("proof_id") }
proof_stages = ledger.fetch("proofs").map { |proof| proof.fetch("stage") }.uniq.sort
raise "ledger proof closure differs" unless proof_ids.uniq.length == proof_ids.length && proof_stages == (0..12).to_a
raise "ledger engine coverage differs" unless ledger.fetch("proofs").all? { |proof| proof.fetch("engines").sort == %w[python ruby rust] }

proofs = ledger.fetch("proofs").map do |proof|
  specification = proof.fetch("command")
  exit_code = execute(specification.fetch("argv"), source_path)
  actual = exit_code == specification.fetch("expected_exit_code") ? proof.fetch("expected_outcome") : "error"
  {"proof_id" => proof.fetch("proof_id"), "expected_outcome" => proof.fetch("expected_outcome"), "actual_outcome" => actual, "exit_code" => exit_code}
end

checks = readback.fetch("checks").map do |check|
  exit_code = execute(check.fetch("argv"), source_path)
  {"id" => check.fetch("id"), "kind" => check.fetch("kind"), "exit_code" => exit_code, "status" => exit_code == check.fetch("expected_exit_code") ? "pass" : "fail"}
end
receipt = {
  "schema_version" => "maestro.external.vnext-final-engine-receipt.v1",
  "engine" => "ruby",
  "snapshot_identity" => identity(snapshot_path),
  "ledger_identity" => identity(ledger_path),
  "proofs" => proofs,
  "semantic_readback" => {"status" => checks.all? { |row| row["status"] == "pass" } ? "pass" : "fail", "checks" => checks}
}
File.write(output_path, JSON.generate(receipt) + "\n", mode: "w:ASCII-8BIT")
