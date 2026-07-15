#!/usr/bin/env ruby
# Design-only ManifestIdentityV1 reference encoder; not product source.

require "digest"
require "json"

def head(major, value)
  raise "unsigned value outside u64" unless value.between?(0, 0xffffffffffffffff)
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
    raise "text must be ASCII" unless value.ascii_only?
    raw = value.b
    head(3, raw.bytesize) + raw
  when Array
    head(4, value.length) + value.map { |item| encode(item) }.join
  when Hash
    raise "only bytes wrapper objects are supported" unless value.keys == ["bytes"]
    text = value.fetch("bytes")
    raise "bytes wrapper requires lowercase even-length hex" unless text.is_a?(String) && text.length.even? && text == text.downcase && text.match?(/\A[0-9a-f]*\z/)
    raw = [text].pack("H*")
    head(2, raw.bytesize) + raw
  else
    raise "unsupported ManifestIdentityV1 value: #{value.inspect}"
  end
end

abort "usage: vnext_manifest_encode_rb.rb VALUE.json" unless ARGV.length == 1
value = JSON.parse(File.read(ARGV.fetch(0), encoding: "US-ASCII"))
encoded = encode(value)
puts encoded.unpack1("H*")
puts encoded.bytesize
puts Digest::SHA256.hexdigest(encoded)
