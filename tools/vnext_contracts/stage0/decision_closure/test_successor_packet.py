#!/usr/bin/env python3
"""Read-only closure checks for the six packet-bound successor Decisions."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path("/private/tmp/maestro-vnext-materialization-successor-packet")
EXPECTED = {
    "dec-canonical-authority-materialization-df3b": (
        "locked",
        "0d7c406f68f04fdf47ce00d56e8189b54159f164323c9511504790b941f715d0",
        "624f81c44b1a6459bc13472df05f547276d694e0f38c7216bb8df732aa3418cf",
    ),
    "dec-canonical-execution-h3-verified-0939": (
        "locked",
        "b5935c389182a7f3ec6447fb2a13dcb70e912108b399d0b1d25fee5f132186a7",
        "a98f1fdb95fcb3f2604936f50e9aa6661ad75bd51469d576e49239c5a6138307",
    ),
    "dec-canonical-foundation-descriptor-a128": (
        "locked",
        "17fb79ef9bc74cf3838d869bf5fb3b0ae0e9ae017670ca7cb207aeb8105c234e",
        "59fc4db26ec24f2f2ddc2df5cd70462f767e5d7e2d81644edc11a61c7fb7b26c",
    ),
    "dec-canonical-installation-consumer-c1fe": (
        "locked",
        "aaba56a8f34fb293a68f26743fbf4ef879d9f5a399a4eb45da74eed70a509e53",
        "5f35840fed183b406baab4cf9044ab05e3677f7061798c7949e80a868d2cd466",
    ),
    "dec-canonical-non-action-protected-90a9": (
        "locked",
        "8c6be56db78d8695b4e85e09fc4217257fee0b2dce0f5b5be8ef10230f24c20e",
        "7f0ea93dddef6354183b48cec27f6dee47f802688956bf552e3cb64ecca88f81",
    ),
    "dec-canonical-trusted-host-protected-1fbc": (
        "locked",
        "e572dc28e0c811c81207558e64b0372f757a873122b7f537f6354af819f118d8",
        "e6e84dea058097be48312ef98154958246763bac7f38d877ea04aee4af030d99",
    ),
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    manifest_path = ROOT / "successor-decision-store-manifest.v1.txt"
    inventory_path = ROOT / "raw-decision-inventory.v1.txt"
    closure_path = ROOT / "external-design-authority-closure.v1.txt"
    packet_path = ROOT / "replacement-build-approval-packet.v1.json"
    expected_hashes = {
        manifest_path: "18f14bce862e15be09c9d88155d62627582df50c7754e2e8e1d6f6bee8f7d522",
        inventory_path: "704c21bd7f1e6c39d5c4c488bba7e0c28d22fcb7af4059b72eb01a83715e0962",
        closure_path: "b58ba8af29e55004b6b34bd8a1b1767c91b23e482c16cfe1d0560655be4f66d6",
        packet_path: "7f13c85b45799e39daedd30846b4a024d1f264134b46c3e3b3cdf720f8e5fb02",
    }
    for path, expected in expected_hashes.items():
        if path.is_symlink() or not path.is_file() or sha256(path) != expected:
            raise SystemExit(f"substituted successor packet artifact: {path}")
    manifest_rows = [line.split("\t") for line in manifest_path.read_text().splitlines()]
    if len(manifest_rows) != 213 or any(len(row) != 4 for row in manifest_rows):
        raise SystemExit("successor Decision manifest is not the exact 213-row closure")
    manifest = {row[0]: tuple(row[1:]) for row in manifest_rows}
    if len(manifest) != 213:
        raise SystemExit("successor Decision manifest repeats an id")
    for decision_id, expected in EXPECTED.items():
        if manifest.get(decision_id) != expected:
            raise SystemExit(f"successor Decision head drifted: {decision_id}")
    counts = {
        status: sum(row[1] == status for row in manifest_rows)
        for status in ("locked", "superseded", "open")
    }
    if counts != {"locked": 117, "superseded": 96, "open": 0}:
        raise SystemExit("successor Decision terminal counts drifted")
    ignored = {
        (fields[1], fields[2])
        for line in closure_path.read_text().splitlines()
        if (fields := line.split("\t"))[0] == "E"
        and len(fields) == 5
        and fields[4] == "ignored_unilateral_claim"
    }
    if (
        "dec-canonical-non-action-protected-90a9",
        "dec-canonical-trusted-host-protected-1fbc",
    ) not in ignored or len(ignored) != 3:
        raise SystemExit("successor ignored unilateral-claim closure drifted")
    packet = json.loads(packet_path.read_text())
    if packet["packet_sha256"] != "fb33b048b59c66df9858558a2c80e59a478d101465761f902366c9a00751cbc5":
        raise SystemExit("successor packet identity drifted")
    for command in (
        [sys.executable, str(Path(__file__).with_name("validate.py"))],
        ["/usr/bin/ruby", str(Path(__file__).with_name("validate.rb"))],
    ):
        completed = subprocess.run(command, check=False, capture_output=True, text=True)
        if completed.returncode != 0:
            raise SystemExit(f"historical Decision closure readback failed: {completed.stderr}")
    print(
        json.dumps(
            {
                "schema": "maestro.vnext.stage0-successor-decision-source-test.v1",
                "decision_total": 213,
                "locked": 117,
                "superseded": 96,
                "open": 0,
                "successor_heads": len(EXPECTED),
                "historical_readback": "pass",
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )


if __name__ == "__main__":
    main()
