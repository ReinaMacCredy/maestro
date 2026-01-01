# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Maestro** is a Claude Code plugin providing structured AI-assisted development workflows. It combines:
- **Conductor**: Structured planning methodology that produces design.md, spec.md and plan.md files
- **Beads**: Persistent issue tracking across sessions with dependency graphs
- **Orchestrator**: Multi-agent parallel execution with worker dispatch
- **TDD/Verification**: Integrated execution with validation gates

This is a skills-based plugin (no build required) - all functionality is delivered through markdown skill definitions.

## Architecture

### Directory Structure

```
skills/                    # 8 skill directories, each with SKILL.md
  ├── conductor/          # Planning + execution + research protocol (references/)
  ├── orchestrator/       # Multi-agent parallel execution (references/)
  ├── design/             # Double Diamond sessions (ds trigger) with bmad/
  ├── beads/              # Issue tracking (fb, rb triggers) with references/
  ├── using-git-worktrees/
  ├── writing-skills/
  └── sharing-skills/

.claude-plugin/          # Plugin metadata
  ├── plugin.json        # Plugin manifest
  └── marketplace.json   # Marketplace configuration

hooks/                   # Lifecycle hooks
  ├── hooks.json         # Hook configuration
  └── session-start.sh   # SessionStart hook

lib/                     # Shared utilities
  └── skills-core.js     # Skill utility functions

conductor/               # Project context + tracks (created per-project)
  ├── product.md
  ├── tech-stack.md
  ├── workflow.md
  ├── tracks.md
  ├── CODEMAPS/          # Architecture documentation (auto-regenerated)
  │   └── overview.md    # System architecture overview
  ├── handoffs/          # Session handoffs (git-committed)
  ├── archive/           # Completed tracks
  └── tracks/<id>/       # Active feature/bug tracks
      ├── design.md      # High-level design (from ds)
      ├── spec.md        # Requirements + acceptance criteria
      ├── plan.md        # Phased task list
      └── metadata.json  # Track state, thread IDs, validation

.beads/                  # Beads issue database (git-tracked)
```

### Skill Hierarchy

| Level | Skill | Role |
|-------|-------|------|
| 1 | conductor | Track orchestration, research protocol |
| 2 | orchestrator | Multi-agent parallel execution |
| 3 | design | Double Diamond sessions |
| 4 | beads | Issue tracking |
| 5 | specialized | worktrees, sharing, writing |

Routing and fallback policies are defined in [AGENTS.md](AGENTS.md).

### Workflow Integration

The plugin uses a **session-based workflow**:

**Session 1 (Planning):**
```
ds → design.md → /conductor-newtrack → spec.md + plan.md + beads + review → HANDOFF
```

**Session 2+ (Execution per Epic):**
```
/resume_handoff → /conductor-implement → TDD → verify → close → HANDOFF
```

**Or Parallel Execution:**
```
/conductor-orchestrate → spawns worker agents → parallel TDD → merge
```

## Key Commands

### Validation
```bash
cat .claude-plugin/plugin.json | jq .   # Validate plugin manifest
bd status --json                        # Check beads database status
bd ready --json                         # List available work
bd show <issue-id>                      # Show issue details
```

### Development Workflow

**This is a skills/documentation plugin - no build or tests required.**

When modifying skills:
1. Edit SKILL.md in the skill directory
2. Ensure YAML frontmatter matches directory name
3. Keep skills self-contained
4. Follow existing patterns in other skills

When adding new skills:
1. Create SKILL.md in `skills/<skill-name>/`
2. Add references/ subdirectory for detailed documentation
3. Follow existing skill patterns

### Beads Integration

**Critical: Always use `--json` flag with `bd` commands in AI agent context**

```bash
# Finding work
bd ready --json              # Unblocked tasks
bd blocked --json            # Tasks waiting on dependencies
bd list --status in_progress --json  # Active work

# Working with issues
bd show <issue-id>           # Full issue details with notes
bd update <id> --status in_progress  # Claim task
bd close <id> --reason "Completed"   # Complete task

# Dependencies
bd dep add <child> <blocker> --type blocks  # Add dependency
bd dep tree <id>             # Show dependency graph
```

**Never use bare `bv` command** - it launches TUI and will hang. Always use `bv --robot-*` flags.

### Conductor Commands

