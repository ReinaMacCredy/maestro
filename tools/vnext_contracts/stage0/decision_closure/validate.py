#!/usr/bin/env python3
"""Independent Stage-0 Decision closure CBOR encoder and semantic validator."""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path


EXTERNAL_DOMAIN = "maestro.vnext.external-design-authority-closure.v1"
DECISION_DOMAIN = "maestro.vnext.decision-closure.v1"


def head(major: int, value: int) -> bytes:
    if not 0 <= value <= 0xFFFFFFFFFFFFFFFF:
        raise ValueError("unsigned u64 required")
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
        return head(0, value)
    if isinstance(value, str):
        raw = value.encode("ascii")
        return head(3, len(raw)) + raw
    if isinstance(value, list):
        return head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict) and list(value) == ["bytes"]:
        raw = bytes.fromhex(value["bytes"])
        return head(2, len(raw)) + raw
    raise ValueError(f"value outside deterministic CBOR subset: {value!r}")


def raw_bytes(record: dict[str, object]) -> dict[str, str]:
    return {"bytes": record["raw_record_bytes"]["bytes"]}  # type: ignore[index]


def optional(value: object) -> list[object]:
    return [0] if value is None else [1, value]


def external_record(record: dict[str, object]) -> list[object]:
    return [
        record["id"], record["terminal_status"], {"bytes": record["raw_record_sha256"]},
        {"bytes": record["raw_body_sha256"]}, record["raw_supersedes"],
        record["raw_superseded_by"], record["external_authoring_disposition"],
        optional(record["normalized_successor"]), record["consequence_classification"],
        optional(record["rationale_disposition"]),
        [{"bytes": value} for value in record["materialization_ids"]],
        record["derived_effect_status"], raw_bytes(record),
    ]


def decision_record(record: dict[str, object]) -> list[object]:
    return [
        record["id"], record["terminal_status"], {"bytes": record["raw_record_sha256"]},
        {"bytes": record["raw_body_sha256"]}, record["raw_supersedes"],
        record["raw_superseded_by"], record["external_authoring_disposition"],
        optional(record["normalized_successor"]), record["consequence_classification"],
        optional(record["rationale_disposition"]),
        [{"bytes": value} for value in record["materialization_ids"]],
        record["derived_effect_status"],
    ]


def materialization(item: dict[str, object]) -> list[object]:
    return [
        {"bytes": item["id"]}, item["artifact_id"], item["component_kind_tag"], 0,
        [[source["id"], {"bytes": source["body_sha256"]}] for source in item["decision_sources"]],
    ]


def value(document: dict[str, object], external: bool) -> list[object]:
    lineage = document["lineage"]
    records = [external_record(item) if external else decision_record(item) for item in document["records"]]
    materials = [materialization(item) for item in document["materializations"]]
    ignored = [[item["source"], item["claimed_predecessor"]] for item in lineage["ignored_unilateral_claims"]]
    composites = [[item["id"], item["raw_supersedes"]] for item in lineage["composite_external_heads"]]
    base: list[object] = [1, records, materials, ignored, composites]
    if external:
        base.append(lineage["recognized_external_composite_heads"])
    return base


