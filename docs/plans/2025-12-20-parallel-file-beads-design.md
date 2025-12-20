# Parallel File-Beads Design

**Date:** 2025-12-20
**Status:** Implemented

## Problem

When using `fb` (file-beads) to create epics and issues from a plan, the main agent's context gets polluted with verbose `bd create` command outputs. This noise is discarded after filing but wastes context tokens during the session.

## Solution

Use **epic-scoped subagents** to parallelize issue filing while keeping the main agent's context clean.

## Approach Comparison

| Approach | Simplicity | Parallelism | Dependency Handling |
|----------|------------|-------------|---------------------|
| Two-phase (epics then issues) | Simple | Moderate | Clean but sync point |
| **Epic-scoped subagents** | Medium | High | Natural - each stream independent |
| Batched parallel | Complex | Highest | Requires upfront graph analysis |

**Winner:** Epic-scoped subagents — natural mapping to plan structure, self-contained execution, cross-epic deps handled in post-pass.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  MAIN AGENT (clean context)                             │
├─────────────────────────────────────────────────────────┤
│  bs → brainstorm → produces plan                        │
│  fb → triggers parallel subagents                       │
│       ├─ Task("File Epic A + issues") → returns IDs     │
│       ├─ Task("File Epic B + issues") → returns IDs     │
│       └─ Task("File Epic N + issues") → returns IDs     │
│  Main agent receives: summary only (not all bd output)  │
│  Post-pass: cross-epic deps + verify                    │
└─────────────────────────────────────────────────────────┘
```

## Phases

### Phase 1: Analyze Plan (Main Agent)
- Parse plan into epic groups
- Identify cross-epic dependencies (save for later)
- Prepare subagent prompts

### Phase 2: Parallel Dispatch (Main Agent)
- For each epic: `Task("File Epic: <title>", <context>)`
- All Tasks in single message block (parallel execution)

### Phase 3: Collect & Link (Main Agent)
- Receive JSON results from all subagents
- Resolve cross-epic deps using returned IDs
- Run: `bd dep add <from> <to> --type blocks`

### Phase 4: Verify & Summarize (Main Agent)
- `bd list --json`
- `bd ready --json`
- Present summary with starting points

## Subagent Contract

Each subagent returns structured JSON:

```json
{
  "epicId": "bd-123",
  "epicTitle": "Authentication",
  "issues": [
    {"id": "bd-124", "title": "Setup auth config", "deps": ["bd-123"]},
    {"id": "bd-125", "title": "Implement JWT", "deps": ["bd-124"]}
  ],
  "crossEpicDeps": [
    {"issueId": "bd-125", "needsLinkTo": "Database Layer"}
  ]
}
```

## Benefits

- **Context hygiene** — `bd create` output stays in subagent contexts
- **Speed** — N epics filed in time of 1
- **Focus** — Each subagent has narrow scope
- **Clean summary** — Main agent only sees results, not noise

## Implementation

Updated skill: `skills/beads/file-beads/SKILL.md`
