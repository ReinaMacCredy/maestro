---
name: maestro:agent-base
description: "Base procedures for all mission agents: startup, cleanup, and handoff. REQUIRED skill for all mission feature implementations."
argument-hint: "[--no-baseline]"
---

# Agent Base Procedures

You are an agent in a multi-agent mission. This skill defines the procedures that ALL agents must follow. After completing startup, you'll invoke your specific agent skill for the actual work procedure.

## Your Assigned Feature

Your feature has been pre-assigned by the system and is shown in your bootstrap message. The feature includes:
- `id` - Feature identifier
- `description` - What to build
- `agentType` - The skill you must invoke for the work procedure
- `expectedBehavior` - What success looks like
- `verificationSteps` - How to verify your work
- `fulfills` - Validation contract assertion IDs (if present)

**Your feature's `fulfills` field lists validation contract assertions that must be true after your work.** Read these assertions carefully before starting — they define what "done" means for your feature. Before completing, ensure that each assertion would pass. If you realize an assertion cannot be fulfilled given your current scope, flag it in your handoff.

---

## CRITICAL: `.maestro/bootstrap/` is the project bootstrap layer

Do not remove or corrupt `.maestro/bootstrap/` while a project relies on Maestro bootstrap assets. This subtree contains the committed project-local setup files agents depend on.

**The `.maestro/bootstrap/` subtree should be committed to the repository** while `.maestro/missions/`, `.maestro/sessions/`, and other runtime state remain ignored.

You MAY read and update these files:
- `.maestro/bootstrap/services.yaml` - Add new services/commands if discovered during work
- `.maestro/bootstrap/library/` - Add knowledge for future agents

---

## Phase 1: Startup

### 1.1 Read Context

**PERFORMANCE TIP:** Parallelize your startup by reading all context files in a single tool call batch:

- `mission.md` - The accepted mission proposal
- `.maestro/AGENTS.md` - Guidance from orchestrator/bootstrap (includes Mission Boundaries)
- `validation-contract.md` - If your feature has `fulfills`, read those assertions
- `.maestro/bootstrap/services.yaml` - Commands and services (single source of truth)
- `features.json` - Feature list and status

### 1.2 Initialize Environment

1. Run `.maestro/bootstrap/init.sh` if it exists (one-time setup, idempotent)

### 1.3 Baseline Validation

Run `commands.test` from `.maestro/bootstrap/services.yaml`. This verifies the mission is in a healthy state before you start.

**CRITICAL: Do NOT pipe validator output through `| tail`, `| head`, or similar.** Pipes can mask failing exit codes.

If baseline fails:
- Call `EndFeatureRun` with `returnToOrchestrator: true` and explain the broken baseline

### 1.4 Understand Your Feature's Context

View all features in your feature's milestone:

```bash
jq --arg m "YOUR_MILESTONE" '.features | map(select(.milestone == $m)) | map({id, description, status})' features.json
```

### 1.5 Check Library

Refer to `.maestro/bootstrap/library/` for knowledge from previous agents (organized by topic).

### 1.6 Start Services

Start any services you'll need from `.maestro/bootstrap/services.yaml`:

1. Check `depends_on` and start dependencies first
2. Run each service's `start` command
3. Wait for `healthcheck` to pass
4. If ANY service fails → return to orchestrator immediately

---

## Phase 2: Work (Defined by Your Specific Skill)

After completing startup, invoke the skill specified in your feature's `agentType` field.

**If the skill does not exist** (Skill tool returns error), do NOT proceed. Return to orchestrator via `EndFeatureRun` with `returnToOrchestrator: true`.

That skill will guide you through the actual work procedure.

---

## Phase 3: Cleanup & Handoff

### 3.1 Final Validation

All validators from `.maestro/bootstrap/services.yaml` must pass before handoff.

**Scope note**: your responsibility here is the per-feature baseline only. Milestone-level validation (code scrutiny and user-flow testing) is run separately by the conductor via `maestro:scrutiny-validator` and `maestro:user-testing-validator` after all features in the milestone are complete. Do not attempt to run those skills yourself.

### 3.2 Environment Cleanup

1. **Stop services**: Use manifest `stop` commands
2. **Stop processes YOU started**: By their specific PID
3. **Ensure clean git status**: Commit or stash changes

### 3.3 Update Manifest (if needed)

If you discovered reusable services/commands, ADD them to `.maestro/bootstrap/services.yaml`.

---

## Agent Report Contract

When completing work, your handoff via `EndFeatureRun` must include:

```typescript
{
  successState: "success" | "partial" | "failure",
  returnToOrchestrator: boolean,
  commitId: string,           // required if success
  validatorsPassed: boolean, // required true if success
  handoff: {
    salientSummary: string,    // 1-4 sentences, under 500 chars
    whatWasImplemented: string, // min 50 chars
    whatWasLeftUndone: string,  // empty if complete
    verification: {
      commandsRun: Array<{
        command: string,
        exitCode: number,
        observation: string   // be specific!
      }>,
      interactiveChecks?: Array<{
        action: string,
        observed: string
      }>
    },
    tests: {
      added: Array<{
        file: string,
        cases: Array<{
          name: string,
          verifies: string
        }>
      }>,
      coverage: string
    },
    discoveredIssues: Array<{
      severity: "blocking" | "non_blocking" | "suggestion",
      description: string,
      suggestedFix?: string
    }>,
    skillFeedback?: {
      followedProcedure: boolean,
      deviations: Array<{
        step: string,
        whatIDidInstead: string,
        why: string
      }>
    }
  }
}
```

### Verification Hygiene

- **Be specific in observations** - "tests passed" → "3 tests passed in tests/unit/auth.test.ts"
- **Do NOT pipe through head/tail** - Use targeted test patterns instead

---

## When to Return to Orchestrator

Set `returnToOrchestrator: true` when:

- **Cannot complete within mission boundaries** - Never violate boundaries
- **Service won't start or healthcheck fails** - Manifest may be broken
- **Dependency that SHOULD exist is inaccessible** - After investigation
- **Blocked by missing dependency, unsatisfied preconditions, or unclear requirements**
- **Previous agent left broken state you can't fix**
- **Decision or input needed from human/orchestrator**
- **Your skill type requires it**

---

## Service Management via Manifest

`.maestro/bootstrap/services.yaml` is the **single source of truth** for all commands.

**Using the manifest:**
- Read it to find commands/services
- For services: use `start`, `stop`, `healthcheck` commands exactly as declared
- For commands: use named commands (e.g., `commands.test`)

**Starting services:**
1. Check `depends_on` and start dependencies first
2. Run the `start` command from the manifest
3. Wait for `healthcheck` to pass
4. If healthcheck fails → return to orchestrator immediately

**CRITICAL: Never Kill User Processes**

**FORBIDDEN:**
- `pkill node`, `killall`, `kill` by process name
- Port-based kills on ports NOT declared in `.maestro/bootstrap/services.yaml`

**ALLOWED:**
- Port-based kills using manifest's declared `stop` command
- Killing processes by PID that YOU started

---

## Code Quality Principles

- **Avoid god files** - Split large files into focused modules
- **Create reusable components** - Don't duplicate code
- **Keep changes focused** - Don't sprawl across unrelated areas
- **Stay in scope** - Note unrelated issues as `non_blocking` with "Pre-existing:" prefix
