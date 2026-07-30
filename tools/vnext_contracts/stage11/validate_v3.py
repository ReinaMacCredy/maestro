#!/usr/bin/env python3
"""Stage-11 V3 quarantine contract validator."""

from __future__ import annotations

import hashlib
import json
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[3]
CONTRACT = ROOT / "tests/fixtures/vnext/stage11/live_set_v3_contract.v1.json"


def fail(message: str) -> None:
    raise SystemExit(f"stage11-v3: {message}")


def main() -> None:
    contract = json.loads(CONTRACT.read_text(encoding="utf-8"))
    if (
        contract["status"]
        != "stage11_corrected_focused_verified_awaiting_main_integration"
    ):
        fail("candidate status must remain focused-verified and await MainIntegration")

    for relative, expected in contract["immutable_v2_sources"].items():
        source = (ROOT / relative).read_bytes()
        observed = hashlib.sha256(source).hexdigest()
        if observed != expected:
            fail(f"immutable V2 source changed: {relative}")

    v3_text = []
    for relative, required in contract["required_sources"].items():
        path = ROOT / relative
        if not path.is_file():
            fail(f"required source is absent: {relative}")
        text = path.read_text(encoding="utf-8")
        v3_text.append((relative, text))
        missing = [needle for needle in required if needle not in text]
        if missing:
            fail(f"{relative} is missing {missing}")

    for relative, text in v3_text:
        present = [
            token
            for token in contract["forbidden_in_v3_sources"]
            if token in text
        ]
        if present:
            fail(f"{relative} imports or adapts historical V2 authority: {present}")
        claims = [
            token for token in contract["forbidden_claims"] if token in text
        ]
        if claims:
            fail(f"{relative} contains a forbidden currentness claim: {claims}")

    runtime_mod = (
        ROOT / "src/domain/migration/runtime/mod.rs"
    ).read_text(encoding="utf-8")
    for exported in (
        "LegacyQuarantineEpochV3",
        "Stage12SightingManifestV2",
        "MigrationClassificationManifestV3",
        "DeclaredOverlapManifestV2",
        "UnavailablePreexistingLossManifestV3",
        "SealedQuarantineManifestV3",
    ):
        if exported not in runtime_mod:
            fail(f"runtime facade does not export {exported}")

    print("stage11-v3 source contract: ok")


if __name__ == "__main__":
    main()
