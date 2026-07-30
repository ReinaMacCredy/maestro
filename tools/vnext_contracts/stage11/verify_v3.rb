#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"

root = File.expand_path("../../..", __dir__)
contract_path = File.join(
  root,
  "tests/fixtures/vnext/stage11/live_set_v3_contract.v1.json"
)
contract = JSON.parse(File.read(contract_path, encoding: "UTF-8"))

unless contract.fetch("status") == "stage11_corrected_focused_verified_awaiting_main_integration"
  abort("stage11-v3: status drift")
end

contract.fetch("immutable_v2_sources").each do |relative, expected|
  observed = Digest::SHA256.file(File.join(root, relative)).hexdigest
  abort("stage11-v3: immutable V2 source changed: #{relative}") unless observed == expected
end

contract.fetch("required_sources").each do |relative, required|
  path = File.join(root, relative)
  abort("stage11-v3: required source absent: #{relative}") unless File.file?(path)

  text = File.read(path, encoding: "UTF-8")
  required.each do |needle|
    abort("stage11-v3: #{relative} missing #{needle}") unless text.include?(needle)
  end
  contract.fetch("forbidden_in_v3_sources").each do |needle|
    abort("stage11-v3: #{relative} adapts historical V2 authority") if text.include?(needle)
  end
  contract.fetch("forbidden_claims").each do |needle|
    abort("stage11-v3: #{relative} contains forbidden claim #{needle}") if text.include?(needle)
  end
end

puts "stage11-v3 ruby contract: ok"
