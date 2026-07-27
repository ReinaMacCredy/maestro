#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"
require "open3"
require "pathname"

ENGINES = %w[python rust ruby].freeze
READBACK_KINDS = %w[
  compiled_namespace_absence generated_resource_absence persisted_identity_parity
  canonical_facade_behavior migration_route_absence retained_reader_absence
  consumer_reader_hold_zero negative_fixture
].sort.freeze

class NoDuplicateHash < Hash
  def []=(key, value)
    raise "duplicate JSON key: #{key}" if key?(key)
    super
  end
end

def load_object(path)
  raw = File.binread(path)
  raise "noncanonical JSON: #{path}" unless raw.end_with?("\n") && !raw.include?("\r") && !raw.start_with?("\xEF\xBB\xBF".b)
  value = JSON.parse(raw, object_class: NoDuplicateHash)
  raise "object required: #{path}" unless value.is_a?(Hash)
  value
end

def sha_bytes(raw)
  "sha256:#{Digest::SHA256.hexdigest(raw)}"
end

def identity(path)
  sha_bytes(File.binread(path))
end

def canonical(value)
  normalized = case value
               when Hash
                 value.keys.sort.to_h { |key| [key, JSON.parse(canonical(value.fetch(key)))] }
               when Array
                 value.map { |item| JSON.parse(canonical(item)) }
               else
                 value
               end
  JSON.generate(normalized) + "\n"
end

def safe_path(root, value)
  raise "portable relative path required" unless value.is_a?(String) && !value.empty? && !value.include?("\\")
  path = Pathname.new(value)
  raise "path escapes source" if path.absolute? || path.each_filename.any? { |part| [".", "..", ""].include?(part) }
  File.join(root, value)
end

def verify_binding(root, binding)
  raise "file binding required" unless binding.is_a?(Hash)
  path = safe_path(root, binding["path"])
  raise "bound file absent or unsafe: #{path}" unless File.file?(path) && !File.symlink?(path)
  raw = File.binread(path)
  raise "bound file differs: #{path}" unless binding["byte_length"] == raw.bytesize && binding["sha256"] == sha_bytes(raw)
  path
end

def validate_snapshot(snapshot, frozen_root, source)
  raise "snapshot schema differs" unless snapshot["schema_version"] == "maestro.external.vnext-final-cumulative-closure-snapshot.v1"
  raise "snapshot is not frozen" unless snapshot["state"] == "frozen"
  raise "approved packet identity differs" unless snapshot["approved_packet_identity"] == "sha256:2026513c84b1993f020f7d0430154ec0bc4e821438ccefd7dd6b91834a3d6283"
  stages = snapshot.fetch("first_parent_stages")
  raise "Stage checkpoint closure differs" unless stages.map { |row| row["stage"] } == (0..12).to_a
  raise "current V4 Stage 12 checkpoint differs" unless stages.last["commit"] == snapshot.fetch("final_integration")["commit"]
  stages.each do |row|
    checkpoint = load_object(verify_binding(frozen_root, row["checkpoint"]))
    raise "Stage checkpoint bytes differ" unless %w[stage commit tree].all? { |field| checkpoint[field] == row[field] }
  end
  raise "immutable roots differ" unless snapshot["immutable_input_roots"] == %w[source packet control]
  roles = snapshot.fetch("writable_root_roles")
  raise "disjoint writable roots differ" unless roles.length == 12 && roles.uniq.length == 12
  raise "sandbox profile differs" unless snapshot["sandbox_profile"] == "macos-sandbox-exec-no-network-v1"
  raise "environment allowlist differs" unless snapshot["environment_allowlist"] == %w[HOME LANG LC_ALL PATH TMPDIR TZ]
  raise "cache policy differs" unless snapshot["cache_policy"] == "immutable_compilation_and_dependency_bytes_only"
  raise "pointer preimage is absent" unless snapshot["pointer_preimage"].is_a?(Hash)
  required_denials = %w[network protected_primary_checkout_write outside_packet_bound_roots_write]
  raise "effect denylist differs" unless (required_denials - snapshot.fetch("effect_denylist")).empty?
  engines = snapshot.fetch("engines")
  raise "engine closure differs" unless engines.map { |row| row["id"] } == ENGINES
  engines.each { |row| verify_binding(source, row["source"]) }
  %w[input_manifest proof_ledger stage12_readback toolchain].each do |field|
    verify_binding(frozen_root, snapshot[field])
  end
