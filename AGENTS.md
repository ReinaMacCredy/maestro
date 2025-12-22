# AGENTS.md - Maestro Plugin

## Overview
Claude Code plugin bundling workflow skills: Conductor (planning), Beads (issue tracking), and Superpowers (TDD, debugging, code review).

## Build/Test Commands
No build required - this is a skill/documentation plugin. Validate JSON:
```bash
cat .claude-plugin/plugin.json | jq .   # Validate plugin manifest
```

## Architecture
```
skills/           # 16 skill directories, each with SKILL.md (frontmatter + instructions)
  beads/          # Issue tracking skill with references/ subdirectory
  conductor/      # Planning methodology (includes /conductor-design)
  ...             # TDD, debugging, code review, etc.
commands/         # Slash command definitions (.md files)
lib/              # Shared utilities (skills-core.js)
.claude-plugin/   # Plugin manifest (plugin.json, marketplace.json)
conductor/        # Unified save location for plans and tracks
  tracks/<id>/    # Active work (design.md + spec.md + plan.md per track)
  archive/        # Completed work
```

## Handoff Mechanism (Planning → Execution)

**Planning session outputs:**
```bash
bd update <epic-id> --notes "HANDOFF_READY: true. PLAN: <plan-path>"
```

**Execution session starts with:** `Start epic <epic-id>`

## Code Style
- Skills: Markdown with YAML frontmatter (`name`, `description` required)
- Commands: Markdown files defining slash command behavior
- Follow existing skill structure: SKILL.md at skill root, optional references/ subdirectory
- Keep skills self-contained with minimal cross-references

## Naming Conventions
- Skill directories: kebab-case (`test-driven-development`, `using-git-worktrees`)
- SKILL.md must match directory name in frontmatter `name` field

## Versioning
- **Major bump** (1.x.x → 2.x.x): Breaking changes (renamed skills, removed features, changed triggers) or significant redesigns
- **Minor bump** (x.1.x → x.2.x): Big updates, new features, significant changes
- **Patch bump** (x.x.1 → x.x.2): Small updates, fixes, minor tweaks
- After updating skills, update `.claude-plugin/plugin.json` & `.claude-plugin/marketplace.json` version with the same bump type

## Key Skills

| Skill | Trigger | Description |
|-------|---------|-------------|
| `design` | `ds` | Design session with mandatory grounding and fb handoff |
| `conductor` | `/conductor-setup`, `/conductor-design`, `/conductor-newtrack`, `/conductor-implement`, `/conductor-status`, `/conductor-revert`, `/conductor-revise`, `/conductor-refresh` | Structured planning and execution through specs and plans |
| `file-beads` | `fb` | File beads from plan (parallel subagents per epic) |
| `review-beads` | `rb` | Review beads (parallel + cross-epic validation) |
| `doc-sync` | `doc-sync`, `/doc-sync` | Sync AGENTS.md from completed thread knowledge |
| `beads` | `bd ready`, `bd status` | Issue tracking for multi-session work |

<!-- bv-agent-instructions-v1 -->

---

## Beads Workflow Integration

This project uses [beads_viewer](https://github.com/Dicklesworthstone/beads_viewer) for issue tracking. Issues are stored in `.beads/` and tracked in git.

### Essential Commands

```bash
# View issues (launches TUI - avoid in automated sessions)
bv

# CLI commands for agents (use these instead)
bd ready              # Show issues ready to work (no blockers)
bd list --status=open # All open issues
bd show <id>          # Full issue details with dependencies
bd create --title="..." --type=task --priority=2
bd update <id> --status=in_progress
bd close <id> --reason="Completed"
bd close <id1> <id2>  # Close multiple issues at once
bd sync               # Commit and push changes

# Cleanup commands (used by doc-sync Phase 7)
bd compact --analyze --json      # Find issues needing summary
bd compact --apply --id <id> --summary "text"  # Add AI summary
bd count --status closed --json  # Count closed issues
bd cleanup --older-than 0 --limit <n> --force  # Remove oldest closed
```

### Workflow Pattern

1. **Start**: Run `bd ready` to find actionable work
2. **Claim**: Use `bd update <id> --status=in_progress`
3. **Work**: Implement the task
4. **Complete**: Use `bd close <id>`
5. **Sync**: Always run `bd sync` at session end

### Key Concepts

- **Dependencies**: Issues can block other issues. `bd ready` shows only unblocked work.
- **Priority**: P0=critical, P1=high, P2=medium, P3=low, P4=backlog (use numbers, not words)
- **Types**: task, bug, feature, epic, question, docs
- **Blocking**: `bd dep add <issue> <depends-on>` to add dependencies

### Session Protocol

**Before ending any session, run this checklist:**

```bash
git status              # Check what changed
git add <files>         # Stage code changes
bd sync                 # Commit beads changes
git commit -m "..."     # Commit code
bd sync                 # Commit any new beads changes
git push                # Push to remote
```

### Best Practices

- Check `bd ready` at session start to find available work
- Update status as you work (in_progress → closed)
- Create new issues with `bd create` when you discover tasks
- Use descriptive titles and set appropriate priority/type
- Always `bd sync` before ending session

<!-- end-bv-agent-instructions -->
