# Setup Guide

**For AI agents setting up maestro globally.**

## Step 1: Install Plugin/Skills

### Claude Code

```bash
# Plugin install (recommended)
/plugin install https://github.com/ReinaMacCredy/maestro

# Or via CLI
claude plugin add https://github.com/ReinaMacCredy/maestro.git
```

Verify: `/plugin list` should show `maestro`

### OpenAI Codex

```
$skill-installer ReinaMacCredy/maestro
```

Or install specific skills:
```
$skill-installer conductor from ReinaMacCredy/maestro
$skill-installer beads from ReinaMacCredy/maestro
```

Skills install to `~/.codex/skills/`

### Amp

```bash
# Add via git URL
amp plugin add https://github.com/ReinaMacCredy/maestro.git
```

### Manual (any agent)

Clone skills to your agent's skill directory:
```bash
# Claude Code
git clone https://github.com/ReinaMacCredy/maestro.git ~/.claude/plugins/maestro

# Codex
git clone https://github.com/ReinaMacCredy/maestro.git /tmp/maestro && cp -r /tmp/maestro/skills/* ~/.codex/skills/

# Amp
git clone https://github.com/ReinaMacCredy/maestro.git ~/.config/amp/plugins/maestro
```

### Verify Installation

```
/skill list   # Claude Code / Amp
/skills       # Codex
```

You should see 16 skills: `beads`, `file-beads`, `review-beads`, `codemaps`, `conductor`, `design`, `dispatching-parallel-agents`, `doc-sync`, `finishing-a-development-branch`, `sharing-skills`, `subagent-driven-development`, `test-driven-development`, `using-git-worktrees`, `using-superpowers`, `verification-before-completion`, `writing-skills`.

## Step 2: Install Superpowers Plugin (Recommended)

The **superpowers plugin** provides additional skills for debugging and code review workflows:

- `systematic-debugging`, `root-cause-tracing`, `condition-based-waiting`, `defense-in-depth`
- `requesting-code-review`, `receiving-code-review`
- `brainstorming`, `writing-plans`, `executing-plans`

### Claude Code

```bash
/plugin install https://github.com/obra/superpowers
```

### Amp

```bash
amp plugin add https://github.com/obra/superpowers.git
```

### Manual

```bash
# Claude Code
git clone https://github.com/obra/superpowers.git ~/.claude/plugins/superpowers

# Amp
git clone https://github.com/obra/superpowers.git ~/.config/amp/plugins/superpowers
```

**Source:** https://github.com/obra/superpowers

## Step 3: Install Beads Village

```bash
npx beads-village    # Recommended
# or: npm install -g beads-village
# or: pip install beads-village
```

## Step 4: Install CLI Tools (Optional)

The plugin provides skills (mental models + workflows). For full functionality, install these optional CLI tools:

### System Dependencies

```bash
# jq - required for conductor-implement and beads parsing
# macOS
brew install jq

# Ubuntu/Debian
sudo apt-get install jq

# Fedora
sudo dnf install jq
```

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

## Step 5: Configure MCP Servers (Optional)

### Beads Village - Multi-Agent Coordination

Prerequisites:
- `pip install beads` (beads CLI already installed in Step 2)
- Node.js 16+

**Install for Claude Code:**
```bash
claude mcp add beads-village -s user -- npx beads-village
```

**Install for Amp:**
Add to `~/.config/amp/settings.json`:
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
Add to `~/.codex/config.toml` under `[mcp]`:
```toml
[mcp.beads-village]
command = "npx"
args = ["beads-village"]
```

**Verify:**
```bash
# Check MCP server responds
/mcp  # Should show beads-village with tools: init, claim, done, reserve, release, msg, inbox, status, assign
```

**Source:** https://github.com/LNS2905/mcp-beads-village

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

## Step 6: Configure Global Agent

Add maestro triggers to your global config:

| Tool | Config File |
|------|-------------|
| Claude Code | `~/.claude/CLAUDE.md` |
| Amp | `~/.config/amp/AGENTS.md` |
| Codex | `~/.codex/AGENTS.md` |

**Append** this snippet to your existing global config:

```markdown
## Maestro Workflow

**Planning:** `/conductor-setup` → `/conductor-design` (or `ds`) → `/conductor-newtrack`

**Execution:** `fb` → `rb` → `/conductor-implement` → `tdd` → `finish branch`

**Maintenance:** `/conductor-revise` (update spec/plan), `/conductor-refresh` (sync stale docs)

**Utilities:** `/doc-sync`, `/compact`, `dispatch`, `git worktree`

**Review:** `rb`, `review code`
```

For the full workflow reference, see [docs/GLOBAL_CONFIG_TEMPLATE.md](./docs/GLOBAL_CONFIG_TEMPLATE.md).

## Step 7: Done

```
Global setup complete!

Installed: maestro plugin (global)
Required: bd CLI
Optional: beads-village MCP

Key triggers:
  /conductor-setup           # Initialize project planning (once)
  ds                         # Double Diamond design session (A/P/C, Party Mode)
  /conductor-design "X"      # Same as ds, with description
  /conductor-newtrack "X"    # Create new feature track
  fb                         # File beads from plan
  rb                         # Review beads
  bd ready --json            # See available work
  /conductor-implement       # Execute tasks with TDD
  /conductor-revise          # Update spec/plan mid-track
  /conductor-refresh         # Sync docs with codebase
  tdd                        # Enter TDD mode

Per-project setup (when starting a new project):
  bd init                    # Initialize beads database
  git add .beads/ && git commit -m "Initialize beads"

Next: Read TUTORIAL.md for complete workflow guide.
```

## Quick Reference

| What | How |
|------|-----|
| Install plugin | `/plugin install maestro` |
| Global config template | [docs/GLOBAL_CONFIG_TEMPLATE.md](./docs/GLOBAL_CONFIG_TEMPLATE.md) |
| Initialize planning | `/conductor-setup` |
| Design feature | `/conductor-design "description"` or `ds` |
| New track from design | `/conductor-newtrack` |
| Execute track | `/conductor-implement` |
| View progress | `/conductor-status` |
| Revert work | `/conductor-revert` |
| Revise spec/plan | `/conductor-revise` |
| Refresh stale docs | `/conductor-refresh` |
| See available work | `bd ready --json` |
| Start TDD | Say `tdd` |
| Multi-agent coordination | `init`, `claim`, `done` via beads-village MCP |
| Full documentation | [TUTORIAL.md](./TUTORIAL.md) |

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Skills not loading | Run `/plugin list` to verify installation |
| `bd: command not found` | Install via Agent Mail installer (Step 3) |
| Agent ignores workflow | Use trigger phrase explicitly: `tdd`, `debug`, `/conductor-design` |
| MCP tools not working | Check `/mcp` shows the server, verify API key |

## Without CLI Tools

The plugin still provides value without `bd`:
- Skills work as mental models and methodologies
- Use `TodoWrite` for session-local task tracking
- Track issues manually in GitHub Issues or markdown
- Full TDD, debugging, and code review workflows still apply

**The skills are the methodology; the CLIs are the persistence layer.**
