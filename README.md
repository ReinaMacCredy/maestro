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
    A[ds] --> B[/conductor-newtrack]
    B --> C[/conductor-implement]
    C --> D[/conductor-finish]
```

1. **`ds`** — Start a design session (Double Diamond methodology)
2. **`/conductor-newtrack`** — Generate spec, plan, and beads from design
3. **`/conductor-implement`** — Execute with TDD checkpoints
4. **`/conductor-finish`** — Archive track and extract learnings

## Skills

| Skill | Trigger | Purpose |
|-------|---------|---------|
| **conductor** | `/conductor-*` | Implementation execution, TDD |
| **designing** | `ds` | Double Diamond design sessions |
| **tracking** | `fb`, `rb`, `bd` | Persistent issue tracking |
| **orchestrator** | `/conductor-orchestrate` | Multi-agent parallel execution |
| **handoff** | `/conductor-handoff` | Session context preservation |
| **maestro-core** | *(auto)* | Routing and fallback policies |
| **creating-skills** | — | Skill authoring guide |
| **sharing-skills** | — | Contribute skills upstream |
| **using-git-worktrees** | `/worktree` | Isolated dev environments |

## Key Rules

- **Design before code** — Run `ds` to explore before implementing
- **TDD by default** — Never write production code without a failing test
- **Beads track work** — Use `bd` CLI for persistent task management
- **Handoffs preserve context** — Session state survives across restarts
- **One question at a time** — Design sessions ask focused questions

## Documentation

| Topic | Path |
|-------|------|
| Tutorial | [TUTORIAL.md](TUTORIAL.md) |
| Setup Guide | [SETUP_GUIDE.md](SETUP_GUIDE.md) |
| Reference | [REFERENCE.md](REFERENCE.md) |
| Changelog | [CHANGELOG.md](CHANGELOG.md) |
| Workflow Chain | [skills/maestro-core/references/workflow-chain.md](.claude/skills/maestro-core/references/workflow-chain.md) |
| Routing Table | [skills/maestro-core/references/routing-table.md](.claude/skills/maestro-core/references/routing-table.md) |

## Credits

Maestro builds on the shoulders of giants:

- **[BMAD-METHOD](https://github.com/bmadcode/BMAD-METHOD)** — Multi-agent design methodology
- **[conductor](https://github.com/cyanheads/conductor)** — Context-driven development patterns
- **[beads](https://github.com/beads-org/beads)** — Issue tracking for AI agents
- **[Agent Mail](https://github.com/agent-mail/agent-mail)** — Multi-agent coordination

## License

[MIT](LICENSE)
