#!/usr/bin/env python3
"""Preflight session detection and conflict analysis for orchestrator."""
import argparse, json, re, sys
from datetime import datetime

PATTERNS = {
    "session_start": re.compile(r"\[SESSION START\]"),
    "heartbeat": re.compile(r"\[HEARTBEAT\]"),
    "session_end": re.compile(r"\[SESSION END\]"),
}

def detect_sessions(inbox_json: list) -> list:
    """Parse inbox messages to detect active sessions."""
    sessions = {}
    for msg in inbox_json:
        subject, body = msg.get("subject", ""), msg.get("body_md", "") or ""
        sender, created = msg.get("from", ""), msg.get("created_ts", "")
        data = _parse_body(body)
        sid = data.get("session_id") or f"{sender}-unknown"
        
        if PATTERNS["session_start"].search(subject):
            sessions[sid] = {"session_id": sid, "agent": sender, "status": "active",
                "track": data.get("track"), "beads_claimed": data.get("beads_claimed", []),
                "files_reserved": data.get("files_reserved", []),
                "last_heartbeat": created, "started_at": created}
        elif PATTERNS["heartbeat"].search(subject):
            if sid in sessions:
                sessions[sid]["last_heartbeat"] = created
            else:
                sessions[sid] = {"session_id": sid, "agent": sender, "status": "active",
                    "track": data.get("track"), "beads_claimed": data.get("beads_claimed", []),
                    "files_reserved": data.get("files_reserved", []),
                    "last_heartbeat": created, "started_at": None}
        elif PATTERNS["session_end"].search(subject) and sid in sessions:
            sessions[sid]["status"] = "ended"
    return [s for s in sessions.values() if s["status"] == "active"]

def _parse_body(body: str) -> dict:
    result = {}
    for line in body.split("\n"):
        if ":" not in line: continue
        k, v = line.split(":", 1)
        k = k.strip().lower().replace(" ", "_")
        if k == "session_id": result["session_id"] = v.strip()
        elif k == "track": result["track"] = v.strip()
        elif k == "beads_claimed": result["beads_claimed"] = [b.strip() for b in v.split(",") if b.strip()]
        elif k == "files_reserved": result["files_reserved"] = [f.strip() for f in v.split(",") if f.strip()]
    return result

def check_conflicts(my_session: dict, active_sessions: list) -> dict:
    """Check for track/file/bead conflicts."""
    conflicts = {"has_conflicts": False, "track_conflicts": [], "file_conflicts": [], "bead_conflicts": []}
    my_track, my_beads, my_files = my_session.get("track"), set(my_session.get("beads_claimed", [])), set(my_session.get("files_reserved", []))
    
    for s in active_sessions:
        sid = s.get("session_id", "unknown")
        if my_track and s.get("track") == my_track:
            conflicts["track_conflicts"].append({"session_id": sid, "track": my_track})
            conflicts["has_conflicts"] = True
        for bead in my_beads & set(s.get("beads_claimed", [])):
            conflicts["bead_conflicts"].append({"session_id": sid, "bead_id": bead})
            conflicts["has_conflicts"] = True
        for mp in my_files:
            for tp in s.get("files_reserved", []):
                p1, p2 = mp.split("*")[0].rstrip("/"), tp.split("*")[0].rstrip("/")
                if p1.startswith(p2) or p2.startswith(p1) or mp == tp:
                    conflicts["file_conflicts"].append({"session_id": sid, "pattern": tp, "overlap": mp})
                    conflicts["has_conflicts"] = True
    return conflicts

def format_active_sessions(sessions: list) -> str:
    if not sessions: return "┌─ No Active Sessions ─┐\n│ (none detected)      │\n└──────────────────────┘"
    lines = ["┌─ Active Sessions ──────────────────────────┐"]
    for s in sessions:
        hb = s.get("last_heartbeat", "")
        try:
            dt = datetime.fromisoformat(hb.replace("Z", "+00:00"))
            age = int((datetime.now(dt.tzinfo) - dt).total_seconds() / 60)
            hb_str = f"{age}m ago" if age < 60 else f"{age // 60}h ago"
        except: hb_str = "?"
        line = f"│ {s.get('agent', '?')}: {s.get('track', '-')} ({len(s.get('beads_claimed', []))} beads) - {hb_str}"
        lines.append(line.ljust(46) + "│")
    lines.append("└" + "─" * 46 + "┘")
    return "\n".join(lines)

def format_conflicts(conflicts: dict) -> str:
    if not conflicts.get("has_conflicts"): return "✅ No conflicts detected"
    lines = ["⚠️  CONFLICTS DETECTED", ""]
    for c in conflicts.get("track_conflicts", []):
        lines.append(f"Track: {c['track']} active in {c['session_id']}")
    for c in conflicts.get("bead_conflicts", []):
        lines.append(f"Bead: {c['bead_id']} claimed by {c['session_id']}")
    for c in conflicts.get("file_conflicts", []):
        lines.append(f"File: {c['pattern']} reserved by {c['session_id']}")
    return "\n".join(lines)

def main():
    p = argparse.ArgumentParser(description="Preflight session detection")
    sp = p.add_subparsers(dest="cmd", required=True)
    sp.add_parser("detect").add_argument("inbox_json")
    sp.add_parser("format-sessions").add_argument("sessions_json")
    sp.add_parser("format-conflicts").add_argument("conflicts_json")
    args = p.parse_args()
    try:
        if args.cmd == "detect":
            r = {"active_sessions": detect_sessions(json.loads(args.inbox_json))}
            r["count"] = len(r["active_sessions"])
        elif args.cmd == "format-sessions":
            r = {"formatted": format_active_sessions(json.loads(args.sessions_json))}
        else:
            r = {"formatted": format_conflicts(json.loads(args.conflicts_json))}
        print(json.dumps(r, indent=2))
    except Exception as e:
        print(json.dumps({"error": str(e)}), file=sys.stderr); return 1
    return 0

if __name__ == "__main__": sys.exit(main())
