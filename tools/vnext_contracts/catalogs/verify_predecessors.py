#!/usr/bin/env python3
"""Rebuild and verify the immutable e346 predecessor catalog closure."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


REPO = Path(__file__).resolve().parents[3]
SOURCE_ROOT = Path(__file__).with_name("predecessor_e346")
NOMINAL_SOURCE = REPO / "contracts/vnext/catalogs/evidence/e346-nominal-source.json"
EXPECTED_RECEIPT = REPO / "contracts/vnext/catalogs/predecessor/e346/reproduction.json"
CAPTURE_RECEIPT = REPO / "contracts/vnext/catalogs/evidence/e346-reproduction-sources.json"

SOURCE_FILES = [
    "vnext_catalog_profile_grammar_build.py",
    "vnext_catalog_profile_grammar_validate.py",
    "vnext_catalog_suite_build.py",
    "vnext_catalog_suite_validate.py",
    "vnext_manifest_encode_py.py",
    "vnext_manifest_encode_rb.rb",
]

GRAMMAR_MUTANTS = [
    "missing_action",
    "duplicate_action_value",
    "wrong_action_owner",
    "wrong_ceremony_owner",
    "wrong_route_symbol",
    "wrong_route_role",
    "wrong_route_basis",
    "wrong_route_context",
    "wrong_dependency_edge",
    "wrong_grammar_identity",
    "duplicate_action",
    "wrong_action_tag",
    "wrong_origin_source_owner",
    "wrong_owner_profile",
    "changed_normative_clause",
    "wrong_semantic_version",
    "forward_dependency",
    "extra_grammar_binding",
    "invalid_canonical_set_field_path",
    "invalid_cross_constraint_field_path",
]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_json(command: list[str], cwd: Path) -> dict[str, object]:
    result = subprocess.run(command, cwd=cwd, check=True, capture_output=True, text=True)
    return json.loads(result.stdout)


def encoder_receipt(command: list[str], input_path: Path, cwd: Path) -> dict[str, object]:
    result = subprocess.run([*command, str(input_path)], cwd=cwd, check=True, capture_output=True, text=True)
    lines = result.stdout.strip().splitlines()
    if len(lines) != 3:
        raise ValueError(f"encoder returned an invalid receipt: {result.stdout}")
    return {"byte_length": int(lines[1]), "sha256": lines[2]}


def reproduce() -> dict[str, object]:
    with tempfile.TemporaryDirectory(prefix="maestro-vnext-e346-") as temporary:
        root = Path(temporary)
        source_hashes = {}
        for name in SOURCE_FILES:
            source = SOURCE_ROOT / name
            if not source.is_file():
                raise FileNotFoundError(source)
            source_hashes[name] = sha256(source)
            shutil.copyfile(source, root / name)
        shutil.copyfile(NOMINAL_SOURCE, root / "vnext-catalog-nominal-source-v1.json")

        grammar_build = run_json(["python3", "vnext_catalog_profile_grammar_build.py"], root)
        grammar_id = str(grammar_build["catalog_profile_grammar_id"])
        grammar_artifact = root / f"vnext-catalog-profile-grammar-v1-sha256-{grammar_id}.json"
        grammar_input = root / "vnext-catalog-profile-grammar-v1-encoder-input.json"
        grammar_python = encoder_receipt(
            ["python3", "vnext_manifest_encode_py.py"], grammar_input, root
        )
        grammar_ruby = encoder_receipt(["ruby", "vnext_manifest_encode_rb.rb"], grammar_input, root)
        if grammar_python != grammar_ruby:
            raise ValueError("predecessor grammar encoders disagree")
        grammar_validation = run_json(
            [
                "python3",
                "vnext_catalog_profile_grammar_validate.py",
                str(grammar_artifact),
                str(root / "vnext-catalog-nominal-source-v1.json"),
            ],
            root,
        )
        if grammar_validation["mutants_rejected"] != len(GRAMMAR_MUTANTS):
            raise ValueError("predecessor grammar mutant suite is incomplete")

        run_json(["python3", "vnext_catalog_suite_build.py"], root)
        suite_validation = run_json(["python3", "vnext_catalog_suite_validate.py"], root)
        index_path = root / "vnext-catalog-literal-suite-v1-index.json"
        index = json.loads(index_path.read_text(encoding="ascii"))

        catalogs = []
        for index_row in index["catalogs"]:
            artifact_path = root / index_row["artifact_path"]
            artifact = json.loads(artifact_path.read_text(encoding="ascii"))
            receipts = artifact["encoder_receipts"]
            if receipts["equal"] is not True:
                raise ValueError(f"catalog {index_row['catalog_tag']} predecessor encoders disagree")
            aggregate = receipts["aggregate"]
            catalogs.append(
                {
                    "artifact_path": index_row["artifact_path"],
                    "artifact_sha256": sha256(artifact_path),
                    "catalog_slug": index_row["catalog_slug"],
                    "catalog_tag": index_row["catalog_tag"],
                    "encoder_input_sha256": artifact["encoder_input_sha256"],
                    "encoder_receipts": {
                        "python": {
                            "implementation_sha256": receipts["python_encoder_sha256"],
                            "byte_length": aggregate["byte_length"],
                            "sha256": aggregate["sha256"],
                        },
                        "ruby": {
                            "implementation_sha256": receipts["ruby_encoder_sha256"],
                            "byte_length": aggregate["byte_length"],
                            "sha256": aggregate["sha256"],
                        },
                    },
                    "manifest_byte_length": artifact["byte_length"],
                    "manifest_id": artifact["manifest_id"],
                    "row_count": len(artifact["descriptors"]),
                }
            )

        return {
            "schema_version": "maestro.vnext.catalog.predecessor-reproduction.v1",
            "status": "immutable_non_current_predecessor_evidence",
            "source_files": source_hashes,
            "nominal_source_sha256": sha256(NOMINAL_SOURCE),
            "grammar": {
                "artifact_path": grammar_artifact.name,
                "artifact_sha256": sha256(grammar_artifact),
                "grammar_id": grammar_id,
                "identity_input_count": grammar_build["identity_input_count"],
                "encoder_input_sha256": sha256(grammar_input),
                "encoder_receipts": {
                    "python": {
                        "implementation_sha256": source_hashes["vnext_manifest_encode_py.py"],
                        **grammar_python,
                    },
                    "ruby": {
                        "implementation_sha256": source_hashes["vnext_manifest_encode_rb.rb"],
                        **grammar_ruby,
                    },
                },
                "semantic_validator_sha256": source_hashes[
                    "vnext_catalog_profile_grammar_validate.py"
                ],
                "semantic_mutants": GRAMMAR_MUTANTS,
                "semantic_mutants_rejected": grammar_validation["mutants_rejected"],
                "action_symbol_count": 136,
                "ceremony_symbol_count": 11,
                "owner_profile_count": 20,
                "owner_symbol_count": 147,
                "route_count": grammar_build["route_entry_count"],
            },
            "catalog_suite": {
                "index_sha256": sha256(index_path),
                "builder_sha256": source_hashes["vnext_catalog_suite_build.py"],
                "semantic_validator_sha256": source_hashes["vnext_catalog_suite_validate.py"],
                "aggregate_counts": index["aggregate_counts"],
                "catalogs": catalogs,
                "semantic_mutants": suite_validation["mutants"],
                "semantic_mutants_rejected": suite_validation["mutants_rejected"],
            },
        }


def verify_capture_receipt(actual: dict[str, object]) -> None:
    receipt = json.loads(CAPTURE_RECEIPT.read_text(encoding="ascii"))
    sources = actual["source_files"]
    expected_rows = [
        {
            "materialized_path": f"tools/vnext_contracts/catalogs/predecessor_e346/{name}",
            "name": name,
            "sha256": digest,
            "source_path": f".maestro/workbench/{name}",
        }
        for name, digest in sorted(sources.items())
    ]
    if receipt["source_count"] != len(expected_rows) or receipt["sources"] != expected_rows:
        raise ValueError("captured predecessor reproduction sources differ from materialized sources")
    if receipt["reproduction_receipt"] != {
        "path": "contracts/vnext/catalogs/predecessor/e346/reproduction.json",
        "sha256": sha256(EXPECTED_RECEIPT),
    }:
        raise ValueError("captured predecessor reproduction receipt binding drifted")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--emit", action="store_true")
    args = parser.parse_args()
    actual = reproduce()
    if args.emit:
        print(json.dumps(actual, indent=2, sort_keys=True))
        return 0
    expected = json.loads(EXPECTED_RECEIPT.read_text(encoding="ascii"))
    if actual != expected:
        raise ValueError("immutable predecessor reproduction differs from the checked receipt")
    verify_capture_receipt(actual)
    print(
        json.dumps(
            {
                "catalogs": len(actual["catalog_suite"]["catalogs"]),
                "grammar_id": actual["grammar"]["grammar_id"],
                "grammar_mutants_rejected": actual["grammar"]["semantic_mutants_rejected"],
                "suite_mutants_rejected": actual["catalog_suite"]["semantic_mutants_rejected"],
                "status": "verified",
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
