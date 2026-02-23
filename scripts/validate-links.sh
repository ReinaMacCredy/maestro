#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF' >&2
Validate Markdown file links resolve to existing files/directories.

Usage:
  ./scripts/validate-links.sh [--include-archive] [root_dir]

Notes:
  - Skips `conductor/archive/` by default (historical snapshots).
  - Ignores external links (http/https/mailto/etc) and pure anchors (#...).
EOF
}

include_archive=0
root="."

while [[ $# -gt 0 ]]; do
  case "$1" in
    --include-archive)
      include_archive=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      root="$1"
      shift
      ;;
  esac
done

export INCLUDE_ARCHIVE="$include_archive"

python3 - "$root" <<'PY'
from __future__ import annotations

import os
import re
import sys
from pathlib import Path
from urllib.parse import unquote


LINK_PATTERN = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
KEY_LINE_PATTERN = re.compile(r"^[A-Za-z0-9_]+:\s*")

EXCLUDE_DIRS = {
    ".git",
    ".claude",  # hard-link mirror of skills/ - links designed for skills/ context
    "node_modules",
    ".venv",
    ".bv",
    ".beads-village",
    ".reservations",
    ".mail",
    ".memory",
    ".memory-index",
    "tmp",
}

EXTERNAL_PREFIXES = (
    "http://",
    "https://",
    "mailto:",
    "tel:",
    "ftp://",
    "data:",
)


def iter_markdown_files(root: Path, include_archive: bool) -> list[Path]:
    files: list[Path] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dir_path = Path(dirpath)
        try:
            rel_parts = dir_path.relative_to(root).parts
        except ValueError:
            rel_parts = ()

        if any(part in EXCLUDE_DIRS for part in rel_parts):
            dirnames[:] = []
            continue

        if not include_archive and len(rel_parts) >= 2 and rel_parts[0] == "conductor" and rel_parts[1] == "archive":
            dirnames[:] = []
            continue

        pruned: list[str] = []
        for d in dirnames:
            if d in EXCLUDE_DIRS:
                continue
            if not include_archive and len(rel_parts) == 1 and rel_parts[0] == "conductor" and d == "archive":
                continue
            pruned.append(d)
        dirnames[:] = pruned

        for filename in filenames:
            if filename.lower().endswith(".md"):
                files.append(dir_path / filename)
    return files


def strip_link_target(raw: str) -> str:
    target = raw.strip()

    if target.startswith("<") and target.endswith(">"):
        target = target[1:-1].strip()

    # Strip optional title: (path "title")
    if not target.startswith("<") and " " in target:
        target = target.split()[0]

    return unquote(target.strip())


def iter_link_targets(lines: list[str]) -> list[tuple[int, str]]:
    results: list[tuple[int, str]] = []
    in_fence = False
    fence = ""

    for idx, line in enumerate(lines, start=1):
        stripped = line.lstrip()
        if stripped.startswith("```") or stripped.startswith("~~~"):
            marker = stripped[:3]
            if not in_fence:
                in_fence = True
                fence = marker
                continue
            if marker == fence:
                in_fence = False
                fence = ""
                continue

        if in_fence:
            continue

        for m in LINK_PATTERN.finditer(line):
            results.append((idx, m.group(1)))

    return results


def resolve_link_path(root: Path, source_file: Path, link_path: str) -> Path | None:
    if not link_path or link_path.startswith("#"):
        return None

    if link_path.startswith(EXTERNAL_PREFIXES) or link_path.startswith("//"):
        return None

    path_part = link_path.split("#", 1)[0]
    if not path_part:
        return None

    if path_part.startswith("/"):
        return (root / path_part.lstrip("/")).resolve()

    return (source_file.parent / path_part).resolve()


def main() -> int:
    include_archive = os.environ.get("INCLUDE_ARCHIVE", "0") == "1"
    root = Path(sys.argv[1]).resolve()

    md_files = iter_markdown_files(root, include_archive)
    errors: list[str] = []

    for md_file in md_files:
        try:
            text = md_file.read_text(encoding="utf-8")
        except Exception as e:
            errors.append(f"{md_file.relative_to(root)}:0: failed to read file: {e}")
            continue

        lines = text.splitlines()
        for line_no, raw_target in iter_link_targets(lines):
            target = strip_link_target(raw_target)
            resolved = resolve_link_path(root, md_file, target)
            if resolved is None:
                continue

            if not resolved.exists():
                errors.append(
                    f"{md_file.relative_to(root)}:{line_no}: broken link -> {raw_target.strip()} (resolved to {resolved})"
                )

    if errors:
        for err in errors:
            print(err)
        print(f"\nFound {len(errors)} broken link(s).")
        return 1

    print(f"OK: no broken file links found across {len(md_files)} markdown file(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
PY

