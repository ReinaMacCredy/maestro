#!/usr/bin/env python3
"""Design-only ManifestIdentityV1 reference encoder; not product source."""

import hashlib
import json
import sys


def head(major: int, value: int) -> bytes:
    if not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("unsigned value outside u64")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def encode(value) -> bytes:
    if value is False:
        return b"\xf4"
    if value is True:
        return b"\xf5"
    if isinstance(value, int) and not isinstance(value, bool):
        return head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return head(3, len(raw)) + raw
    if isinstance(value, list):
        return head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        text = value["bytes"]
        if not isinstance(text, str) or len(text) % 2 or text.lower() != text:
            raise ValueError("bytes wrapper requires lowercase even-length hex")
        raw = bytes.fromhex(text)
        return head(2, len(raw)) + raw
    raise ValueError(f"unsupported ManifestIdentityV1 value: {value!r}")


if len(sys.argv) != 2:
    raise SystemExit("usage: vnext_manifest_encode_py.py VALUE.json")
value = json.load(open(sys.argv[1], "r", encoding="ascii"))
encoded = encode(value)
print(encoded.hex())
print(len(encoded))
print(hashlib.sha256(encoded).hexdigest())
