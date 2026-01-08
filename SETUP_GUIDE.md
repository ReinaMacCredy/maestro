# Maestro Setup Guide

<!--
AGENT-QUICK-SETUP

1. Install plugin using tool's method (see table below)
2. Apply config block to user's global config file
3. Check `bd --version` - if missing, install CLI tools
4. Verify: skills loaded, bd available, Agent Mail configured
5. Run `ds` to confirm skill loading works
-->

## Plugin Installation

| Tool | Command |
|------|---------|
| **Claude Code** | `/plugin install https://github.com/ReinaMacCredy/maestro` |
| **Amp** | `amp skill add https://github.com/ReinaMacCredy/maestro --global` |
| Codex | `git clone https://github.com/ReinaMacCredy/maestro ~/.codex/plugins/maestro` |
| Cursor | `git clone https://github.com/ReinaMacCredy/maestro ~/.cursor/plugins/maestro` |
| Gemini CLI | `git clone https://github.com/ReinaMacCredy/maestro ~/.gemini/plugins/maestro` |
| Manual | `git clone https://github.com/ReinaMacCredy/maestro <skill-directory>` |

---

## Global Config

Apply the config block below to your agent's global config file:

| Tool | Config Path |
|------|-------------|
| Claude Code | `~/.claude/CLAUDE.md` |
| Amp | `~/.config/amp/AGENTS.md` |
| Codex | `~/.codex/AGENTS.md` |
| Gemini CLI | `~/.gemini/GEMINI.md` |
| Cursor | `~/.cursor/rules/maestro.md` |

Replace any existing `<!-- BEGIN maestro-config -->` block or append:

<!-- BEGIN maestro-config -->
<!-- version: 2.2.0 | bmad-v6 | 2026-01-09 -->

## Maestro Workflow

Context-driven development with TDD execution.

**First message:** Check `conductor/handoffs/` for prior session context.

### Project Detection

Maestro project if any exist:
- `conductor/` directory
- `.beads/` directory

When detected, use Conductor commands instead of ad-hoc planning.

### Triggers

| Category | Triggers |
|----------|----------|
| **Design** | `ds` (design session), `cn` (new track), `pl` (planning) |
| **Execute** | `ci` (implement), `co` (orchestrate), `ca` (autonomous), `tdd` |
| **Track** | `fb` (file beads), `rb` (review beads), `bd *` (beads CLI) |
| **Session** | `ho` (handoff), `finish branch` |

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
bd close <id> --reason completed
bd sync
```

### Critical Rules

- Use `--json` with `bd` for structured output
- Use `--robot-*` with `bv` (bare `bv` hangs)
- Never write production code without failing test first
- Always commit `.beads/` with code changes

<!-- END maestro-config -->

---

## CLI Tools Installation

### Beads CLI (bd)

```bash
# Via pip (recommended)
pip install beads-cli

# Or from source
git clone https://github.com/beads-org/beads-cli
cd beads-cli && pip install -e .
```

### System Dependencies

```bash
# macOS
brew install jq

# Ubuntu/Debian
sudo apt install jq
```

Verify installation:
```bash
bd --version
# Expected: bd version 0.x.x
```

---

## MCP Server Config

### Agent Mail (multi-agent coordination)

**Claude Code:**
```bash
claude mcp add agent-mail -s user -- npx @anthropic-ai/agent-mail
```

**Amp** (`~/.config/amp/settings.json`):
```json
{
  "mcpServers": {
    "agent-mail": {
      "command": "npx",
      "args": ["@anthropic-ai/agent-mail"]
    }
  }
}
```

**Codex** (`~/.codex/mcp.json`):
```json
{
  "servers": {
    "agent-mail": {
      "command": "npx",
      "args": ["@anthropic-ai/agent-mail"]
    }
  }
}
```

---

## MCPorter Toolboxes

CLI wrappers generated from MCP servers via [MCPorter](https://github.com/steipete/mcporter).

### Location

```
toolboxes/
└── agent-mail/
    └── agent-mail.js    # CLI wrapper for Agent Mail
```

### Usage

```bash
# From project root
toolboxes/agent-mail/agent-mail.js <command> [args...]

# Examples
toolboxes/agent-mail/agent-mail.js health-check
toolboxes/agent-mail/agent-mail.js send_message to:BlueLake subject:"Hello"
```

### When to Use

- Agent Mail MCP unavailable but CLI needed
- Scripting multi-agent coordination
- Debugging Agent Mail connectivity

---

## Verification Checklist

| Check | Command | Expected |
|-------|---------|----------|
| Plugin loaded | `ds` | Design session starts |
| Beads CLI | `bd --version` | Version output |
| Agent Mail (MCP) | `/mcp` (Claude Code) | `agent-mail` listed |
| Agent Mail (CLI) | `toolboxes/agent-mail/agent-mail.js health-check` | OK response |
| Project structure | `ls conductor/` | `product.md`, `tracks/`, etc. |

---

## Quick Reference

```mermaid
flowchart LR
    A["/conductor-setup"] --> B["ds"]
    B --> C["/conductor-newtrack"]
    C --> D["/conductor-implement"]
    D --> E["/conductor-finish"]
    
    style A fill:#1a1a2e,stroke:#e94560,color:#fff
    style D fill:#1a1a2e,stroke:#0f3460,color:#fff
```

| Action | Command |
|--------|---------|
| Initialize project | `/conductor-setup` |
| Design feature | `ds` |
| Create track | `/conductor-newtrack` or `cn` |
| Execute tasks | `/conductor-implement` or `ci` |
| Parallel workers | `/conductor-orchestrate` or `co` |
| Autonomous mode | `/conductor-autonomous` or `ca` |
| See available work | `bd ready --json` |
| Complete track | `/conductor-finish` |
| Save context | `/conductor-handoff` or `ho` |

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Skills not loading | Check plugin directory exists; run `ds` to test |
| `bd: command not found` | Install via `pip install beads-cli` |
| Agent ignores workflow | Use explicit trigger: `ds`, `tdd`, `ci` |
| Agent Mail not working | Check `/mcp` or run CLI health check |
| Handoff not loading | Check `conductor/handoffs/` exists with recent files |
| `bv` hangs | Always use `bv --robot-stdout` (never bare `bv`) |

> **Note:** Skills work without CLI tools, but `bd` is required for full functionality. See [REFERENCE.md](REFERENCE.md) for fallback policies.

---

## Next Steps

1. Initialize your project: `/conductor-setup`
2. Start your first design: `ds`
3. Read the full tutorial: [TUTORIAL.md](TUTORIAL.md)
4. Quick command lookup: [REFERENCE.md](REFERENCE.md)
