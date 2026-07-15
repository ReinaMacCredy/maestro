#!/usr/bin/env ruby
# Independent Ruby encoder for the frozen ManifestIdentityV1 CBOR subset.

require "digest"
require "json"

def head(major, value)
  raise "ManifestIdentityV1 integers and lengths are unsigned u64" unless value.between?(0, 0xffffffffffffffff)

  if value < 24
    [(major << 5) | value].pack("C")
  elsif value <= 0xff
    [(major << 5) | 24, value].pack("CC")
  elsif value <= 0xffff
    [(major << 5) | 25, value].pack("Cn")
  elsif value <= 0xffffffff
    [(major << 5) | 26, value].pack("CN")
  else
    [(major << 5) | 27, value].pack("CQ>")
  end
end

def encode(value)
  case value
  when false
    "\xf4".b
  when true
    "\xf5".b
  when Integer
    head(0, value)
  when String
    raw = value.encode(Encoding::US_ASCII).b
    head(3, raw.bytesize) + raw
  when Array
    head(4, value.length) + value.map { |item| encode(item) }.join
  when Hash
    raise "only the canonical raw-byte wrapper is allowed" unless value.keys == ["bytes"]

    raw = [value.fetch("bytes")].pack("H*")
    head(2, raw.bytesize) + raw
  else
    raise "value is outside the ManifestIdentityV1 subset: #{value.inspect}"
  end
end

abort "usage: cbor_rb.rb INPUT.json" unless ARGV.length == 1

value = JSON.parse(File.read(ARGV.fetch(0), encoding: Encoding::US_ASCII))
encoded = encode(value)
puts encoded.unpack1("H*")
puts encoded.bytesize
puts Digest::SHA256.hexdigest(encoded)
