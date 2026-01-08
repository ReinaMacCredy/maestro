# Conductor Workflows

Context-Driven Development for Claude Code. Measure twice, code once.

## Usage

```
/conductor-[command] [args]
```

## Commands Index

> **Authoritative Workflow Docs:** Each command has a detailed workflow file that is the **single source of truth** for that command's behavior, including validation gates.

| Command | Description | Authoritative Doc |
|---------|-------------|-------------------|
| `setup` | Initialize project with product.md, tech-stack.md, workflow.md | [workflows/setup.md](workflows/setup.md) |
| `design` | Design a feature/bug through collaborative dialogue | See [designing skill](../../designing/SKILL.md) |
| `newtrack` | Create spec and plan from design.md, file beads | [workflows/newtrack.md](workflows/newtrack.md) |
| `implement` | Execute tasks from track's plan following TDD workflow | [workflows/implement.md](workflows/implement.md) |
| `status` | Display progress overview | [workflows/status.md](workflows/status.md) (inline below) |
| `revert` | Git-aware revert of tracks, phases, or tasks | (inline below) |
| `revise` | Update spec/plan when implementation reveals issues | [workflows.md#revisions](#revisions) |
| `finish` | Complete track: extract learnings, compact beads, archive | [workflows.md#finish-workflow](#finish-workflow) |

## Validation Gates

Validation gates are triggered at specific workflow points. See [validation/lifecycle.md](validation/lifecycle.md) for the complete gate registry.

| Gate | Trigger | Authoritative Doc |
|------|---------|-------------------|
| `validate-design` | Each checkpoint (CP1-4); CP4 full gate | [validation/lifecycle.md](validation/lifecycle.md) |
| `validate-spec` | After spec.md generation | [workflows/newtrack.md](workflows/newtrack.md#phase-3) |
| `validate-plan-structure` | After plan.md generation | [workflows/newtrack.md](workflows/newtrack.md#phase-4) |
| `validate-plan-execution` | After TDD REFACTOR | [tdd/cycle.md](tdd/cycle.md) |
| `validate-completion` | Before /conductor-finish | [workflows.md#finish-workflow](#finish-workflow) |

---

## Quick Reference (Non-Authoritative)

The sections below provide a quick overview. **For full behavior including validation gates, always refer to the authoritative doc.**

### Workflow: Status

**Trigger:** `/conductor-status`

1. Read `conductor/tracks.md` and all `conductor/tracks/*/plan.md`
2. Calculate progress (total, completed, in-progress, pending)
3. Present summary with current task and next action

### Workflow: Revert

**Trigger:** `/conductor-revert`

1. Identify target (track, phase, or task)
2. Find related commits
3. Present revert plan
4. Execute `git revert` in reverse order
5. Reset status markers in plan.md

---

## State Files Reference

| File | Purpose |
|------|---------|
| `conductor/product.md` | Product vision, users, goals |
| `conductor/tech-stack.md` | Technology choices |
| `conductor/workflow.md` | Development workflow (TDD, commits) |
| `conductor/tracks.md` | Master track list with status |
| `conductor/tracks/<id>/metadata.json` | Track metadata + validation state |
| `conductor/tracks/<id>/design.md` | High-level design |
| `conductor/tracks/<id>/spec.md` | Requirements |
| `conductor/tracks/<id>/plan.md` | Phased task list |

## Status Markers

- `[ ]` - Pending/New
- `[~]` - In Progress
- `[x]` - Completed
- `[!]` - Blocked (with reason)
- `[-]` - Skipped (with reason)

---

## finish-workflow

> **⚠️ MOVED TO HANDOFF SKILL**
> 
> This workflow has moved to the [handoff skill](../../handoff/SKILL.md).
> 
> **Triggers:** `ho`, `/conductor-finish`, `/conductor-handoff`

---

## checkpoint

Quick entry point for progress checkpointing operations.

**Primary Reference:** [tracking skill](../../tracking/SKILL.md)

| Trigger | Action |
|---------|--------|
| Token budget > 70% | Proactive checkpoint |
| Token budget > 85% | Checkpoint + warn user |
| Token budget > 90% | Auto-checkpoint |
| Major milestone | Checkpoint notes |
| Hit a blocker | Capture what was tried |

**Degradation Signals:** 2+ signals → trigger compression

---

## remember

Quick entry point for handoff protocol operations.

**Primary Reference:** [handoff skill](../../handoff/SKILL.md)

| Aspect | Description |
|--------|-------------|
| Storage | `conductor/handoffs/<track>/` |
| Format | Structured JSON + Markdown summary |
| TTL | 7 days (configurable) |

→ See [handoff skill](../../handoff/SKILL.md) for templates and workflows

---

## revisions

Guidelines for revising specs and plans mid-implementation.

**When to Revise:**
- New requirements discovered during implementation
- Technical blockers requiring approach changes
- User feedback invalidating assumptions

**Workflow:** Use `/conductor-revise` → [workflows/revise.md](workflows/revise.md)
