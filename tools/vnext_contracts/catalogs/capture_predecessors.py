#!/usr/bin/env python3
"""Freeze the Decision-referenced predecessor evidence without activating it."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
from pathlib import Path


PATH_PATTERN = re.compile(r"\.maestro/workbench/[A-Za-z0-9._/-]+")
DECISION_ID_PATTERN = re.compile(r"^  id: ([A-Za-z0-9-]+)$", re.MULTILINE)
GRAMMAR_ARTIFACT = (
    ".maestro/workbench/"
    "vnext-catalog-profile-grammar-v1-sha256-"
    "2b428f8444253794cd0abb41b32da482cc0805359c2a37bf0cba90a70e3186e9.json"
)
NOMINAL_SOURCE = ".maestro/workbench/vnext-catalog-nominal-source-v1.json"
REPOSITORY_CONTINUITY = (
    ".maestro/workbench/"
    "vnext-repository-continuity-v1-sha256-"
    "e2afe5e3ab9792ae02b663ad4370c0a43acfecc25c902bfae1500e79b4ba4c35.json"
)
INSTALLATION_CONTINUITY = (
    ".maestro/workbench/"
    "vnext-installation-continuity-v1-sha256-"
    "92408154eb60f022f5562e8f547075cac6762d6d707e4e9471bfd3af51604e5e.json"
)
REPRODUCTION_SOURCE_HASHES = {
    "vnext_catalog_profile_grammar_build.py": "b987b9ce376ee2bcb8bd8d8016e07b98fb14d64cc7c6cc0d691a7e36d4caa7a5",
    "vnext_catalog_profile_grammar_validate.py": "907a5bd1c2f1026ca3b69496c578d1a67cf7e6f659f428f8b57858146cc6fdc9",
    "vnext_catalog_suite_build.py": "c1e48c88de19dca9b93ddce225ec4746be935b2d852b8cfbb7dfafc766870d63",
    "vnext_catalog_suite_validate.py": "821cfc6e055167f61945a2e11c1f2018bf12efe55fe2ad4e3dbfc01d46b3f043",
    "vnext_manifest_encode_py.py": "76c36afa7c730fef8ec8402250c7aab779e2da7e6c373990c58774e58a8fa0c1",
    "vnext_manifest_encode_rb.rb": "1861a6fc37bcbd35b1bbe64717cecd58fb0f115999c8458b928b89771844ca73",
}
REPRODUCTION_RECEIPT = "contracts/vnext/catalogs/predecessor/e346/reproduction.json"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def decision_blocks(text: str) -> list[str]:
    starts = [match.start() for match in re.finditer(r"^- schema_version:", text, re.MULTILINE)]
    return [text[start : starts[index + 1] if index + 1 < len(starts) else len(text)] for index, start in enumerate(starts)]


def continuity_rows(path: Path, owner_names: dict[int, str]) -> list[dict[str, object]]:
    artifact = json.loads(path.read_text(encoding="ascii"))
    relation = artifact["primary_owner_relation"]["rows"]
    descriptors = artifact["descriptors"]
    if len(descriptors) != len(relation):
        raise ValueError(f"continuity descriptor/relation mismatch in {path}")
    rows = []
    for descriptor, owner_row in zip(descriptors, relation, strict=True):
        tag = descriptor["value"][0]
        if tag != owner_row[0]:
            raise ValueError(f"continuity tag mismatch in {path}: {tag}")
        rows.append({"tag": tag, "name": descriptor["name"], "owner": owner_names[owner_row[1]]})
    return rows


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-repo", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    source_repo = args.source_repo.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    decisions_path = source_repo / ".maestro/cards/maestro-whole-flow-architecture-refoundation/decisions.yaml"
    decisions_text = decisions_path.read_text(encoding="utf-8")

    references: dict[str, set[str]] = {}
    blocks_by_id: dict[str, str] = {}
    for block in decision_blocks(decisions_text):
        decision_id_match = DECISION_ID_PATTERN.search(block)
        if decision_id_match is None or "locked_at:" not in block:
            continue
        decision_id = decision_id_match.group(1)
        blocks_by_id[decision_id] = block
        for relative_path in PATH_PATTERN.findall(block):
            references.setdefault(relative_path, set()).add(decision_id)

    if len(references) != 32:
        raise ValueError(f"expected exactly 32 Decision-referenced predecessor paths, got {len(references)}")

    evidence_rows = []
    for relative_path in sorted(references):
        absolute_path = source_repo / relative_path
        if not absolute_path.is_file():
            raise FileNotFoundError(absolute_path)
        actual_sha256 = sha256(absolute_path)
        decision_ids = sorted(references[relative_path])
        mentioned = [decision_id for decision_id in decision_ids if actual_sha256 in blocks_by_id[decision_id]]
        if not mentioned:
            raise ValueError(f"no referencing Decision body names the actual hash for {relative_path}")
        evidence_rows.append(
            {
                "path": relative_path,
                "sha256": actual_sha256,
                "byte_length": absolute_path.stat().st_size,
                "referenced_by_decisions": decision_ids,
                "hash_named_by_decisions": mentioned,
                "disposition": "immutable_non_current_predecessor_evidence",
            }
        )

    reproduction_sources = []
    materialized_root = Path(__file__).with_name("predecessor_e346")
    for name, expected_sha256 in sorted(REPRODUCTION_SOURCE_HASHES.items()):
        source = source_repo / ".maestro/workbench" / name
        materialized = materialized_root / name
        if sha256(source) != expected_sha256:
            raise ValueError(f"the locked predecessor reproduction source changed: {source}")
        if sha256(materialized) != expected_sha256:
            raise ValueError(f"the materialized predecessor reproduction source changed: {materialized}")
        reproduction_sources.append(
            {
                "name": name,
                "sha256": expected_sha256,
                "source_path": f".maestro/workbench/{name}",
                "materialized_path": f"tools/vnext_contracts/catalogs/predecessor_e346/{name}",
            }
        )

    reproduction_receipt = Path(__file__).resolve().parents[3] / REPRODUCTION_RECEIPT
    if not reproduction_receipt.is_file():
        raise FileNotFoundError(reproduction_receipt)

    predecessor_receipt = {
        "schema_version": "maestro.vnext.catalog.predecessor-evidence.v1",
        "status": "immutable_non_current_evidence",
        "source_decisions_sha256": sha256(decisions_path),
        "artifact_count": len(evidence_rows),
        "missing_count": 0,
        "artifacts": evidence_rows,
    }
    (output / "predecessors.json").write_text(
        json.dumps(predecessor_receipt, indent=2, sort_keys=True) + "\n", encoding="ascii"
    )
    reproduction_source_receipt = {
        "schema_version": "maestro.vnext.catalog.predecessor-reproduction-sources.v1",
        "status": "immutable_non_current_predecessor_evidence",
        "source_count": len(reproduction_sources),
        "sources": reproduction_sources,
        "reproduction_receipt": {
            "path": REPRODUCTION_RECEIPT,
            "sha256": sha256(reproduction_receipt),
        },
    }
    (output / "e346-reproduction-sources.json").write_text(
        json.dumps(reproduction_source_receipt, indent=2, sort_keys=True) + "\n",
        encoding="ascii",
    )

    grammar = json.loads((source_repo / GRAMMAR_ARTIFACT).read_text(encoding="ascii"))
    owner_names = {row["tag"]: row["name"] for row in grammar["owner_profiles"]}
    baseline = {
        "schema_version": "maestro.vnext.catalog.predecessor-semantic-baseline.v1",
        "status": "immutable_non_current_evidence",
        "predecessor_grammar_id": grammar["catalog_profile_grammar"]["catalog_profile_grammar_id"],
        "predecessor_action_symbol_count": len(grammar["action_leaf_symbols"]),
        "predecessor_ceremony_symbol_count": len(grammar["ceremony_symbols"]),
        "predecessor_route_count": sum(row["route_count"] for row in grammar["effect_origin_routes"]),
        "repository_continuity": continuity_rows(source_repo / REPOSITORY_CONTINUITY, owner_names),
        "installation_continuity": continuity_rows(source_repo / INSTALLATION_CONTINUITY, owner_names),
    }
    (output / "e346-semantic-baseline.json").write_text(
        json.dumps(baseline, indent=2, sort_keys=True) + "\n", encoding="ascii"
    )

    nominal_source = source_repo / NOMINAL_SOURCE
    if sha256(nominal_source) != "3142ff4334ddeb9b77c49786d29ff75de2ef6f023bb7942c827e1c54a84b69c2":
        raise ValueError("the e346 nominal predecessor source hash changed")
    shutil.copyfile(nominal_source, output / "e346-nominal-source.json")

    print(
        json.dumps(
            {
                "artifact_count": 32,
                "missing_count": 0,
                "output": str(output),
                "reproduction_source_count": len(reproduction_sources),
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
