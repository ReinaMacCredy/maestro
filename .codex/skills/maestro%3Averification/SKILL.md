---
name: maestro:verification
description: Use when about to claim work is complete, fixed, or passing, before committing or creating PRs - requires running verification commands and confirming output before making any success claims; evidence before assertions always
---

# Verification Before Completion

## Overview

Claiming work is complete without verification is dishonesty, not efficiency.

**Core principle:** Evidence before claims, always.

**Violating the letter of this rule is violating the spirit of this rule.**

## When to Use

**Always before:**
- Claiming a task is complete
- Committing code
- Creating a pull request
- Merging a worktree
- Moving to the next task
- Reporting status to your human partner
- Marking a feature complete

**Three scopes of verification:**

| Scope | When | What to verify |
|-------|------|----------------|
| **Task** | After completing a single task | Build, tests, lint for changed code |
| **Phase** | After merging multiple tasks in a plan phase | Cross-module integration, no regressions |
| **Feature** | Before `feature-complete` | Full test suite, all acceptance criteria, end-to-end |

Thinking "verification scope doesn't matter"? It does. A task-level check misses integration bugs. A feature-level check on every commit wastes time.

## The Iron Law

```
NO COMPLETION CLAIMS WITHOUT FRESH VERIFICATION EVIDENCE
```

If you haven't run the verification command in this message, you cannot claim it passes.

"Fresh" means:
- Run after your latest change (not before it)
- Full output examined (not truncated or skimmed)
- Exit code checked (not assumed)

## The Gate Function

```
BEFORE claiming any status or expressing satisfaction:

1. IDENTIFY: What commands prove this claim?
   - Build: compiler/bundler exit code
   - Tests: test runner output with pass/fail counts
   - Lint: linter output with error/warning counts
   - Types: type checker output
   - Integration: cross-module test suite
   - Acceptance: line-by-line requirement check

2. RUN: Execute EVERY required command (fresh, complete)
   - Do not rely on cached results
   - Do not run partial suites
   - Do not skip "slow" checks

3. READ: Full output for each command
   - Check exit code (0 = success, non-zero = failure)
   - Count failures, errors, warnings
   - Read error messages -- do not skim

4. CROSS-CHECK: Does output match the claim?
   - Zero failures AND zero errors = pass
   - Any failure or error = fail (even if "unrelated")
   - Warnings warrant investigation

5. REPORT: State claim WITH evidence
   - If PASS: "Build passes (exit 0), 47/47 tests pass, 0 lint errors"
   - If FAIL: "Build fails: 2 type errors in src/parser.ts:34,56"

Skip any step = lying, not verifying
```

## Task Verification

After completing a single task, verify the task's own deliverables.

### What to Run

```bash
# 1. Build -- does it compile?
bun run build
# Check: exit 0, no errors

# 2. Tests -- do they pass?
bun test
# Check: all pass, 0 failures, 0 errors

# 3. Lint -- is the code clean?
bun run lint
# Check: 0 errors (warnings acceptable if pre-existing)

# 4. Type check (if separate from build)
bun run typecheck
# Check: 0 errors
```

### How to Interpret Results

<Good>
```
$ bun run build
Build completed in 1.2s

$ bun test
 PASS  src/parser.test.ts (12 tests)
 PASS  src/cli.test.ts (8 tests)
Tests: 20 passed, 0 failed
```
Clear pass. State: "Build passes, 20/20 tests pass."
</Good>

<Bad>
```
$ bun run build
Build completed in 1.2s

$ bun test
 PASS  src/parser.test.ts (12 tests)
 FAIL  src/cli.test.ts
   x handles empty input (expected "error" got undefined)
Tests: 19 passed, 1 failed
```
One failure. Do NOT say "tests mostly pass" or "just one unrelated failure."
State: "1 test fails: cli.test.ts 'handles empty input'. Investigating."
</Bad>

### Task Verification Checklist

- [ ] Build exits 0
- [ ] All tests pass (exact count reported)
- [ ] Lint reports 0 errors
- [ ] Type check reports 0 errors
- [ ] Changed files reviewed in diff
- [ ] Task acceptance criteria checked line-by-line

## Phase Verification

After merging multiple tasks that form a plan phase, verify integration.

### What to Run

Everything from Task Verification, plus:

```bash
# 1. Full test suite (not just changed files)
bun test
# Check: ALL tests pass, including ones from other tasks

# 2. Integration tests specifically
bun test --grep "integration"
# Check: cross-module interactions work

# 3. Git status -- clean working tree
git status
# Check: no untracked files that should be committed
# Check: no uncommitted changes

# 4. Diff against main -- review all changes in the phase
git diff main...HEAD --stat
# Check: only expected files changed
# Check: no accidental inclusions (lockfiles, generated code, secrets)
```

### Phase-Specific Checks

| Phase type | Additional verification |
|------------|----------------------|
| API changes | Contract tests pass, no breaking changes to consumers |
| Schema changes | Migration runs clean, rollback tested |
| Dependency updates | Full build from clean state, no version conflicts |
| Refactoring | Behavior tests unchanged, coverage not decreased |
| New feature | Feature flag works in both states (on/off) |

