#!/usr/bin/env python3
"""Render the exact Stage 12 canonical Rust namespace promotion manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from collections import Counter
from pathlib import Path, PurePosixPath


BASE_COMMIT = "f2334ce54f6eb6d09b9cf683147e487c92a0d5c5"
BASE_TREE = "295abb967e017fe005e69272d28fc2176cc6f93e"
SOURCE_ROOTS = (
    PurePosixPath("src/domain/vnext"),
    PurePosixPath("src/interfaces/vnext"),
    PurePosixPath("src/operations/vnext"),
)
EXCLUDED_FOLDED_SOURCE = PurePosixPath(
    "src/domain/vnext/migration/runtime/cohort_observation.rs"
)
FOLD_DESTINATION = PurePosixPath("src/domain/migration/runtime/consumer.rs")
EXPECTED_COUNTS = {
    "src/domain/vnext": 186,
    "src/interfaces/vnext": 8,
    "src/operations/vnext": 16,
}
EXPECTED_COLLISIONS = {
    "src/domain/memory/mod.rs",
    "src/domain/mod.rs",
    "src/domain/search/mod.rs",
    "src/interfaces/cli/mod.rs",
    "src/interfaces/hooks/mod.rs",
    "src/interfaces/mcp/mod.rs",
    "src/interfaces/mod.rs",
    "src/interfaces/shell/mod.rs",
    "src/interfaces/tui/mod.rs",
    "src/operations/mod.rs",
}
SCHEMA = "maestro.stage12.namespace-promotion-manifest.v1"


class NamespacePromotionError(RuntimeError):
    """The exact promotion surface differs from the authorized Stage 12 set."""


def _git(repo: Path, *arguments: str) -> bytes:
    process = subprocess.run(
        ["git", *arguments],
        cwd=repo,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if process.returncode != 0:
        raise NamespacePromotionError(
            f"git {' '.join(arguments)} failed: "
            f"{process.stderr.decode('utf-8', errors='replace').strip()}"
        )
    return process.stdout


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _destination(source: PurePosixPath) -> PurePosixPath:
    parts = source.parts
    index = parts.index("vnext")
    return PurePosixPath(*parts[:index], *parts[index + 1 :])


def _base_bytes(repo: Path, path: PurePosixPath) -> bytes:
    return _git(repo, "show", f"{BASE_COMMIT}:{path.as_posix()}")


def _base_sources(repo: Path) -> list[PurePosixPath]:
    tree = _git(repo, "rev-parse", f"{BASE_COMMIT}^{{tree}}").decode().strip()
    if tree != BASE_TREE:
        raise NamespacePromotionError(
            f"base tree differs: expected {BASE_TREE}, observed {tree}"
        )
    rows = _git(
        repo,
        "ls-tree",
        "-r",
        "--name-only",
        BASE_COMMIT,
        *(root.as_posix() for root in SOURCE_ROOTS),
    ).decode("utf-8").splitlines()
    sources = [PurePosixPath(row) for row in rows if row]
    if EXCLUDED_FOLDED_SOURCE not in sources:
        raise NamespacePromotionError("Stage 11 cohort test source is absent from base")
    sources.remove(EXCLUDED_FOLDED_SOURCE)
    return sources


def _root_for(source: PurePosixPath) -> str:
    for root in SOURCE_ROOTS:
        if source.is_relative_to(root):
            return root.as_posix()
    raise NamespacePromotionError(f"source is outside the promotion roots: {source}")


def build_manifest(repo: Path) -> dict[str, object]:
    sources = _base_sources(repo)
    counts = Counter(_root_for(source) for source in sources)
    if dict(counts) != EXPECTED_COUNTS or len(sources) != 210:
        raise NamespacePromotionError(
            f"promotion surface differs: counts={dict(counts)}, total={len(sources)}"
        )

    destinations = [_destination(source) for source in sources]
    if len(set(destinations)) != 210:
        raise NamespacePromotionError("promotion destinations are not one-to-one")
    base_paths = set(
        _git(repo, "ls-tree", "-r", "--name-only", BASE_COMMIT)
        .decode("utf-8")
        .splitlines()
    )
    collisions = {
        destination.as_posix()
        for destination in destinations
        if destination.as_posix() in base_paths
    }
    if collisions != EXPECTED_COLLISIONS:
        raise NamespacePromotionError(
            "collision closure differs: " + ", ".join(sorted(collisions))
        )

    entries: list[dict[str, object]] = []
    for source, destination in zip(sources, destinations, strict=True):
        absolute_destination = repo / destination
        if absolute_destination.is_symlink() or not absolute_destination.is_file():
            raise NamespacePromotionError(
                f"unsafe or missing canonical destination: {destination}"
            )
        if (repo / source).exists():
            raise NamespacePromotionError(
                f"temporary source remains after promotion: {source}"
            )
        destination_path = destination.as_posix()
        collision = destination_path in EXPECTED_COLLISIONS
        entries.append(
            {
                "base_source_sha256": _sha256(_base_bytes(repo, source)),
                "collision": collision,
                "destination": destination_path,
                "destination_preimage_sha256": (
                    _sha256(_base_bytes(repo, destination)) if collision else None
                ),
                "destination_sha256": _sha256(absolute_destination.read_bytes()),
                "disposition": (
                    "merged_canonical_facade"
                    if collision
                    else (
                        "canonicalized_move_with_stage11_test_fold"
                        if destination == FOLD_DESTINATION
                        else "canonicalized_move"
                    )
                ),
                "source": source.as_posix(),
            }
        )

    encoded_entries = json.dumps(
        entries, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    destination_digest_rows = [
        f"{entry['destination']}\0{entry['destination_sha256']}" for entry in entries
    ]
    folded_source_bytes = _base_bytes(repo, EXCLUDED_FOLDED_SOURCE)
    folded_destination = (repo / FOLD_DESTINATION).read_bytes()
    if b"mod cohort_observation_tests" not in folded_destination:
        raise NamespacePromotionError("Stage 11 cohort tests were not folded")
    temporary_roots = [
        root.as_posix() for root in SOURCE_ROOTS if (repo / root).exists()
    ]
    if temporary_roots:
        raise NamespacePromotionError(
            "temporary namespace roots remain: " + ", ".join(temporary_roots)
        )

    return {
        "authority_scope": "canonical_rust_namespace_only",
        "base_commit": BASE_COMMIT,
        "base_tree": BASE_TREE,
        "closed_world": True,
        "collision_count": len(collisions),
        "collision_destinations": sorted(collisions),
        "destination_set_sha256": _sha256(
            "\n".join(destination_digest_rows).encode("utf-8")
        ),
        "entries": entries,
        "entry_count": len(entries),
        "fold": {
            "destination": FOLD_DESTINATION.as_posix(),
            "source": EXCLUDED_FOLDED_SOURCE.as_posix(),
            "source_sha256": _sha256(folded_source_bytes),
            "state": "folded_into_cfg_test_module",
        },
        "legacy_pruning_authorized": False,
        "manifest_sha256": _sha256(encoded_entries),
        "namespace_counts": dict(sorted(counts.items())),
        "postconditions": {
            "canonical_destination_count": 210,
            "temporary_namespace_count": 0,
            "temporary_namespace_roots": [],
        },
        "schema_version": SCHEMA,
        "state": "canonical_namespace_promoted_legacy_pruning_blocked",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    arguments = parser.parse_args()
    try:
        manifest = build_manifest(arguments.repo.resolve())
    except (NamespacePromotionError, OSError) as error:
        print(json.dumps({"error": str(error), "status": "error"}, sort_keys=True))
        return 1
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