end

def validate_packet(snapshot, packet_root)
  binding = snapshot["packet_manifest"]
  raise "packet-manifest binding differs" unless binding.is_a?(Hash) &&
                                                  binding["path"] == "packet/packet-manifest.v1.json"
  manifest_path = File.join(packet_root, "packet-manifest.v1.json")
  raw = File.binread(manifest_path)
  raise "packet-manifest bytes differ" unless binding["byte_length"] == raw.bytesize &&
                                               binding["sha256"] == sha_bytes(raw)
  manifest = load_object(manifest_path)
  raise "packet manifest schema differs" unless manifest["schema_version"] == "maestro.external.vnext-final-packet-manifest.v1"
  raise "packet identity differs" unless manifest["approved_packet_identity"] == snapshot["approved_packet_identity"]
  rows = manifest.fetch("files")
  seen = rows.map do |row|
    adjusted = row.merge("path" => row.fetch("path").delete_prefix("packet/"))
    verify_binding(packet_root, adjusted)
    adjusted["path"]
  end
  actual = Dir.children(packet_root).select do |name|
    File.file?(File.join(packet_root, name)) && name != "packet-manifest.v1.json"
  end
  raise "packet manifest has an omission" unless actual.sort == seen.sort
  raise "packet manifest totals differ" unless manifest["file_count"] == rows.length &&
                                                manifest["byte_length"] == rows.sum { |row| row["byte_length"] }
end

def validate_manifest(manifest, source)
  raise "input manifest schema differs" unless manifest["schema_version"] == "maestro.external.vnext-final-input-manifest.v1"
  rows = manifest.fetch("entries")
  seen = {}
  total = 0
  rows.each do |row|
    raise "input manifest duplicates a path" if seen.key?(row["path"])
    verify_binding(source, row)
    seen[row["path"]] = true
    total += row.fetch("byte_length")
  end
  actual = Dir.glob(File.join(source, "**", "*"), File::FNM_DOTMATCH)
              .select { |path| File.file?(path) }
              .map { |path| Pathname.new(path).relative_path_from(Pathname.new(source)).to_s }
              .sort
  raise "input manifest has an omission or extra path" unless actual == seen.keys.sort
  raise "input manifest totals differ" unless manifest["entry_count"] == rows.length && manifest["byte_length"] == total
end

def stream_identity(raw)
  {"byte_length" => raw.bytesize, "sha256" => sha_bytes(raw)}
end

def validate_toolchain(toolchain, source)
  raise "toolchain schema differs" unless toolchain["schema_version"] == "maestro.external.vnext-final-toolchain.v1"
  tools = toolchain.fetch("tools")
  raise "toolchain closure differs" unless tools.keys.sort == %w[cargo git python ruby rust]
  raise "toolchain target, profile, or environment differs" unless toolchain["target"].is_a?(String) &&
                                                                   toolchain["profile"].is_a?(String) &&
                                                                   toolchain["environment"] == {"LC_ALL" => "C", "LANG" => "C", "TZ" => "UTC"}
  lockfiles = toolchain["lockfiles"]
  raise "lockfile closure is absent" unless lockfiles.is_a?(Array) && !lockfiles.empty?
  lockfiles.each { |binding| verify_binding(source, binding) }
  resolved = tools.to_h do |name, row|
    path = row.fetch("resolved_path")
    raise "tool is absent or unsafe: #{name}" unless File.file?(path) && !File.symlink?(path)
    raw = File.binread(path)
    raise "tool bytes differ: #{name}" unless row["byte_length"] == raw.bytesize && row["sha256"] == sha_bytes(raw)
    stdout, stderr, status = Open3.capture3(*row.fetch("probe_argv"))
    raise "tool probe differs: #{name}" unless status.exitstatus == row["probe_exit_code"] &&
                                                stream_identity(stdout.b) == row["probe_stdout"] &&
                                                stream_identity(stderr.b) == row["probe_stderr"]
    [name, path]
  end
  dependencies = toolchain["dependency_outputs"]
  raise "dependency-output closure is absent" unless dependencies.is_a?(Array) && !dependencies.empty?
  dependencies.each do |dependency|
    root = dependency.fetch("resolved_path")
    raise "dependency-output root is absent or unsafe" unless File.directory?(root) && !File.symlink?(root)
    rows = dependency.fetch("files").map do |row|
      path = safe_path(root, row["path"])
      raise "dependency-output file is absent or unsafe" unless File.file?(path) && !File.symlink?(path)
      raw = File.binread(path)
      actual = {"path" => row["path"], "byte_length" => raw.bytesize, "sha256" => sha_bytes(raw)}
      raise "dependency-output bytes differ" unless actual == row
      actual
    end
    actual_paths = Dir.glob(File.join(root, "**", "*"), File::FNM_DOTMATCH)
                      .select { |path| File.file?(path) }
                      .map { |path| Pathname.new(path).relative_path_from(Pathname.new(root)).to_s }
                      .sort
    raise "dependency-output manifest has an omission" unless actual_paths == rows.map { |row| row["path"] }.sort
    closure = JSON.generate(rows.map { |row| row.sort.to_h }) + "\n"
    raise "dependency-output identity differs" unless dependency["file_count"] == rows.length &&
                                                     dependency["byte_length"] == rows.sum { |row| row["byte_length"] } &&
                                                     dependency["identity"] == sha_bytes(closure)
  end
  resolved
