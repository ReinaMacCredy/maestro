#!/usr/bin/env python3
"""Render the exact, read-only Stage 12 namespace promotion manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath


SOURCE_ROOTS = (
    PurePosixPath("src/domain/vnext"),
    PurePosixPath("src/interfaces/vnext"),
    PurePosixPath("src/operations/vnext"),
)
SCHEMA = "maestro.stage12.namespace-promotion-manifest.v1"


class NamespacePromotionError(RuntimeError):
    """The candidate namespace cannot produce a safe exact manifest."""


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _destination(source: PurePosixPath) -> PurePosixPath:
    parts = source.parts
    try:
        index = parts.index("vnext")
    except ValueError as error:
        raise NamespacePromotionError(f"source is outside vnext: {source}") from error
    return PurePosixPath(*parts[:index], *parts[index + 1 :])


def build_manifest(repo: Path) -> dict[str, object]:
    entries: list[dict[str, object]] = []
    seen_destinations: set[PurePosixPath] = set()
    for root in SOURCE_ROOTS:
        absolute_root = repo / root
        if not absolute_root.is_dir() or absolute_root.is_symlink():
            raise NamespacePromotionError(f"unsafe or missing source root: {root}")
        for absolute_source in sorted(absolute_root.rglob("*")):
            if absolute_source.is_symlink():
                raise NamespacePromotionError(
                    f"symbolic links are forbidden in promotion source: {absolute_source}"
                )
            if not absolute_source.is_file():
                continue
            source = PurePosixPath(absolute_source.relative_to(repo).as_posix())
            destination = _destination(source)
            if destination in seen_destinations:
                raise NamespacePromotionError(
                    f"duplicate promotion destination: {destination}"
                )
            seen_destinations.add(destination)
            absolute_destination = repo / destination
            destination_exists = absolute_destination.is_file()
            entries.append(
                {
                    "destination": destination.as_posix(),
                    "destination_exists": destination_exists,
                    "destination_sha256": (
                        _sha256(absolute_destination) if destination_exists else None
                    ),
                    "requires_merge_or_removal_authority": destination_exists,
                    "source": source.as_posix(),
                    "source_sha256": _sha256(absolute_source),
                }
            )
    if not entries:
        raise NamespacePromotionError("namespace promotion source is empty")
    encoded_entries = json.dumps(
        entries, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode()
    return {
        "closed_world": False,
        "entries": entries,
        "entry_count": len(entries),
        "manifest_sha256": hashlib.sha256(encoded_entries).hexdigest(),
        "mutation_authorized": False,
        "postconditions": {
            "canonical_owner_facade_mismatch_count": 0,
            "content_identity_mismatch_count": 0,
            "temporary_namespace_count": 0,
        },
        "schema_version": SCHEMA,
        "state": "deferred_until_combined_stage6_through_stage11_closure",
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
