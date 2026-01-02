#!/usr/bin/env python3
# /// script
# dependencies = ["pyyaml"]
# ///
"""
Artifact Index - Build searchable index of session handoffs.

Usage:
    uv run skills/conductor/scripts/artifact_index.py           # Build/rebuild index
    uv run skills/conductor/scripts/artifact_index.py --verify  # Verify index integrity
    uv run skills/conductor/scripts/artifact_index.py --json    # Output as JSON
"""

import argparse
import json
import re
import sqlite3
import sys
from datetime import datetime
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


def init_db(db_path: Path) -> sqlite3.Connection:
    """Initialize SQLite database with FTS5."""
    conn = sqlite3.connect(db_path)
    conn.execute("""
        CREATE TABLE IF NOT EXISTS handoffs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            filename TEXT UNIQUE NOT NULL,
            date TEXT,
            trigger TEXT,
            session_id TEXT,
            status TEXT,
            summary TEXT,
            content TEXT NOT NULL,
            indexed_at TEXT NOT NULL
        )
    """)
    conn.execute("""
        CREATE VIRTUAL TABLE IF NOT EXISTS handoffs_fts USING fts5(
            filename,
            summary,
            content,
            content='handoffs',
            content_rowid='id'
        )
    """)
    conn.execute("""
        CREATE TRIGGER IF NOT EXISTS handoffs_ai AFTER INSERT ON handoffs BEGIN
            INSERT INTO handoffs_fts(rowid, filename, summary, content)
            VALUES (new.id, new.filename, new.summary, new.content);
        END
    """)
    conn.execute("""
        CREATE TRIGGER IF NOT EXISTS handoffs_ad AFTER DELETE ON handoffs BEGIN
            INSERT INTO handoffs_fts(handoffs_fts, rowid, filename, summary, content)
            VALUES ('delete', old.id, old.filename, old.summary, old.content);
        END
    """)
    conn.execute("""
        CREATE TRIGGER IF NOT EXISTS handoffs_au AFTER UPDATE ON handoffs BEGIN
            INSERT INTO handoffs_fts(handoffs_fts, rowid, filename, summary, content)
            VALUES ('delete', old.id, old.filename, old.summary, old.content);
            INSERT INTO handoffs_fts(rowid, filename, summary, content)
            VALUES (new.id, new.filename, new.summary, new.content);
        END
    """)
    conn.commit()
    return conn


def extract_summary(content: str) -> Optional[str]:
    """Extract summary section from handoff."""
    match = re.search(r"## Summary\s*\n\s*(.+?)(?:\n##|\Z)", content, re.DOTALL)
    if match:
        return match.group(1).strip()[:500]
    return None


def index_handoffs(conn: sqlite3.Connection, archive_dir: Path) -> tuple[int, int]:
    """Index all handoffs from archive directory. Returns (indexed, skipped)."""
    indexed = 0
    skipped = 0
    
    if not archive_dir.exists():
        return indexed, skipped
    
    existing = {row[0] for row in conn.execute("SELECT filename FROM handoffs")}
    
    for handoff_path in sorted(archive_dir.glob("*.md")):
        filename = handoff_path.name
        if filename in existing:
            skipped += 1
            continue
        
        content = handoff_path.read_text(encoding="utf-8")
        frontmatter = parse_frontmatter(content)
        summary = extract_summary(content)
        
        conn.execute("""
            INSERT INTO handoffs (filename, date, trigger, session_id, status, summary, content, indexed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            filename,
            frontmatter.get("date"),
            frontmatter.get("trigger"),
            frontmatter.get("session_id"),
            frontmatter.get("status"),
            summary,
            content,
            datetime.now().isoformat()
        ))
        indexed += 1
    
    conn.commit()
    return indexed, skipped


def verify_index(conn: sqlite3.Connection, archive_dir: Path) -> tuple[int, int, int]:
    """Verify index integrity. Returns (db_count, file_count, orphaned)."""
    db_count = conn.execute("SELECT COUNT(*) FROM handoffs").fetchone()[0]
    
    file_count = 0
    if archive_dir.exists():
        file_count = len(list(archive_dir.glob("*.md")))
    
    orphaned = 0
    for (filename,) in conn.execute("SELECT filename FROM handoffs"):
        if not (archive_dir / filename).exists():
            orphaned += 1
    
    return db_count, file_count, orphaned


def main():
    parser = argparse.ArgumentParser(description="Build artifact index")
    parser.add_argument("--verify", action="store_true", help="Verify index integrity")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    args = parser.parse_args()
    
    conductor_root = find_conductor_root()
    if not conductor_root:
        if args.json:
            print(json.dumps({"error": "No conductor/ directory found"}))
        else:
            print("Error: No conductor/ directory found", file=sys.stderr)
        sys.exit(1)
    
    db_path = get_db_path(conductor_root, ensure_cache=True)
    archive_dir = conductor_root / "sessions" / "archive"
    
    conn = init_db(db_path)
    
    if args.verify:
        db_count, file_count, orphaned = verify_index(conn, archive_dir)
        status = "ok" if (db_count == file_count and orphaned == 0) else "needs_rebuild"
        
        if args.json:
            print(json.dumps({
                "db_count": db_count,
                "file_count": file_count,
                "orphaned": orphaned,
                "status": status
            }))
        else:
            print(f"Database: {db_count} handoffs indexed")
            print(f"Archive:  {file_count} files")
            if orphaned:
                print(f"Warning:  {orphaned} orphaned entries (files deleted)")
            if status == "ok":
                print("Status:   OK")
            else:
                print("Status:   Needs rebuild (run without --verify to fix)")
    else:
        indexed, skipped = index_handoffs(conn, archive_dir)
        
        if args.json:
            print(json.dumps({
                "indexed": indexed,
                "skipped": skipped,
                "db_path": str(db_path)
            }))
        else:
            print(f"Indexed: {indexed} new handoffs")
            print(f"Skipped: {skipped} already indexed")
            print(f"Database: {db_path}")
    
    conn.close()


if __name__ == "__main__":
    main()
