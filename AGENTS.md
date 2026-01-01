# AGENTS.md - Maestro Plugin

## Overview
Claude Code plugin bundling workflow skills: Conductor (planning), Design (Double Diamond sessions), Beads (issue tracking), Orchestrator (multi-agent parallel execution), and Superpowers (TDD, debugging, code review).

## Build/Test Commands
No build required - this is a skill/documentation plugin. Validate JSON:
```bash
cat .claude-plugin/plugin.json | jq .   # Validate plugin manifest
```

## Architecture
```
skills/           # Skill directories, each with SKILL.md (frontmatter + instructions)
  beads/          # Issue tracking skill with references/ subdirectory
  conductor/      # Planning methodology (includes /conductor-design, CODEMAPS generation, handoff system)
  design/         # Double Diamond design sessions (ds trigger), includes bmad/
  orchestrator/   # Multi-agent parallel execution with autonomous workers
  ...             # TDD, debugging, code review, etc.
lib/              # Shared utilities (skills-core.js)
.claude-plugin/   # Plugin manifest (plugin.json, marketplace.json)
conductor/        # Unified save location for plans and tracks
  tracks/<id>/    # Active work (design.md + spec.md + plan.md per track)
  handoffs/       # Session handoffs (git-committed, shareable)
  CODEMAPS/       # Architecture documentation (overview.md, module codemaps)
  archive/        # Completed work
```

## Handoff Mechanism

Handoff preserves context between sessions via persistent files (`design.md`, `spec.md`, `plan.md`) and trackable beads.

**Flow:** `ds → design.md → /conductor-newtrack → spec.md + plan.md + beads`

**Commands:**
- `/create_handoff` - Save current context
- `/resume_handoff` - Load most recent handoff
- `/conductor-implement <track-id>` - Start execution

See [docs/handoff-system.md](docs/handoff-system.md) for full documentation.

## Code Style
- Skills: Markdown with YAML frontmatter (`name`, `description` required)
- Skill directories: kebab-case (`test-driven-development`, `using-git-worktrees`)
- SKILL.md must match directory name in frontmatter `name` field

## Versioning
- **Plugin version**: Auto-bumped by CI (`feat:` → minor, `fix:` → patch, `feat!:` → major)
- **Skill versions**: Manual update in SKILL.md frontmatter
- **Escape hatch**: `[skip ci]` in commit message

## Key Skills

| Skill | Trigger | Description |
|-------|---------|-------------|
| `design` | `ds` | Double Diamond design session |
| `conductor` | `/conductor-*` | Structured planning and execution |
| `orchestrator` | `/conductor-orchestrate`, "run parallel" | Multi-agent parallel execution |
| `beads` | `fb`, `rb`, `bd ready` | Issue tracking |

## Quick Reference

| Task | Command |
|------|---------|
| Start design | `ds` |
| Create track from design | `/conductor-newtrack` |
| Find ready work | `bd ready` |
| Start implementation | `/conductor-implement <track>` |
| Finish track | `/conductor-finish` |

## Fallback Policy

| Condition | Action |
|-----------|--------|
| `bd` unavailable | HALT |
| `conductor/` missing | DEGRADE (standalone mode) |
| Village MCP unavailable | DEGRADE |

## Session Lifecycle

### Auto-Load Handoffs (First Message)

On first user message of a new session, before processing:

1. Check if `conductor/handoffs/` exists
2. Scan for recent handoffs (< 7 days old)
3. If found: Display `📋 Prior context: [track] (Xh ago)` and load silently
4. Proceed with user's request

**Skip if:** User says "fresh start", no `conductor/`, or all handoffs > 7 days (show stale warning).

### Idle Detection

On every user message, before routing:

1. Check `conductor/.last_activity` mtime
2. If gap > 30min (configurable in `workflow.md`):
   ```
   ⏰ It's been X minutes. Create handoff? [Y/n/skip]
   ```
3. **Y** = create handoff with `idle` trigger, **n** = skip once, **skip** = disable for session

See [conductor/references/handoff/idle-detection.md](skills/conductor/references/handoff/idle-detection.md).

## Session Protocol

### Session Identity

- Format: `{BaseAgent}-{timestamp}` (internal), `{BaseAgent} (session HH:MM)` (display)
- Registered with Agent Mail on `/conductor-implement` or `/conductor-orchestrate`
- Skipped for `ds`, `bd ready`, `bd show`, `bd list`

### Preflight Triggers

| Command | Preflight |
|---------|-----------|
| `/conductor-implement` | ✅ Yes |
| `/conductor-orchestrate` | ✅ Yes |
| `ds` | ❌ Skip |
| `bd ready/show/list` | ❌ Skip |

### Stale Threshold

10 minutes since last activity → takeover prompt

---

## Skill Discipline

**Check for skills BEFORE ANY RESPONSE.** Even 1% chance means invoke the Skill tool first.

### The Rule

If a skill might apply, you MUST read it. This is not negotiable.

### Red Flags (Stop - You're Rationalizing)

| Thought | Reality |
|---------|---------|
| "This is just a simple question" | Questions are tasks. Check for skills. |
| "I need more context first" | Skill check comes BEFORE clarifying questions. |
| "Let me explore the codebase first" | Skills tell you HOW to explore. Check first. |
| "This doesn't need a formal skill" | If a skill exists, use it. |
| "I remember this skill" | Skills evolve. Read current version. |
| "The skill is overkill" | Simple things become complex. Use it. |

### Skill Priority

1. **Process skills first** (`ds`, `/conductor-design`) - determine HOW to approach
2. **Implementation skills second** (`frontend-design`, `mcp-builder`) - guide execution

### Skill Types

- **Rigid** (TDD): Follow exactly
- **Flexible** (patterns): Adapt to context

---

## Detailed References

For detailed workflows, load the appropriate skill or see:
- [Beads workflow](skills/beads/references/workflow-integration.md)
- [Handoff system](docs/handoff-system.md)
- [Agent coordination](skills/orchestrator/references/agent-coordination.md)
- [Router](skills/orchestrator/references/router.md)
