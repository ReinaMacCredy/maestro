#!/usr/bin/env ruby
# frozen_string_literal: true

# Independent Ruby encoder for the Stage-0 Effect Home canonical-CBOR input.

require "digest"
require "json"

def head(major, value)
  raise "unsigned u64 required" unless value.is_a?(Integer) && value.between?(0, 0xffff_ffff_ffff_ffff)

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

def encode(value)
  case value
  when false then "\xf4".b
  when true then "\xf5".b
  when Integer then head(0, value)
  when String
    raw = value.encode(Encoding::US_ASCII).b
    head(3, raw.bytesize) + raw
  when Array
    head(4, value.length) + value.map { |item| encode(item) }.join
  when Hash
    raise "only the raw byte wrapper is permitted" unless value.keys == ["bytes"]

    raw = [value.fetch("bytes")].pack("H*")
    head(2, raw.bytesize) + raw
  else
    raise "value outside deterministic CBOR subset: #{value.inspect}"
  end
end

abort "usage: encode.rb INPUT.json" unless ARGV.length == 1

input = JSON.parse(File.read(ARGV.fetch(0), encoding: Encoding::US_ASCII))
abort "wrong encoder input schema" unless input.fetch("schema_version") == "maestro.vnext.stage0.effect-home-encoder-input.v1"
artifacts = input.fetch("artifacts").to_h do |record|
  encoded = encode(record.fetch("value"))
  [record.fetch("name"), { "cbor_hex" => encoded.unpack1("H*"), "byte_length" => encoded.bytesize, "sha256" => Digest::SHA256.hexdigest(encoded) }]
end
puts JSON.generate({ "schema_version" => "maestro.vnext.stage0.effect-home-ruby-encoder-receipt.v1", "artifacts" => artifacts })
