# Amp Code Setup

> **DEPRECATED (2025-12-29)**
>
> This document is kept for historical reference only.
> 
> **Session continuity is now automatic** via Conductor workflow entry points:
> - `ds` → loads prior context
> - `/conductor-implement` → loads + binds to track/bead
> - `/conductor-finish` → handoff + archive
>
> See [docs/GLOBAL_CONFIG.md](../../../../docs/GLOBAL_CONFIG.md) for current approach.

---

**Historical Content (Pre-Auto-Continuity)**

Amp Code doesn't support hooks like Claude Code, so we use manual commands with reminders.

## Configuration

Add to `~/.config/amp/AGENTS.md`:

```markdown
## Continuity Protocol

### Session Start
Run `continuity load` at the start of every session.

### After File Edits
Run `continuity save` after making significant changes.

### Session End
Run `continuity handoff` before ending the session.
```

## amp.hooks Configuration (Optional)

If Amp supports hooks in the future, use this configuration:

```json
{
  "hooks": {
    "tool:post-execute": {
      "filter": {
        "tool": ["edit_file", "create_file"]
      },
      "action": "reminder",
      "message": "💾 Consider running `continuity save` to update session state."
    }
  }
}
```

## Manual Workflow

Since Amp doesn't auto-trigger hooks:

### 1. Session Start

```bash
continuity load
```

This loads:
- LEDGER.md (current session state)
- Last handoff from archive (context from previous session)

### 2. During Work

After significant milestones:

```bash
continuity save
```

This updates:
- Modified files list
- Current state (Done/Now/Next)
- Timestamp

### 3. Session End

```bash
continuity handoff
```

This:
- Archives current LEDGER.md as handoff
- Creates new empty ledger for next session

## Session Protocol Reminder

Add this to your Amp startup prompt or first message:

```text
Remember to run `continuity load` at session start and `continuity handoff` at session end.
```

## Differences from Claude Code

| Feature | Claude Code | Amp Code |
|---------|-------------|----------|
| Auto-load on start | ✅ SessionStart hook | ❌ Manual |
| Auto-save before compact | ✅ PreCompact hook | ❌ Manual |
| File tracking | ✅ PostToolUse hook | ❌ Manual |
| Session end archive | ✅ Stop hook | ❌ Manual |
| Reminders | N/A | ⚠️ In AGENTS.md |

## Troubleshooting

### Context not loading?

Run manually:

```bash
continuity load
```

### State not saved?

Run manually:

```bash
continuity save
```

### Lost session context?

Check for handoffs:

```bash
continuity status
```

Or search:

```bash
continuity search <keyword>
```

## Installing the Skill

The continuity content is available in `skills/conductor/references/ledger/`.

No special installation needed for Amp - just use the manual commands.
