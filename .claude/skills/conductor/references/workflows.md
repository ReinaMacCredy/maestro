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
| `design` | Design a feature/bug through collaborative dialogue | See [design skill](../../design/SKILL.md) |
| `newtrack` | Create spec and plan from design.md, file beads | [workflows/newtrack.md](workflows/newtrack.md) |
| `implement` | Execute tasks from track's plan following TDD workflow | [workflows/implement.md](workflows/implement.md) |
| `status` | Display progress overview | [workflows/status.md](workflows/status.md) (inline below) |
| `revert` | Git-aware revert of tracks, phases, or tasks | (inline below) |
| `revise` | Update spec/plan when implementation reveals issues | [revisions.md](revisions.md) |
| `finish` | Complete track: extract learnings, compact beads, archive | [finish-workflow.md](finish-workflow.md) |

## Validation Gates

Validation gates are triggered at specific workflow points. See [validation/lifecycle.md](validation/lifecycle.md) for the complete gate registry.

| Gate | Trigger | Authoritative Doc |
|------|---------|-------------------|
| `validate-design` | After DELIVER phase | [design/SKILL.md](../../design/SKILL.md) |
| `validate-spec` | After spec.md generation | [workflows/newtrack.md](workflows/newtrack.md#phase-3) |
| `validate-plan-structure` | After plan.md generation | [workflows/newtrack.md](workflows/newtrack.md#phase-4) |
| `validate-plan-execution` | After TDD REFACTOR | [tdd/cycle.md](tdd/cycle.md) |
| `validate-completion` | Before /conductor-finish | [finish-workflow.md](finish-workflow.md) |

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
