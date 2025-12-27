# Maestro - Global Configuration

## For Agents

Apply the config block below to user's global config at:

| Tool | Path |
|------|------|
| Claude Code | `~/.claude/CLAUDE.md` |
| Amp | `~/.config/amp/AGENTS.md` |
| Codex | `~/.codex/AGENTS.md` |
| Gemini CLI | `~/.gemini/GEMINI.md` |
| Cursor | `~/.cursor/rules/maestro.md` |

**Steps:**
1. Check for existing `<!-- BEGIN maestro-config -->`. If found, replace content. If not, append.
2. If `.beads-village/` exists, also apply the Village block.

---

## Default Config

<!-- BEGIN maestro-config -->
<!-- version: 2.0.0 | bmad-v6 | 2025-12-27 -->
**Current year is 2025.** Always use 2025 for dates, not 2024.
## Maestro Workflow

Context-driven development with TDD execution.

### Triggers

**Planning:**
- `ds` or `/conductor-design` - Double Diamond design session with A/P/C checkpoints
- `/conductor-setup` - Initialize project context (once per project)
- `/conductor-newtrack` - Create spec + plan + beads from design

**Design Session (A/P/C Checkpoints):**
- `[A]` Advanced - deeper analysis, assumption audit
- `[P]` Party - multi-agent review (25 BMAD agents: Core/BMM/CIS/BMB/BMGD)
- `[C]` Continue - proceed to next phase

**Execution:**
- `bd ready --json` - Find available work
- `/conductor-implement` - Execute epic with TDD
- `/conductor-implement --tdd` - Execute with TDD checkpoints (RED/GREEN/REFACTOR tracking)
- `tdd` - Enter TDD mode (RED-GREEN-REFACTOR)
- `finish branch` - Finalize and merge/PR

**Maintenance:**
- `/conductor-revise` - Update spec/plan mid-implementation
- `/conductor-finish` - Complete track (learnings, context refresh, archive)
- `/doc-sync` - Sync documentation with code changes

**Beads:**
- `fb` - File beads from plan
- `rb` - Review beads
- `bd status` - Show ready + in_progress

### Session Protocol

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
- TDD checkpoints tracked if `--tdd` flag used
- Close tasks with reason: `completed`, `skipped`, or `blocked`

**End:**
```bash
bd update <id> --notes "COMPLETED: X. NEXT: Y"
bd close <id> --reason completed     # Close current task
bd sync                              # Sync to git
```

### Critical Rules

- Use `--json` with `bd` for structured output
- Use `--robot-*` with `bv` (bare `bv` hangs)
- Never write production code without failing test first
- Always commit `.beads/` with code changes

> **Note:** Skills work without CLI tools. Use `TodoWrite` for session-local tracking if `bd` is not installed.

### Project Detection

Maestro project if any exist:
- `conductor/` directory
- `.beads/` directory

When detected, use Conductor commands instead of ad-hoc planning.

For complete workflow guide, see [TUTORIAL.md](../TUTORIAL.md).

<!-- END maestro-config -->

---

## Optional: Beads Village (Multi-Agent)

Apply this block only if `.beads-village/` exists.

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

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Triggers not working | Verify plugin installed: `/plugin list` |
| `bd: command not found` | See SETUP_GUIDE.md Step 4 |
| `bv` hangs | Use `--robot-*` flags |
| Skills not loading | Check plugin settings |

---

## Amp-Specific: Continuity Protocol

Amp doesn't support automatic hooks like Claude Code. Add this section to `~/.config/amp/AGENTS.md`:

```markdown
## Continuity Protocol

Amp doesn't support automatic hooks - use manual commands.

### Session Start
Run `continuity load` at the start of every session.

### After Significant Changes
Run `continuity save` after making significant changes.

### Session End
Run `continuity handoff` before ending the session.

### Commands Reference
| Command | When |
|---------|------|
| `continuity load` | Session start |
| `continuity save` | After milestones |
| `continuity handoff` | Session end |
| `continuity status` | Check state |
| `continuity search <keyword>` | Find past context |
```