end

def command_identity(argv, expected_exit_code)
  sha_bytes(JSON.generate({"argv" => argv, "expected_exit_code" => expected_exit_code}.sort.to_h) + "\n")
end

def validate_ledger(ledger, source)
  raise "ledger schema differs" unless ledger["schema_version"] == "maestro.external.vnext-final-proof-ledger.v1"
  rows = ledger.fetch("proofs")
  raise "ledger count differs" unless ledger["proof_count"] == rows.length
  ids = {}
  stages = []
  kinds = []
  rows.each do |row|
    raise "duplicate proof id" if ids.key?(row["proof_id"])
    ids[row["proof_id"]] = true
    stages << row["stage"]
    kinds << row["kind"]
    raise "proof engine coverage differs" unless row["engines"] == ENGINES
    command = row.fetch("command")
    raise "proof command identity differs" unless command["identity"] == command_identity(command["argv"], command["expected_exit_code"])
    row.fetch("input_bindings").each { |binding| verify_binding(source, binding) }
    verify_binding(source, command["fault_schedule"]) if %w[race crash_replay].include?(row["kind"])
    if %w[migration rollback].include?(row["kind"])
      cohort = command.fetch("cohort")
      raise "migration cohort absent" unless %w[old_reader new_reader writer fixture].all? { |key| cohort.key?(key) }
      verify_binding(source, cohort["fixture"])
    end
  end
  raise "proof Stage or kind closure differs" unless stages.uniq.sort == (0..12).to_a && kinds.uniq.length == 13
  rows
end

def expand(argv, tools, source)
  argv.map do |value|
    if value == "{source}"
      source
    elsif value.start_with?("{tool:") && value.end_with?("}")
      tools.fetch(value[6...-1])
    elsif value.include?("{") || value.include?("}")
      raise "unknown command placeholder: #{value}"
    else
      value
    end
  end
end

def produced_artifacts(source, paths)
  paths.map do |value|
    path = safe_path(source, value)
    raise "declared produced artifact is absent: #{path}" unless File.file?(path) && !File.symlink?(path)
    raw = File.binread(path)
    {"path" => value, "byte_length" => raw.bytesize, "sha256" => sha_bytes(raw)}
  end
end

def execute_proof(row, source, tools)
  command = row.fetch("command")
  stdout, stderr, status = Open3.capture3(*expand(command.fetch("argv"), tools, source), chdir: source)
  passed = status.exitstatus == command["expected_exit_code"]
  receipt = {
    "proof_id" => row["proof_id"], "stage" => row["stage"], "kind" => row["kind"],
    "command_identity" => command["identity"], "expected_outcome" => row["expected_outcome"],
    "actual_outcome" => passed ? row["expected_outcome"] : "error", "exit_code" => status.exitstatus,
    "stdout" => stream_identity(stdout.b), "stderr" => stream_identity(stderr.b),
    "produced_artifacts" => produced_artifacts(source, row.fetch("produced_artifacts"))
  }
  if %w[race crash_replay].include?(row["kind"])
    schedule_path = verify_binding(source, command.fetch("fault_schedule"))
    schedule = load_object(schedule_path)
    schedules = schedule["schedules"]
    raise "fault schedule is empty" unless schedules.is_a?(Array) && !schedules.empty?
    receipt["fault_schedule_identity"] = identity(schedule_path)
    receipt["injection_points_reached"] = schedules.map { |item| item.fetch("id") }
  end
  if %w[migration rollback].include?(row["kind"])
    receipt["cohort_identity"] = sha_bytes(canonical(command.fetch("cohort")))
  end
  receipt
