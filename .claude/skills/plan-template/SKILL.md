---
name: plan-template
description: Scaffold a blank plan with required sections in .maestro/plans/. Use /plan-template <name> to create a new plan file.
user-invocable: true
disable-model-invocation: true
---

# Plan Template

> Scaffold a blank plan file with all required sections.

## Usage

```
/plan-template <name>
```

## Process

1. Take the `$ARGUMENTS` as the plan name
2. Convert to a filename slug (lowercase, hyphens, no spaces)
3. Create the plan file at `.maestro/plans/{slug}.md`

## Template

Write the following template to `.maestro/plans/{slug}.md`:

```markdown
# {Plan Name}

**Goal**: <!-- One sentence — what are we building and why -->
**Architecture**: <!-- 2-3 sentences — how the pieces fit together -->
**Tech Stack**: <!-- Relevant technologies, frameworks, tools -->

## Objective
<!-- One sentence: what are we trying to achieve? -->

## Scope

**In scope:**
<!-- What we ARE doing -->
-

**Out of scope:**
<!-- What we are explicitly NOT doing -->
-

## Tasks

<!-- Each task = single atomic action. Include complete code/diffs — never vague instructions. -->

- [ ] Task 1: [Short title]
  - **Agent**: kraken | spark
  - **Acceptance criteria**: [Objectively verifiable outcomes]
  - **Dependencies**: none
  - **Files**: [Exact paths to create/modify/test]
  - **Steps**:
    1. Write failing test (if applicable)
    2. Run test — expect failure
    3. Implement the change
    4. Run tests — expect pass
    5. Commit

## Verification

<!-- Exact commands with expected output -->
- [ ] `command here` — expected output or behavior
- [ ] `another command` — what it verifies

## Notes

<!-- Technical decisions, constraints, research findings -->
-
```

4. Report: "Plan template created at `.maestro/plans/{slug}.md` — fill in the sections and run `/work` to execute."
