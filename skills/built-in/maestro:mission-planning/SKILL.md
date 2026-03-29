---
name: maestro:mission-planning
description: "Plan and structure new missions using the Mission Control CLI. Create mission definitions, set milestones, and prepare for multi-agent execution."
argument-hint: "<mission description>"
---

# Mission Planning

Plan and structure new missions using the Mission Control CLI. Create mission definitions, set milestones, and prepare for multi-agent execution.

## Arguments

`$ARGUMENTS`

The mission description or command to execute.

---

## Step 1: Validate Prerequisites

**Inputs:** Filesystem state.

**Actions:**
1. Check current directory is initialized for Maestro. Run `maestro mission list` to verify.
2. If no mission runtime state exists, you may need to initialize Mission Control first.

**Outputs:** Confirmed Mission Control is available.

**Transition:** Proceed to Step 2.

---

## Step 2: Parse Input

**Inputs:** `$ARGUMENTS` string.

**Actions:**
1. Extract mission description from `$ARGUMENTS`.
2. If arguments contain `--json`, JSON output will be used.
3. If arguments contain `--file <path>`, read mission definition from file.

**Outputs:** Parsed mission parameters.

---

## Step 3: Create Mission

Use the Mission Control CLI to create a new mission:

```bash
maestro mission create --file <plan.json> [--json]
```

The plan file should contain:
- `description`: Mission overview
- `milestones`: Array of milestone definitions
- `features`: Array of feature specifications

**Outputs:** Mission ID generated and runtime state initialized under `.maestro/missions/{id}/`.

---

## Step 4: Define Milestones

Missions progress through phases (milestones). Define them with:

```bash
maestro milestone list --mission <mission-id>
maestro milestone seal <milestone-name> --mission <mission-id>
```

Common milestone sequence:
1. `bootstrap` - Initial setup and scaffolding
2. `core` - Core functionality implementation
3. `integration` - Integration and wiring
4. `polish` - Final refinements and validation
5. `complete` - Mission completion

---

## Step 5: Assign Features

Features are the atomic units of work within a mission. Use:

```bash
maestro feature list --mission <mission-id>
maestro feature show <feature-id> --mission <mission-id>
maestro feature approve <feature-id> --mission <mission-id>
```

Each feature has:
- `id`: Unique identifier
- `description`: What to implement
- `skillName`: The skill to use for implementation
- `milestone`: Which milestone it belongs to
- `verificationSteps`: How to verify completion

---

## Step 6: Generate Worker Prompts

When ready to execute a feature, generate worker prompts:

```bash
maestro feature prompt <feature-id> --mission <mission-id> --out <path>
```

This creates a prompt file for the assigned worker with:
- Mission context
- Feature description and requirements
- Verification steps
- Skill instructions

---

## Lifecycle States

Missions progress through states:

| State | Description | CLI Command |
|-------|-------------|-------------|
| `proposed` | Initial mission definition | `maestro mission create` |
| `active` | Mission in progress | `maestro mission activate` |
| `paused` | Temporarily halted | `maestro mission pause` |
| `completed` | All features done | `maestro mission complete` |

---

## Related Commands

| Command | Purpose |
|---------|---------|
| `maestro mission list` | List all missions |
| `maestro mission show <id>` | Show mission details |
| `maestro mission create` | Create new mission |
| `maestro milestone list` | List milestones |
| `maestro milestone seal` | Seal a milestone |
| `maestro feature list` | List features |
| `maestro feature approve` | Approve a feature |
| `maestro validation show` | Show validation state |
| `maestro checkpoint save` | Save checkpoint |

---

## Best Practices

1. **Keep missions focused**: A mission should have 5-15 features maximum
2. **Use descriptive IDs**: Feature IDs should indicate purpose (e.g., `auth-login-flow`)
3. **Match skills to features**: Assign the most specific skill for each feature
4. **Verification steps matter**: Clear verification steps enable automatic validation
5. **Checkpoint frequently**: Save checkpoints after major milestones

---

## Example Workflow

```bash
# Create mission with plan file
maestro mission create --file ./plans/api-refactor.json

# Check mission status
maestro mission show <generated-id>

# List features in the mission
maestro feature list --mission <id>

# Approve first feature for work
maestro feature approve feat-001 --mission <id>

# Generate worker prompt
maestro feature prompt feat-001 --mission <id> --out ./prompt.md
```
