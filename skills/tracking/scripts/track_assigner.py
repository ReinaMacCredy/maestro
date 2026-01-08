#!/usr/bin/env python3
"""
Track Assigner - Generate track assignments from beads dependency graph.

Usage:
    uv run skills/tracking/scripts/track_assigner.py <beads.json>
    uv run skills/tracking/scripts/track_assigner.py <beads.json> --max-workers 4
    uv run skills/tracking/scripts/track_assigner.py <beads.json> --json
"""
import argparse
import json
import sys
from pathlib import Path
from typing import Any


def merge_smallest_two_tracks(tracks: list[list[str]]) -> None:
    """Sort by length, merge two smallest tracks."""
    tracks.sort(key=len)
    smallest = tracks.pop(0)
    tracks[0] = tracks[0] + smallest


def generate_track_assignments(beads: list[dict[str, Any]], max_workers: int = 3) -> list[dict[str, Any]]:
    """Generate track assignments from beads dependency graph."""
    tasks = [b for b in beads if b.get("type") != "epic"]
    
    ready = [b for b in tasks if b.get("ready", False)]
    blocked = [b for b in tasks if not b.get("ready", False)]
    
    tracks: list[list[str]] = [[b["id"]] for b in ready]
    track_map: dict[str, int] = {b["id"]: i for i, b in enumerate(ready)}
    
    for b in blocked:
        blocked_by = b.get("blocked_by", [])
        if blocked_by:
            primary_blocker = blocked_by[0]
            if primary_blocker in track_map:
                track_idx = track_map[primary_blocker]
                tracks[track_idx].append(b["id"])
                track_map[b["id"]] = track_idx
            else:
                tracks.append([b["id"]])
                track_map[b["id"]] = len(tracks) - 1
        else:
            tracks.append([b["id"]])
            track_map[b["id"]] = len(tracks) - 1
    
    while len(tracks) > max_workers:
        merge_smallest_two_tracks(tracks)
    
    blocker_lookup = {b["id"]: b.get("blocked_by", []) for b in beads}
    
    result = []
    for i, track_beads in enumerate(tracks, 1):
        track_bead_set = set(track_beads)
        depends_on = []
        for bead_id in track_beads:
            for dep in blocker_lookup.get(bead_id, []):
                if dep not in track_bead_set and dep not in depends_on:
                    depends_on.append(dep)
        
        result.append({
            "track": i,
            "beads": track_beads,
            "depends_on": depends_on
        })
    
    return result


def print_table(tracks: list[dict[str, Any]]) -> None:
    """Print track assignments as a human-readable table."""
    print(f"{'Track':<8} {'Beads':<40} {'Depends On':<30}")
    print("-" * 78)
    for t in tracks:
        beads_str = ", ".join(t["beads"])
        deps_str = ", ".join(t["depends_on"]) if t["depends_on"] else "-"
        print(f"{t['track']:<8} {beads_str:<40} {deps_str:<30}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate track assignments from beads dependency graph."
    )
    parser.add_argument("beads_file", type=Path, help="Path to beads JSON file")
    parser.add_argument("--max-workers", type=int, default=3, help="Maximum number of tracks (default: 3)")
    parser.add_argument("--json", dest="json_output", action="store_true", help="Output as JSON")
    
    args = parser.parse_args()
    
    if not args.beads_file.exists():
        print(f"Error: File not found: {args.beads_file}", file=sys.stderr)
        return 1
    
    try:
        beads = json.loads(args.beads_file.read_text())
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON: {e}", file=sys.stderr)
        return 1
    
    tracks = generate_track_assignments(beads, args.max_workers)
    total_beads = sum(len(t["beads"]) for t in tracks)
    
    if args.json_output:
        output = {
            "tracks": tracks,
            "summary": f"{len(tracks)} tracks, {total_beads} beads"
        }
        print(json.dumps(output, indent=2))
    else:
        print_table(tracks)
        print(f"\nSummary: {len(tracks)} tracks, {total_beads} beads")
    
    return 0


if __name__ == "__main__":
    sys.exit(main())
