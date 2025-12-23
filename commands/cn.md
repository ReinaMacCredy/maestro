---
description: Create new track (spec + plan + beads + review)
argument-hint: [track_id or description]
---

# Conductor New Track (cn)

Alias for `/conductor-newtrack`. Create spec and plan for: $ARGUMENTS

Load the `conductor` skill and execute the `/conductor-newtrack` workflow.

**What this does:**
1. Verifies conductor setup exists (product.md, tech-stack.md, workflow.md)
2. Resolves or creates track ID from $ARGUMENTS
3. Uses existing design.md if present, otherwise asks clarifying questions
4. Generates spec.md (requirements, acceptance criteria)
5. Generates plan.md (phases, tasks, subtasks)
6. Auto-files beads issues via fb subagent
7. Auto-reviews beads via rb subagent

## Usage

```
cn "Add user authentication"     # New track from description
cn auth_20241223                  # Resume existing track
cn auth_20241223 --no-beads       # Skip beads filing
cn auth_20241223 --force          # Overwrite existing
```

## Flags

- `--no-beads` / `-nb`: Skip beads filing (spec + plan only)
- `--plan-only` / `-po`: Alias for --no-beads
- `--force`: Overwrite existing track or remove stale locks

## After Track Creation

- Run `bd ready` to see available work
- Run `bd update <id> --status in_progress` to claim a task
- Run `/conductor-implement <track_id>` to start implementation
