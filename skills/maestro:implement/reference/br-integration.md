# BR/BV Integration Reference

Central reference for all `br`/`bv` patterns used by maestro skills.

## Tool Roles

| Tool | Role | Scope |
|------|------|-------|
| `br` | State management | Create, update, close issues. Manage deps. Sync JSONL. |
| `bv` | Analysis & planning | Dependency-aware execution plans, insights, triage, next-action. |

## Discriminator

The `beads_epic_id` field in `metadata.json` determines which path to use:

- **Present**: Use br/bv for state tracking. Plan.md is updated alongside as a human-readable mirror.
- **Absent**: Use legacy plan.md checkbox parsing (unchanged behavior).

## CLI Conventions

- `br` uses double-dash flags: `--json`, `--status`, `--label`, `--parent`, `--reason`
- `bv` uses single-dash flags: `-robot-plan`, `-robot-insights`, `-robot-next`, `-format json`, `-label`
- **Always append `--json` (br) or `-format json` (bv)** for all automated commands. Never parse human-formatted output.

## Issue Lifecycle

```
create (open) --> claim (in_progress) --> close (closed)
                                     --> reopen (open)  [revert path]
```

### Create

```bash
br create --title "P{N}T{M}: {title}" \
  --parent {epic_id} \
  --labels "phase:{N}-{kebab},type:{type}" \
  --description "{sub-tasks + acceptance criteria}" \
  --json
```

### Claim (mark in-progress)

```bash
br update {issue_id} --claim --json
```

Sets assignee to actor and status to `in_progress` atomically.

### Close (mark complete)

```bash
br close {issue_id} --reason "sha:{sha7} | {evidence}" --suggest-next --json
```

The `--suggest-next` flag returns newly unblocked issues after closing.

### Reopen (revert path)

```bash
br update {issue_id} --status open --json
```

## Epic Management

### Create epic (track-level)

```bash
br create --title "Track: {description}" \
  --labels "type:{type}" \
  --json
```

### Check epic status

```bash
br epic status --json
```

### Auto-close eligible epics

```bash
br epic close-eligible --json
```

## Dependency Management

### Add dependency

```bash
br dep add {child_id} {parent_id}
```

Means: `child_id` depends on (is blocked by) `parent_id`.

### Validate no cycles

```bash
br dep cycles --json
```

### View dependency tree

```bash
br dep tree {epic_id} --json
```

## Status Queries

### List open issues for a phase

```bash
br list --status open --label "phase:{N}-{kebab}" --json
```

### List all closed issues for a track type

```bash
br list --status closed --label "type:{type}" --all --json
```

### List ready (unblocked) issues

```bash
br ready --parent {epic_id} --json
```

### Workspace stats

```bash
br stats --json
```

## BV Analysis Commands

### Dependency-respecting execution plan

```bash
bv -robot-plan -label "track:{epic_id}" -format json
```

### Graph health (cycles, bottlenecks)

```bash
bv -robot-insights -format json
```

### Top next action

```bash
bv -robot-next -format json
```

## Sync Protocol

`.beads/` is committed (not gitignored). After any session with br mutations:

```bash
br sync --flush-only && git add .beads/ && git commit -m "sync beads"
```

This runs at:
- Track/phase completion boundaries
- Session end (if br state changed)
- After plan-to-br sync during new-track creation

## Plan.md Mirror Protocol

BR is the source of truth for task state. Plan.md is updated alongside for human readability:

1. When claiming a task: edit plan.md `[ ]` -> `[~]` AND run `br update {id} --claim --json`
2. When completing a task: edit plan.md `[~]` -> `[x] {sha}` AND run `br close {id} --reason "sha:{sha7} | {evidence}" --json`
3. When reverting: edit plan.md `[x] {sha}` -> `[ ]` AND run `br update {id} --status open --json`

Both updates happen together. If one fails, report the inconsistency.
