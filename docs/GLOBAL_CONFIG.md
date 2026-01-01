# Maestro - Global Configuration

Apply to global config:

| Tool | Path |
|------|------|
| Claude Code | `~/.claude/CLAUDE.md` |
| Amp | `~/.config/amp/AGENTS.md` |
| Codex | `~/.codex/AGENTS.md` |
| Gemini CLI | `~/.gemini/GEMINI.md` |
| Cursor | `~/.cursor/rules/maestro.md` |

Replace existing `<!-- BEGIN maestro-config -->` block or append. Add Village block if `.beads-village/` exists.

---

## Config Block

<!-- BEGIN maestro-config -->
<!-- version: 2.2.0 | bmad-v6 | 2026-1-1 -->

## Maestro Workflow

Context-driven development with TDD execution.

**First message:** Check `conductor/handoffs/` for prior session context.

### Project Detection

Maestro project if any exist:
- `conductor/` directory
- `.beads/` directory

When detected, use Conductor commands instead of ad-hoc planning.

### Triggers

**Planning:**
- `ds` or `/conductor-design` - Double Diamond design session with A/P/C checkpoints
- `/conductor-setup` - Initialize project context (once per project)
- `/conductor-newtrack` - Create spec + plan + beads from design

**Execution:**
- `bd ready --json` - Find available work
- `/conductor-implement` - Execute epic with TDD checkpoints (use `--no-tdd` to disable)
- `tdd` - Enter TDD mode (RED-GREEN-REFACTOR)
- `finish branch` - Finalize and merge/PR

**Maintenance:**
- `/conductor-revise` - Update spec/plan mid-implementation
- `/conductor-finish` - Complete track (learnings, context refresh, archive)

**Beads:**
- `fb` - File beads from plan
- `rb` - Review beads
- `bd status` - Show ready + in_progress

### Session Protocol

**First message (automatic handoff load):**
On first message of any session, before processing request:
1. Check `conductor/handoffs/` for recent handoffs (< 7 days)
2. If found, auto-load and display: `📋 Prior session context: [track] (Xh ago)`
3. Skip if user says "fresh start" or no conductor/ exists

**Preflight (automatic):**
Conductor commands run preflight automatically:
- Checks `bd` availability (HALT if unavailable)
- Detects mode (SA/MA) and locks for session
- Creates session state file
- Recovers pending operations from crashed sessions

**Start:**
```bash
bd ready --json                      # Find work
bd show <id>                         # Read context
bd update <id> --status in_progress  # Claim
```

**During Session:**
- Heartbeat updates every 5 minutes (automatic)
- TDD checkpoints tracked by default (use `--no-tdd` to disable)
- Close tasks with reason: `completed`, `skipped`, or `blocked`

**End:**
```bash
bd update <id> --notes "COMPLETED: X. NEXT: Y"
bd close <id> --reason completed     # Close current task
bd sync                              # Sync to git
```

### Session Lifecycle

Session continuity is **automatic** via Conductor workflow entry points (`ds`, `/conductor-implement`, `/conductor-finish`). No manual commands needed.

> Details in project AGENTS.md.

### Critical Rules

- Use `--json` with `bd` for structured output
- Use `--robot-*` with `bv` (bare `bv` hangs)
- Never write production code without failing test first
- Always commit `.beads/` with code changes

> **Note:** Skills work without CLI tools. Use `TodoWrite` for session-local tracking if `bd` is not installed.

For complete workflow guide, see [TUTORIAL.md](../TUTORIAL.md).

<!-- END maestro-config -->

---

## Village Block (if `.beads-village/` exists)

<!-- BEGIN maestro-village -->

### Beads Village

MCP server for multi-agent coordination via `npx beads-village`.

**Session Start:**
```bash
bv --robot-status  # Check team state
```

**Tools:** `init`, `claim`, `done`, `reserve`, `release`, `msg`, `inbox`, `status`

**Paths:** `.beads-village/`, `.reservations/`, `.mail/`

<!-- END maestro-village -->

---

## Amp Handoff Protocol

Amp requires manual handoffs. Use workflow commands for automatic handling:

| Workflow | Command | Auto-Handoff |
|----------|---------|--------------|
| Design → Track | `ds` → `/conductor-newtrack` | ✅ |
| Implementation | `/conductor-implement` | ✅ |
| Finish | `/conductor-finish` | ✅ |

Manual alternative: `/create_handoff <type>` at each phase.

### Legacy → New (Historical Reference)

| Old (Deprecated) | New |
|-----|-----|
| `continuity load/save/handoff` | `/resume_handoff`, `/create_handoff manual` |
| `continuity status` | `conductor/handoffs/<track>/index.md` |
