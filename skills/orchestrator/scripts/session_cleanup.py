#!/usr/bin/env python3
"""Session cleanup for stale session detection and takeover prompts."""
import argparse, json, sys
from datetime import datetime, timezone

def find_stale_sessions(sessions: list, threshold_min: int = 10) -> list:
    """Find sessions that haven't heartbeated within threshold."""
    now, stale = datetime.now(timezone.utc), []
    for s in sessions:
        hb = s.get("last_heartbeat")
        if not hb:
            stale.append({**s, "stale_minutes": None, "stale_reason": "no_heartbeat"})
            continue
        try:
            dt = datetime.fromisoformat(hb.replace("Z", "+00:00"))
            age = int((now - dt).total_seconds() / 60)
            if age >= threshold_min:
                stale.append({**s, "stale_minutes": age, "stale_reason": "heartbeat_timeout"})
        except:
            stale.append({**s, "stale_minutes": None, "stale_reason": "invalid_timestamp"})
    return stale

def format_takeover_prompt(session: dict) -> str:
    """Format [T]ake/[W]ait/[I]gnore prompt for stale session."""
    sm = session.get("stale_minutes")
    status = f"⚠️  STALE: {sm}m without heartbeat" if sm else f"⚠️  STALE: {session.get('stale_reason', '?')}"
    beads = session.get("beads_claimed", [])
    beads_str = ", ".join(beads[:3]) + (f" (+{len(beads)-3})" if len(beads) > 3 else "") if beads else "-"
    lines = [
        "┌" + "─" * 58 + "┐",
        f"│ {status}".ljust(59) + "│",
        "├" + "─" * 58 + "┤",
        f"│ Agent: {session.get('agent', '?')}".ljust(59) + "│",
        f"│ Session: {session.get('session_id', '?')}".ljust(59) + "│",
        f"│ Track: {session.get('track', '-')}".ljust(59) + "│",
        f"│ Beads: {beads_str}".ljust(59) + "│",
        "├" + "─" * 58 + "┤",
        "│ [T] Take over  [W] Wait  [I] Ignore".ljust(59) + "│",
        "└" + "─" * 58 + "┘",
    ]
    return "\n".join(lines)

def format_stale_summary(stale_sessions: list) -> str:
    if not stale_sessions: return "✅ No stale sessions detected"
    lines = [f"⚠️  Found {len(stale_sessions)} stale session(s):", ""]
    for s in stale_sessions:
        sm = s.get("stale_minutes")
        lines.append(f"  • {s.get('agent', '?')} on {s.get('track', '-')} - {f'{sm}m stale' if sm else 'status unknown'}")
    return "\n".join(lines)

def main():
    p = argparse.ArgumentParser(description="Session cleanup utilities")
    sp = p.add_subparsers(dest="cmd", required=True)
    fs = sp.add_parser("find-stale")
    fs.add_argument("sessions_json")
    fs.add_argument("--threshold", type=int, default=10)
    sp.add_parser("format-prompt").add_argument("session_json")
    sp.add_parser("format-summary").add_argument("sessions_json")
    args = p.parse_args()
    try:
        if args.cmd == "find-stale":
            stale = find_stale_sessions(json.loads(args.sessions_json), args.threshold)
            r = {"stale_sessions": stale, "count": len(stale), "threshold_min": args.threshold}
        elif args.cmd == "format-prompt":
            r = {"prompt": format_takeover_prompt(json.loads(args.session_json))}
        else:
            r = {"summary": format_stale_summary(json.loads(args.sessions_json))}
        print(json.dumps(r, indent=2))
    except Exception as e:
        print(json.dumps({"error": str(e)}), file=sys.stderr); return 1
    return 0

if __name__ == "__main__": sys.exit(main())
