#!/usr/bin/env python3
"""
Plan Parser

Parses .claude/plans/{name}.md files to extract TODOs and track progress.
"""

import re
import sys
import json
from pathlib import Path
from dataclasses import dataclass
from typing import List, Optional


@dataclass
class Task:
    """A parsed TODO task."""
    id: str
    text: str
    completed: bool
    line_number: int
    
    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "text": self.text,
            "completed": self.completed,
            "line": self.line_number
        }


def parse_plan(plan_path: str) -> List[Task]:
    """Parse a plan file and extract all TODO tasks."""
    path = Path(plan_path)
    if not path.exists():
        raise FileNotFoundError(f"Plan not found: {plan_path}")
    
    tasks = []
    content = path.read_text()
    
    # Match checkbox items: - [ ] or - [x]
    checkbox_pattern = re.compile(r'^(\s*)-\s*\[([ xX])\]\s*(\d+\.?\s*)?(.*?)$', re.MULTILINE)
    
    for i, match in enumerate(checkbox_pattern.finditer(content)):
        indent = match.group(1)
        checked = match.group(2).lower() == 'x'
        task_num = match.group(3) or ""
        text = match.group(4).strip()
        
        # Calculate line number
        line_num = content[:match.start()].count('\n') + 1
        
        # Generate task ID from number or index
        task_id = task_num.strip().rstrip('.') if task_num else str(i + 1)
        
        tasks.append(Task(
            id=task_id,
            text=text,
            completed=checked,
            line_number=line_num
        ))
    
    return tasks


def get_progress(tasks: List[Task]) -> dict:
    """Calculate progress statistics."""
    total = len(tasks)
    completed = sum(1 for t in tasks if t.completed)
    remaining = total - completed
    
    return {
        "total": total,
        "completed": completed,
        "remaining": remaining,
        "percent": round(completed / total * 100, 1) if total > 0 else 0
    }


def find_next_task(tasks: List[Task]) -> Optional[Task]:
    """Find the next incomplete task."""
    for task in tasks:
        if not task.completed:
            return task
    return None


def list_incomplete(tasks: List[Task]) -> List[Task]:
    """List all incomplete tasks."""
    return [t for t in tasks if not t.completed]


def main():
    """CLI interface."""
    if len(sys.argv) < 3:
        print("Usage: plan_parser.py <command> <plan_path>")
        print("Commands: parse, progress, next, incomplete")
        sys.exit(1)
    
    cmd = sys.argv[1]
    plan_path = sys.argv[2]
    
    try:
        tasks = parse_plan(plan_path)
        
        if cmd == "parse":
            result = [t.to_dict() for t in tasks]
            print(json.dumps(result, indent=2))
        
        elif cmd == "progress":
            result = get_progress(tasks)
            print(json.dumps(result, indent=2))
        
        elif cmd == "next":
            task = find_next_task(tasks)
            if task:
                print(json.dumps(task.to_dict(), indent=2))
            else:
                print(json.dumps({"status": "complete", "message": "All tasks done!"}))
        
        elif cmd == "incomplete":
            result = [t.to_dict() for t in list_incomplete(tasks)]
            print(json.dumps(result, indent=2))
        
        else:
            print(f"Unknown command: {cmd}")
            sys.exit(1)
            
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)


if __name__ == "__main__":
    main()
