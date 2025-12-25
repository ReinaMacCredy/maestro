---
name: beads
version: "2.0.0"
description: "Beads issue tracking (bd, fb, rb). Use for multi-session work, file-beads, review-beads, filing beads from plan, reviewing beads."
---

# Beads

Issue tracking for multi-session work with dependency graphs.

## Entry Points

| Trigger | Workflow | Action |
|---------|----------|--------|
| `bd`, `beads` | `workflows/beads/workflow.md` | Core CLI operations |
| `fb`, `file-beads` | `workflows/beads/references/FILE_BEADS.md` | File beads from plan |
| `rb`, `review-beads` | `workflows/beads/references/REVIEW_BEADS.md` | Review filed beads |

## Load Workflow

1. Identify trigger from user input
2. Load corresponding workflow file (see table above)
3. Follow instructions in loaded file

## Quick Decision

- **Multi-session work?** → Use beads
- **Single-session linear task?** → Use TodoWrite
