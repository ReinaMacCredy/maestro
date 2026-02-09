---
name: team-lifecycle
description: Shared lifecycle protocols for creating, contextualizing, and cleaning up Agent Teams.
type: internal
---

# Team Lifecycle

Shared protocols used by both `/design` and `/work` for consistent Agent Team orchestration.

## Common Errors

| Error | Cause | Fix |
|-------|-------|-----|
| "unknown tool: TeamCreate" | Agent Teams not enabled | Add `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS: "1"` to `~/.claude/settings.json` env, restart Claude Code |
| "team already exists" | Previous session not cleaned up | Run `/reset` to clean stale state |

## Team Creation Pattern

```text
TeamCreate(
  team_name: "{phase}-{topic}",
  description: "{description}"
)
```

## Handoff File Protocol

Write session state to `.maestro/handoff/{topic}.json`.

```bash
mkdir -p .maestro/handoff/
```

```json
{
  "topic": "{topic}",
  "status": "{designing|executing|complete|archived}",
  "started": "{ISO timestamp}",
  "plan_destination": ".maestro/plans/{topic}.md"
}
```

## Loading Priority Context (Wisdom + Notepad)

### Loading Wisdom

```text
Glob(pattern: ".maestro/wisdom/*.md")
```

If wisdom files exist:
1. Read the first line (title) of each file.
2. Summarize titles and key points.
3. Inject that summary into agent prompts as prior learnings.

If no wisdom files exist: skip silently.

### Loading Notepad Priority Context

Read `.maestro/notepad.md`. If `## Priority Context` exists and has items:
1. Extract the section items.
2. Inject the extracted items into prompts as active priority constraints.

If no notepad exists or the section is empty: skip silently.

## Team Cleanup Pattern

Shutdown teammates, then delete the team:

```text
SendMessage(type: "shutdown_request", recipient: "{teammate-name}")
TeamDelete(reason: "Session complete")
```

If `TeamDelete` fails, fall back to manual cleanup:

```bash
rm -rf ~/.claude/teams/{team-name} ~/.claude/tasks/{team-name}
```

Ignore shutdown errors for teammates that do not exist in the current mode.
