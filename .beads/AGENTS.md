# .beads

## Purpose
AI-native, git-integrated issue tracking system for persistent task management across sessions.

## Key Files

| File | Purpose |
|------|---------|
| issues.jsonl | Source of truth - human-readable, git-mergeable issue store |
| beads.db | SQLite cache for query performance |
| config.yaml | Beads configuration (sync branch, daemon settings) |

## Issue Structure

Each issue tracks:
- id: Unique identifier
- title: Issue title
- status: Current state (open, in_progress, closed)
- priority: Priority level
- dependencies: Blocking/parent-child relationships
- labels: Categorization tags

## Patterns

- JSONL Format: One JSON object per line for easy git merging
- SQLite Cache: beads.db is regenerated from issues.jsonl
- Sync Branch: beads-sync branch for issue state synchronization
- Daemon Mode: Auto-starts for RPC communication with agents

## CLI Commands

```bash
bd ready --json     # Find available work
bd show <id>        # Read task context
bd update <id> --status in_progress  # Claim task
bd close <id> --reason completed     # Close task
bd sync             # Sync to git
```

## Dependencies

- External: bd CLI (beads command-line tool)
- Internal: Integrated with conductor workflow

## Notes for AI Agents

- Always commit .beads/ with code changes to keep tracking in sync
- Use --json flag with bd for structured output
- Issues serve as "memory" for complex tasks (Epics) and atomic work (Tasks)
- The daemon must be running for RPC operations
- Never manually edit issues.jsonl - use bd CLI