### Phase Verification Checklist

- [ ] All task-level verifications pass
- [ ] Full test suite passes (not just per-task tests)
- [ ] No untracked or uncommitted changes
- [ ] Diff review: only expected files changed
- [ ] Cross-module integration verified
- [ ] No new warnings introduced
- [ ] Phase acceptance criteria from plan checked line-by-line

## Feature Verification

Before calling `feature-complete`, verify everything end-to-end.

### What to Run

Everything from Phase Verification, plus:

```bash
# 1. Clean build from scratch
rm -rf node_modules dist .cache
bun install
bun run build
# Check: builds from clean state

# 2. Full test suite
bun test
# Check: every test passes

# 3. End-to-end / smoke test
# Run the actual feature manually or via E2E tests
bun test:e2e
# Check: feature works as specified

# 4. Diff against main -- full feature review
git log main..HEAD --oneline
git diff main...HEAD --stat
# Check: all commits are intentional
# Check: no debug code, console.logs, TODO hacks
```

### Feature Verification Checklist

- [ ] Clean build from scratch succeeds
- [ ] Full test suite passes
- [ ] E2E / smoke tests pass
- [ ] Every acceptance criterion verified with evidence
- [ ] No debug code or temporary hacks in diff
- [ ] No secrets, credentials, or .env files in diff
- [ ] Documentation updated (if required by plan)
- [ ] Breaking changes documented (if any)
- [ ] Rollback strategy confirmed (feature flag, revert plan)

## Post-Merge Verification

After merging a worktree back to the main branch, the merge itself can introduce problems.

### The Merge Verification Protocol

```
1. MERGE: Complete the worktree merge

2. BUILD: Immediately build on the target branch
   - Merge conflicts resolved incorrectly cause build failures
   - "It built in the worktree" does not mean it builds after merge

3. TEST: Run full test suite on target branch
   - Tests that passed in isolation may fail when combined
   - Other worktrees' tests may break from your changes

4. DIFF: Review the merge commit
   - git diff HEAD~1 -- verify only expected changes landed
   - Check for conflict markers (<<<<<<< ======= >>>>>>>)
   - Check for duplicate code from bad conflict resolution

5. REPORT: State merge result with evidence
   - "Merged task-3. Build passes, 47/47 tests pass on main."
   - NOT "Merge complete" (no evidence)
```

### Common Merge Failures

| Symptom | Cause | Fix |
|---------|-------|-----|
| Build fails after merge | Conflict resolved incorrectly | Review conflict markers, fix, rebuild |
| Tests fail after merge | Two tasks changed same behavior | Determine correct behavior, update test + code |
| New warnings after merge | Import order, unused vars from merge | Clean up merged code |
| Missing files after merge | Conflict deleted file incorrectly | Check git log for the file, restore |

## Cross-Worktree Verification

When multiple worktrees are active (parallel task execution), verify that work in one worktree does not conflict with another.

### Before Starting a New Worktree

```bash
# Check what other worktrees are active
maestro status
# Note: which files are being modified in other active tasks
```

### Before Merging Parallel Worktrees

```bash
# 1. Merge first worktree
maestro merge --task <first-task>

# 2. Verify on main
bun run build && bun test

# 3. Merge second worktree
maestro merge --task <second-task>

# 4. Verify again -- this is where integration breaks surface
bun run build && bun test

# 5. If tests fail: the second merge introduced a conflict
# Do NOT blame the first merge. Investigate the interaction.
```

### Cross-Worktree Red Flags

- Two tasks modifying the same file
- Two tasks changing the same API
- One task adding a dependency another task also adds (version conflict)
- One task renaming what another task imports

## Handling Verification Failures

When verification fails, follow this decision tree:

```
Verification failed
  |
  +--> Is the failure in YOUR changed code?
  |     |
  |     +--> YES: Fix it. Re-verify. Do not proceed until green.
  |     |
  |     +--> NO: Is it a pre-existing failure?
  |           |
  |           +--> YES: Document it. Verify YOUR changes don't make it worse.
  |           |         Proceed only if pre-existing failure is unrelated.
  |           |
  |           +--> NO: Is it caused by a parallel merge?
  |                 |
  |                 +--> YES: Coordinate with the other task.
  |                 |         Fix before proceeding.
  |                 |
  |                 +--> NO: Investigate further.
  |                           Do not proceed until understood.
  |
  +--> NEVER: Ignore failures. "Unrelated" failures are usually related.
  +--> NEVER: Skip re-verification after fixing. The fix might break something else.
  +--> NEVER: Mark task complete with known failures.
```

### Flaky Test Protocol

```
Test fails intermittently?

1. Run 3 times. If it fails 2+ times, it is a REAL failure. Fix it.
2. If it fails 1/3 times:
   - Document the flaky test
   - Investigate root cause (timing, state leakage, external dependency)
   - Do NOT ignore it. Flaky tests mask real failures.
3. If test is flaky AND unrelated to your changes:
   - Document in task report
   - Verify your changes with the flaky test excluded
   - File a follow-up to fix the flaky test
```

