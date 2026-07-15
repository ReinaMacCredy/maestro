#!/usr/bin/env python3
"""Independent Python encoder for the frozen ManifestIdentityV1 CBOR subset."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def _head(major: int, value: int) -> bytes:
    if not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("ManifestIdentityV1 integers and lengths are unsigned u64")
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def encode(value: object) -> bytes:
    if value is False:
        return b"\xf4"
    if value is True:
        return b"\xf5"
    if isinstance(value, int) and not isinstance(value, bool):
        return _head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return _head(3, len(raw)) + raw
    if isinstance(value, list):
        return _head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        raw = bytes.fromhex(value["bytes"])
        return _head(2, len(raw)) + raw
    raise ValueError(f"value is outside the ManifestIdentityV1 subset: {value!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("input", type=Path)
    args = parser.parse_args()
    value = json.loads(args.input.read_text(encoding="ascii"))
    encoded = encode(value)
    print(encoded.hex())
    print(len(encoded))
    print(hashlib.sha256(encoded).hexdigest())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
