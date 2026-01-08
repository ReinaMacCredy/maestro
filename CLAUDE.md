# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Maestro** is a Claude Code plugin for structured AI-assisted development. It combines:
- **Conductor**: Planning methodology producing design.md, spec.md, and plan.md
- **Beads**: Persistent issue tracking with dependency graphs
- **Orchestrator**: Multi-agent parallel execution
- **Design**: Double Diamond brainstorming sessions

This is a skills-based plugin (no build required) - all functionality is delivered through markdown skill definitions in `.claude/skills/`.

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

## Session Close Protocol

Before completing any session, run this checklist:

```bash
git status                  # Check what changed
git add <files>             # Stage code changes
bd sync                     # Commit beads changes
git commit -m "..."         # Commit code
bd sync                     # Commit any new beads changes
git push                    # Push to remote
```

## Commands & Triggers

| Command/Trigger | Description | Preflight |
|-----------------|-------------|-----------|
| `/conductor-setup` | Initialize project (once) | No |
| `ds` | Design session (Double Diamond) | No |
| `/conductor-newtrack` | Create spec + plan + beads from design | No |
| `/conductor-implement` | Execute epic with TDD | Yes |
| `/conductor-orchestrate` | Parallel execution with workers | Yes |
| `/conductor-finish` | Complete track, archive | No |
| `tdd` | Enter RED-GREEN-REFACTOR cycle | No |
| `fb` | File beads from plan.md | No |
| `rb` | Review beads status | No |
| `finish branch` | Complete dev work, merge/PR | No |

## Beads CLI

**Always use `--json` flag with `bd` commands in AI agent context.**

```bash
bd ready --json                          # Find available work
bd show <id>                             # View issue details
bd update <id> --status in_progress      # Claim task
bd close <id> --reason "Completed"       # Complete task
bd sync                                  # Sync to git
```

## Skill Loading Rule

**Always load `maestro-core` FIRST** before any workflow skill for routing table and fallback policies.

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
