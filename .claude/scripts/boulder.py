#!/usr/bin/env python3
"""
Boulder State Management

Manages .sisyphus/boulder.json which tracks active execution state.
"""

import json
import os
import sys
from datetime import datetime
from pathlib import Path
from typing import Optional

SISYPHUS_DIR = Path(".sisyphus")
BOULDER_FILE = SISYPHUS_DIR / "boulder.json"

DEFAULT_STATE = {
    "active": None,
    "plan": None,
    "started": None,
    "progress": {},
    "wisdom": {
        "conventions": [],
        "successes": [],
        "failures": [],
        "gotchas": []
    }
}


def load_boulder() -> dict:
    """Load boulder state, creating default if missing."""
    if not BOULDER_FILE.exists():
        return DEFAULT_STATE.copy()
    with open(BOULDER_FILE, "r") as f:
        return json.load(f)


def save_boulder(state: dict) -> None:
    """Save boulder state atomically."""
    SISYPHUS_DIR.mkdir(parents=True, exist_ok=True)
    tmp_file = BOULDER_FILE.with_suffix(".json.tmp")
    with open(tmp_file, "w") as f:
        json.dump(state, f, indent=2)
    tmp_file.rename(BOULDER_FILE)


def start_work(plan_name: str) -> dict:
    """Start work on a plan."""
    state = load_boulder()
    if state["active"]:
        raise RuntimeError(f"Already working on: {state['plan']}")
    
    state["active"] = True
    state["plan"] = plan_name
    state["started"] = datetime.now().isoformat()
    state["progress"] = {}
    save_boulder(state)
    return state


def stop_work() -> dict:
    """Stop current work session."""
    state = load_boulder()
    state["active"] = None
    state["plan"] = None
    state["started"] = None
    save_boulder(state)
    return state


def mark_task(task_id: str, status: str) -> dict:
    """Mark a task with status (in_progress, completed, skipped, blocked)."""
    state = load_boulder()
    if not state["active"]:
        raise RuntimeError("No active work session")
    
    state["progress"][task_id] = {
        "status": status,
        "updated": datetime.now().isoformat()
    }
    save_boulder(state)
    return state


def add_wisdom(category: str, learning: str) -> dict:
    """Add a learning to the wisdom accumulator."""
    state = load_boulder()
    if category not in state["wisdom"]:
        state["wisdom"][category] = []
    state["wisdom"][category].append(learning)
    save_boulder(state)
    return state


def get_status() -> dict:
    """Get current boulder status."""
    return load_boulder()


def main():
    """CLI interface."""
    if len(sys.argv) < 2:
        print("Usage: boulder.py <command> [args]")
        print("Commands: start <plan>, stop, task <id> <status>, wisdom <category> <text>, status")
        sys.exit(1)
    
    cmd = sys.argv[1]
    
    if cmd == "start" and len(sys.argv) > 2:
        result = start_work(sys.argv[2])
        print(f"Started work on: {sys.argv[2]}")
    elif cmd == "stop":
        result = stop_work()
        print("Stopped work session")
    elif cmd == "task" and len(sys.argv) > 3:
        result = mark_task(sys.argv[2], sys.argv[3])
        print(f"Marked {sys.argv[2]}: {sys.argv[3]}")
    elif cmd == "wisdom" and len(sys.argv) > 3:
        result = add_wisdom(sys.argv[2], " ".join(sys.argv[3:]))
        print(f"Added wisdom to {sys.argv[2]}")
    elif cmd == "status":
        result = get_status()
        print(json.dumps(result, indent=2))
    else:
        print(f"Unknown command: {cmd}")
        sys.exit(1)


if __name__ == "__main__":
    main()