## Common Failures

| Claim | Requires | Not Sufficient |
|-------|----------|----------------|
| Tests pass | Test command output: 0 failures | Previous run, "should pass" |
| Linter clean | Linter output: 0 errors | Partial check, extrapolation |
| Build succeeds | Build command: exit 0 | Linter passing, logs look good |
| Bug fixed | Test original symptom: passes | Code changed, assumed fixed |
| Regression test works | Red-green cycle verified | Test passes once |
| Agent completed | VCS diff shows changes | Agent reports "success" |
| Requirements met | Line-by-line checklist | Tests passing |
| Merge clean | Build + tests on target branch | "No conflicts" |
| Feature complete | All acceptance criteria with evidence | "All tasks done" |

## Red Flags -- STOP

### Signs You Are About to Skip Verification

- Using "should", "probably", "seems to"
- Expressing satisfaction before verification ("Great!", "Perfect!", "Done!")
- About to commit/push/PR without running commands
- Trusting agent success reports without checking diff
- Relying on partial verification ("tests pass" without build check)
- Thinking "just this once"
- Tired and wanting work to be over
- **ANY wording implying success without having run verification**

### Signs Verification Is Incomplete

- Only ran build, not tests
- Only ran tests for changed files, not full suite
- Did not check exit code (command printed errors but you did not scroll up)
- Ran commands but did not read output carefully
- Verified in worktree but not after merge
- Checked code changes but not acceptance criteria
- Ran verification once but made changes after

### Signs Verification Is Unreliable

- Tests pass but coverage is low on changed code
- Build passes but with warnings you did not investigate
- Tests pass but they test mocks, not real behavior
- "All tests pass" but you added no new tests for new behavior
- Verification passes but you cannot explain what each check proved
- Same test suite has been passing for weeks with no new tests added

## Rationalization Prevention

| Excuse | Reality |
|--------|---------|
| "Should work now" | RUN the verification |
| "I'm confident" | Confidence is not evidence |
| "Just this once" | No exceptions |
| "Linter passed" | Linter is not compiler |
| "Agent said success" | Verify independently |
| "I'm tired" | Exhaustion is not an excuse |
| "Partial check is enough" | Partial proves nothing |
| "Different words so rule doesn't apply" | Spirit over letter |
| "Tests passed in the worktree" | Verify after merge |
| "It's just a refactor" | Refactors break things. Verify. |
| "No tests changed so they still pass" | Run them anyway. Prove it. |
| "The CI will catch it" | CI is a backstop, not a substitute |
| "I'll verify the next one more carefully" | Verify this one now |

## Verification Commands by Stack

Adapt to your project. The principle is the same everywhere.

| Stack | Build | Test | Lint | Types |
|-------|-------|------|------|-------|
| **TypeScript/Bun** | `bun run build` | `bun test` | `bun run lint` | `bun run typecheck` |
| **TypeScript/Node** | `npm run build` | `npm test` | `npm run lint` | `npx tsc --noEmit` |
| **Python/uv** | N/A | `uv run pytest` | `uv run ruff check` | `uv run mypy .` |
| **Rust** | `cargo build` | `cargo test` | `cargo clippy` | (included in build) |
| **Go** | `go build ./...` | `go test ./...` | `golangci-lint run` | (included in build) |
| **Java/Gradle** | `./gradlew build` | `./gradlew test` | `./gradlew check` | (included in build) |

## When Stuck

| Problem | Solution |
|---------|----------|
| Don't know what to verify | Re-read the task spec. Every requirement = one verification item. |
| Verification takes too long | Run targeted tests first (`bun test <file>`), then full suite. Never skip full suite. |
| Can't reproduce a failure | Clean state: `rm -rf node_modules dist && bun install && bun run build && bun test` |
| Flaky test blocking progress | Follow the Flaky Test Protocol above. |
| Pre-existing failures confuse results | Document them. Diff test results before/after your changes. |
| Not sure if failure is related | `git stash && bun test` -- if it fails without your changes, it is pre-existing. |
| Verification passes but behavior seems wrong | Manual smoke test. Automated tests can have blind spots. |

## Integration with Other Skills

| Skill | How verification interacts |
|-------|--------------------------|
| **maestro:tdd** | TDD provides red-green cycle for individual tests. Verification ensures the full suite passes after TDD work. |
| **maestro:review** | Review checks code quality. Verification checks code correctness. Both required before completion. |
| **maestro:implement** | Implementation produces code. Verification proves the code works. Never skip between them. |

## The Bottom Line

```
Run the command. Read the output. THEN claim the result.
```

Three scopes, one principle: evidence before claims.

- **Task**: build + test + lint + acceptance criteria
- **Phase**: full suite + integration + diff review
- **Feature**: clean build + full suite + E2E + every acceptance criterion

No shortcuts. No exceptions. This is non-negotiable.
