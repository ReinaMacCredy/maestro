#!/usr/bin/env python3
# /// script
# dependencies = ["pyyaml"]
# ///
"""
Artifact Cleanup - Remove old handoffs and sync index.

Usage:
    uv run skills/conductor/scripts/artifact_cleanup.py               # Cleanup older than 30 days
    uv run skills/conductor/scripts/artifact_cleanup.py --max-age 7   # Cleanup older than 7 days
    uv run skills/conductor/scripts/artifact_cleanup.py --dry-run     # Show what would be deleted
    uv run skills/conductor/scripts/artifact_cleanup.py --json        # Output JSON
"""

import argparse
import json
import re
import sqlite3
import sys
from datetime import datetime, timedelta
from pathlib import Path
from typing import Optional

import yaml


def find_conductor_root() -> Optional[Path]:
    """Find conductor/ directory by walking up from cwd."""
    current = Path.cwd()
    while current != current.parent:
        conductor = current / "conductor"
        if conductor.is_dir():
            return conductor
        current = current.parent
    return None


def get_db_path(conductor_root: Path, ensure_cache: bool = False) -> Path:
    cache_dir = conductor_root / ".cache"
    if ensure_cache:
        cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir / "artifact-index.db"


def parse_frontmatter(content: str) -> dict:
    try:
        match = re.match(r"^---\s*\n(.*?)\n---\s*\n", content, re.DOTALL)
        if match:
            return yaml.safe_load(match.group(1)) or {}
    except (yaml.YAMLError, ValueError):
        pass
    return {}


def parse_handoff_date(file_path: Path) -> Optional[datetime]:
    """Parse date from handoff file frontmatter, falling back to filename.
    
    Prefers the 'date' field in YAML frontmatter for robustness.
    Falls back to filename parsing (YYYY-MM-DD-HH-MM-trigger.md) if frontmatter missing.
    """
    try:
        content = file_path.read_text(encoding="utf-8", errors="ignore")
        frontmatter = parse_frontmatter(content)
        if "date" in frontmatter:
            date_str = str(frontmatter["date"])
            return datetime.fromisoformat(date_str.replace("Z", "+00:00")).replace(tzinfo=None)
    except (OSError, ValueError):
        pass
    
    try:
        parts = file_path.name.replace(".md", "").split("-")
        if len(parts) >= 5:
            return datetime(
                int(parts[0]), int(parts[1]), int(parts[2]),
                int(parts[3]), int(parts[4])
            )
    except (ValueError, IndexError):
        pass
    return None


def cleanup(
    archive_dir: Path,
    db_path: Path,
    max_age_days: int,
    dry_run: bool = False
) -> tuple[list[str], list[str]]:
    """
    Cleanup old handoffs. Returns (deleted, kept).
    
    Also removes orphaned index entries.
    """
    cutoff = datetime.now() - timedelta(days=max_age_days)
    deleted = []
    kept = []
    
    if not archive_dir.exists():
        return deleted, kept
    
    for handoff_path in sorted(archive_dir.glob("*.md")):
        filename = handoff_path.name
        file_date = parse_handoff_date(handoff_path)
        
        if file_date and file_date < cutoff:
            if not dry_run:
                handoff_path.unlink()
            deleted.append(filename)
        else:
            kept.append(filename)
    
    if not dry_run and db_path.exists() and deleted:
        conn = sqlite3.connect(db_path)
        for filename in deleted:
            conn.execute("DELETE FROM handoffs WHERE filename = ?", (filename,))
        conn.commit()
        conn.close()
    
    return deleted, kept


def sync_index(archive_dir: Path, db_path: Path, dry_run: bool = False) -> list[str]:
    """Remove orphaned index entries. Returns list of removed entries."""
    orphaned = []
    
    if not db_path.exists():
        return orphaned
    
    conn = sqlite3.connect(db_path)
    
    rows = conn.execute("SELECT filename FROM handoffs").fetchall()
    for (filename,) in rows:
        if not (archive_dir / filename).exists():
            orphaned.append(filename)
            if not dry_run:
                conn.execute("DELETE FROM handoffs WHERE filename = ?", (filename,))
    
    if not dry_run:
        conn.commit()
    conn.close()
    
    return orphaned


def main():
    parser = argparse.ArgumentParser(description="Cleanup old handoffs")
    parser.add_argument("--max-age", type=int, default=30, help="Max age in days (default: 30)")
    parser.add_argument("--dry-run", action="store_true", help="Show what would be deleted")
    parser.add_argument("--json", action="store_true", help="Output JSON")
    args = parser.parse_args()
    
    conductor_root = find_conductor_root()
    if not conductor_root:
        if args.json:
            print(json.dumps({"error": "No conductor/ directory found"}))
        else:
            print("Error: No conductor/ directory found", file=sys.stderr)
        sys.exit(1)
    
    archive_dir = conductor_root / "sessions" / "archive"
    db_path = get_db_path(conductor_root)
    
    deleted, kept = cleanup(archive_dir, db_path, args.max_age, args.dry_run)
    orphaned = sync_index(archive_dir, db_path, args.dry_run)
    
    if args.json:
        print(json.dumps({
            "deleted": deleted,
            "kept": kept,
            "orphaned": orphaned,
            "max_age_days": args.max_age,
            "dry_run": args.dry_run
        }))
        return
    
    if args.dry_run:
        print("DRY RUN - No files will be deleted\n")
    
    print(f"Cutoff: {args.max_age} days ago")
    print(f"Archive: {archive_dir}")
    print()
    
    if deleted:
        print(f"{'Would delete' if args.dry_run else 'Deleted'}: {len(deleted)} handoffs")
        for f in deleted[:10]:
            print(f"  - {f}")
        if len(deleted) > 10:
            print(f"  ... and {len(deleted) - 10} more")
    else:
        print("No handoffs older than cutoff")
    
    print(f"Kept: {len(kept)} handoffs")
    
    if orphaned:
        print(f"\n{'Would remove' if args.dry_run else 'Removed'}: {len(orphaned)} orphaned index entries")
        for f in orphaned:
            print(f"  - {f}")


if __name__ == "__main__":
    main()
