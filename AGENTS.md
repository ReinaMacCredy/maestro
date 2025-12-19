# AGENTS.md - my-workflow Plugin

## Overview
Claude Code plugin bundling workflow skills: Conductor (planning), Beads (issue tracking), and Superpowers (TDD, debugging, code review).

## Build/Test Commands
No build required - this is a skill/documentation plugin. Validate JSON:
```bash
cat .claude-plugin/plugin.json | jq .   # Validate plugin manifest
```

## Architecture
```
skills/           # 24 skill directories, each with SKILL.md (frontmatter + instructions)
  beads/          # Issue tracking skill with references/ subdirectory
  conductor/      # Planning methodology
  ...             # TDD, debugging, code review, etc.
commands/         # Slash command definitions (.md files)
lib/              # Shared utilities (memory_search.py, skills-core.js)
.claude-plugin/   # Plugin manifest (plugin.json, marketplace.json)
```

## Code Style
- Skills: Markdown with YAML frontmatter (`name`, `description` required)
- Commands: Markdown files defining slash command behavior
- Follow existing skill structure: SKILL.md at skill root, optional references/ subdirectory
- Keep skills self-contained with minimal cross-references

## Naming Conventions
- Skill directories: kebab-case (`test-driven-development`, `using-git-worktrees`)
- SKILL.md must match directory name in frontmatter `name` field