| Command | Shortcut | Description |
|---------|----------|-------------|
| `/conductor-setup` | - | Initialize project planning (once) |
| `/conductor-design` | `ds` | Design through Double Diamond dialogue |
| `/conductor-newtrack` | - | Create spec + plan + beads from design |
| `/conductor-implement` | `ci` | Execute ONE EPIC with TDD |
| `/conductor-orchestrate` | `co` | Execute tracks in parallel with workers |
| `/conductor-finish` | `cf` | Complete track: learnings, archive |
| `/conductor-status` | - | View progress overview |
| `/conductor-revise` | - | Update spec/plan mid-implementation |
| `/conductor-revert` | - | Git-aware revert of work |
| `/conductor-validate` | - | Validate track health and state |
| `/conductor-block` | - | Mark task as blocked |
| `/conductor-skip` | - | Skip task with documented reason |

### Handoff Commands

| Command | Description |
|---------|-------------|
| `/create_handoff` | Create handoff file with current context |
| `/resume_handoff` | Find and load most recent handoff |

### Research Protocol

| Command | Description |
|---------|-------------|
| `/research` | Verify patterns against codebase |

Research runs automatically at design phase transitions using parallel agents: Locator, Analyzer, Pattern, Web, Impact.

### Git Integration

```bash
git add .beads/ && git commit -m "Update beads"  # Commit beads with code
bd sync                                           # Sync beads to git
git add -A && git commit && git push             # Session end protocol
```

## Workflow Triggers

| Trigger | Skill | Use When |
|---------|-------|----------|
| `ds` | design | Start Double Diamond design session |
| `/conductor-newtrack` | conductor | Create spec + plan from design |
| `/conductor-implement` | conductor | Execute ONE EPIC from track's plan |
| `/conductor-orchestrate` | orchestrator | Execute tracks in parallel |
| `/conductor-finish` | conductor | Complete track and archive |
| `fb` | beads | Convert plan to beads issues |
| `rb` | beads | Review/refine filed beads |
| `tdd` | conductor | Enter TDD mode (RED-GREEN-REFACTOR) |
| `finish branch` | conductor | Complete and merge work |

## Important Patterns

### Skill Structure
Each skill follows a consistent pattern:
- **SKILL.md**: YAML frontmatter (`name`, `description`, `version`) + markdown instructions
- **Optional references/**: Supporting documentation (single source of truth)
- Skills are self-contained, minimal cross-references

### Validation Gates

5 validation gates integrated into the lifecycle:

| Gate | Trigger Point | Enforcement |
|------|---------------|-------------|
| design | After DELIVER phase | SPEED=WARN, FULL=HALT |
| spec | After spec.md generation | WARN |
| plan-structure | After plan.md generation | WARN |
| plan-execution | After TDD REFACTOR | SPEED=WARN, FULL=HALT |
| completion | Before /conductor-finish | SPEED=WARN, FULL=HALT |

Max 2 retries before escalating to human review.

### Fallback Policy

| Condition | Action |
|-----------|--------|
| `bd` unavailable | HALT |
| `conductor/` missing | DEGRADE |
| Village MCP unavailable | DEGRADE |

### Skill Naming
- Directory names: kebab-case (`test-driven-development`)
- SKILL.md frontmatter `name` field must match directory name
- Triggers can be shorthand (`fb` for file beads, `rb` for review beads)

### TDD Methodology
**Iron law**: No production code without a failing test first.
- RED: Write failing test
- GREEN: Minimal code to pass
- REFACTOR: Clean up while staying green

TDD is auto-enabled in `/conductor-implement`. Use `--no-tdd` to disable.

### Session Resumability
Beads survive context compaction. Store recovery context in issue notes:
```bash
bd update <id> --notes "COMPLETED: X. IN PROGRESS: Y. NEXT: Z"
```

### CODEMAPS
Architecture documentation in `conductor/CODEMAPS/` is auto-regenerated by `/conductor-finish` (Phase 6).

## Code Quality Standards

- **No emojis** in code, comments, variable names, or documentation
- **Conventional commits**: `feat:`, `fix:`, `test:`, `refactor:`, `docs:`
- **TDD for all implementation** - watch tests fail before implementing
- **Verification before completion** - run tests/builds before claiming done
- Skills are self-contained, avoid tight coupling

## Related Documentation

- [README.md](README.md) - Full plugin overview and usage guide
- [SETUP_GUIDE.md](SETUP_GUIDE.md) - Installation instructions
- [TUTORIAL.md](TUTORIAL.md) - Complete workflow guide with examples
- [AGENTS.md](AGENTS.md) - Project-specific agent instructions
- [docs/GLOBAL_CONFIG.md](docs/GLOBAL_CONFIG.md) - Global agent configuration
- [conductor/CODEMAPS/overview.md](conductor/CODEMAPS/overview.md) - Architecture overview
