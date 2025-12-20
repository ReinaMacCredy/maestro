# Setup Guide

**For AI agents setting up my-workflow.**

## Step 1: Install Plugin

In Claude Code or Amp:

```
/plugin marketplace add ReinaMacCredy/my-workflow
/plugin install my-workflow
```

Verify skills are loaded:
```
/skill list
```

You should see 27 skills including `conductor`, `beads`, `test-driven-development`, etc.

## Step 2: Install CLI Tools (Optional)

The plugin provides skills (mental models + workflows). For full functionality, install these optional CLI tools:

### Beads CLI (`bd`) - Recommended

Persistent issue tracking across sessions:

```bash
# Agent Mail installer (includes bd, bv, am)
curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/mcp_agent_mail/main/scripts/install.sh | bash -s -- --dir "$HOME/mcp_agent_mail" --yes
```

Verify:
```bash
bd --version && bv --version && echo "✓ Beads installed"
```

### Other Tools (Optional)

```bash
# CASS - session search
curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/coding_agent_session_search/main/install.sh | bash -s -- --easy-mode

# UBS - bug scanner
curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/ultimate_bug_scanner/master/install.sh | bash -s -- --easy-mode
```

## Step 3: Initialize Project

```bash
# Initialize beads database (if bd installed)
bd init

# Commit beads database with project
git add .beads/ && git commit -m "Initialize beads"
```

## Step 4: Configure MCP Servers (Optional)

### Beads Village - Multi-Agent Coordination (Required)

Prerequisites:
- `pip install beads` (beads CLI already installed in Step 2)
- Node.js 16+

**Install for Claude Code:**
```bash
claude mcp add beads-village -s user -- npx beads-village
```

**Install for Amp:**
Add to `.amp/settings.json`:
```json
{
  "mcpServers": {
    "beads-village": {
      "command": "npx",
      "args": ["beads-village"]
    }
  }
}
```

**Install for Codex:**
Add to your MCP configuration file:
```json
{
  "beads-village": {
    "command": "npx",
    "args": ["beads-village"]
  }
}
```

**Verify:**
```bash
# Check MCP server responds
/mcp  # Should show beads-village with tools: init, claim, done, reserve, release, msg, inbox, status, assign
```

**Source:** https://github.com/LNS2905/mcp-beads-village

**Add ephemeral directories to .gitignore:**
```bash
# Village creates ephemeral state directories - don't commit these
echo ".reservations/" >> .gitignore
echo ".mail/" >> .gitignore
```

**Key tools:**
| Tool | Purpose |
|------|---------|
| `init` | Join workspace with team/role |
| `claim` | Atomic task claiming |
| `done` | Complete task, release locks |
| `reserve` | Lock file for editing |
| `release` | Release file lock |
| `msg` | Send message to team |
| `inbox` | Read messages |
| `status` | View team state |
| `assign` | (Leader only) Assign tasks |

### Enhanced Search (Optional)

For enhanced search capabilities:

**Get API keys:**
- Morph (Warp-Grep): https://morphllm.com
- Exa (web/code search): https://dashboard.exa.ai

**Install:**
```bash
# Warp-Grep - parallel codebase search
claude mcp add morph-fast-tools -s user -e MORPH_API_KEY=<key> -e ALL_TOOLS=true -- npx -y @morphllm/morphmcp

# Exa - real-time web and code search
claude mcp add exa -s user -e EXA_API_KEY=<key> -- npx -y @anthropic-labs/exa-mcp-server
```

## Step 5: Done

Tell the user:

```
Setup complete!

Installed: my-workflow plugin (27 skills)
Required: beads-village MCP (multi-agent coordination)
Optional: bd, bv, cass, ubs (CLI tools)

Key triggers:
  /conductor-setup           # Initialize project planning
  /conductor-newtrack "X"    # Create new feature track
  bs                         # Brainstorm before implementing
  tdd                        # Enter TDD mode
  debug                      # Systematic debugging
  fb                         # File beads from plan
  bd ready --json            # See available work

Next: Read TUTORIAL.md for complete workflow guide.
```

## Quick Reference

| What | How |
|------|-----|
| Install plugin | `/plugin install my-workflow` |
| Initialize planning | `/conductor-setup` |
| New feature | `/conductor-newtrack "description"` |
| See available work | `bd ready --json` |
| Start TDD | Say `tdd` |
| Debug issue | Say `debug` |
| Multi-agent coordination | `init`, `claim`, `done` via beads-village MCP |
| Full documentation | [TUTORIAL.md](./TUTORIAL.md) |

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Skills not loading | Run `/plugin list` to verify installation |
| `bd: command not found` | Install via Agent Mail installer (Step 2) |
| Agent ignores workflow | Use trigger phrase explicitly: `tdd`, `debug`, `bs` |
| MCP tools not working | Check `/mcp` shows the server, verify API key |

## Without CLI Tools

The plugin still provides value without `bd`:
- Skills work as mental models and methodologies
- Use `TodoWrite` for session-local task tracking
- Track issues manually in GitHub Issues or markdown
- Full TDD, debugging, and code review workflows still apply

**The skills are the methodology; the CLIs are the persistence layer.**
