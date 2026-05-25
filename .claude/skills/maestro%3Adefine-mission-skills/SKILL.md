---
name: maestro:define-mission-skills
description: "Define and register custom skills for use in Mission Control missions. Create skill definitions with frontmatter, procedures, and validation rules."
argument-hint: "<skill-name> [--project|--global]"
---

# Define Mission Skills

Define and register custom skills for use in Mission Control missions. Create skill definitions with frontmatter, procedures, and validation rules.

## Arguments

`$ARGUMENTS`

The skill name and optional scope flag.

- `<skill-name>`: The skill identifier (e.g., `my-feature-agent`)
- `--project`: Store skill in project `.maestro/skills/` (default runtime lookup path)
- `--global`: Store skill in personal `~/.maestro/skills/`

When a skill name contains `:`, replace it with `%3A` in the on-disk directory name.

---

## Step 1: Validate Prerequisites

**Inputs:** Filesystem state.

**Actions:**
1. Verify Maestro CLI is available: `maestro --version`
2. Check if skill directory exists: `.maestro/skills/` or `~/.maestro/skills/`
3. Confirm the skill name is valid (alphanumeric, hyphens, colons allowed)

**Outputs:** Ready to create skill.

---

## Step 2: Skill Structure

A valid skill file has three parts:

### 2.1 Frontmatter (YAML)

```yaml
---
name: <skill-name>
description: "Brief description of what this skill does"
argument-hint: "<arg1> [<arg2>] [--option]"
---
```

**Required fields:**
- `name`: Unique identifier (match the filename)
- `description`: One-sentence summary
- `argument-hint`: Help text for arguments

### 2.2 Required Skills (Optional)

List skills this one depends on:

```markdown
## Required Skills

- `maestro:agent-base` - For standard agent procedures
- `other-skill` - Description of dependency
```

### 2.3 Work Procedure

The main body describing:
- When to use this skill
- Step-by-step procedures
- Expected inputs and outputs
- Failure handling
- Handoff requirements

---

## Step 3: Create Skill File

**For project-scoped skills:**

```bash
mkdir -p .maestro/skills/<skill-dir>
# Create .maestro/skills/<skill-dir>/SKILL.md
```

Where `<skill-dir>` is the skill name with `:` replaced by `%3A`.

**For global skills:**

```bash
mkdir -p ~/.maestro/skills/<skill-dir>
# Create ~/.maestro/skills/<skill-dir>/SKILL.md
```

---

## Step 4: Skill Content Template

```markdown
---
name: <skill-name>
description: "What this skill enables"
argument-hint: "<required-arg> [optional-arg]"
---

# <Skill Title>

Brief description of the skill's purpose.

## When to Use This Skill

Specific scenarios where this skill applies.

## Required Skills

- `skill-1` - Why it's needed
- `skill-2` - Why it's needed

## Work Procedure

### Phase 1: Preparation

1. **Input validation**: Check prerequisites
2. **Load context**: Read necessary files
3. **Initialize**: Set up required state

### Phase 2: Execution

1. **Step one**: Description
2. **Step two**: Description
3. **Step three**: Description

### Phase 3: Validation

1. **Run tests**: Verification command
2. **Type check**: `bun run typecheck`
3. **Commit**: Save changes

## Handoff Requirements

When calling `EndFeatureRun`, include:
- `whatWasImplemented`: Concrete description
- `verification.commandsRun`: List of verification commands
- `tests.added`: New test coverage

## When to Return to Orchestrator

- Blocked by missing dependency
- Skill does not exist (error from Skill tool)
- Broken baseline that can't be fixed
```

---

## Validation

After creating a skill, validate it:

1. **Frontmatter check**: YAML is valid, required fields present
2. **Name consistency**: File path matches `name` in frontmatter
3. **Procedures clear**: Work procedure has actionable steps

**Next step**: after the skill passes validation, return to the caller that invoked you. If `maestro:mission-planning` sent you here, return to its Step 3 (Match agent types) to assign the new agentType to the feature that triggered this registration. If `maestro:conduct` sent you here mid-execution, return to conduct so it can dispatch the feature with the new agentType.
4. **Handoff documented**: Clear what the handoff should contain

---

## Related Commands

| Command | Purpose |
|---------|---------|
| `maestro mission create` | Create mission using skills |
| `maestro feature approve` | Assign skill to feature |
| `maestro validation show` | Validate skill usage |

---

## Best Practices

1. **Reuse existing skills**: Check if a built-in skill already covers your need
2. **Extend, don't duplicate**: Inherit from `maestro:agent-base` for agents
3. **Document return conditions**: Clearly state when to return to orchestrator
4. **Include verification**: Every skill should specify how to verify its work
5. **Keep skills focused**: One skill = one responsibility
6. **Document lookup order**: Runtime agents load `.maestro/skills/{agentType with : replaced by %3A}/SKILL.md` first, then fall back to `skills/built-in/{agentType with : replaced by %3A}/SKILL.md`
