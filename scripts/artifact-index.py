#!/usr/bin/env python3
"""
Artifact Index - Build searchable index of session handoffs.

Usage:
    uv run scripts/artifact-index.py           # Build/rebuild index
    uv run scripts/artifact-index.py --verify  # Verify index integrity
"""

import argparse
import os
import re
import sqlite3
import sys
from datetime import datetime
from pathlib import Path


def find_conductor_root() -> Path | None:
    """Find conductor/ directory by walking up from cwd."""
    current = Path.cwd()
    while current != current.parent:
        conductor = current / "conductor"
        if conductor.is_dir():
            return conductor
        current = current.parent
    return None


def get_db_path(conductor_root: Path) -> Path:
    """Get path to SQLite database."""
    cache_dir = conductor_root / ".cache"
    cache_dir.mkdir(parents=True, exist_ok=True)
    return cache_dir / "artifact-index.db"


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


def parse_frontmatter(content: str) -> dict:
    """Parse YAML frontmatter from markdown content."""
    frontmatter = {}
    match = re.match(r"^---\s*\n(.*?)\n---\s*\n", content, re.DOTALL)
    if match:
        for line in match.group(1).split("\n"):
            if ":" in line:
                key, value = line.split(":", 1)
                frontmatter[key.strip()] = value.strip()
    return frontmatter


def extract_summary(content: str) -> str | None:
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
    args = parser.parse_args()
    
    conductor_root = find_conductor_root()
    if not conductor_root:
        print("Error: No conductor/ directory found", file=sys.stderr)
        sys.exit(1)
    
    db_path = get_db_path(conductor_root)
    archive_dir = conductor_root / "sessions" / "archive"
    
    conn = init_db(db_path)
    
    if args.verify:
        db_count, file_count, orphaned = verify_index(conn, archive_dir)
        print(f"Database: {db_count} handoffs indexed")
        print(f"Archive:  {file_count} files")
        if orphaned:
            print(f"Warning:  {orphaned} orphaned entries (files deleted)")
        if db_count == file_count and orphaned == 0:
            print("Status:   OK")
        else:
            print("Status:   Needs rebuild")
    else:
        indexed, skipped = index_handoffs(conn, archive_dir)
        print(f"Indexed: {indexed} new handoffs")
        print(f"Skipped: {skipped} already indexed")
        print(f"Database: {db_path}")
    
    conn.close()


if __name__ == "__main__":
    main()
