# AGENTS.md - Maestro Plugin

## Overview
Claude Code plugin bundling workflow skills: Conductor (planning), Design (Double Diamond sessions), Beads (issue tracking), and Superpowers (TDD, debugging, code review).

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
  design/         # Double Diamond design sessions (ds trigger)
  ...             # TDD, debugging, code review, etc.
commands/         # Slash command definitions (.md files)
workflows/        # Workflow definitions
  party-mode/     # Multi-agent collaborative design review (A/P/C [P] option)
lib/              # Shared utilities (skills-core.js)
.claude-plugin/   # Plugin manifest (plugin.json, marketplace.json)
conductor/        # Unified save location for plans and tracks
  tracks/<id>/    # Active work (design.md + spec.md + plan.md per track)
    .fb-progress.json   # Beads filing state (resume capability)
    .fb-progress.lock   # Concurrent session lock (30min timeout)
    .track-progress.json # Spec/plan generation checkpoints
    metadata.json       # Track info + thread IDs for audit trail
  archive/        # Completed work
```

## Handoff Mechanism (Planning → Execution)

**Unified flow via `/conductor-newtrack`:**
```
ds → design.md → /conductor-newtrack → spec.md + plan.md + beads + review
```

**Flags:**
- `--no-beads` / `-nb`: Skip beads filing (spec + plan only)
- `--plan-only` / `-po`: Alias for --no-beads
- `--force`: Overwrite existing track or remove stale locks

**State files:**
- `.fb-progress.json`: Beads filing state with resume capability
- `.fb-progress.lock`: Concurrent session lock (30min timeout)
- `.track-progress.json`: Spec/plan generation checkpoints

**Execution session starts with:** `Start epic <epic-id>` or `/conductor-implement <track-id>`

## Code Style
- Skills: Markdown with YAML frontmatter (`name`, `description` required)
- Commands: Markdown files defining slash command behavior
- Follow existing skill structure: SKILL.md at skill root, optional references/ subdirectory
- Keep skills self-contained with minimal cross-references

## Naming Conventions
- Skill directories: kebab-case (`test-driven-development`, `using-git-worktrees`)
- SKILL.md must match directory name in frontmatter `name` field

## Versioning

### Plugin Version (Automated)
Plugin version in `.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json` is **auto-bumped by CI** based on conventional commits:
- `feat:` → minor bump (1.5.0 → 1.6.0)
- `fix:` → patch bump (1.5.0 → 1.5.1)
- `feat!:` or `BREAKING CHANGE:` → major bump (1.5.0 → 2.0.0)
- `docs:`, `chore:` → changelog only, no version bump

### Skill Versions (Manual)
Individual skill versions in SKILL.md frontmatter remain **manually updated**:
- **Major bump** (1.x.x → 2.x.x): Breaking changes, renamed triggers, removed features
- **Minor bump** (x.1.x → x.2.x): New features, significant changes
- **Patch bump** (x.x.1 → x.x.2): Small fixes, tweaks

### Escape Hatch
Add `[skip ci]` to commit message to bypass all automation (changelog + version bump).

## Key Skills

| Skill | Trigger | Description |
|-------|---------|-------------|
| `design` | `ds` | Double Diamond design session with A/P/C checkpoints and Party Mode option |
| `conductor` | `/conductor-setup`, `/conductor-design`, `/conductor-newtrack`, `/conductor-implement`, `/conductor-status`, `/conductor-revert`, `/conductor-revise`, `/conductor-refresh`, `/conductor-finish` | Structured planning and execution through specs and plans |
| `file-beads` | `fb` | File beads from plan (batched in groups of 5, checkpointed for resume) |
| `review-beads` | `rb` | Review beads (parallel + cross-epic validation, dual tracking: progress file + beads label) |
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

# Cleanup commands (used by /conductor-finish Phase 2)
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
git push                # Push to remote
```

### Best Practices

- Check `bd ready` at session start to find available work
- Update status as you work (in_progress → closed)
- Create new issues with `bd create` when you discover tasks
- Use descriptive titles and set appropriate priority/type
- Always `bd sync` before ending session

<!-- end-bv-agent-instructions -->
