# Handoff System

HumanLayer-inspired cross-session context preservation for AI agent workflows.

## Overview

The handoff system replaces the previous LEDGER.md/continuity approach with a git-committed, shareable system for preserving context across sessions.

### Key Benefits

- **Git-committed** - Handoffs are version-controlled and shareable with teammates
- **Standalone** - Works with or without Conductor tracks
- **Structured** - YAML frontmatter with metadata (timestamp, trigger, git info)
- **Automatic** - Triggers at key workflow points (design-end, epic-start, etc.)

## Quick Start

### Automatic Session Resume (Amp)

On session start, handoffs are automatically loaded:

```
📋 Prior session context found:

• auto-orchestrate (2h ago) - pre-finish: Track complete

Loading context...
```

Skip conditions:
- User says "fresh start" or "new session"
- No `conductor/` directory exists
- All handoffs are > 7 days old (shows stale warning)

### Create a Handoff

```bash
/create_handoff
```

Creates a handoff file capturing:
- Current work context
- Files changed
- Learnings and gotchas
- Next steps as checkboxes

### Resume from a Handoff

```bash
/resume_handoff
```

Smart discovery:
- Auto-selects if only 1 track exists
- Lists recent handoffs if multiple tracks
- Validates git branch and file state
- Creates todo list from Next Steps

## File Structure

```
conductor/handoffs/
├── general/                          # Non-track handoffs
│   ├── index.md                      # Handoff log
│   └── YYYY-MM-DD_HH-MM-SS-mmm_general_<trigger>.md
└── <track-id>/                       # Per-track handoffs
    ├── index.md                      # Handoff log
    ├── YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md
    └── archive/                      # After /conductor-finish
```

### File Naming

Millisecond-precision timestamps ensure uniqueness:

```
2025-12-29_10-00-00-123_auth-system_design-end.md
2025-12-29_11-30-00-456_auth-system_E1_epic-start.md
2025-12-29_14-15-00-789_general_manual.md
```

## Handoff Template

```markdown
---
timestamp: 2025-12-29T10:00:00.123+07:00
trigger: design-end
track_id: auth-system
bead_id: E1-user-login          # Only for epic triggers
git_commit: abc123f
git_branch: feat/auth-system
author: agent
validation_snapshot:
  gates_passed: [design, spec, plan-structure]
  current_gate: plan-execution
---

# Handoff: auth-system | design-end

## Context

{What you were working on, current state, active decisions}

## Changes

{Files modified with line references}
- `path/to/file.ts:10-45` - Added login handler

## Learnings

{Patterns discovered, gotchas, important context}

## Next Steps

{Immediate actions for resuming agent - converted to todos}
1. [ ] First task
2. [ ] Second task
```

## Triggers

Six automatic triggers integrate with Conductor workflows:

| Trigger | When | Automatic |
|---------|------|-----------|
| `design-end` | After `/conductor-newtrack` completes | ✅ |
| `epic-start` | Before each epic in `/conductor-implement` | ✅ |
| `epic-end` | After each epic closes | ✅ |
| `pre-finish` | At start of `/conductor-finish` | ✅ |
| `manual` | User runs `/create_handoff` | ❌ |
| `idle` | 30min inactivity gap detected | ✅ (prompted) |

### Configuration

In `conductor/workflow.md`:

```yaml
handoff:
  quiet: false                    # Suppress prompts
  idle_threshold_minutes: 30      # Idle detection gap
  auto_triggers:
    design-end: true              # Always recommended
    epic-start: true              # Can disable for short epics
    epic-end: true                # Can disable for short epics
    pre-finish: true              # Always recommended
```

## Idle Detection

After 30+ minutes of inactivity, the next message triggers a prompt:

```
⏰ It's been 45 minutes since your last activity.

Create a handoff to preserve context?

[Y] Yes - Create handoff first (recommended)
[n] No  - Skip this time
[s] Skip - Don't ask again this session
```

The `.last_activity` marker file tracks session activity.

## Validation on Resume

When loading a handoff, several checks run:

| Check | Action |
|-------|--------|
| Branch mismatch | Warn, ask to continue |
| Stale (>7 days) | Warn, ask to continue |
| Files deleted | Warn, list missing files |
| Malformed YAML | Infer from filename, warn |

## Index Auto-Repair

If `index.md` is corrupted or missing, `/resume_handoff` automatically rebuilds it by scanning the handoff directory.

## Secrets Scanning

Before writing handoffs, content is scanned for:

- OpenAI API keys (`sk-*`)
- GitHub PATs (`ghp_*`, `github_pat_*`)
- AWS keys (`AKIA*`)
- Private keys (`-----BEGIN.*PRIVATE KEY-----`)
- Generic API key patterns

On detection:

```
⚠️ Potential secret detected in handoff content:
   Pattern: sk-******* (OpenAI API Key)

[P]roceed anyway  [A]bort
```

## Archive on Finish

When `/conductor-finish` runs:

1. Handoff files move to `archive/` subdirectory
2. `index.md` entries marked as archived
3. Historical reference preserved

## Command Reference

| Command | Description |
|---------|-------------|
| `/resume_handoff` | Load prior session context |
| `/create_handoff` | Save current session context |

Validation state stored in `metadata.json.validation`.

## Reference Files

Full implementation details in:

- [skills/conductor/references/handoff/template.md](../skills/conductor/references/handoff/template.md)
- [skills/conductor/references/handoff/create.md](../skills/conductor/references/handoff/create.md)
- [skills/conductor/references/handoff/resume.md](../skills/conductor/references/handoff/resume.md)
- [skills/conductor/references/handoff/triggers.md](../skills/conductor/references/handoff/triggers.md)
- [skills/conductor/references/handoff/idle-detection.md](../skills/conductor/references/handoff/idle-detection.md)
