# Maestro

> AI agent workflow skillpack. Plan first, code once.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## Install

**Universal install (`skills.sh` CLI):**
```bash
npx skills add ReinaMacCredy/maestro
```

**Install specific skills and agents:**
```bash
npx skills add ReinaMacCredy/maestro --skill planning --agent claude-code --agent amp
```

**Legacy direct installs (still supported):**
```bash
# Claude Code plugin route
/plugin install https://github.com/ReinaMacCredy/maestro

# Amp route
amp skill add https://github.com/ReinaMacCredy/maestro --global
```

## Quick Start

```mermaid
graph LR
    A["/maestro:setup"] --> B["/maestro:new-track"]
    B --> C[Spec + Plan Generated]
    C --> D["/maestro:implement"]
    D --> E["/maestro:review"]
```

1. **`/maestro:setup`** — Scaffold project context (product, tech stack, guidelines)
2. **`/maestro:new-track`** — Create a feature/bug track with spec and plan
3. **`/maestro:implement`** — Execute tasks (single-agent or `--team` for parallel)
4. **`/maestro:review`** — Verify implementation correctness

## Setup

Enable Agent Teams in `~/.claude/settings.json`:

```json
{
  "env": {
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
  }
}
```

## Commands

### Core Workflow

| Command | Description |
|---------|-------------|
| `/maestro:setup` | Scaffold project context (product, tech stack, guidelines) |
| `/maestro:new-track <request>` | Create a feature/bug track with spec and plan |
| `/maestro:implement [<track>] [--team]` | Execute track tasks (supports `--resume`) |
| `/maestro:review` | Post-execution review with auto-fix |
| `/maestro:status` | Show track progress across all tracks |
| `/maestro:revert` | Undo implementation if needed |
| `/maestro:note [--priority <P0-P3>] <text>` | Capture decisions and context to persistent notepad |
| `/maestro:AGENTS.md` | Generate AGENTS.md context file |



## Agents

| Agent | Purpose | Model | Team Lead? |
|-------|---------|-------|------------|
| `prometheus` | Interview-driven planner | sonnet | Yes |
| `orchestrator` | Execution coordinator | sonnet | Yes |
| `kraken` | TDD implementation | sonnet | No |
| `spark` | Quick fixes | sonnet | No |
| `build-fixer` | Build/compile/lint error specialist | sonnet | No |
| `oracle` | Strategic advisor | sonnet | No |
| `critic` | Post-implementation reviewer | sonnet | No |
| `security-reviewer` | Security analysis specialist | sonnet | No |
| `explore` | Codebase search | haiku | No |
| `leviathan` | Deep plan reviewer | sonnet | No |
| `wisdom-synthesizer` | Knowledge consolidator | haiku | No |
| `progress-reporter` | Status tracker | haiku | No |

## Key Rules

- **Plan before code** — Run `/maestro:new-track` to create a spec and plan before implementing
- **TDD by default** — Use kraken for new features
- **Track-based workflow** — All work is organized into tracks with specs and plans
- **Verify subagent claims** — Always verify, agents can make mistakes

## Recommended MCP Servers

These MCP servers enhance the Maestro experience:

| Server | Purpose |
|--------|---------|
| [Context7](https://github.com/upstash/context7) | Up-to-date library documentation |
| [Sequential Thinking](https://github.com/modelcontextprotocol/servers/tree/main/src/sequentialthinking) | Dynamic reasoning for complex planning |

## Documentation

| Topic | Path |
|-------|------|
| Skills | [skills/](skills/) |
| Agent Definitions | [.claude/agents/](.claude/agents/) |
| Agent Teams Guide | [docs/AGENT-TEAMS.md](docs/AGENT-TEAMS.md) |
| Universal Skills Format | [docs/AGENT-SKILLS.md](docs/AGENT-SKILLS.md) |

## Credits

- **[BMAD-METHOD](https://github.com/bmadcode/BMAD-METHOD)** — Multi-agent design methodology
- **[conductor](https://github.com/cyanheads/conductor)** — Context-driven development patterns

## License

[MIT](LICENSE)
