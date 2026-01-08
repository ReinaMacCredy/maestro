# Design: Ralph Integration into Maestro

## Summary

Integrate Ralph (autonomous AI agent loop) as a third execution mode (`ca`) in the Maestro Conductor workflow, alongside `ci` (implement) and `co` (orchestrate).

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Integration style | Native (not adapter) | Single source of truth in metadata.json |
| Ralph location | `toolboxes/ralph/` | Consistent with existing toolbox pattern |
| State storage | `metadata.json.ralph` section | No separate prd.json |
| DS output | Both plan.md + ralph.stories | User chooses execution mode later |
| AI backend | Amp CLI (unchanged) | Keep Ralph upstream-compatible |

## Architecture

```
ds → design.md → spec.md + plan.md   → ci/co (Beads)
              └→ metadata.ralph      → ca (Ralph loop)
```

### Components

| Component | Location | Purpose |
|-----------|----------|---------|
| `ralph.sh` | `toolboxes/ralph/` | Loop script reading metadata.json |
| `prompt.md` | `toolboxes/ralph/` | Amp instructions for Maestro context |
| Routing | `maestro-core` | `ca` trigger → conductor |
| Command | `conductor` | `/conductor-autonomous` handler |
| DS output | `designing` | Populate `metadata.ralph.stories` |

## Data Model

### metadata.json.ralph

```json
{
  "ralph": {
    "enabled": true,
    "active": false,
    "maxIterations": 10,
    "currentIteration": 0,
    "progressFile": "progress.txt",
    "stories": {
      "1.1": {
        "title": "Task title from plan",
        "acceptanceCriteria": ["Criterion 1", "Criterion 2"],
        "passes": false,
        "notes": ""
      }
    }
  }
}
```

### Mapping

| Ralph PRD | Maestro metadata | Notes |
|-----------|------------------|-------|
| `branchName` | `workflow.branch` | Direct |
| `description` | `description` | Direct |
| `userStories[].id` | `ralph.stories` keys | Match planTasks |
| `userStories[].passes` | `ralph.stories[id].passes` | New field |
| `acceptanceCriteria` | `ralph.stories[id].acceptanceCriteria` | New field |

## Gaps Addressed

1. **State divergence**: plan.md + beads = canonical; ralph.stories = derived
2. **workflow.history.command**: Add `"autonomous"` to enum
3. **progress.txt**: Keep in track dir
4. **Exclusive lock**: `ralph.active` prevents ci/co during ca

## Oracle Audit

✅ Feasibility: HIGH  
⚠️ Completeness: GAPS (addressed above)  
✅ Consistency: GOOD  
⚠️ Risk: State divergence (mitigated)  
✅ Dependencies: CLEAR  
