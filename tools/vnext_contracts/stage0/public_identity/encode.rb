#!/usr/bin/env ruby
# Independent Ruby encoder for the public-identity canonical CBOR closure value.

require "digest"
require "json"

def head(major, value)
  raise "canonical public identity values require unsigned u64" unless value.between?(0, 0xffffffffffffffff)

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
    raise "only raw-byte wrappers are permitted" unless value.keys == ["bytes"]

    raw = [value.fetch("bytes")].pack("H*")
    head(2, raw.bytesize) + raw
  else
    raise "unsupported canonical public identity value"
  end
end

abort "usage: encode.rb INPUT.json" unless ARGV.length == 1

input = JSON.parse(File.read(ARGV.fetch(0), encoding: Encoding::UTF_8))
encoded = encode(input.fetch("closure_value"))
puts JSON.generate({ "hex" => encoded.unpack1("H*"), "sha256" => Digest::SHA256.hexdigest(encoded), "bytes" => encoded.bytesize })
