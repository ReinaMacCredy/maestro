---
name: ultraqa
description: Iterative fix-and-verify loop for tests, build, lint, or typecheck. Diagnoses failures, applies fixes, re-verifies — up to 5 cycles.
argument-hint: "[--tests|--build|--lint|--typecheck|--custom '<command>']"
allowed-tools: Read, Write, Edit, Bash, Grep, Glob, Task, TeamCreate, TeamDelete, SendMessage, AskUserQuestion
disable-model-invocation: true
---

# UltraQA — Iterative Fix-and-Verify Loop

> Run a verification command, diagnose failures, fix them, repeat — until green or cycle limit hit. Never commits.

## Arguments

- `--tests` — Run project test suite (default)
- `--build` — Run build command
- `--lint` — Run linter
- `--typecheck` — Run type checker
- `--custom '<command>'` — Run a custom verification command

If no argument given, default to `--tests`.

## Hard Rules

- **Never run `git commit`** — UltraQA fixes code but NEVER commits. The user decides when to commit.
- **Max 5 cycles** — Stop after 5 fix-verify iterations regardless of outcome.
- **Stop on repeat failure** — If the same failure appears 3 times in a row, stop and report.
- **One goal at a time** — Each invocation targets a single verification goal.

## Workflow

### Step 1: Detect Verification Command

Based on the argument, determine the command to run:

| Goal | Detection | Command |
|------|-----------|---------|
| `--tests` | `package.json` → bun test; `pyproject.toml` → pytest; `Cargo.toml` → cargo test | Auto-detected |
| `--build` | `package.json` → bun run build; `Cargo.toml` → cargo build | Auto-detected |
| `--lint` | `package.json` → bun run lint; `.eslintrc*` → eslint | Auto-detected |
| `--typecheck` | `tsconfig.json` → tsc --noEmit; `pyproject.toml` → mypy | Auto-detected |
| `--custom` | User-provided command | Exact command |

If auto-detection fails, ask the user:

```
AskUserQuestion(
  questions: [{
    question: "What command should I run for verification?",
    header: "QA Command",
    options: [
      { label: "bun test", description: "JavaScript/TypeScript tests" },
      { label: "pytest", description: "Python tests" },
      { label: "cargo test", description: "Rust tests" }
    ],
    multiSelect: false
  }]
)
```

### Step 2: Initial Run

Run the verification command and capture output:

```bash
{command} 2>&1
```

Save state:

```json
// .maestro/handoff/ultraqa-state.json
{
  "goal": "--tests",
  "command": "bun test",
  "cycle": 1,
  "max_cycles": 5,
  "status": "running",
  "failures": [],
  "repeat_count": 0,
  "started": "{ISO timestamp}"
}
```

If the command passes on first run:
> All checks pass. No fixes needed.

Exit immediately.

### Step 3: Diagnose

Create a team for diagnosis:

```
TeamCreate(team_name: "ultraqa-{goal}", description: "UltraQA {goal} cycle {N}")
```

Parse the failure output. Spawn workers:

- `oracle` — for complex/unclear failures: "Analyze this failure output and identify root cause: {output}"
- `build-fixer` — for build/lint/typecheck errors with clear error messages
- `kraken` — for test failures requiring new test fixtures or multi-file changes

**Worker selection heuristic:**
- Clear error message with file:line → `build-fixer`
- Test assertion failure → `kraken`
- Unclear or cascading failures → `oracle` for diagnosis first, then `kraken`/`build-fixer` for fix

### Step 4: Fix

Delegate the fix to the appropriate worker. Provide:
- Full error output
- Relevant file paths
- What the verification command expects

### Step 5: Re-Verify

Run the same verification command again. Update state:

```json
{
  "cycle": 2,
  "status": "running",
  "failures": ["previous failure summary"],
  "repeat_count": 0
}
```

### Step 6: Loop or Stop

**Continue** if:
- New failures (different from previous) AND cycle < 5

**Stop** if ANY of these:
- All checks pass → report SUCCESS
- Cycle >= 5 → report CYCLE_LIMIT
- Same failure 3x → report STUCK
- No actionable fix identified → report BLOCKED

### Step 7: Report

```markdown
## UltraQA Report

### Result: SUCCESS | CYCLE_LIMIT | STUCK | BLOCKED

### Cycles Run: N/5
### Goal: {goal}
### Command: `{command}`

### Fixes Applied
1. Cycle 1: [what was fixed] — [file:line]
2. Cycle 2: [what was fixed] — [file:line]

### Remaining Failures (if any)
- [failure description]

### State File
`.maestro/handoff/ultraqa-state.json`
```

### Step 8: Cleanup

```
TeamDelete()
```

Update state file:

```json
{
  "status": "completed",
  "result": "SUCCESS",
  "cycles_used": 3,
  "completed": "{ISO timestamp}"
}
```

## State Management

State is persisted at `.maestro/handoff/ultraqa-state.json` so that:
- `/status` can report on active UltraQA sessions
- Interrupted sessions can be diagnosed
- Results are available for post-mortem

## Anti-Patterns

| Don't | Do Instead |
|-------|-----------|
| Commit fixes | Leave uncommitted — user decides when |
| Run multiple goals | One goal per invocation |
| Keep going past 5 cycles | Stop and report |
| Retry same fix | If a fix didn't work, try a different approach |
| Fix unrelated issues | Only fix failures from the verification command |