end

def scan_counts(check, source)
  target = File.join(ENV.fetch("CARGO_TARGET_DIR"), "release")
  files = check.fetch("scan_roots").flat_map do |root|
    case root
    when "source:src"
      Dir.glob(File.join(source, "src", "**", "*")).select { |path| File.file?(path) }
    when "source:embedded"
      Dir.glob(File.join(source, "embedded", "**", "*")).select { |path| File.file?(path) }
    when "target:release"
      raise "semantic target root is unavailable" unless File.directory?(target)
      Dir.glob(File.join(target, "maestro*")).select { |path| File.file?(path) }
    else
      raise "unknown semantic scan root: #{root}"
    end
  end.uniq
  literals = check.fetch("count_literals")
  %w[consumers readers holds].to_h do |label|
    values = literals.fetch(label)
    count = files.sum do |path|
      raw = File.binread(path)
      relative = path.start_with?(source) ? Pathname.new(path).relative_path_from(Pathname.new(source)).to_s : File.basename(path)
      values.count { |value| raw.include?(value.b) || relative.include?(value) }
    end
    [label, count]
  end
end

def semantic_readback(plan, source, tools)
  raise "readback schema differs" unless plan["schema_version"] == "maestro.external.vnext-stage12-semantic-readback-plan.v1"
  checks = plan.fetch("checks")
  raise "readback closure differs" unless checks.map { |row| row["kind"] }.sort == READBACK_KINDS
  rows = checks.map do |check|
    raise "readback command identity differs" unless check["command_identity"] == command_identity(check["argv"], check["expected_exit_code"])
    raise "readback zero-count contract differs" unless check["expected_counts"] == {"consumers" => 0, "readers" => 0, "holds" => 0}
    _stdout, _stderr, status = Open3.capture3(*expand(check["argv"], tools, source), chdir: source)
    counts = scan_counts(check, source)
    passed = status.exitstatus == check["expected_exit_code"] && counts == check["expected_counts"]
    {
      "id" => check["id"], "kind" => check["kind"], "command_identity" => check["command_identity"],
      "exit_code" => status.exitstatus, "status" => passed ? "pass" : "fail",
      "consumer_count" => counts["consumers"], "reader_count" => counts["readers"], "hold_count" => counts["holds"]
    }
  end
  {
    "status" => rows.all? { |row| row["status"] == "pass" } ? "pass" : "fail",
    "consumer_count" => rows.map { |row| row["consumer_count"] }.max,
    "reader_count" => rows.map { |row| row["reader_count"] }.max,
    "hold_count" => rows.map { |row| row["hold_count"] }.max,
    "checks" => rows
  }
end

raise "expected snapshot manifest ledger readback toolchain packet source output" unless ARGV.length == 8
snapshot_path, manifest_path, ledger_path, readback_path, toolchain_path, packet, source, output = ARGV.map { |value| File.expand_path(value) }
snapshot = load_object(snapshot_path)
manifest = load_object(manifest_path)
ledger = load_object(ledger_path)
readback = load_object(readback_path)
toolchain = load_object(toolchain_path)
validate_snapshot(snapshot, File.dirname(File.dirname(snapshot_path)), source)
validate_packet(snapshot, packet)
validate_manifest(manifest, source)
tools = validate_toolchain(toolchain, source)
proofs = validate_ledger(ledger, source).map { |row| execute_proof(row, source, tools) }
receipt = {
  "schema_version" => "maestro.external.vnext-final-engine-ledger.v1",
  "engine" => "ruby",
  "snapshot_identity" => identity(snapshot_path),
  "input_manifest_identity" => identity(manifest_path),
  "ledger_identity" => identity(ledger_path),
  "readback_plan_identity" => identity(readback_path),
  "toolchain_identity" => identity(toolchain_path),
  "proofs" => proofs,
  "semantic_readback" => semantic_readback(readback, source, tools)
}
File.binwrite(output, JSON.generate(receipt.sort.to_h) + "\n")
