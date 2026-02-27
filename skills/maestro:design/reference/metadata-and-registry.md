# Metadata, Index, and Registry

## metadata.json

```json
{
  "track_id": "{track_id}",
  "type": "{feature | bug | chore}",
  "status": "new",
  "description": "{track description}",
  "created_at": "{ISO 8601 timestamp}",
  "updated_at": "{ISO 8601 timestamp}",
  "phases": {phase_count},
  "tasks": {task_count},
  "skills": [
    {
      "name": "skill-name",
      "relevance": "matched",
      "matched_on": ["keyword1", "keyword2"]
    }
  ],
  "beads_epic_id": "{br_epic_id | null}",
  "beads_issue_map": {
    "P1T1": "{br_issue_id}",
    "P1T2": "{br_issue_id}"
  }
}
```

Note: `"skills"` is `[]` if no skills were detected.

Note: `"beads_epic_id"` and `"beads_issue_map"` are set by the plan-to-BR sync step (Step 9.5). If BR sync was skipped or failed, omit both fields. When present, `beads_epic_id` is the discriminator that tells downstream skills to use BR for state tracking.

Write to `.maestro/tracks/{track_id}/metadata.json`.

## Track Index (index.md)

```markdown
# Track: {track description}

> ID: {track_id}
> Type: {type}
> Status: New
> Created: {date}

## Files
- [Specification](./spec.md)
- [Implementation Plan](./plan.md)
- [Metadata](./metadata.json)

## Quick Links
- Registry: [tracks.md](../../tracks.md)
- Implement: `/maestro:implement {track_id}`
- Status: `/maestro:status`
```

Write to `.maestro/tracks/{track_id}/index.md`.

## Registry Update

Append to `.maestro/tracks.md`:

```markdown
---
- [ ] **Track: {track description}**
  *Type: {type} | ID: [{track_id}](./tracks/{track_id}/)*
```

## Commit

```bash
git add .maestro/tracks/{track_id} .maestro/tracks.md
git commit -m "chore(maestro:design): add track {track_id}"
```

## Summary Output

```
## Track Created

**{track description}**
- ID: `{track_id}`
- Type: {type}
- Phases: {count}
- Tasks: {count}

**Files**:
- `.maestro/tracks/{track_id}/spec.md`
- `.maestro/tracks/{track_id}/plan.md`
- `.maestro/tracks/{track_id}/metadata.json`
- `.maestro/tracks/{track_id}/index.md`

**Next**: `/maestro:implement {track_id}`
```
