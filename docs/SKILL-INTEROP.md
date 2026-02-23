# Skill Interoperability

Maestro automatically discovers and injects skill guidance into worker prompts during `/work` execution. This enables specialized knowledge to flow into your workflows without manual configuration.

## How It Works

When you run `/work`, Maestro:

1. **Discovers** installed skills from project and global locations
2. **Matches** relevant skills to each task based on triggers and keywords
3. **Injects** matched skill guidance into worker prompts via a `## SKILL GUIDANCE` section

Workers receive contextual expertise without needing to know which skills exist.

## Skill Discovery Locations

Skills are discovered from universal and agent-specific locations, with project skills taking precedence:

| Priority | Location | Description |
|----------|----------|-------------|
| 1 | `skills/*/SKILL.md` | Canonical project skill location (agent-agnostic) |
| 2 | `.github/skills/*/SKILL.md` | GitHub Copilot and compatible tools |
| 3 | `.agents/skills/*/SKILL.md` | Universal agent path (Amp/OpenCode/Replit/etc.) |
| 4 | `.claude/skills/*/SKILL.md` | Claude-specific path (kept for compatibility) |
| 5 (lowest) | `~/.claude/skills/*/SKILL.md` | Global user skills |

**Override behavior:** If a project skill has the same name as a global skill, the project skill wins.

## Installing External Skills

Use the [skills.sh CLI](https://skills.sh/docs) to install from public repositories:

```bash
# Install this repository's skills
npx skills add ReinaMacCredy/maestro

# Install specific skills to specific agents
npx skills add ReinaMacCredy/maestro --skill planning --agent claude-code --agent amp

# List skills without installing
npx skills add ReinaMacCredy/maestro --list
```

### Recommended Skills

| Skill | Purpose |
|-------|---------|
| `frontend-design` | Production-grade UI with high design quality |
| `web-design-guidelines` | Modern web design principles and patterns |
| `react-best-practices` | React conventions, hooks, and performance |
| `code-styleguides` | Language-specific coding conventions injected into CLAUDE.md via `/styleguide` |

## Creating Skill-Aware Skills

Skills declare their metadata in YAML frontmatter at the top of `SKILL.md`:

```yaml
---
name: my-skill              # Required: unique identifier
description: What it does and when to use it  # Required
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
| `name` | Yes | Unique identifier. Must be lowercase alphanumeric + hyphens, max 64 chars, and match the parent directory name |
| `description` | Yes | Description of what the skill does and when to use it (max 1024 chars) |
| `triggers` | No | Keywords that activate this skill for a task |
| `priority` | No | Numeric weight for ranking (lower = higher priority, default: 100) |

### Trigger Matching

When Maestro matches tasks to skills:

1. **Trigger match** (strongest): Any trigger word appears in the task description
2. **Keyword match** (weaker): Skill name or description words appear in the task

Matching is case-insensitive and supports partial matches (e.g., "testing" matches trigger "test").

## Example: Custom Skill

Create `skills/api-design/SKILL.md`:

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
