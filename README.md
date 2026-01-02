# Maestro

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Context-driven development for AI coding agents.** Plan first, code once.

A skill plugin for structured AI-assisted development: persistent memory (Beads), structured planning (Conductor), and TDD methodology—refined daily through real builds.

---

## Quick Install

**Claude Code:**
```
/plugin install https://github.com/ReinaMacCredy/maestro
```

**Amp:**
```bash
amp skill add https://github.com/ReinaMacCredy/maestro --global
```

**Codex:**
```bash
curl -fsSL https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/scripts/install-codex.sh | bash
```

→ Full installation: [SETUP_GUIDE.md](./SETUP_GUIDE.md)

---

## Quick Start

```
/conductor-setup              # Initialize project (once)
ds                            # Design session → design.md
/conductor-newtrack           # Create spec + plan + beads
/conductor-implement          # Execute with TDD
```

→ Complete guide: [TUTORIAL.md](./TUTORIAL.md)

---

## Skills

| Skill | Trigger | Purpose |
|-------|---------|---------|
| **conductor** | `/conductor-*` | Structured planning, TDD execution, handoffs |
| **design** | `ds` | Double Diamond design sessions with A/P/C checkpoints |
| **beads** | `fb`, `rb`, `bd` | Persistent issue tracking across sessions |
| **orchestrator** | `/conductor-orchestrate` | Multi-agent parallel execution |
| **using-git-worktrees** | `/worktree` | Isolated development environments |
| **writing-skills** | — | Skill creation guide |
| **sharing-skills** | — | Contribute skills upstream |

---

## Key Rules

- Use `--json` with `bd` for structured output
- Never write production code without failing test first
- Always commit `.beads/` with code changes

---

## Documentation

| Document | Description |
|----------|-------------|
| [SETUP_GUIDE.md](./SETUP_GUIDE.md) | Full installation instructions |
| [TUTORIAL.md](./TUTORIAL.md) | Complete workflow guide |
| [REFERENCE.md](./REFERENCE.md) | Commands, triggers, troubleshooting |
| [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) | System architecture and pipeline |

---

## Credits

Built on:
- [BMAD-METHOD](https://github.com/bmad-code-org/BMAD-METHOD) — Multi-agent orchestration
- [conductor](https://github.com/NguyenSiTrung/conductor) — Context-driven planning
- [beads](https://github.com/steveyegge/beads) — Persistent issue tracking
- [Agent Mail](https://github.com/Dicklesworthstone/mcp_agent_mail) — Multi-agent coordination
---

## License

MIT
