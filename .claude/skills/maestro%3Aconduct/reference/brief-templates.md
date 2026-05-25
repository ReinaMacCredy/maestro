# Agent Brief Templates

Three ready-to-use templates for common conductor dispatch scenarios. Each follows the five-section format (Goal, Scope, Context, Constraints, Output).

---

## Template 1: Mission Feature Implementation

Use when dispatching an agent for a mission feature. The `maestro feature prompt` command generates most of the content -- this template shows how to augment it with conductor notes.

```
## Goal
Implement feature {featureId}: {one-sentence description of done state}.

## Scope
Files to create or modify:
- {file1} -- {what changes}
- {file2} -- {what changes}

Out of bounds:
- {dir/file that must not be touched}
- {pattern to avoid, e.g. "do not modify the public API surface of X"}

## Context
Mission: {missionId} -- {mission description}
Milestone: {milestoneId} ({profile}) -- {milestone description}

Prior decisions:
- {decision from brainstorm or earlier milestone that constrains this feature}

Related features in this milestone:
- {featureId}: {status} -- {brief description, so agent knows what siblings are doing}

Relevant code:
- {function signature or file path the agent needs to know about}
- {error message or test output that motivated this feature}

## Constraints
- Follow maestro:agent-base startup/cleanup procedures
- Do NOT commit -- conductor commits after verification
- Do NOT modify .maestro/ files
- Do NOT touch files outside the Scope section
- {feature-specific constraint, e.g. "preserve backward compatibility with v2 API"}

## Output
Report back with:
- Files created/modified (with brief description of each change)
- Tests added (file, case name, what each verifies)
- Verification commands run and their results
- Issues discovered (blocking, non-blocking, or suggestions)
- If blocked: what you need and why, via EndFeatureRun with returnToOrchestrator: true
```

---

## Template 2: Bug Investigation then Fix

Use for bugs where the root cause is unknown. This is a two-phase dispatch: investigate first, then fix after the conductor synthesizes findings.

### Phase 1: Investigation

```
## Goal
Identify the root cause of: {bug description / error message}.

## Scope
Start investigation in:
- {file or directory where the bug manifests}
- {test file that reproduces it, if known}

You may read any file in the codebase. Do NOT edit any files in this phase.

## Context
Symptoms:
- {what the user observed}
- {error message, stack trace, or failing test output}

What has been tried:
- {any prior debugging attempts and their results}
- {hypotheses that have been ruled out}

## Constraints
- READ ONLY -- no edits, no fixes, no refactoring
- Do NOT run destructive commands
- If you need to run a command to reproduce, describe it first

## Output
Report back with:
- Root cause (file, line, explanation of why it fails)
- Reproduction steps (minimal command or test that triggers the bug)
- Suggested fix (description, not implementation)
- Confidence level (certain / likely / uncertain -- be honest)
- Blast radius (what else might be affected by the fix)
```

### Phase 2: Fix (dispatched after conductor reviews investigation)

```
## Goal
Fix the bug identified in {investigation summary}: {root cause in one sentence}.

## Scope
Files to modify:
- {file1}:{line range} -- {what to change}

Test files:
- {test file} -- add regression test for this bug

## Context
Root cause: {from investigation report}
Reproduction: {minimal reproduction steps}
Blast radius: {from investigation report}

## Constraints
- Fix the root cause, not a symptom
- Add a regression test that fails without the fix and passes with it
- Do NOT refactor surrounding code
- Do NOT commit

## Output
Report back with:
- Files changed (with diff summary)
- Regression test added (file, case name, what it verifies)
- All test suite results (command run, pass/fail count)
- Any side effects or risks from the fix
```

---

## Template 3: Exploration / Research

Use when you need information before making decisions. Agents read and report; they do not edit.

```
## Goal
Answer: {specific question, e.g. "How many callers of validateSession() exist and what do they pass as the second argument?"}

## Scope
Search area:
- {directory or file pattern to search}
- {specific files to read if known}

## Context
Why this matters:
- {what decision this research informs}
- {what you already know that narrows the search}

## Constraints
- READ ONLY -- no edits, no fixes, no refactoring
- Do NOT run tests or builds
- If the search area is too large, report what you found and what you skipped

## Output
Report back with:
- Direct answer to the question
- Evidence (file paths, line numbers, code snippets)
- Anything unexpected you found that the conductor should know about
- If the answer is "it depends" or "unclear": state what additional information would resolve it
```
