# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Maestro** is a Claude Code plugin for structured AI-assisted development. It combines:
- **Conductor**: Planning methodology producing design.md, spec.md, and plan.md
- **Beads**: Persistent issue tracking with dependency graphs
- **Orchestrator**: Multi-agent parallel execution
- **Design**: Double Diamond brainstorming sessions

This is a skills-based plugin (no build required) - all functionality is delivered through markdown skill definitions in `.claude/skills/`.

## Architecture

```
.claude/skills/            # Skill directories with SKILL.md + optional references/
  ├── conductor/          # Planning + execution + TDD
  ├── orchestrator/       # Multi-agent parallel execution
  ├── design/             # Double Diamond sessions (ds trigger)
  ├── beads/              # Issue tracking (fb, rb triggers)
  ├── maestro-core/       # Routing policies, fallback rules
  ├── handoff/            # Session context persistence
  └── ...                 # Additional specialized skills

.claude-plugin/           # Plugin manifest (plugin.json)
hooks/                    # Lifecycle hooks (session-start.sh)
conductor/                # Per-project context (created by /conductor-setup)
.beads/                   # Issue database (git-tracked)
```

**Key insight**: Beads operations are abstracted behind Conductor (facade pattern). In the happy path, you use `/conductor-*` commands and beads are managed automatically.

## Validation

```bash
cat .claude-plugin/plugin.json | jq .   # Validate plugin manifest
```

## Development Workflow

**No build or tests required.** All functionality is in markdown skill files.

When modifying skills:
1. Edit SKILL.md in `.claude/skills/<skill-name>/`
2. Ensure YAML frontmatter `name` matches directory name
3. Keep skills self-contained with minimal cross-references
4. Add supporting docs to `references/` subdirectory

Skill structure:
```
.claude/skills/<skill-name>/
├── SKILL.md          # YAML frontmatter + markdown instructions
└── references/       # Optional supporting documentation
```

## Commands & Triggers

| Command/Trigger | Description |
|-----------------|-------------|
| `/conductor-setup` | Initialize project (once) |
| `ds` | Design session (Double Diamond) → `design.md` |
| `/conductor-newtrack` | Create spec + plan + beads from design |
| `/conductor-implement` | Execute epic with TDD |
| `/conductor-orchestrate` | Parallel execution with workers |
| `/conductor-finish` | Complete track, archive |
| `tdd` | Enter RED-GREEN-REFACTOR cycle |
| `fb` | File beads from plan.md |
| `rb` | Review beads status |
| `finish branch` | Complete dev work, merge/PR |

## Beads CLI

**Always use `--json` flag with `bd` commands in AI agent context.**

```bash
bd ready --json                          # Find available work
bd show <id>                             # View issue details
bd update <id> --status in_progress      # Claim task
bd close <id> --reason "Completed"       # Complete task
bd sync                                  # Sync to git
```

## Fallback Policy

| Condition | Action |
|-----------|--------|
| `bd` unavailable | HALT |
| `conductor/` missing | DEGRADE (standalone mode) |
| Agent Mail unavailable | HALT |

## Code Quality

- No emojis in code or documentation
- Conventional commits: `feat:`, `fix:`, `test:`, `refactor:`, `docs:`
- TDD for implementation (watch tests fail before implementing)
- Skills must be self-contained

## Versioning

- Plugin version in `.claude-plugin/plugin.json` is auto-bumped by CI
- Skill versions are manually updated in SKILL.md frontmatter
- Pre-1.0: breaking changes bump MINOR, not MAJOR

## Related Documentation

- [REFERENCE.md](REFERENCE.md) - Full command reference and troubleshooting
- [AGENTS.md](AGENTS.md) - Decision trees, session protocol, skill discipline
- [TUTORIAL.md](TUTORIAL.md) - Complete workflow walkthrough
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - System architecture
