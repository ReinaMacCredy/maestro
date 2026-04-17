---
name: maestro-skill-author
description: "Create, update, or debug maestro built-in skills. Covers SKILL.md frontmatter, reference directory structure, step-file architecture, build-time embedding, naming conventions, alias management, and registry validation. Use when creating a new maestro built-in skill, modifying an existing SKILL.md, adding reference files, debugging skill loading failures, updating the skills registry, or working on the skills full port. Also use when frontmatter validation fails, skills don't appear in skill-list, or reference files fail to load."
---

# Maestro Skill Authoring Guide

## Skill Anatomy

Every built-in skill is a directory under `skills/built-in/`:

```
skills/built-in/maestro%3A{name}/
  SKILL.md              (required -- frontmatter + content)
  reference/            (optional)
    {file}.md           (reference documents)
    steps/              (for multi-step skills)
      step-01-{name}.md
      step-02-{name}.md
```

The directory name is the URL-escaped skill slug. For built-in `maestro:*` skills, replace `:` with `%3A` on disk while keeping the frontmatter name unchanged.

## SKILL.md Format

### Frontmatter (required)

```yaml
---
name: maestro:{slug}
description: "When to trigger and what this skill does. Be specific about contexts."
argument-hint: "<track description>"
---
```

| Field | Required | Notes |
|-------|----------|-------|
| `name` | Yes | Must match the logical skill slug; the on-disk directory uses `%3A` instead of `:` |
| `description` | Yes | Primary trigger mechanism -- be specific about when to use |
| `argument-hint` | No | Shown in `skill-list` output. Bracket notation, not camelCase. |

**Frontmatter rules:**
- Delimiters are `---` on their own lines (lines 1 and 3+)
- Simple `key: value` pairs only -- no multiline values
- Quotes are stripped from values (`"value"` and `'value'` both work)
- Missing `name` or `description` causes the skill to be skipped during build

### Body Content

The body after frontmatter is pure markdown. Structure it for the agent that will read it:

```markdown
# {Skill Name}

## Overview
One-paragraph purpose and when to use.

## When to Use / When NOT to Use
Explicit trigger conditions.

## The Workflow / Steps
Numbered actions the agent should follow.

## Key Principles / Rules
Concise behavioral constraints.
```

**Description writing:** The description is how Claude decides whether to load the skill. Make it "pushy" -- include specific trigger phrases, file patterns, and contexts. A generic description means the skill never triggers.

## Reference Files

Reference files provide deeper content loaded on demand. They keep SKILL.md under 500 lines while giving the skill access to unlimited reference material.

### Simple References (single files)

```
reference/
  interview-guide.md    -- Question banks, decision trees
  templates.md          -- Code or document templates
  checklist.md          -- Verification checklists
```

Load from SKILL.md: "Read `reference/interview-guide.md` and follow its question sequence."

### Step Files (multi-step skills)

For skills with sequential workflows (like `maestro:design` with 16 steps):

```
reference/steps/
  step-01-init.md
  step-02-classify.md
  step-03-vision.md
  ...
```

Each step file follows this template:

```markdown
# Step N: {Title}

**Progress: Step N of {total}** -- Next: {Next Step Title}

### Goal
{One-sentence objective for this step}

### Execution
1. {Action}
2. {Action}
3. {Action}

### Next Step
Read and follow `reference/steps/step-{N+1}-{name}.md`.
```

**Step file rules:**
- Load ONE step file at a time, execute fully, then load NEXT
- Never load multiple steps simultaneously
- Never skip or reorder steps
- Each step tells the agent what to load next

### Nested References

References can be nested (e.g., `reference/codex-skills/land/SKILL.md`). The build system handles recursive embedding. Path keys in the generated registry use forward slashes: `codex-skills/land/SKILL.md`.

## Build-Time Embedding

When you run `bun run build`, the generator (`src/skills/generate.ts`) processes all skills:

1. Reads every `skills/built-in/maestro%3A*/SKILL.md`
2. Parses frontmatter (validates name + description)
3. Recursively reads all files in `reference/`
4. Outputs `src/skills/registry.generated.ts` (~320KB)

The generated file embeds:
- All SKILL.md content (including frontmatter)
- All reference files as a flat `Record<string, string>` map
- Skill names, descriptions, and argument hints as typed constants

**After any skill change, you must rebuild:**
```bash
bun run build
```

If you skip the rebuild, the MCP server and CLI will serve stale skill content.

## Naming Conventions

| Convention | Example | Notes |
|-----------|---------|-------|
| Directory name | `maestro%3Adesign` | URL-escaped on-disk path |
| Slug in code | `'maestro:design'` | String key in registry |
| CLI usage | `maestro skill maestro:design` | User-facing |

## Aliases

For renamed skills, add aliases in `src/skills/aliases.ts`:

```typescript
export const SKILL_ALIASES: Record<string, string> = {
  'brainstorming': 'maestro:brainstorming',
  'systematic-debugging': 'maestro:debugging',
  // old name -> new name
};
```

When a user loads an aliased name, they get the skill with a deprecation warning.

## Deprecation

To deprecate a skill, add it to `DEPRECATED_SKILLS` in the registry:

```typescript
const DEPRECATED_SKILLS = new Map<string, string>([
  ['maestro:symphony-setup', 'Use `maestro symphony install` instead.'],
]);
```

Deprecated skills still load but prepend a warning banner.

## External Skills

Users can also create skills outside the built-in directory. Discovery priority (highest first):

1. `skills/external/` -- source: `'external'`
2. `.maestro/skills/` -- source: `'maestro'`
3. `.claude/skills/` -- source: `'claude'`
4. Built-in (embedded) -- source: `'builtin'`

External skills use the same directory layout (`SKILL.md` + `reference/`) but are loaded from the filesystem at runtime, not embedded at build time.

## Verification Checklist

After creating or modifying a skill:

- [ ] `name` in frontmatter matches directory slug exactly
- [ ] `description` is present and specific enough to trigger correctly
- [ ] `argument-hint` uses bracket notation (not camelCase)
- [ ] SKILL.md body is under 500 lines
- [ ] Reference files are referenced from SKILL.md with clear loading guidance
- [ ] Step files follow the template (Progress, Goal, Execution, Next Step)
- [ ] `bun run build` succeeds without warnings
- [ ] `maestro skill-list` shows the skill with correct metadata
- [ ] `maestro skill {name}` returns the full content
- [ ] `maestro skill {name} --ref {path}` loads reference files correctly
- [ ] No alias conflicts with existing skills