def validate(document: dict[str, object], external: bool) -> tuple[str, bytes]:
    summary = document["summary"]
    records = document["records"]
    if document["closure_state"] != "closed":
        raise ValueError("closure is not closed")
    if [item["id"] for item in records] != sorted(item["id"] for item in records):
        raise ValueError("records are not sorted")
    if len({item["id"] for item in records}) != 207:
        raise ValueError("omitted or duplicate Decision record")
    if summary != {"total": 207, "locked": 112, "superseded": 95, "open": 0, "material": 204, "rationale_only": 3, "unresolved_mappings": 0, "pending_component_slots": 109, "normalized_one_to_one_edges": 23}:
        raise ValueError("summary drift")
    materialization_base = {
        "kind": "initial_external_design_closure",
        "decision_closure_id": document["decision_closure_reference"] if external else document["identity"],
    }
    root_assembly = document["root_assembly"]
    if root_assembly != {"state": "pending_exact_component_resolution", "resolved_component_ids": [], "materialization_base": materialization_base, "candidate_root_after": None, "finalization_manifest_id": None}:
        raise ValueError("fabricated or incomplete root resolution")
    ids = {item["id"] for item in records}
    successors = {item["id"]: item["normalized_successor"] for item in records}
    for start in successors:
        seen: set[str] = set()
        current: str | None = start
        while current is not None:
            if current in seen:
                raise ValueError("normalized successor cycle")
            seen.add(current)
            current = successors[current]
    materialization_rows = document["materializations"]
    if len({item["id"] for item in materialization_rows}) != len(materialization_rows):
        raise ValueError("duplicate materialization")
    material_ids: dict[str, dict[str, object]] = {item["id"]: item for item in materialization_rows}
    seen_sources: set[str] = set()
    for item in records:
        if external:
            raw = bytes.fromhex(item["raw_record_bytes"]["bytes"])
            if hashlib.sha256(raw).hexdigest() != item["raw_record_sha256"]:
                raise ValueError("raw record hash changed")
        if item["terminal_status"] not in {"locked", "superseded"}:
            raise ValueError("open or invalid terminal status")
        if item["terminal_status"] == "superseded" and not item["raw_superseded_by"]:
            raise ValueError("raw lineage omission")
        if item["external_authoring_disposition"] == "composite_external_authoring" and item["normalized_successor"] is not None:
            raise ValueError("composite promotion")
        if item["external_authoring_disposition"] == "unilateral_raw_claim" and item["normalized_successor"] is not None:
            raise ValueError("unilateral repair")
        if item["consequence_classification"] == "rationale_only":
            if item["rationale_disposition"] is None or item["materialization_ids"]:
                raise ValueError("missing rationale disposition")
        else:
            if not item["materialization_ids"]:
                raise ValueError("missing materialization")
            if item["derived_effect_status"] not in {"unapplied", "superseded_but_effect_live"}:
                raise ValueError("missing effect-live coverage")
        successor = item["normalized_successor"]
        if successor is not None and successor not in ids:
            raise ValueError("unknown normalized successor")
    for material_id, item in material_ids.items():
        if item["materialization_base"] != materialization_base:
            raise ValueError("materialization base drift")
        if item["binding_state"] != "required_component_slot_pending" or "before_root_id" in item or any(item[key] is not None for key in ("exact_component_id", "after_root_id", "finalization_manifest_id")):
            raise ValueError("materialization must remain pending exact root resolution")
        source_ids = [source["id"] for source in item["decision_sources"]]
        if source_ids != sorted(source_ids) or len(source_ids) != len(set(source_ids)):
            raise ValueError("duplicate or unordered materialization source")
        for source in item["decision_sources"]:
            record = next(row for row in records if row["id"] == source["id"])
            if source["body_sha256"] != record["raw_body_sha256"]:
                raise ValueError("stale materialization")
            if material_id not in record["materialization_ids"]:
                raise ValueError("non-reciprocal materialization")
            seen_sources.add(source["id"])
    if seen_sources != {item["id"] for item in records if item["consequence_classification"] == "material"}:
        raise ValueError("incomplete materialization closure")
    domain = EXTERNAL_DOMAIN if external else DECISION_DOMAIN
    encoded = encode(value(document, external))
    identity = hashlib.sha256(encode([domain, value(document, external)])).hexdigest()
    if document["identity"] != f"sha256:{identity}":
        raise ValueError("identity mismatch")
    if document["canonical_cbor_sha256"] != hashlib.sha256(encoded).hexdigest():
        raise ValueError("canonical CBOR mismatch")
    return identity, encoded


def main() -> int:
    root = Path(os.environ.get("STAGE0_DECISION_CLOSURE_ROOT", Path(__file__).resolve().parents[4] / "contracts/vnext/stage0/decision-closure"))
    external = json.loads((root / "external-design-authority-closure.v1.json").read_text(encoding="ascii"))
    decision = json.loads((root / "decision-closure.v1.json").read_text(encoding="ascii"))
    external_id, external_cbor = validate(external, True)
    decision_id, decision_cbor = validate(decision, False)
    if external_cbor != (root / "external-design-authority-closure.v1.cbor").read_bytes():
        raise ValueError("external CBOR file mismatch")
    if decision_cbor != (root / "decision-closure.v1.cbor").read_bytes():
        raise ValueError("Decision CBOR file mismatch")
    print(json.dumps({"external_closure_id": f"sha256:{external_id}", "decision_closure_id": f"sha256:{decision_id}", "encoder": "python", "semantic_validation": "pass"}, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
