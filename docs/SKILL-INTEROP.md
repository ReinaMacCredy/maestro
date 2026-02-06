# Skill Interoperability

Maestro automatically discovers and injects skill guidance into worker prompts during `/work` execution. This enables specialized knowledge to flow into your workflows without manual configuration.

## How It Works

When you run `/work`, Maestro:

1. **Discovers** installed skills from project and global locations
2. **Matches** relevant skills to each task based on triggers and keywords
3. **Injects** matched skill guidance into worker prompts via a `## SKILL GUIDANCE` section

Workers receive contextual expertise without needing to know which skills exist.

## Skill Discovery Locations

Skills are discovered from two locations, with project skills taking precedence:

| Priority | Location | Description |
|----------|----------|-------------|
| 1 (highest) | `.claude/skills/*/SKILL.md` | Project-specific skills |
| 2 (lowest) | `~/.claude/skills/*/SKILL.md` | Global skills (shared across projects) |

**Override behavior:** If a project skill has the same name as a global skill, the project skill wins.

## Installing External Skills

The [Vercel AI skills ecosystem](https://github.com/vercel/ai-skills) provides pre-built skills you can install:

```bash
# Install a skill globally
npx @anthropic-ai/skills add frontend-design

# Install into current project
npx @anthropic-ai/skills add frontend-design --project
```

### Recommended Skills

| Skill | Purpose |
|-------|---------|
| `frontend-design` | Production-grade UI with high design quality |
| `web-design-guidelines` | Modern web design principles and patterns |
| `react-best-practices` | React conventions, hooks, and performance |

## Creating Skill-Aware Skills

Skills declare their metadata in YAML frontmatter at the top of `SKILL.md`:

```yaml
---
name: my-skill              # Required: unique identifier
description: What it does   # Required: one-line summary
triggers:                   # Optional: activation keywords
  - "component"
  - "ui"
  - "design"
priority: 50                # Optional: ranking weight (default: 100)
---

# My Skill

Your skill content here...
```

### Field Reference

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Unique identifier for the skill |
| `description` | Yes | One-line description shown in skill listings |
| `triggers` | No | Keywords that activate this skill for a task |
| `priority` | No | Numeric weight for ranking (lower = higher priority, default: 100) |

### Trigger Matching

When Maestro matches tasks to skills:

1. **Trigger match** (strongest): Any trigger word appears in the task description
2. **Keyword match** (weaker): Skill name or description words appear in the task

Matching is case-insensitive and supports partial matches (e.g., "testing" matches trigger "test").

## Example: Custom Skill

Create `.claude/skills/api-design/SKILL.md`:

```yaml
---
name: api-design
description: RESTful API design patterns and conventions
triggers:
  - "api"
  - "endpoint"
  - "rest"
priority: 50
---

# API Design Guidelines

## URL Structure
- Use nouns, not verbs: `/users` not `/getUsers`
- Plural resource names: `/orders` not `/order`
- Nested resources for relationships: `/users/123/orders`

## HTTP Methods
- GET: Read (idempotent)
- POST: Create
- PUT: Full update
- PATCH: Partial update
- DELETE: Remove

## Response Codes
- 200: Success
- 201: Created
- 400: Bad request
- 404: Not found
- 500: Server error
```

When a task mentions "API" or "endpoint", this skill's guidance automatically flows to the assigned worker.

## Workflow Integration

Skills enhance Maestro workflows transparently:

```
/work → Orchestrator discovers skills
      → Matches skills per task
      → Injects guidance into worker prompts
      → Workers execute with specialized knowledge
```

No changes to your plans or commands required. Install skills, and they activate when relevant.
