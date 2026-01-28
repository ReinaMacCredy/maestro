---
description: Generate hierarchical AGENTS.md files documenting each directory's purpose and patterns
allowed-tools: Bash, Read, Write, Glob, Grep, Task
model: sonnet
---

# Initialize AGENTS.md Knowledge Base

Generate AGENTS.md files in each major directory to document purpose, patterns, and conventions.

## Target

$ARGUMENTS

If no target specified, start from project root.

## Process

1. **Scan** - Analyze directory structure
2. **Identify** - Find major directories (src, lib, components, etc.)
3. **Analyze** - For each directory:
   - Read key files to understand purpose
   - Identify patterns and conventions
   - Note entry points and exports
4. **Document** - Create AGENTS.md files

## AGENTS.md Template

```markdown
# <Directory Name>

## Purpose
<1-2 sentence description>

## Key Files

| File | Purpose |
|------|---------|
| index.ts | Main entry point |
| types.ts | Type definitions |

## Patterns

- <Pattern 1>: <Brief description>
- <Pattern 2>: <Brief description>

## Dependencies

- **Internal**: Directories this depends on
- **External**: Key npm packages

## Notes for AI Agents

<Special considerations for AI agents working here>
```

## Depth Control

- `--depth 1` - Top-level only (src/, lib/, tests/)
- `--depth 2` - Include immediate subdirectories
- `--depth 3` - Full tree (may be slow)

Example: `/init-deep src/ --depth 2`

---

## References

- [Atlas SKILL.md](../skills/atlas/SKILL.md)

**Uses**: atlas-explore (for codebase analysis)
**Uses Skills**: atlas
