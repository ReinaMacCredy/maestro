---
name: conductor
description: Context-driven development methodology. Trigger with ci, /conductor-implement, cn, /conductor-newtrack, or /conductor-* commands. Use when working with conductor/ directories, tracks, specs, plans, or when user mentions context-driven development.
---

# Conductor: Context-Driven Development

Measure twice, code once.

## Entry Points

| Trigger | Action | Reference |
|---------|--------|-----------|
| `/conductor-setup` | Initialize project context | [workflows/setup.md](references/workflows/setup.md) |
| `/conductor-design` | Design feature (Double Diamond) | [design skill](../design/SKILL.md) |
| `/conductor-newtrack` | Create spec + plan from design | [workflows/newtrack.md](references/workflows/newtrack.md) |
| `/conductor-implement` | Execute track (auto-routes if parallel) | [workflows/implement.md](references/workflows/implement.md) |
| `/conductor-status` | Display progress overview | [structure.md](references/structure.md) |
| `/conductor-revise` | Update spec/plan mid-work | [revisions.md](references/revisions.md) |
| `/conductor-finish` | Complete track, extract learnings | [finish-workflow.md](references/finish-workflow.md) |
| `/conductor-handoff` | Unified handoff (auto-detect create/resume) | [workflows/handoff.md](references/workflows/handoff.md) |

## Quick Reference

| Phase | Purpose | Output |
|-------|---------|--------|
| Requirements | Understand problem | design.md |
| Plan | Create detailed plan | spec.md + plan.md |
| Implement | Build with TDD | Code + tests |
| Reflect | Verify before shipping | LEARNINGS.md |

## Core Principles

- **Load core first** - Load [maestro-core](../maestro-core/SKILL.md) for routing table and fallback policies
- **Design before code** - `/conductor-design` → `/conductor-newtrack` → implement
- **TDD by default** - RED → GREEN → REFACTOR (use `--no-tdd` to disable)
- **Beads integration** - Zero manual `bd` commands in happy path
- **Parallel routing** - `## Track Assignments` in plan.md triggers orchestrator
- **Validation gates** - Automatic checks at each phase transition

## Directory Structure

```
conductor/
├── product.md, tech-stack.md, workflow.md  # Project context
├── code_styleguides/                       # Language-specific style rules
├── CODEMAPS/                               # Architecture docs
├── handoffs/                               # Session context
├── spikes/                                 # Research spikes (pl output)
└── tracks/<track_id>/                      # Per-track work
    ├── design.md, spec.md, plan.md         # Planning artifacts
    └── metadata.json                       # State tracking (includes planning state)
```

See [structure.md](references/structure.md) for full details.

## Beads Integration

All execution routes through orchestrator with Agent Mail coordination:
- Workers claim beads via `bd update --status in_progress`
- Workers close beads via `bd close --reason completed|skipped|blocked`
- File reservations via `file_reservation_paths`
- Communication via `send_message`/`fetch_inbox`

See [beads-integration.md](references/beads-integration.md) for all 13 integration points.

## `/conductor-implement` Auto-Routing

1. Read `metadata.json` - check `orchestrated` flag
2. Read `plan.md` - check for `## Track Assignments`
3. Check `beads.fileScopes` - file-scope based grouping (see [file-scope-extractor](references/file-scope-extractor.md))
4. If parallel detected (≥2 non-overlapping groups) → Load [orchestrator skill](../orchestrator/SKILL.md)
5. Else → Sequential execution with TDD

### File Scope Detection

`/conductor-newtrack` Phase 4.5 extracts file paths from tasks and groups them:
- Tasks touching same files → sequential (same track)
- Tasks touching different files → parallel (separate tracks)
- See [parallel-grouping](references/parallel-grouping.md) for algorithm

## Anti-Patterns

- ❌ Skipping design phase for complex features
- ❌ Creating spec/plan without metadata.json
- ❌ Manual `bd` commands when workflow commands exist
- ❌ Ignoring validation gate failures

## Related

- [design](../design/SKILL.md) - Double Diamond design sessions
- [beads](../beads/SKILL.md) - Issue tracking
- [orchestrator](../orchestrator/SKILL.md) - Parallel execution
- [maestro-core](../maestro-core/SKILL.md) - Routing policies in AGENTS.md
