#!/usr/bin/env python3
"""Build the inactive Stage-0 SubmissionClaimSetV1 literal and vectors."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


REPO = Path(__file__).resolve().parents[4]
OUTPUT = REPO / "contracts/vnext/stage0/submission-claim"
DOMAIN = b"maestro.submission-claim-set.v1"


def head(major: int, value: int) -> bytes:
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    if value <= 0xFFFFFFFFFFFFFFFF:
        return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")
    raise ValueError("unsigned value exceeds u64")


def cbor(value: Any) -> bytes:
    if isinstance(value, int):
        return head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return head(3, len(raw)) + raw
    if isinstance(value, list):
        return head(4, len(value)) + b"".join(cbor(item) for item in value)
    raise ValueError(f"unsupported deterministic CBOR value: {value!r}")


def schema_descriptor() -> list[Any]:
    no_additional = [[1]]
    entry_type = [8, [[3], [4, 32], [4, 32]]]
    return [
        "SubmissionClaimSetV1",
        1,
        [
            [1, "submission_id", [3], no_additional],
            [2, "claim_count", [1], [[4, 1, 0xFFFFFFFFFFFFFFFF]]],
            [3, "entries", [7, entry_type], [[2, 1, 0xFFFFFFFFFFFFFFFF]]],
            [4, "digest", [4, 32], no_additional],
        ],
        [],
    ]


def schema_id(descriptor: list[Any]) -> str:
    return "sha256:" + hashlib.sha256(
        cbor(["maestro.vnext.schema.v1", descriptor])
    ).hexdigest()


def length_prefixed(raw: bytes) -> bytes:
    return len(raw).to_bytes(8, "big") + raw


def claim_digest(submission_id: str, entries: list[dict[str, str]]) -> tuple[str, str]:
    ordered = sorted(
        entries,
        key=lambda entry: (
            bytes.fromhex(entry["normalized_proposition_hash"]),
            entry["claim_id"].encode("ascii"),
        ),
    )
    if entries != ordered:
        raise ValueError("entries are not canonically ordered")
    claim_ids = [entry["claim_id"] for entry in entries]
    propositions = [entry["normalized_proposition_hash"] for entry in entries]
    records = [entry["claim_record_hash"] for entry in entries]
    if not entries:
        raise ValueError("zero-Claim set")
    if len(set(claim_ids)) != len(claim_ids):
        raise ValueError("duplicate claim_id")
    if len(set(propositions)) != len(propositions):
        raise ValueError("duplicate normalized_proposition_hash")
    if len(set(records)) != len(records):
        raise ValueError("duplicate claim_record_hash")

    raw = DOMAIN + length_prefixed(submission_id.encode("ascii"))
    raw += len(entries).to_bytes(8, "big")
    for entry in entries:
        raw += length_prefixed(entry["claim_id"].encode("ascii"))
        raw += bytes.fromhex(entry["normalized_proposition_hash"])
        raw += bytes.fromhex(entry["claim_record_hash"])
    return hashlib.sha256(raw).hexdigest(), raw.hex()


def build() -> dict[str, Any]:
    descriptor = schema_descriptor()
    descriptor_cbor = cbor(descriptor)
    identity_input_cbor = cbor(["maestro.vnext.schema.v1", descriptor])
    vectors = [
        {
            "name": "one-claim",
            "submission_id": "submission-1",
            "entries": [
                {
                    "claim_id": "claim-a",
                    "normalized_proposition_hash": (bytes([1]) * 32).hex(),
                    "claim_record_hash": (bytes([11]) * 32).hex(),
                }
            ],
        },
        {
            "name": "many-claims",
            "submission_id": "submission-2",
            "entries": [
                {
                    "claim_id": "claim-z",
                    "normalized_proposition_hash": (bytes([1]) * 32).hex(),
                    "claim_record_hash": (bytes([12]) * 32).hex(),
                },
                {
                    "claim_id": "claim-a",
                    "normalized_proposition_hash": (bytes([2]) * 32).hex(),
                    "claim_record_hash": (bytes([13]) * 32).hex(),
                },
                {
                    "claim_id": "claim-b",
                    "normalized_proposition_hash": (bytes([3]) * 32).hex(),
                    "claim_record_hash": (bytes([14]) * 32).hex(),
                },
            ],
        },
    ]
    for vector in vectors:
        digest, digest_input = claim_digest(vector["submission_id"], vector["entries"])
        vector["claim_count"] = len(vector["entries"])
        vector["digest"] = digest
        vector["canonical_digest_input_hex"] = digest_input

    mutants = [
        "zero_claims",
        "claim_count_mismatch",
        "noncanonical_order",
        "duplicate_claim_id",
        "duplicate_normalized_proposition_hash",
        "duplicate_claim_record_hash",
        "digest_mismatch",
        "unknown_field",
        "non_ascii_submission_id",
        "non_ascii_claim_id",
    ]
    return {
        "schema": "maestro.vnext.submission-claim-set-literal.v1",
        "candidate_only": True,
        "runtime_activation": False,
        "domain_hex": DOMAIN.hex(),
        "schema_descriptor": descriptor,
        "schema_id": schema_id(descriptor),
        "schema_descriptor_cbor_hex": descriptor_cbor.hex(),
        "schema_descriptor_cbor_sha256": hashlib.sha256(descriptor_cbor).hexdigest(),
        "schema_identity_input_cbor_hex": identity_input_cbor.hex(),
        "schema_identity_input_cbor_sha256": hashlib.sha256(identity_input_cbor).hexdigest(),
        "ordering": ["normalized_proposition_hash", "claim_id"],
        "variable_field_encoding": "u64be-length-then-exact-bytes",
        "fixed_hash_encoding": "raw-32-bytes",
        "semantic_mutants_rejected": mutants,
        "vectors": vectors,
    }


def canonical_json(value: dict[str, Any]) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    value = build()
    payload = canonical_json(value)
    artifact = OUTPUT / "submission-claim-set.v1.json"
    receipt_value = {
        "schema": "maestro.vnext.submission-claim-set-encoder-receipt.v1",
        "candidate_only": True,
        "runtime_activation": False,
        "artifact_sha256": hashlib.sha256(payload).hexdigest(),
        "schema_id": value["schema_id"],
        "vector_digests": [vector["digest"] for vector in value["vectors"]],
        "required_encoders": ["python-stdlib", "ruby-stdlib"],
        "semantic_mutant_count": len(value["semantic_mutants_rejected"]),
    }
    receipt = canonical_json(receipt_value)
    receipt_path = OUTPUT / "encoder-receipt.v1.json"
    if args.check:
        if artifact.read_bytes() != payload or receipt_path.read_bytes() != receipt:
            raise SystemExit("SubmissionClaimSetV1 generated artifacts drifted")
    else:
        OUTPUT.mkdir(parents=True, exist_ok=True)
        artifact.write_bytes(payload)
        receipt_path.write_bytes(receipt)
    print(json.dumps(receipt_value, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
