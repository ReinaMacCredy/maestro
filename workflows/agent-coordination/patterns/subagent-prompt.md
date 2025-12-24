# Subagent Coordination Block

Inject this block into Task prompts when dispatching coordinated subagents.

## Template

Add this to the end of each Task prompt:

```markdown
---
**Coordination:**
- Working inside reservation: {file_patterns}
- If you need files outside this, call `register_agent` then `file_reservation_paths`
- On conflict with unreserved file: warn + skip
- Do NOT release reservations; coordinator handles cleanup
---
```

## Parameters

| Parameter | Description | Example |
|-----------|-------------|---------|
| `{file_patterns}` | Files reserved for this subagent | `skills/beads/SKILL.md, skills/beads/references/*` |

## Example Injection

Task prompt for a subagent working on the beads skill:

```markdown
Update the beads skill documentation to include the new compact commands.

1. Read the current SKILL.md
2. Add section for compact commands
3. Update references if needed

---
**Coordination:**
- Working inside reservation: skills/beads/SKILL.md, skills/beads/references/*
- If you need files outside this, call `register_agent` then `file_reservation_paths`
- On conflict with unreserved file: warn + skip
- Do NOT release reservations; coordinator handles cleanup
---
```

## Subagent Behavior

When subagent sees this block:

1. **Stay in scope**: Prefer editing reserved files
2. **Extend if needed**: Register self and reserve additional files
3. **Handle conflicts**: If unreserved file has conflict, warn and skip
4. **Don't cleanup**: Leave reservations for coordinator to release

## Notes

- Subagents working inside coordinator's reservation don't need to register
- Only register if needing files outside the reserved scope
- Coordinator is responsible for final cleanup
