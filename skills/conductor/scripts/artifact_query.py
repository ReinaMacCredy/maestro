#!/usr/bin/env python3
# /// script
# dependencies = ["pyyaml"]
# ///
"""
Artifact Query - Search archived handoffs using FTS5.

Usage:
    uv run skills/conductor/scripts/artifact_query.py <query>           # Search handoffs
    uv run skills/conductor/scripts/artifact_query.py <query> --limit 5 # Limit results
    uv run skills/conductor/scripts/artifact_query.py <query> --json    # JSON output
"""

import argparse
import json
import sqlite3
import sys
from pathlib import Path
from typing import Optional


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


def search(conn: sqlite3.Connection, query: str, limit: int = 10) -> list[dict]:
    """Search handoffs using FTS5. Returns list of results with snippets."""
    results = []
    
    cursor = conn.execute("""
        SELECT 
            h.filename,
            h.date,
            h.trigger,
            h.status,
            h.summary,
            snippet(handoffs_fts, 2, '>>>', '<<<', '...', 50) as snippet
        FROM handoffs_fts
        JOIN handoffs h ON handoffs_fts.rowid = h.id
        WHERE handoffs_fts MATCH ?
        ORDER BY rank
        LIMIT ?
    """, (query, limit))
    
    for row in cursor:
        results.append({
            "filename": row[0],
            "date": row[1],
            "trigger": row[2],
            "status": row[3],
            "summary": row[4],
            "snippet": row[5],
        })
    
    return results


def format_result(result: dict, index: int) -> str:
    """Format a single search result for display."""
    lines = [
        f"{index}. {result['filename']}",
        f"   Date: {result['date'] or 'unknown'} | Trigger: {result['trigger'] or 'unknown'}",
    ]
    if result['summary']:
        summary = result['summary'][:100] + "..." if len(result['summary']) > 100 else result['summary']
        lines.append(f"   Summary: {summary}")
    if result['snippet']:
        snippet = result['snippet'].replace("\n", " ")[:150]
        lines.append(f"   Match: {snippet}")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="Search archived handoffs")
    parser.add_argument("query", nargs="+", help="Search query (FTS5 syntax)")
    parser.add_argument("--limit", type=int, default=10, help="Max results (default: 10)")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    args = parser.parse_args()
    
    conductor_root = find_conductor_root()
    if not conductor_root:
        if args.json:
            print(json.dumps({"error": "No conductor/ directory found"}))
        else:
            print("Error: No conductor/ directory found", file=sys.stderr)
        sys.exit(1)
    
    db_path = get_db_path(conductor_root)
    if not db_path.exists():
        if args.json:
            print(json.dumps({"error": "No index found. Run artifact-index.py first."}))
        else:
            print("Error: No index found. Run artifact-index.py first.", file=sys.stderr)
        sys.exit(1)
    
    conn = sqlite3.connect(db_path)
    query = " ".join(args.query)
    
    try:
        results = search(conn, query, args.limit)
    except sqlite3.OperationalError as e:
        if "no such table" in str(e):
            if args.json:
                print(json.dumps({"error": "Index not initialized. Run artifact-index.py first."}))
            else:
                print("Error: Index not initialized. Run artifact-index.py first.", file=sys.stderr)
            sys.exit(1)
        raise
    
    conn.close()
    
    if args.json:
        print(json.dumps(results))
        sys.exit(0)
    
    if not results:
        print(f"No results for: {query}")
        sys.exit(0)
    
    print(f"Found {len(results)} result(s) for: {query}\n")
    for i, result in enumerate(results, 1):
        print(format_result(result, i))
        print()


if __name__ == "__main__":
    main()
