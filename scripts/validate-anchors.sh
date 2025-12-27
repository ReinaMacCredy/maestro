#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF' >&2
Validate Markdown anchor links (#section) point to existing headers.

Usage:
  ./scripts/validate-anchors.sh [--include-archive] [root_dir]

Notes:
  - Skips `conductor/archive/` by default (historical snapshots).
  - Ignores external links and GitHub line anchors (#L123, #L123C4).
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
HEADING_PATTERN = re.compile(r"^(#{1,6})\s+(.*)$")

EXCLUDE_DIRS = {
    ".git",
    "node_modules",
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

GITHUB_LINE_ANCHOR = re.compile(r"^L\d+(C\d+)?$", re.IGNORECASE)


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


def resolve_target_file(root: Path, source_file: Path, link: str) -> Path | None:
    if link.startswith(EXTERNAL_PREFIXES) or link.startswith("//"):
        return None

    path_part = link.split("#", 1)[0]
    if not path_part:
        return source_file

    if path_part.startswith("/"):
        return (root / path_part.lstrip("/")).resolve()

    return (source_file.parent / path_part).resolve()


def slugify_github(heading: str) -> str:
    text = heading.strip().lower()
    text = re.sub(r"[^\w\s-]", "", text)
    text = re.sub(r"\s+", "-", text)
    text = re.sub(r"-+", "-", text)
    return text.strip("-")


def build_anchor_set(text: str) -> set[str]:
    anchors: set[str] = set()
    seen: dict[str, int] = {}

    in_fence = False
    fence = ""
    for line in text.splitlines():
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

        m = HEADING_PATTERN.match(line)
        if not m:
            continue

        heading_text = m.group(2).strip()
        heading_text = re.sub(r"\s+#*$", "", heading_text).strip()
        base = slugify_github(heading_text)
        if not base:
            continue

        if base not in seen:
            seen[base] = 0
            anchors.add(base)
        else:
            seen[base] += 1
            anchors.add(f"{base}-{seen[base]}")

    return anchors


def main() -> int:
    include_archive = os.environ.get("INCLUDE_ARCHIVE", "0") == "1"
    root = Path(sys.argv[1]).resolve()

    md_files = iter_markdown_files(root, include_archive)

    anchor_cache: dict[Path, set[str]] = {}
    errors: list[str] = []

    for md_file in md_files:
        try:
            text = md_file.read_text(encoding="utf-8")
        except Exception as e:
            errors.append(f"{md_file.relative_to(root)}:0: failed to read file: {e}")
            continue

        anchor_cache[md_file.resolve()] = build_anchor_set(text)

    for md_file in md_files:
        try:
            text = md_file.read_text(encoding="utf-8")
        except Exception:
            continue

        lines = text.splitlines()
        for line_no, raw_target in iter_link_targets(lines):
            target = strip_link_target(raw_target)
            if "#" not in target:
                continue

            if target.startswith(EXTERNAL_PREFIXES) or target.startswith("//"):
                continue

            file_part, anchor_part = target.split("#", 1)
            anchor = anchor_part.strip()
            if not anchor:
                continue

            if GITHUB_LINE_ANCHOR.match(anchor):
                continue

            if anchor.startswith("user-content-"):
                anchor = anchor.removeprefix("user-content-")

            anchor = anchor.lower()

            target_file = resolve_target_file(root, md_file, target)
            if target_file is None:
                continue

            resolved_target = target_file.resolve()
            if not resolved_target.exists():
                errors.append(
                    f"{md_file.relative_to(root)}:{line_no}: anchor target file missing -> {raw_target.strip()} (resolved to {resolved_target})"
                )
                continue

            anchors = anchor_cache.get(resolved_target)
            if anchors is None:
                try:
                    anchors = build_anchor_set(resolved_target.read_text(encoding="utf-8"))
                except Exception:
                    anchors = set()
                anchor_cache[resolved_target] = anchors

            if anchor not in anchors:
                errors.append(
                    f"{md_file.relative_to(root)}:{line_no}: broken anchor -> {raw_target.strip()} (missing #{anchor} in {resolved_target})"
                )

    if errors:
        for err in errors:
            print(err)
        print(f"\nFound {len(errors)} broken anchor(s).")
        return 1

    print(f"OK: no broken anchors found across {len(md_files)} markdown file(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
PY

