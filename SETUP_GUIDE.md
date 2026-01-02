# Maestro Setup Guide

<!--
AGENT-QUICK-SETUP

1. Install plugin (use tool's method or git clone)
2. Apply config block to user's global config (paths table below)
3. Check `which bd` - if missing, install CLI tools
4. Verify: skills loaded, bd available, Agent Mail configured
5. Report results to user
-->

## Plugin Installation

| Tool | Command |
|------|---------|
| Claude Code | `/plugin install https://github.com/ReinaMacCredy/maestro` |
| Amp | `amp skill add https://github.com/ReinaMacCredy/maestro --global` |
| Codex | `curl -fsSL https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/scripts/install-codex.sh \| bash` |
| Manual | `git clone https://github.com/ReinaMacCredy/maestro.git <skill-directory>` |

**Skill directories:** `~/.claude/skills/maestro` | `~/.config/agents/skills/maestro` | `~/.codex/skills/maestro`

---

## Global Config

Apply config block to user's global config file:

| Tool | Config Path |
|------|-------------|
| Claude Code | `~/.claude/CLAUDE.md` |
| Amp | `~/.config/amp/AGENTS.md` |
| Codex | `~/.codex/AGENTS.md` |
| Gemini CLI | `~/.gemini/GEMINI.md` |
| Cursor | `~/.cursor/rules/maestro.md` |

Replace existing `<!-- BEGIN maestro-config -->` block or append:

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
1. Check `conductor/handoffs/` for recent handoffs (< 7 days)
2. If found, auto-load and display: `📋 Prior session context: [track] (Xh ago)`
3. Skip if user says "fresh start" or no conductor/ exists

**Start:** `bd ready --json` → `bd show <id>` → `bd update <id> --status in_progress`

**End:** `bd update <id> --notes "COMPLETED: X. NEXT: Y"` → `bd close <id> --reason completed` → `bd sync`

### Critical Rules

- Use `--json` with `bd` for structured output
- Never write production code without failing test first
- Always commit `.beads/` with code changes

> **Note:** Skills work without CLI tools. Use `TodoWrite` for session-local tracking if `bd` is not installed.

<!-- END maestro-config -->

---

## CLI Tools Installation

**Beads CLI (bd):**
```bash
curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/mcp_agent_mail/main/scripts/install.sh | bash -s -- --dir "$HOME/mcp_agent_mail" --yes
```

**System dependency:**
```bash
brew install jq        # macOS
sudo apt install jq    # Ubuntu/Debian
```

---

## MCP Server Config

**Agent Mail** (multi-agent coordination):

```bash
# Claude Code
claude mcp add agent-mail -s user -- npx @anthropic-ai/agent-mail

# Amp (~/.config/amp/settings.json)
{ "mcpServers": { "agent-mail": { "command": "npx", "args": ["@anthropic-ai/agent-mail"] } } }
```

---

## Verification Checklist

| Check | Command | Expected |
|-------|---------|----------|
| Plugin loaded | `/skill list` or check skill directory | Skills visible |
| Beads CLI | `bd --version` | Version output |
| Agent Mail MCP | `/mcp` (Claude) | agent-mail listed |

---

## Quick Reference

| Action | Command |
|--------|---------|
| Initialize project | `/conductor-setup` |
| Design feature | `ds` or `/conductor-design` |
| Create track | `/conductor-newtrack` |
| Execute tasks | `/conductor-implement` |
| Enter TDD mode | `tdd` |
| See available work | `bd ready --json` |
| Validate track | `/conductor-validate` |
| Complete track | `/conductor-finish` |

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Skills not loading | Check `/skill list` or skill directory exists |
| `bd: command not found` | Run CLI tools installation above |
| Agent ignores workflow | Use trigger explicitly: `tdd`, `ds`, `/conductor-design` |
| Agent Mail not working | Check `/mcp` shows agent-mail |
| Handoff not loading | Check `conductor/handoffs/` exists with recent files |

**Skills work without CLI** - methodology and workflows still apply, use `TodoWrite` for task tracking.
