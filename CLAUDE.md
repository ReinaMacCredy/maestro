# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Maestro** is a Claude Code plugin providing structured AI-assisted development workflows. It combines:
- **Conductor**: Structured planning methodology that produces spec.md and plan.md files
- **Beads**: Persistent issue tracking across sessions with dependency graphs
- **TDD/Debugging/Review**: Systematic development methodologies

This is a skills-based plugin (no build required) - all functionality is delivered through markdown skill definitions.

## Architecture

### Directory Structure

```
skills/                    # 24 skill directories, each with SKILL.md
  ├── beads/              # Issue tracking (main + file-beads/review-beads)
  ├── conductor/          # Planning methodology
  ├── test-driven-development/
  ├── brainstorming/
  ├── systematic-debugging/
  └── ...                 # Other workflow skills

commands/                 # Slash command definitions (.md files)
  ├── conductor/          # /conductor-setup, /conductor-newtrack, etc.
  ├── compact.md
  ├── decompose-task.md
  ├── doc-sync.md
  └── ground.md

workflows/                # Conductor workflow definitions (format-agnostic)
  ├── setup.md           # Project initialization workflow
  ├── newtrack.md        # Track creation workflow
  ├── implement.md       # Task implementation workflow
  ├── status.md          # Progress reporting workflow
  └── schemas/           # JSON schemas for state files

.claude-plugin/          # Plugin metadata
  ├── plugin.json        # Plugin manifest
  └── marketplace.json   # Marketplace configuration

hooks/                   # Lifecycle hooks
  ├── hooks.json         # Hook configuration
  └── session-start.sh   # SessionStart hook (injects using-superpowers)

lib/                     # Shared utilities
  └── skills-core.js     # Skill utility functions

conductor/               # Conductor context (created per-project when initialized)
  ├── product.md
  ├── tech-stack.md
  ├── workflow.md
  ├── tracks.md
  └── tracks/<id>/      # Feature/bug tracks
      ├── spec.md       # Requirements + acceptance criteria
      └── plan.md       # Phased task list

.beads/                  # Beads issue database (git-tracked)
```

### Skill Structure

Each skill follows a consistent pattern:
- **SKILL.md**: YAML frontmatter (`name`, `description`) + markdown instructions
- **Optional references/**: Supporting documentation
- Skills are self-contained, minimal cross-references

### Workflow Integration

The plugin uses a **two-session workflow**:

**Session 1 (Planning):**
```
brainstorm → /conductor-newtrack → fb (file beads) → rb (review beads) → HANDOFF block
```

**Session 2 (Execution):**
```
HANDOFF block → ct (claim task) → tdd → verify → close → finish branch
```

## Key Commands

### Validation
```bash
# Validate plugin manifest
cat .claude-plugin/plugin.json | jq .

# Check beads database status
bd status --json

# List available work
bd ready --json

# Show issue details
bd show <issue-id>
```

### Development Workflow

**This is a skills/documentation plugin - no build or tests required.**

When modifying skills:
1. Edit SKILL.md in the skill directory
2. Ensure YAML frontmatter matches directory name
3. Keep skills self-contained
4. Follow existing patterns in other skills

When adding new commands:
1. Create .md file in `commands/` or `commands/conductor/`
2. Follow existing command structure
3. Reference workflow definitions from `workflows/` when applicable

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

### Conductor Workflow

```bash
# Initialize project planning (once per project)
/conductor-setup

# Create new feature track
/conductor-newtrack "feature description"

# Execute track tasks
/conductor-implement

# Check progress
/conductor-status
```

### Git Integration

```bash
# Beads are git-tracked, commit with code changes
git add .beads/ && git commit -m "Update beads"

# Session end protocol
bd sync                     # Sync beads with remote
git add -A && git commit && git push
```

## Workflow Triggers

| Trigger | Skill | Use When |
|---------|-------|----------|
| `bs` | brainstorming | Deep exploration before implementation |
| `fb` | file-beads | Convert plan to beads issues |
| `rb` | review-beads | Review/refine filed beads |
| `tdd` | test-driven-development | Enter TDD mode |
| `finish branch` | finishing-a-development-branch | Complete and merge work |
| `dispatch` | dispatching-parallel-agents | Run independent tasks in parallel |

## Important Patterns

### SessionStart Hook
The `hooks/session-start.sh` script injects the `using-superpowers` skill content at session start, establishing skill discovery and usage patterns.

### Workflow Definitions
The `workflows/` directory contains **single source of truth** for Conductor logic:
- Format-agnostic (markdown, referenced by TOML/Claude/etc.)
- Centralized workflow updates
- JSON schemas in `workflows/schemas/` define state file structures

### Skill Naming
- Directory names: kebab-case (`test-driven-development`)
- SKILL.md frontmatter `name` field must match directory name
- Triggers can be shorthand (`bs` for brainstorming, `fb` for file-beads)

### TDD Methodology
**Iron law**: No production code without a failing test first.
- RED: Write failing test
- GREEN: Minimal code to pass
- REFACTOR: Clean up while staying green

### Session Resumability
Beads survive context compaction. Store recovery context in issue notes:
```bash
bd update <id> --notes "COMPLETED: X. IN PROGRESS: Y. NEXT: Z"
```

## Code Quality Standards

- **No emojis** in code, comments, variable names, or documentation
- **Conventional commits**: `feat:`, `fix:`, `test:`, `refactor:`, `docs:`
- **TDD for all implementation** - watch tests fail before implementing
- **Verification before completion** - run tests/builds before claiming done
- Skills are self-contained, avoid tight coupling

## Related Documentation

- [README.md](README.md) - Full plugin overview and usage guide
- [SETUP_GUIDE.md](SETUP_GUIDE.md) - Installation instructions for different environments
- [TUTORIAL.md](TUTORIAL.md) - Complete workflow guide with examples
- [AGENTS.md](AGENTS.md) - Current project-specific agent instructions
- [docs/GLOBAL_CONFIG_TEMPLATE.md](docs/GLOBAL_CONFIG_TEMPLATE.md) - Global configuration template
