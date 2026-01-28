#!/usr/bin/env python3
"""
Notepad Wisdom Manager

Manages .sisyphus/notepads/{plan}/ directories for wisdom accumulation.
"""

import sys
import json
from datetime import datetime
from pathlib import Path

SISYPHUS_DIR = Path(".sisyphus")
NOTEPADS_DIR = SISYPHUS_DIR / "notepads"

NOTEPAD_FILES = ["learnings.md", "issues.md", "decisions.md", "problems.md"]


def ensure_notepad(plan: str) -> Path:
    """Create notepad directory for a plan if it doesn't exist."""
    notepad_dir = NOTEPADS_DIR / plan
    notepad_dir.mkdir(parents=True, exist_ok=True)
    
    for filename in NOTEPAD_FILES:
        filepath = notepad_dir / filename
        if not filepath.exists():
            title = filename.replace(".md", "").title()
            filepath.write_text(f"# {title}\n\nWisdom from plan: {plan}\n\n")
    
    return notepad_dir


def append_wisdom(plan: str, category: str, task: str, content: str) -> None:
    """Append a wisdom entry to a notepad file."""
    notepad_dir = ensure_notepad(plan)
    
    filename = f"{category}.md"
    if filename not in NOTEPAD_FILES:
        raise ValueError(f"Invalid category: {category}")
    
    filepath = notepad_dir / filename
    
    entry = f"""
## {datetime.now().strftime("%Y-%m-%d %H:%M")} - {task}

{content}

---
"""
    
    with open(filepath, "a") as f:
        f.write(entry)


def read_wisdom(plan: str) -> dict:
    """Read all wisdom for a plan."""
    notepad_dir = NOTEPADS_DIR / plan
    if not notepad_dir.exists():
        return {}
    
    wisdom = {}
    for filename in NOTEPAD_FILES:
        filepath = notepad_dir / filename
        if filepath.exists():
            category = filename.replace(".md", "")
            wisdom[category] = filepath.read_text()
    
    return wisdom


def main():
    if len(sys.argv) < 3:
        print("Usage: notepad.py <command> <plan> [args]")
        print("Commands: init, append <category> <task> <content>, read")
        sys.exit(1)
    
    cmd = sys.argv[1]
    plan = sys.argv[2]
    
    if cmd == "init":
        notepad_dir = ensure_notepad(plan)
        print(f"Created notepad: {notepad_dir}")
    
    elif cmd == "append" and len(sys.argv) >= 6:
        category = sys.argv[3]
        task = sys.argv[4]
        content = " ".join(sys.argv[5:])
        append_wisdom(plan, category, task, content)
        print(f"Added to {category}")
    
    elif cmd == "read":
        wisdom = read_wisdom(plan)
        print(json.dumps(wisdom, indent=2))
    
    else:
        print(f"Unknown command: {cmd}")
        sys.exit(1)


if __name__ == "__main__":
    main()
