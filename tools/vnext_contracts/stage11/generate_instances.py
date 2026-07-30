#!/usr/bin/env python3
"""Generate the hermetic instance-level Stage-11 migration fixture."""

from __future__ import annotations

import json
import hashlib
from pathlib import Path
from typing import Any, Iterator


ROOT = Path(__file__).resolve().parents[3]
PUBLIC = ROOT / "contracts/vnext/public"
OUTPUT = ROOT / "tests/fixtures/vnext/stage11/migration_instances.v1.jsonl"
def load(name: str) -> dict[str, Any]:
    return json.loads((PUBLIC / name).read_text(encoding="utf-8"))


def sha256_file(name: str) -> str:
    return hashlib.sha256((PUBLIC / name).read_bytes()).hexdigest()


def canonical_line(value: object) -> str:
    return json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True)


def main() -> None:
    e204 = load("embedded_resources.e204.v1.json")
    c325 = load("direct_consumers.c325.v1.json")
    skills = load("v1_skill_ledger.v1.json")
    physical = load("physical_census.commitment.v1.json")
    header = {
        "schema": "maestro.vnext.stage11.migration-instances.v1",
        "record": "header",
        "source_files": {
            "e204": {
                "path": "contracts/vnext/public/embedded_resources.e204.v1.json",
                "sha256": sha256_file("embedded_resources.e204.v1.json"),
            },
            "c325": {
                "path": "contracts/vnext/public/direct_consumers.c325.v1.json",
                "sha256": sha256_file("direct_consumers.c325.v1.json"),
            },
            "physical": {
                "path": "contracts/vnext/public/physical_census.commitment.v1.json",
                "sha256": sha256_file("physical_census.commitment.v1.json"),
                "literal_historical_rows_retained": False,
                "live_recensus_required": True,
                "historical_node_count": physical["historical_attested_receipt"]["node_count"],
                "fixture_posture": "aggregate_commitment_only_no_fabricated_rows",
            },
            "skill_ledger": {
                "path": "contracts/vnext/public/v1_skill_ledger.v1.json",
                "sha256": sha256_file("v1_skill_ledger.v1.json"),
            },
        },
        "row_counts": {
            "e204": len(e204["rows"]),
            "c325": len(c325["rows"]),
            "skill_ledger": len(skills["rows"]),
        },
    }
    records: Iterator[dict[str, object]] = iter(
        [
            *(
                {"family": "e204", "ordinal": index, "row": row}
                for index, row in enumerate(e204["rows"], start=1)
            ),
            *(
                {"family": "c325", "ordinal": index, "row": row}
                for index, row in enumerate(c325["rows"], start=1)
            ),
        ]
    )
    with OUTPUT.open("w", encoding="ascii", newline="\n") as output:
        output.write(canonical_line(header))
        output.write("\n")
        for record in records:
            output.write(canonical_line(record))
            output.write("\n")
        for index, row in enumerate(skills["rows"], start=1):
            output.write(
                canonical_line({"family": "skill_ledger", "ordinal": index, "row": row})
            )
            output.write("\n")


if __name__ == "__main__":
    main()
