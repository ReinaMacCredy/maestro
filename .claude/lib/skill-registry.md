---
name: skill-registry
description: Internal skill for discovering and indexing available skills from project and global locations.
type: internal
---

# Skill Registry

Discovers skills from project and global locations, returning a structured index for orchestrator use.

## Discovery Locations

| Priority | Location | Source Label |
|----------|----------|--------------|
| 1 (highest) | `.claude/skills/` | `project` |
| 2 | `.agents/skills/` | `project` (npx skills add) |
| 3 | `~/.claude/skills/` | `global` |
| 4 (lowest) | `~/.claude/plugins/marketplaces/**/skills/*/` | `plugin` |

Project skills with the same name override global and plugin skills.

## Discovery Process

### Step 1: Scan Project Skills

**Important**: The Glob tool doesn't follow symlinks. Skills installed via `npx skills add` create symlinks in `.claude/skills/` pointing to `.agents/skills/`. To discover all skills including symlinked ones, use Bash with `find -L`:

```bash
find .claude/skills -L -name "SKILL.md" -type f 2>/dev/null
```

Alternatively, scan both locations:
```
Glob pattern: .claude/skills/*/SKILL.md
Glob pattern: .agents/skills/*/SKILL.md
```

For each file found:
1. Read the file
2. Parse YAML frontmatter (between `---` markers)
3. Extract: `name`, `description`, `triggers` (if present), `priority` (default: 100)
4. Record path and mark `source: "project"`

### Step 2: Scan Global Skills

```bash
find ~/.claude/skills -L -name "SKILL.md" -type f 2>/dev/null
```

For each file found:
1. Skip if a project skill with the same `name` already exists
2. Read the file
3. Parse YAML frontmatter
4. Extract: `name`, `description`, `triggers` (if present), `priority` (default: 100)
5. Record path and mark `source: "global"`

### Step 3: Scan Plugin Skills

Claude Code plugins install skills to `~/.claude/plugins/marketplaces/`. Discover them with:

```bash
find ~/.claude/plugins/marketplaces -L -name "SKILL.md" -type f 2>/dev/null
```

For each file found:
1. Skip if a project or global skill with the same `name` already exists
2. Read the file
3. Parse YAML frontmatter
4. Extract: `name`, `description`, `triggers` (if present), `priority` (default: 100)
5. Record path and mark `source: "plugin"`

### Step 4: Build Index

Combine all discovered skills into a list, sorted by priority (lower = higher priority).

## Output Format

```yaml
skills:
  - name: "skill-name"
    description: "What this skill does"
    triggers:
      - "/command"
      - "@mention"
    priority: 100
    path: ".claude/skills/skill-name/SKILL.md"
    source: "project"
```

## YAML Frontmatter Schema

Skills define their metadata in YAML frontmatter:

```yaml
---
name: my-skill           # Required: unique identifier
description: ...         # Required: one-line description
triggers:                # Optional: activation patterns
  - "/my-command"
  - "@my-mention"
priority: 100            # Optional: sort order (lower = higher priority, default: 100)
---
```

## Graceful Degradation

When no skills are found:
- Return an empty list: `skills: []`
- Do not error or warn
- Orchestrator proceeds without skill injection

When a skill file is malformed:
- Skip that skill
- Log which file was skipped and why
- Continue processing remaining skills

## Usage by Orchestrator

The orchestrator references this registry at workflow start:

1. Call discovery process (Steps 1-3)
2. Inject skill summaries into worker context
3. Workers can reference skills by name when relevant

This enables skill-aware delegation without hardcoding skill knowledge into agent definitions.
