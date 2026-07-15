#!/usr/bin/env python3
"""Build the non-promoting historical E204 and C325 coverage ledgers."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


E204_DIGEST = "c8fc4c6cd53d81272d19c3b402e99a0ca3f69ebd18cf9464539db1d1ecf85388"
C302_DIGEST = "3e43d5c5689ff05c79e63e7c9a4577291c3f504ca41e788a087c36bdc925f562"
C23_DIGEST = "45d3e81a128681436ebdc17ec52c5af39eacc7bd354d90a5097e93ad1496011b"
C325_DIGEST = "9aee8ea371f770e8694131079d4bfb4845f849d59d0b545005a2f0371a42976a"

C23_PATHS = [
    "src/domain/feature/mod.rs",
    "src/domain/proof/mod.rs",
    "src/domain/task/mod.rs",
    "src/features/evidence/index.ts",
    "src/features/principle/index.ts",
    "src/features/reply/index.ts",
    "src/features/verdict/index.ts",
    "src/foundation/core/git.rs",
    "src/foundation/core/time.rs",
    "src/infra/domain/config-types.ts",
    "src/infra/domain/git-types.ts",
    "src/infra/domain/status-types.ts",
    "src/infra/ports/config.port.ts",
    "src/infra/ports/git.port.ts",
    "src/infra/usecases/config-edit.usecase.ts",
    "src/interfaces/cli/watch.rs",
    "src/repo/contract-store.port.ts",
    "src/repo/run-state-store.port.ts",
    "src/service/contract-helpers.ts",
    "src/shared/domain/legacy-mission.ts",
    "src/shared/domain/task/index.ts",
    "src/shared/errors.ts",
    "src/shared/lib/sanitize.ts",
]


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def manifest_digest(rows: dict[str, str]) -> str:
    stream = "".join(f"{digest}  {path}\n" for path, digest in sorted(rows.items()))
    return sha256_bytes(stream.encode("utf-8"))


def embedded_family(path: str) -> str:
    if path in {"embedded/AGENTS.md", "embedded/CLAUDE.md"}:
        return "root-agent-instruction"
    for prefix, family in [
        ("embedded/design/", "design"),
        ("embedded/harness/", "harness"),
        ("embedded/hooks/", "hook"),
        ("embedded/loop-recipes/", "recipe-profile"),
        ("embedded/playbook/", "playbook"),
        ("embedded/schemas/", "schema"),
        ("embedded/shell/", "shell"),
        ("embedded/skills/", "skill"),
    ]:
        if path.startswith(prefix):
            return family
    raise SystemExit(f"unclassified E204 historical path: {path}")


def rows_by_path(rows: list[dict]) -> dict[str, str]:
    result = {row["path"]: row["sha256"] for row in rows}
    if len(result) != len(rows):
        raise SystemExit("duplicate path in historical coverage inputs")
    return result


def encoded_json(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def emit(path: Path, value: object, check: bool, mismatches: list[str]) -> None:
    expected = encoded_json(value)
    if check:
        if not path.is_file() or path.read_bytes() != expected:
            mismatches.append(str(path))
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(expected)


def build(input_path: Path, output_dir: Path, check: bool) -> dict:
    source = json.loads(input_path.read_text(encoding="utf-8"))
    if not (
        source["candidate_only"] is True
        and source["runtime_activation"] is False
        and source["runtime_registration"] is False
        and source["attestation"] == "non_promoting_historical_coverage"
    ):
        raise SystemExit("historical coverage inputs are not sealed candidate-only evidence")

    embedded = rows_by_path(source["embedded_rows"])
    consumers = rows_by_path(source["direct_consumer_rows"])
    if len(embedded) != 204 or manifest_digest(embedded) != E204_DIGEST:
        raise SystemExit("E204 historical coverage commitment mismatch")
    if len(consumers) != 325 or manifest_digest(consumers) != C325_DIGEST:
        raise SystemExit("C325 historical coverage commitment mismatch")
    c23 = {path: consumers[path] for path in C23_PATHS}
    c302 = {path: digest for path, digest in consumers.items() if path not in c23}
    if len(c302) != 302 or manifest_digest(c302) != C302_DIGEST:
        raise SystemExit("C302 historical checkpoint mismatch")
    if len(c23) != 23 or manifest_digest(c23) != C23_DIGEST:
        raise SystemExit("C23 historical additive closure mismatch")

    input_sha256 = sha256_bytes(input_path.read_bytes())
    mismatches: list[str] = []
    emit(
        output_dir / "embedded_resources.e204.v1.json",
        {
            "schema": "maestro.vnext.embedded-resource-evidence-ledger.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "evidence_classification": "non_promoting_historical_coverage",
            "current_source_equality_claimed": False,
            "source_input": {
                "path": "contracts/vnext/public/historical_source_coverage_inputs.v1.json",
                "sha256": input_sha256,
            },
            "digest_algorithm": "sha256(sorted(lowercase_file_sha256 + two_spaces + path + LF))",
            "expected_count": 204,
            "expected_digest": E204_DIGEST,
            "rows": [
                {"path": path, "sha256": digest, "family": embedded_family(path)}
                for path, digest in sorted(embedded.items())
            ],
        },
        check,
        mismatches,
    )
    emit(
        output_dir / "direct_consumers.c325.v1.json",
        {
            "schema": "maestro.vnext.direct-consumer-evidence-ledger.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "evidence_classification": "non_promoting_historical_coverage",
            "current_source_equality_claimed": False,
            "source_input": {
                "path": "contracts/vnext/public/historical_source_coverage_inputs.v1.json",
                "sha256": input_sha256,
            },
            "digest_algorithm": "sha256(sorted(lowercase_file_sha256 + two_spaces + path + LF))",
            "expected_count": 325,
            "expected_digest": C325_DIGEST,
            "predecessor": {"count": 302, "digest": C302_DIGEST},
            "additive_closure": {"count": 23, "digest": C23_DIGEST, "paths": C23_PATHS},
            "rows": [
                {
                    "path": path,
                    "sha256": digest,
                    "source": "family11_additive" if path in c23 else "bound_historical_coverage",
                }
                for path, digest in sorted(consumers.items())
            ],
        },
        check,
        mismatches,
    )
    receipt = {
        "schema": "maestro.vnext.historical-census-ledger-build-receipt.v1",
        "mode": "check" if check else "write",
        "status": "pass" if not mismatches else "fail",
        "e204_count": 204,
        "e204_digest": E204_DIGEST,
        "c325_count": 325,
        "c325_digest": C325_DIGEST,
        "physical_receipt_emitted": False,
        "mismatches": mismatches,
    }
    if mismatches:
        print(json.dumps(receipt, indent=2, sort_keys=True))
        raise SystemExit(1)
    return receipt


def main() -> None:
    repo = Path(__file__).resolve().parents[3]
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--input",
        type=Path,
        default=repo / "contracts/vnext/public/historical_source_coverage_inputs.v1.json",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=repo / "contracts/vnext/public",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    receipt = build(args.input.resolve(), args.output_dir.resolve(), args.check)
    print(json.dumps(receipt, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
