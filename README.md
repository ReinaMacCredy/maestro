# Maestro

> Context-driven development for AI coding agents. Plan first, code once.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## Install

**Claude Code:**
```bash
/plugin install https://github.com/ReinaMacCredy/maestro
```

**Amp:**
```bash
amp skill add https://github.com/ReinaMacCredy/maestro --global
```

<details>
<summary>Other agents (Codex, Cursor, Gemini CLI)</summary>

**Codex CLI:**
```bash
# Add to .codex/AGENTS.md
git clone https://github.com/ReinaMacCredy/maestro ~/.codex/plugins/maestro
```

**Cursor:**
```bash
# Copy skills to .cursor/skills/
git clone https://github.com/ReinaMacCredy/maestro ~/.cursor/plugins/maestro
```

**Gemini CLI:**
```bash
# Add to GEMINI.md
git clone https://github.com/ReinaMacCredy/maestro ~/.gemini/plugins/maestro
```

</details>

## Quick Start

```mermaid
graph LR
    A[@plan] --> B[Interview]
    B --> C[Plan Generated]
    C --> D[/atlas-work]
    D --> E[Implementation]
```

1. **`@plan`** — Start an interview-driven planning session (Prometheus)
2. **Review plan** — Metis identifies gaps, Momus reviews for quality
3. **`/atlas-work`** — Execute via orchestrator with specialized agents
4. **Verify** — Wisdom accumulated, learnings extracted

## Skills & Agents

| Skill | Triggers | Purpose |
|-------|----------|---------|
| **atlas** | `@plan`, `/atlas-plan`, `/atlas-work` | Interview planning, orchestrated execution |
| **orchestration** | `/atlas-work` | Task()-based delegation |
| **git-master** | Atomic commits | Git operations specialist |
| **playwright** | E2E tests | Browser automation |

### Atlas Agents

| Agent | Purpose | Trigger |
|-------|---------|---------|
| `atlas-prometheus` | Strategic planner, interview mode | `@plan` |
| `atlas-orchestrator` | Master delegator (never works directly) | `/atlas-work` |
| `atlas-leviathan` | Focused task executor | (orchestrator delegates) |
| `atlas-kraken` | TDD implementation | `@tdd` |
| `atlas-spark` | Quick fixes | (orchestrator delegates) |
| `atlas-oracle` | Strategic advisor (opus) | `@oracle` |
| `atlas-explore` | Codebase search | `@explore` |
| `atlas-librarian` | External docs/research | `@librarian` |
| `atlas-metis` | Pre-planning consultant | `@metis` |
| `atlas-momus` | Plan reviewer | `@momus` |
| `atlas-code-reviewer` | Code quality review | `@review` |
| `atlas-document-writer` | Technical documentation | `@docs` |

## Key Rules

- **Interview before code** — Run `@plan` to explore before implementing
- **TDD by default** — Never write production code without a failing test
- **Beads track work** — Use `bd` CLI for persistent task management
- **Orchestrator delegates** — Never works directly, always spawns agents
- **Verify subagent claims** — Always verify, agents can make mistakes

## Documentation

| Topic | Path |
|-------|------|
| Tutorial | [TUTORIAL.md](TUTORIAL.md) |
| Setup Guide | [SETUP_GUIDE.md](SETUP_GUIDE.md) |
| Reference | [REFERENCE.md](REFERENCE.md) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |
| Atlas Workflow | [.claude/skills/atlas/SKILL.md](.claude/skills/atlas/SKILL.md) |
| Agent Definitions | [.claude/skills/atlas/references/agents/](.claude/skills/atlas/references/agents/) |

## Credits

Maestro builds on the shoulders of giants:

- **[BMAD-METHOD](https://github.com/bmadcode/BMAD-METHOD)** — Multi-agent design methodology
- **[conductor](https://github.com/cyanheads/conductor)** — Context-driven development patterns
- **[beads](https://github.com/beads-org/beads)** — Issue tracking for AI agents
- **[Agent Mail](https://github.com/agent-mail/agent-mail)** — Multi-agent coordination

## License

[MIT](LICENSE)
