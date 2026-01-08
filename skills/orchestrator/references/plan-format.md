# plan.md Extended Format

Extended plan.md format for orchestrator parallel execution.

## Overview

When plan.md includes orchestration sections, the orchestrator skill can dispatch parallel workers to execute tracks concurrently.

## Standard plan.md Structure

All plans have these sections:

```markdown
# Implementation Plan: <Title>

## Overview
<Brief description>

## Phase N: <Phase Name>

### Epic N.M: <Epic Name>

- [ ] **N.M.X** Task description

## Summary
<Phase/Epic/Task counts>
```

## Extended Orchestration Sections

Add these sections to enable parallel execution:

### Orchestration Config

```markdown
## Orchestration Config

epic_id: <bead-id>
max_workers: 3
mode: autonomous
```

| Field | Required | Description |
|-------|----------|-------------|
| `epic_id` | Yes | Epic bead ID (or "pending" before fb) |
| `max_workers` | No | Maximum concurrent workers (default: 3) |
| `mode` | No | "autonomous" or "supervised" (default: autonomous) |

### Track Assignments

```markdown
## Track Assignments

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.* | skills/orchestrator/** | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/design/** | 1.2.3 |
| 3 | RedStone | 3.*, 4.* | conductor/CODEMAPS/** | 2.2.2 |
```

| Column | Required | Description |
|--------|----------|-------------|
| Track | Yes | Track number (1, 2, 3...) |
| Agent | Yes | Worker name (adjective+noun format) |
| Tasks | Yes | Task IDs or wildcards (1.1.*, 1.2.1) |
| File Scope | Yes | Glob pattern for files this track owns |
| Depends On | No | Task ID that must complete first |

### Cross-Track Dependencies

```markdown
### Cross-Track Dependencies
- Track 2 waits for 1.2.3 (SKILL.md complete)
- Track 3 waits for 2.2.2 (routing complete)
```

Free-form list explaining dependency rationale.

## Example: Full Orchestrated Plan

```markdown
# Implementation Plan: Orchestrator Skill

## Overview
Create orchestrator skill for multi-agent parallel execution.

## Orchestration Config

epic_id: my-workflow:3-3cmw
max_workers: 3
mode: autonomous

## Track Assignments

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.* | skills/orchestrator/** | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/design/** | 1.2.3 |
| 3 | RedStone | 3.*, 4.* | conductor/CODEMAPS/** | 2.2.2 |

### Cross-Track Dependencies
- Track 2 waits for 1.2.3 (SKILL.md complete)
- Track 3 waits for 2.2.2 (routing complete)

---

## Phase 1: Skill Setup (1h)

### Epic 1.1: Directory Structure

- [ ] **1.1.1** Create `skills/orchestrator/` directory
- [ ] **1.1.2** Create `skills/orchestrator/references/` directory

...
```

## Detection

Orchestrator skill activates when:

1. User runs `/conductor-orchestrate`
2. User says "run parallel", "spawn workers", "dispatch agents"
3. Plan.md contains "## Track Assignments" section

## Backward Compatibility

Plans without orchestration sections work normally with `/conductor-implement` for sequential execution.

## Task Wildcards

| Pattern | Matches |
|---------|---------|
| `1.1.*` | 1.1.1, 1.1.2, 1.1.3, ... |
| `1.*` | 1.1.1, 1.1.2, 1.2.1, 1.2.2, ... |
| `1.1.1, 1.2.1` | Explicit list |
| `1.1.*, 1.2.*` | Multiple wildcards |

## File Scope Patterns

| Pattern | Matches |
|---------|---------|
| `skills/orchestrator/**` | All files under skills/orchestrator/ |
| `conductor/CODEMAPS/**` | All files under conductor/CODEMAPS/ |
| `*.md` | All markdown files in root |
| `src/**/*.ts` | All TypeScript files under src/ |

## Metadata Integration

After `fb` (file beads), orchestrator reads task-to-bead mapping from `metadata.json`:

```json
{
  "beads": {
    "epicId": "my-workflow:3-3cmw",
    "planTasks": {
      "1.1.1": "my-workflow:3-3cmw.1",
      "1.1.2": "my-workflow:3-3cmw.2",
      "1.2.1": "my-workflow:3-3cmw.3"
    }
  }
}
```

Orchestrator uses this to assign bead IDs to workers.
