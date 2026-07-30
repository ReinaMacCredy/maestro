#!/usr/bin/env python3
"""Emit a deterministic, read-only Stage 12 consumer census."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from collections import Counter
from pathlib import Path, PurePosixPath
from typing import Any, Iterable, Mapping, Sequence, cast


TOOLS = Path(__file__).resolve().parent
WORKSPACE = TOOLS.parents[2]
DEFAULT_POLICY = (
    WORKSPACE
    / "tests/fixtures/vnext/stage12/consumer-census-policy.v1.json"
)
POLICY_SCHEMA = "maestro.test-only.vnext-stage12-consumer-census-policy.v1"
CENSUS_SCHEMA = "maestro.test-only.vnext-stage12-consumer-census.v1"
FANOUT_COMMIT = "7080fb6cd1e286998ff47fb6205e90dca990ba40"
FANOUT_TREE = "926f6f0f6a169716a8815105adc8609ac289c717"
DESIGN_SHA256 = "5092ff84ac3bca050802ea81858375d328e1d0ffe678a71ef2f8dae65ed00a18"
FANOUT_MANIFEST_SCHEMA = "maestro.external.vnext-successor-fanout.v4"
FANOUT_MANIFEST_SHA256 = (
    "e299556c31c6a788285d984f9cd3040cfde200ba24e7ed5a5d90caff96ee5954"
)
FANOUT_MANIFEST_STATE = "design_locked_not_dispatch_ready"
MATERIALIZATION_DECISIONS = {
    "dec-canonical-authority-materialization-df3b": (
        "0d7c406f68f04fdf47ce00d56e8189b54159f164323c9511504790b941f715d0"
    ),
    "dec-canonical-execution-h3-verified-0939": (
        "b5935c389182a7f3ec6447fb2a13dcb70e912108b399d0b1d25fee5f132186a7"
    ),
    "dec-canonical-final-cumulative-stage-0-1652": (
        "214bb83b8d0d13315250b7330ec4f44d520efcfb0b2d0011fa5cd268f4d48114"
    ),
    "dec-canonical-foundation-descriptor-a128": (
        "17fb79ef9bc74cf3838d869bf5fb3b0ae0e9ae017670ca7cb207aeb8105c234e"
    ),
    "dec-canonical-foundation-owned-admitted-d215": (
        "f3e19535a81d5b6eb11836d4b90bbd01c339cee8f9e964bf33e702d90f55d20f"
    ),
    "dec-canonical-installation-consumer-c1fe": (
        "aaba56a8f34fb293a68f26743fbf4ef879d9f5a399a4eb45da74eed70a509e53"
    ),
    "dec-canonical-pre-candidate-protected-370d": (
        "3f2d88bd1659f1f6622d405e6a63158a230bd766ff091990c69bd56bdccfd6fc"
    ),
}
EXPECTED_RULE_IDS = (
    "temporary_vnext_source_path",
    "temporary_domain_namespace_reference",
    "temporary_domain_module_export",
    "legacy_skill_surface",
    "legacy_next_surface",
    "legacy_harness_resource",
)
sys.dont_write_bytecode = True


class CensusError(RuntimeError):
    """The provisional census could not be produced without ambiguity."""


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def _reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
    value: dict[str, object] = {}
    for key, item in pairs:
        if key in value:
            raise CensusError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_bytes(), object_pairs_hook=_reject_duplicates)
    except (OSError, json.JSONDecodeError) as error:
        raise CensusError(f"cannot load JSON input {path}: {error}") from error
    if not isinstance(value, dict):
        raise CensusError(f"JSON input must be one object: {path}")
    return cast(dict[str, Any], value)


def string_array(value: object, field: str) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise CensusError(f"{field} must be a string array")
    return cast(list[str], value)


def normalized_relative(value: str) -> str:
    if not value or value.startswith("/") or "\\" in value:
        raise CensusError(f"unsafe repository-relative path: {value!r}")
    parts = PurePosixPath(value).parts
    if not parts or any(part in {"", ".", ".."} for part in parts):
        raise CensusError(f"non-normalized repository-relative path: {value!r}")
    normalized = str(PurePosixPath(*parts))
    if normalized != value.rstrip("/"):
        raise CensusError(f"noncanonical repository-relative path: {value!r}")
    return value


def validate_policy(policy: Mapping[str, Any]) -> list[dict[str, Any]]:
    if policy.get("schema_version") != POLICY_SCHEMA:
        raise CensusError("consumer census policy schema differs")
    if policy.get("authority_state") != "noncanonical_read_only_test_input":
        raise CensusError("consumer census policy authority state differs")
    if (
        policy.get("fanout_commit") != FANOUT_COMMIT
        or policy.get("fanout_tree") != FANOUT_TREE
        or policy.get("design_sha256") != DESIGN_SHA256
        or policy.get("fanout_manifest_schema") != FANOUT_MANIFEST_SCHEMA
        or policy.get("fanout_manifest_sha256") != FANOUT_MANIFEST_SHA256
        or policy.get("fanout_manifest_state") != FANOUT_MANIFEST_STATE
        or policy.get("fanout_manifest_preservation_only") is not True
        or policy.get("materialization_decisions") != MATERIALIZATION_DECISIONS
    ):
        raise CensusError("consumer census policy input binding differs")
    if policy.get("coverage") != {
        "closed_world": False,
        "kind": "provisional_stage12_seed",
        "requires_ordered_stage6_through_stage11_integration": True,
        "requires_stage11_consumer_closure": True,
        "semantics": "literal_and_path_evidence_only",
    }:
        raise CensusError("consumer census policy overstates its coverage")
    rules = policy.get("rules")
    if not isinstance(rules, list) or not all(isinstance(rule, dict) for rule in rules):
        raise CensusError("consumer census policy rules must be an object array")
    typed_rules = cast(list[dict[str, Any]], rules)
    if tuple(rule.get("id") for rule in typed_rules) != EXPECTED_RULE_IDS:
        raise CensusError("consumer census policy rule closure differs")
    ids: set[str] = set()
    for rule in typed_rules:
        rule_id = rule.get("id")
        if not isinstance(rule_id, str) or not rule_id or rule_id in ids:
            raise CensusError("consumer census rule ids must be unique nonempty strings")
        ids.add(rule_id)
        if rule.get("matcher") not in {
            "literal",
            "literal_or_path_contains",
            "path_prefix",
        }:
            raise CensusError(f"consumer census rule {rule_id} has an unknown matcher")
        if rule.get("release_requirement") not in {
            "zero",
            "zero_or_stage11_sealed_reader",
        }:
            raise CensusError(
                f"consumer census rule {rule_id} has an unknown release requirement"
            )
        roots = string_array(rule.get("roots"), f"{rule_id}.roots")
        suffixes = string_array(rule.get("suffixes"), f"{rule_id}.suffixes")
        values = string_array(rule.get("values"), f"{rule_id}.values")
        if not roots or not suffixes or not values:
            raise CensusError(f"consumer census rule {rule_id} cannot be empty")
        if len(roots) != len(set(roots)) or len(values) != len(set(values)):
            raise CensusError(f"consumer census rule {rule_id} contains duplicates")
        for root in roots:
            normalized_relative(root)
        for value in values:
            if rule["matcher"] == "path_prefix":
                normalized_relative(value)
    return typed_rules


def _files_under(repo: Path, roots: Sequence[str], suffixes: Sequence[str]) -> tuple[list[Path], list[dict[str, str]]]:
    files: set[Path] = set()
    warnings: list[dict[str, str]] = []
    for relative_root in roots:
        root = repo / relative_root
        if root.is_symlink():
            warnings.append({"kind": "symlink_root", "path": relative_root})
            continue
        if not root.exists():
            warnings.append({"kind": "missing_root", "path": relative_root})
            continue
        if root.is_file():
            if root.name.endswith(tuple(suffixes)):
                files.add(root)
            continue
        for directory, directory_names, file_names in os.walk(root, followlinks=False):
            directory_path = Path(directory)
            retained_directories: list[str] = []
            for name in sorted(directory_names):
                child = directory_path / name
                relative = child.relative_to(repo).as_posix()
                if child.is_symlink():
                    warnings.append({"kind": "symlink_directory", "path": relative})
                else:
                    retained_directories.append(name)
            directory_names[:] = retained_directories
            for name in sorted(file_names):
                path = directory_path / name
                relative = path.relative_to(repo).as_posix()
                if path.is_symlink():
                    warnings.append({"kind": "symlink_file", "path": relative})
                elif name.endswith(tuple(suffixes)):
                    files.add(path)
    return sorted(files), sorted(warnings, key=lambda row: (row["kind"], row["path"]))


def _classification(rule: Mapping[str, Any], relative: str) -> str:
    if (
        rule["release_requirement"] == "zero_or_stage11_sealed_reader"
        and "/migration/" in f"/{relative}"
    ):
        return "sealed_reader_candidate_unverified"
    return "active_or_unclassified"


def _row(
    *,
    rule: Mapping[str, Any],
    relative: str,
    evidence_kind: str,
    value: str,
    line: int,
    column: int,
    file_sha256: str,
) -> dict[str, object]:
    evidence = f"{rule['id']}\0{relative}\0{evidence_kind}\0{value}\0{line}\0{column}"
    return {
        "classification": _classification(rule, relative),
        "column": column,
        "evidence_kind": evidence_kind,
        "evidence_sha256": hashlib.sha256(evidence.encode("utf-8")).hexdigest(),
        "file_sha256": file_sha256,
        "line": line,
        "matched_value": value,
        "path": relative,
        "release_requirement": rule["release_requirement"],
        "rule_id": rule["id"],
    }


def _literal_rows(
    rule: Mapping[str, Any],
    relative: str,
    data: bytes,
    file_sha256: str,
) -> Iterable[dict[str, object]]:
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CensusError(f"consumer input is not UTF-8: {relative}: {error}") from error
    values = cast(list[str], rule["values"])
    for line_number, line in enumerate(text.splitlines(), start=1):
        for value in values:
            start = 0
            while True:
                offset = line.find(value, start)
                if offset < 0:
                    break
                yield _row(
                    rule=rule,
                    relative=relative,
                    evidence_kind="literal",
                    value=value,
                    line=line_number,
                    column=offset + 1,
                    file_sha256=file_sha256,
                )
                start = offset + len(value)


def build_census(repo: Path, policy: Mapping[str, Any]) -> dict[str, object]:
    repo = repo.resolve()
    if not repo.is_dir():
        raise CensusError(f"repository root is not a directory: {repo}")
    rules = validate_policy(policy)
    rows: list[dict[str, object]] = []
    warnings: list[dict[str, str]] = []
    scanned_files: set[str] = set()
    for rule in rules:
        roots = string_array(rule["roots"], f"{rule['id']}.roots")
        suffixes = string_array(rule["suffixes"], f"{rule['id']}.suffixes")
        files, file_warnings = _files_under(repo, roots, suffixes)
        warnings.extend(file_warnings)
        for path in files:
            relative = path.relative_to(repo).as_posix()
            scanned_files.add(relative)
            data = path.read_bytes()
            file_sha256 = hashlib.sha256(data).hexdigest()
            matcher = rule["matcher"]
            values = string_array(rule["values"], f"{rule['id']}.values")
            if matcher in {"path_prefix", "literal_or_path_contains"}:
                for value in values:
                    matched = relative.startswith(value) if matcher == "path_prefix" else value in relative
                    if matched:
                        rows.append(
                            _row(
                                rule=rule,
                                relative=relative,
                                evidence_kind="path",
                                value=value,
                                line=0,
                                column=0,
                                file_sha256=file_sha256,
                            )
                        )
            if matcher in {"literal", "literal_or_path_contains"}:
                rows.extend(_literal_rows(rule, relative, data, file_sha256))
    rows.sort(
        key=lambda row: (
            str(row["rule_id"]),
            str(row["path"]),
            int(row["line"]),
            int(row["column"]),
            str(row["matched_value"]),
            str(row["evidence_kind"]),
        )
    )
    warnings = sorted(
        {json.dumps(row, sort_keys=True): row for row in warnings}.values(),
        key=lambda row: (row["kind"], row["path"]),
    )
    rule_counts = Counter({str(rule["id"]): 0 for rule in rules})
    rule_counts.update(str(row["rule_id"]) for row in rows)
    classification_counts = Counter(str(row["classification"]) for row in rows)
    census_core = {
        "rows": rows,
        "scan_warnings": warnings,
    }
    return {
        "authority_state": "none",
        "classification_counts": dict(sorted(classification_counts.items())),
        "closed_world": False,
          "design_sha256": policy["design_sha256"],
          "fanout_commit": policy["fanout_commit"],
          "fanout_manifest_sha256": policy["fanout_manifest_sha256"],
          "fanout_tree": policy["fanout_tree"],
        "release_claim": False,
        "row_count": len(rows),
        "rows": rows,
        "rule_counts": dict(sorted(rule_counts.items())),
        "scan_sha256": hashlib.sha256(canonical_json(census_core)).hexdigest(),
        "scan_warnings": warnings,
        "scanned_file_count": len(scanned_files),
        "schema_version": CENSUS_SCHEMA,
    }


def summary(census: Mapping[str, Any]) -> dict[str, object]:
    return {
        "classification_counts": census["classification_counts"],
        "closed_world": census["closed_world"],
        "release_claim": census["release_claim"],
        "row_count": census["row_count"],
        "rule_counts": census["rule_counts"],
        "scan_sha256": census["scan_sha256"],
        "scan_warnings": census["scan_warnings"],
        "scanned_file_count": census["scanned_file_count"],
        "schema_version": census["schema_version"],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=WORKSPACE)
    parser.add_argument("--policy", type=Path, default=DEFAULT_POLICY)
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()
    try:
        census = build_census(args.repo, load_json(args.policy))
    except CensusError as error:
        print(json.dumps({"status": "error", "error": str(error)}, sort_keys=True))
        return 1
    payload: Mapping[str, Any] = summary(census) if args.summary else census
    print(json.dumps(payload, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
