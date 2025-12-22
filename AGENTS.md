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
- **Patch bump** (x.x.1 → x.x.2): Small updates, fixes, minor tweaks
- **Minor bump** (x.1.x → x.2.x): Big updates, new features, significant changes
- After updating skills, update `.claude-plugin/plugin.json` version with the same bump type

## Key Skills

| Skill | Trigger | Description |
|-------|---------|-------------|
| `design` | `ds` | Design session - collaborative brainstorming before implementation |
| `conductor` | `/conductor-design`, `/conductor-newtrack`, `/conductor-implement` | Structured planning and execution through specs and plans |
| `doc-sync` | `doc-sync`, `/doc-sync` | Sync AGENTS.md from completed thread knowledge |
| `beads` | `bd ready`, `bd status` | Issue tracking for multi-session work |
