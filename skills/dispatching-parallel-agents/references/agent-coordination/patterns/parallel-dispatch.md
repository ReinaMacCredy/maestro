# Parallel Dispatch Pattern

Coordinate file access when dispatching 2+ parallel subagents via Task tool.

## When to Use

Before dispatching parallel subagents that may touch overlapping files.

## File Detection Heuristics

Parse task descriptions for file patterns:

| Pattern | Example | Extracts |
|---------|---------|----------|
| Explicit path | "Edit skills/beads/SKILL.md" | `skills/beads/SKILL.md` |
| Directory reference | "Update the beads skill" | `skills/beads/**` |
| File type | "Fix the test file" | `**/*.test.{ts,js}` |
| Component name | "Modify the conductor workflow" | `workflows/**/`, `skills/conductor/**` |
| Quoted paths | "`src/api/users.ts`" | `src/api/users.ts` |
| Backtick code | "Change the `UserService` class" | Search for file containing `UserService` |

**Fallback:** If no patterns detected, don't reserve. Subagent self-reserves if needed.

## Flow

### 1. Parse Tasks for Files

Extract file patterns from task descriptions using heuristics above.
- Explicit paths take priority
- Infer from context ("edit the beads skill" → `skills/beads/**`)
- Best-effort; subagents can reserve additional files

### 2. Reserve Files (3s timeout)

```python
file_reservation_paths(
  project_key: <workspace>,
  agent_name: <coordinator>,
  paths: [<file patterns>],
  ttl_seconds: 3600,  # 1h default
  exclusive: true
)
```

On timeout/failure: log warning, proceed without reservation.

### 3. Inject Coordination Block

Add to each Task prompt. See [subagent-prompt.md](subagent-prompt.md).

### 4. Dispatch Subagents

Use Task tool for each parallel agent.

### 5. Release on Completion

```python
release_file_reservations(
  project_key: <workspace>,
  agent_name: <coordinator>
)
```

On failure: log warning (TTL expires anyway).

## Visible Feedback

Show user before dispatch:
```text
🔒 Reserved: skills/foo/SKILL.md, skills/bar/SKILL.md (1h)
Dispatching 3 agents...
```

Show user after completion:
```text
🔓 Released reservations
```

## Conflict Handling

If `file_reservation_paths` returns conflicts:
- Log which files are already reserved and by whom
- Skip those files (don't block dispatch)
- Warn user which agents may have limited scope

```text
⚠️ skills/beads/SKILL.md reserved by GreenCastle - skipping
```
