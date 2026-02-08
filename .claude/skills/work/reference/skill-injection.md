# Skill Injection — Work Phase

## Discovering Available Skills

Before spawning teammates, discover skills that can provide guidance for task delegation.

**Important**: The Glob tool doesn't follow symlinks. Use Bash with `find` to discover all skills. Note: Remove `-type f` for plugin paths on macOS:

```bash
# Project skills (highest priority) - use -L to follow symlinks
find .claude/skills -L -name "SKILL.md" -type f 2>/dev/null
find .agents/skills -L -name "SKILL.md" -type f 2>/dev/null

# Global skills
find ~/.claude/skills -name "SKILL.md" 2>/dev/null

# Plugin-installed skills (lowest priority) - no -L or -type f for macOS compatibility
find ~/.claude/plugins/marketplaces -name "SKILL.md" 2>/dev/null
```

For each SKILL.md file found:
1. Read the file
2. Parse YAML frontmatter (between `---` markers)
3. Extract: `name`, `description`, `triggers` (optional), `priority` (default: 100)
4. Store the full content after frontmatter

**Priority**: Project skills override global skills, which override plugin skills (same name = skip lower priority).

See `.claude/lib/skill-registry.md` for the complete discovery process.

**Build a skill registry** for use in Step 4:

```yaml
skills:
  - name: "skill-name"
    description: "What this skill does"
    triggers: ["trigger1", "trigger2"]
    priority: 100
    content: "Full SKILL.md content after frontmatter"
    source: "project"  # or "global"
```

**Graceful degradation**: If no skills are found, proceed without skill injection. Do not error or warn.

## Injecting Skill Guidance into Task Prompts

For each task, match the task description against the skill registry using the algorithm in `.claude/lib/skill-matcher.md`:

1. **Normalize** task description to lowercase words
2. **Match** skills by triggers (highest relevance) or keywords from name/description
3. **Rank** by priority (lower = higher priority)

If matching skills are found, add a `## SKILL GUIDANCE` section after `## CONTEXT`:

```
## SKILL GUIDANCE

### {skill-name}
{Full SKILL.md content after frontmatter}

### {another-skill}
{Content}
```

**If no skills match the task, omit the `## SKILL GUIDANCE` section entirely.** Do not include an empty section — graceful degradation means the prompt works without it.
