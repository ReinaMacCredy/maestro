# Skill Structure

## Directory Structure

```
skills/
  skill-name/
    SKILL.md              # Main reference (required)
    references/           # Detailed sections (optional)
    supporting-file.*     # Only if needed
```

**Flat namespace** - all skills in one searchable namespace

**Separate files for:**
1. **Heavy reference** (100+ lines) - API docs, comprehensive syntax
2. **Reusable tools** - Scripts, utilities, templates

**Keep inline:**
- Principles and concepts
- Code patterns (< 50 lines)
- Everything else

## SKILL.md Template

```markdown
---
name: Skill-Name-With-Hyphens
description: Use when [specific triggering conditions and symptoms]
---

# Skill Name

## Overview

What is this? Core principle in 1-2 sentences.

## When To Use

[Small inline flowchart IF decision non-obvious]

Bullet list with SYMPTOMS and use cases
When NOT to use

## Core Pattern (for techniques/patterns)

Before/after code comparison

## Quick Reference

Table or bullets for scanning common operations

## Implementation

Inline code for simple patterns
Link to file for heavy reference or reusable tools

## Common Mistakes

What goes wrong + fixes

## Real-World Impact (optional)

Concrete results
```

## Frontmatter Requirements

**Required fields:** `name` and `description`

**Optional fields:** `metadata` (containing `version`, `keywords`, `author`, `repository`), `license`, `compatibility`

**Max 1024 characters for description**

- `name`: Use letters, numbers, and hyphens only (no parentheses, special chars)
- `description`: Third-person, describes ONLY when to use (NOT what it does)
  - Start with "Use when..." to focus on triggering conditions
  - Include specific symptoms, situations, and contexts
  - **NEVER summarize the skill's process or workflow**
  - Keep under 500 characters if possible

## Skill Dependencies

When a skill requires another skill to be loaded first, declare it in the Prerequisites section.

### The Prerequisites Pattern

Add a Prerequisites section immediately after the frontmatter, before the main heading:

```markdown
---
name: my-skill
description: Use when...
---

## Prerequisites

Routing and fallback policies are defined in [AGENTS.md](../../AGENTS.md).

**Additional requirements:**
- gh CLI installed
- Some other tool

# My Skill

...
```

### HALT vs DEGRADE Guidelines

**HALT when:**
- Dependency blocks ALL functionality
- No fallback exists
- Corrupted state that can't recover

**DEGRADE when:**
- Feature is optional
- Fallback behavior available
- Work can continue with reduced functionality

**Message formats:**
- HALT: `❌ Cannot proceed: [reason]. [fix instruction].`
- DEGRADE: `⚠️ [Feature] unavailable. [Fallback behavior].`
