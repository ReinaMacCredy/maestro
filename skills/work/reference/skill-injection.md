# Skill Injection — Work Phase

## Discovering Available Skills

Before spawning teammates, discover skills that can provide guidance for task delegation.

Discover skills using runtime-visible inventories and local skill folders. Prefer this order:

1. Runtime-provided skill list (if available)
2. Project-local skills (for example `.agents/skills/**/SKILL.md`, `.claude/skills/**/SKILL.md`, or `skills/**/SKILL.md`)
3. User-global skills (runtime dependent)

Deduplicate by skill name, preferring project-local definitions over global ones.

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

For each task, match the task description against the discovered skill registry using this algorithm:

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
