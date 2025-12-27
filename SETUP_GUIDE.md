# Setup Guide

**For AI agents setting up Maestro.**

<!--
AGENT-QUICK-SETUP

Execute these steps in order:

1. Install plugin:
   - Claude Code: /plugin install https://github.com/ReinaMacCredy/maestro
   - Amp: amp skill add https://github.com/ReinaMacCredy/maestro --global
   - Codex: git clone https://github.com/ReinaMacCredy/maestro.git ~/.codex/skills/maestro
   - Other: git clone to skill directory

2. Read docs/GLOBAL_CONFIG.md, apply config block to user's global config

3. Check if `bd` CLI exists (run: which bd)
   - If exists: Setup complete
   - If not: Tell user to install CLI tools manually

4. Report what was installed and what needs manual installation
-->

## Step 1: Install Plugin

Install using your tool's method:

| Tool | Command |
|------|---------|
| Claude Code | `/plugin install https://github.com/ReinaMacCredy/maestro` |
| Amp | `amp skill add https://github.com/ReinaMacCredy/maestro --global` |
| Codex | `git clone https://github.com/ReinaMacCredy/maestro.git ~/.codex/skills/maestro` |

Quick install/update (Codex, one command):
```bash
curl -fsSL https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/scripts/install-codex.sh | bash
```

To update later (Codex):
```bash
git -C ~/.codex/skills/maestro pull
```

Restart Codex to pick up new skills.

**Manual fallback** (any tool):
```bash
git clone https://github.com/ReinaMacCredy/maestro.git <skill-directory>
```

Skill directories:
- Claude Code: `~/.claude/skills/maestro`
- Amp: `~/.config/agents/skills/maestro`
- Codex: `~/.codex/skills/maestro`

**Verify:** Run `/skill list` or check skill directory exists.

## Step 2: Configure Global Agent

Read [docs/GLOBAL_CONFIG.md](./docs/GLOBAL_CONFIG.md) and apply the config block to user's global config.

The file contains:
- Config paths for each tool
- Default config block with BEGIN/END markers
- Optional Beads Village block

## Step 3: Check CLI Tools

Check if `bd` CLI is installed:
```bash
which bd && echo "CLI installed" || echo "CLI not found"
```

If not found, inform user to install manually (see Manual Installation below).

## Step 4: Report Results

Tell user:
```
Maestro setup complete.

Installed:
- Maestro plugin (skills loaded)
- Global config updated

For full functionality, install CLI tools:
  curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/mcp_agent_mail/main/scripts/install.sh | bash -s -- --dir "$HOME/mcp_agent_mail" --yes

Skills work without CLI - use TodoWrite for session tracking.

Next: Run /conductor-setup in your project to initialize planning.
```

---

<!-- HUMAN-ONLY: Manual installation reference below -->

## Manual Installation

### CLI Tools

**Beads CLI (bd, bv)** - Persistent issue tracking:
```bash
curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/mcp_agent_mail/main/scripts/install.sh | bash -s -- --dir "$HOME/mcp_agent_mail" --yes
```

Verify:
```bash
bd --version && bv --version
```

**System dependencies:**
```bash
# jq - required for beads parsing
brew install jq        # macOS
sudo apt install jq    # Ubuntu/Debian
```

**Other tools (optional):**
```bash
# CASS - session search
curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/coding_agent_session_search/main/install.sh | bash -s -- --easy-mode

# UBS - bug scanner
curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/ultimate_bug_scanner/master/install.sh | bash -s -- --easy-mode
```

### MCP Servers

**Beads Village** - Multi-agent coordination:

```bash
# Install package
npx beads-village

# Add to Claude Code
claude mcp add beads-village -s user -- npx beads-village

# Add to Amp (~/.config/amp/settings.json)
{
  "mcpServers": {
    "beads-village": {
      "command": "npx",
      "args": ["beads-village"]
    }
  }
}
```

**Enhanced Search (optional):**
```bash
# Warp-Grep - parallel codebase search
claude mcp add morph-fast-tools -s user -e MORPH_API_KEY=<key> -e ALL_TOOLS=true -- npx -y @morphllm/morphmcp

# Exa - real-time web search
claude mcp add exa -s user -e EXA_API_KEY=<key> -- npx -y @anthropic-labs/exa-mcp-server
```

---

## Quick Reference

| What | How |
|------|-----|
| Install plugin | See Step 1 above |
| Global config | [docs/GLOBAL_CONFIG.md](./docs/GLOBAL_CONFIG.md) |
| Initialize planning | `/conductor-setup` |
| Design feature | `ds` or `/conductor-design` |
| Create track | `/conductor-newtrack` |
| Execute tasks | `/conductor-implement` |
| Validate track health | `/conductor-validate` |
| Enter TDD mode | `tdd` |
| See available work | `bd ready --json` |
| Full documentation | [TUTORIAL.md](./TUTORIAL.md) |

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Skills not loading | Check `/skill list` or skill directory |
| `bd: command not found` | Install CLI tools (see Manual Installation) |
| Agent ignores workflow | Use trigger explicitly: `tdd`, `ds`, `/conductor-design` |
| MCP not working | Check `/mcp` shows the server |

## Without CLI Tools

The plugin works without `bd`:
- Skills provide methodology and workflows
- Use `TodoWrite` for session-local task tracking
- Track issues manually in GitHub Issues or markdown

**The skills are the methodology; the CLIs are the persistence layer.**

---

## Understanding Handoff

**Handoff** is how work context transfers between AI agent sessions. Since sessions have limited memory and can end unexpectedly, handoff ensures your progress survives.

### The Problem

- AI context windows are limited and get compacted
- Sessions crash, timeout, or get closed
- Complex features take multiple sessions to complete

### The Solution (Beads-Conductor Integration)

With PR #6, Conductor commands **automatically manage beads** via a facade pattern. Zero manual `bd` commands in the happy path:

```
Session 1 (Planning):
  ds → design.md
  /conductor-newtrack → spec.md + plan.md + AUTO: epic + issues
  → HANDOFF (auto-generated)

Session 2+ (Execution):
  /conductor-implement → AUTO: claim → TDD → close → sync
  ...repeat until all epics done
```

### Dual-Mode Architecture

| Mode | Description | Used When |
|------|-------------|-----------|
| **SA** (Single-Agent) | Direct `bd` CLI calls | Default, one agent |
| **MA** (Multi-Agent) | Village MCP server | Parallel agents coordinating |

Mode is detected at session start (preflight) and locked for the session.

### State Files (Auto-Managed)

| File | Location | Purpose |
|------|----------|---------|
| `LEDGER.md` | `conductor/sessions/active/` | Session state in frontmatter (mode, bound_track, bound_bead, tdd_phase) |
| `session-lock_<track>.json` | `.conductor/` | Concurrent session prevention |
| `pending_*.jsonl` | `.conductor/` | Failed operations for replay |

### In Practice

**With Conductor (recommended - zero manual bd):**
```bash
# Start
/conductor-implement    # Auto-claims from beads

# Work (with TDD checkpoints)
tdd                     # RED → GREEN → REFACTOR (auto-logged)

# End - automatic sync on session close
```

**Manual fallback:**
```bash
# End of session: save context
bd update <id> --notes "COMPLETED: X. NEXT: Y."
git commit -am "progress" && git push

# Start of session: resume
bd ready --json    # See what's unblocked
bd show <id>       # Read notes for context
```

For detailed explanation, see [docs/manual-workflow-guide.md](./docs/manual-workflow-guide.md#understanding-handoff).
