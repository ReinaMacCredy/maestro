#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"

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
  when Bytes then head(2, value.value.bytesize) + value.value
  when String
    raw = value.encode(Encoding::US_ASCII).b
    head(3, raw.bytesize) + raw
  when Integer then head(0, value)
  when Array then head(4, value.length) + value.map { |item| cbor(item) }.join
  else raise "unsupported deterministic CBOR value #{value.class}"
  end
end

def parse(value)
  if value.is_a?(Hash) && value.keys == ["bytes"]
    raw = [value.fetch("bytes")].pack("H*")
    raise "invalid raw identity bytes" unless raw.bytesize == 32

    Bytes.new(raw)
  elsif value.is_a?(Array)
    value.map { |item| parse(item) }
  else
    value
  end
end

root = ARGV.fetch(0)
names = %w[
  candidate-root-schema-descriptors.v1.json
  design-revision.v1.json
  candidate-contract-root.v1.json
  design-finalization-manifest.v1.json
  canonical-build-handoff.v1.json
]
artifacts = names.to_h do |name|
  document = JSON.parse(File.read(File.join(root, name)))
  [name, cbor(parse(document.fetch("canonical_value"))).unpack1("H*")]
end
puts JSON.generate({ "schema" => "maestro.vnext.stage0-candidate-root-ruby-encoder.v1", "artifacts" => artifacts })
