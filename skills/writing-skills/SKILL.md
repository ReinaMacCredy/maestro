---
name: writing-skills
description: Use when creating new skills, editing existing skills, or verifying skills work before deployment
---

# Writing Skills

## Core Principles

1. **Skills ARE TDD for documentation** - Write failing test (baseline), write skill, watch pass, refactor
2. **No skill without failing test first** - If you didn't watch an agent fail without it, you don't know what to teach
3. **Description = When to Use, NOT What It Does** - Summaries create shortcuts agents will take

## Quick Reference

| Phase | Action |
|-------|--------|
| RED | Run pressure scenario WITHOUT skill, document rationalizations |
| GREEN | Write minimal skill addressing those failures |
| REFACTOR | Find new loopholes, plug them, re-test |

| Skill Type | Test Focus |
|------------|------------|
| Technique | Recognition + application under pressure |
| Pattern | When to apply + when NOT to apply |
| Reference | Can agent find and use information? |

## When to Create

**Create when:** Technique wasn't obvious, applies broadly, others benefit
**Don't create for:** One-offs, standard practices, project-specific (use CLAUDE.md)

## Frontmatter

```yaml
---
name: skill-name-with-hyphens
description: Use when [triggering conditions only, never workflow summary]
---
```

- Name: letters, numbers, hyphens only
- Description: Start "Use when...", max 1024 chars, third person

## Anti-Patterns

- ❌ **Narrative** - "In session 2025-10-03, we found..." (too specific)
- ❌ **Multi-language** - example-js.js, example-py.py (maintenance burden)
- ❌ **Workflow in description** - Creates shortcut, agent skips body
- ❌ **Batching skills** - Test each before moving to next

## STOP Before Next Skill

After writing ANY skill, complete deployment before creating another. Untested skills = untested code.

## References

- [TDD Mapping](references/tdd-mapping.md) - RED-GREEN-REFACTOR cycle for skills
- [Skill Types](references/skill-types.md) - Technique, Pattern, Reference testing
- [CSO](references/cso.md) - Claude Search Optimization for discovery
- [Testing Methodology](references/testing-methodology.md) - Full checklist
- [Bulletproofing](references/bulletproofing.md) - Closing rationalization loopholes
- [Skill Structure](references/skill-structure.md) - Directory layout and template

## Related

- [test-driven-development](../test-driven-development/SKILL.md) - Core TDD cycle this adapts
- [sharing-skills](../sharing-skills/SKILL.md) - Contributing skills upstream
- [maestro-core](../maestro-core/SKILL.md) - Workflow routing and skill hierarchy
