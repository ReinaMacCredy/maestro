# Agent Prompt Templates

Concrete templates for constructing worker prompts by task type. Each template includes the minimum viable context a worker needs to succeed.

## Template Structure

Every worker prompt follows this skeleton:

```
[GOAL] - One sentence. What does "done" look like?
[SCOPE] - Which files/directories the worker may touch.
[CONTEXT] - Error messages, test names, API signatures -- whatever the worker needs to understand the problem without exploring.
[CONSTRAINTS] - What the worker must NOT do.
[OUTPUT] - What the worker must report back.
```

---

## Implementation Task

Use when adding a new feature or capability.

```markdown
## Goal

Implement [feature name]: [one-sentence description of behavior].

## Scope

- Primary files: `src/[module]/[file].ts`
- Test file: `src/__tests__/[file].test.ts`
- You may create new files under `src/[module]/` if needed.
- Do NOT modify files outside this scope.

## Context

This feature is part of [broader feature]. Related code:
- `src/[module]/types.ts` defines the types you will use (see `InterfaceName`)
- `src/[module]/index.ts` exports the public API -- add your export here
- The existing pattern for similar features is [describe pattern or point to example file]

Design decisions:
- [Key decision 1 and why]
- [Key decision 2 and why]

## Constraints

- Follow existing code patterns in `src/[module]/`
- Write tests first (TDD) -- see `maestro:tdd` skill
- No new dependencies unless the existing stack cannot solve it
- Do NOT refactor adjacent code

## Expected Output

- Implementation in `src/[module]/[file].ts`
- Tests in `src/__tests__/[file].test.ts`
- All tests pass (`bun test [path]`)
- Summary: what you implemented, any design choices you made
```

---

## Testing Task

Use when adding tests for existing code, improving coverage, or writing integration tests.

```markdown
## Goal

Write tests for [module/function]: cover [specific behaviors].

## Scope

- Test file: `src/__tests__/[file].test.ts`
- Read-only: `src/[module]/[file].ts` (the code under test)
- Do NOT modify production code.

## Context

The function/module does: [brief description of what it does]
Current coverage gaps:
- [Missing case 1: e.g., "error path when network fails"]
- [Missing case 2: e.g., "edge case with empty input"]
- [Missing case 3: e.g., "concurrent access"]

Existing test patterns in this project:
- Test runner: `bun test`
- Assertion style: `expect(x).toBe(y)` (vitest/bun:test)
- Mocking: [describe mocking approach or say "no mocks -- use real implementations"]

## Constraints

- Do NOT modify production code to make tests easier
- Each test should verify one behavior
- Use descriptive test names: `test('rejects input when X is missing', ...)`
- If a test is hard to write, note the design smell in your summary -- do not redesign

## Expected Output

- Test file with [N]+ test cases covering the listed gaps
- All tests pass
- Summary: what you tested, any design concerns discovered
```

---

## Bugfix Task

Use when fixing a known bug with a reproducible symptom.

```markdown
## Goal

Fix: [bug description]. Currently [what happens]. Should [what should happen].

## Scope

- Likely location: `src/[module]/[file].ts` (but investigate -- root cause may be elsewhere)
- Test file: `src/__tests__/[file].test.ts`
- You may touch other files if the root cause is there, but explain why.

## Context

Reproduction:
```
[Exact error message, stack trace, or failing test output]
```

Steps to reproduce:
1. [Step 1]
2. [Step 2]
3. [Observe: error / wrong behavior]

Hypothesis (if any): [Your best guess at root cause, or "unknown -- investigate"]

Related recent changes: [mention recent commits or PRs that might have caused this]

## Constraints

- Write a failing test FIRST that reproduces the bug
- Fix the root cause, not the symptom
- Do NOT increase timeouts or add retries as a "fix"
- Minimize blast radius -- smallest change that fixes the bug

## Expected Output

- Failing test that reproduces the bug
- Fix that makes the test pass
- All other tests still pass
- Summary: root cause, what you changed, why this fix is correct
```

---

## Refactoring Task

Use when restructuring code without changing behavior.

```markdown
## Goal

Refactor [module/area]: [what structural change and why].

## Scope

- Files to refactor: `src/[module]/[files]`
- Test files: `src/__tests__/[files]` (must stay green throughout)
- Do NOT change behavior. If you find a bug, note it but do not fix it.

## Context

Current structure: [describe what exists and why it is problematic]
Target structure: [describe what it should look like after refactoring]

Motivation:
- [Why this refactoring matters: e.g., "module has grown to 800 lines", "circular dependency"]

Existing tests that must keep passing:
- `src/__tests__/[test1].test.ts`
- `src/__tests__/[test2].test.ts`

## Constraints

- Zero behavior changes -- tests must pass before and after every step
- Refactor in small incremental steps (move one function, verify, repeat)
- Do NOT rename public API exports without noting it in summary
- Do NOT mix refactoring with behavior changes

## Expected Output

- Refactored code with same behavior
- All existing tests pass (no test modifications unless imports changed)
- Summary: what you moved/renamed/extracted, any follow-up work needed
```

---

## Anti-Patterns in Worker Prompts

| Mistake | Why It Fails | Fix |
|---------|-------------|-----|
| "Fix the tests" | Too broad -- worker thrashes | Name specific test files and failure messages |
| Pasting 200 lines of context | Drowns the signal | Paste only the error, the relevant function signature, and the design decision |
| No constraints section | Worker refactors the world | Always say what NOT to touch |
| "Return: done" | You cannot verify the work | Ask for root cause, changes made, test results |
| Including irrelevant files in scope | Worker reads everything | List only files relevant to the task |
| Omitting existing patterns | Worker invents new patterns | Point to an example file that shows the convention |
