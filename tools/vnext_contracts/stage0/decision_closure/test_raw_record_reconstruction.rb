#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"
require "open3"
require "pathname"
require "tmpdir"

root = Pathname.new(__dir__).join("../../../../").realpath
fixture = root.join(
  ".maestro/cards/dec-canonical-authority-materialization-df3b/card.yaml"
)
environment = {
  "STAGE0_RAW_RECORD_RECONSTRUCTION_TEST" => "1",
  "STAGE0_RAW_RECORD_FIXTURE" => fixture.to_s
}
stdout, stderr, status = Open3.capture3(
  environment,
  "ruby",
  Pathname.new(__dir__).join("build.rb").to_s
)
raise "standalone Decision reconstruction test failed: #{stderr}" unless status.success?

result = JSON.parse(stdout)
raise "standalone Decision reconstruction schema drifted" unless
  result.fetch("schema") ==
    "maestro.vnext.stage0-standalone-decision-record-reconstruction-test.v1"
raise "standalone Decision reconstruction mutant coverage drifted" unless
  result.fetch("mutants_rejected") == %w[one_byte indent final_lf]

Dir.mktmpdir("maestro-stage0-descriptor-") do |directory|
  fixture_path = Pathname.new(directory).join("fixture")
  fixture_path.binwrite("descriptor-captured\n")
  fixture_path.chmod(0o600)
  descriptor_environment = {
    "STAGE0_DESCRIPTOR_CAPTURE_TEST" => "1",
    "STAGE0_DESCRIPTOR_CAPTURE_FIXTURE" => fixture_path.to_s
  }
  captured_stdout, captured_stderr, captured_status = Open3.capture3(
    descriptor_environment,
    "ruby",
    Pathname.new(__dir__).join("build.rb").to_s
  )
  raise "descriptor capture positive test failed: #{captured_stderr}" unless
    captured_status.success?
  captured = JSON.parse(captured_stdout)
  raise "descriptor capture changed exact fixture bytes" unless
    captured.fetch("sha256") == Digest::SHA256.hexdigest("descriptor-captured\n")

  symlink_path = Pathname.new(directory).join("substituted")
  symlink_path.make_symlink(fixture_path.basename)
  _, _, symlink_status = Open3.capture3(
    descriptor_environment.merge("STAGE0_DESCRIPTOR_CAPTURE_FIXTURE" => symlink_path.to_s),
    "ruby",
    Pathname.new(__dir__).join("build.rb").to_s
  )
  raise "descriptor capture accepted a symlink substitution" if symlink_status.success?
end

puts JSON.generate(result)
