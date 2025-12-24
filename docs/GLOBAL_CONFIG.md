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

## Maestro Workflow

Context-driven development with TDD execution.

### Triggers

**Planning:**
- `ds` or `/conductor-design` - Double Diamond design session
- `/conductor-setup` - Initialize project context (once per project)
- `/conductor-newtrack` - Create spec + plan + beads from design

**Execution:**
- `bd ready --json` - Find available work
- `/conductor-implement` - Execute epic with TDD
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

**Start:**
```bash
bd ready --json                      # Find work
bd show <id>                         # Read context
bd update <id> --status in_progress  # Claim
```

**End:**
```bash
bd update <id> --notes "COMPLETED: X. NEXT: Y"
bd sync
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
