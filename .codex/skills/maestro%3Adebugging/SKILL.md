---
name: maestro:debugging
description: Use when encountering any bug, test failure, or unexpected behavior, before proposing fixes
---

# Systematic Debugging

## Overview

Random fixes waste time and create new bugs. Quick patches mask underlying issues.

**Core principle:** ALWAYS find root cause before attempting fixes. Symptom fixes are failure.

**Violating the letter of this process is violating the spirit of debugging.**

## The Iron Law

```
NO FIXES WITHOUT ROOT CAUSE INVESTIGATION FIRST
```

If you haven't completed Phase 1, you cannot propose fixes.

## When to Use

Use for ANY technical issue:
- Test failures
- Bugs in production
- Unexpected behavior
- Performance problems
- Build failures
- Integration issues

**Use this ESPECIALLY when:**
- Under time pressure (emergencies make guessing tempting)
- "Just one quick fix" seems obvious
- You've already tried multiple fixes
- Previous fix didn't work
- You don't fully understand the issue

**Don't skip when:**
- Issue seems simple (simple bugs have root causes too)
- You're in a hurry (rushing guarantees rework)
- Manager wants it fixed NOW (systematic is faster than thrashing)

## Debugging Decision Tree

Given a symptom, start with the highest-probability approach:

| Symptom | First Move | Tool |
|---------|-----------|------|
| Runtime error / exception | Read full stack trace backward | `bun test <file> 2>&1 \| tail -80` |
| Type error (compile-time) | Find "expected X, got Y" mismatch source | `rg 'type Y\b\|interface Y\b' --type ts` |
| Test passes alone, fails in suite | Shared state pollution | Run pairs: `bun test a.test.ts b.test.ts` |
| Flaky / intermittent | Reproduce in loop, add timestamps | `for i in {1..50}; do bun test <file> \|\| break; done` |
| Performance issue | Measure before guessing | `time bun run script.ts` + instrumentation |
| Integration failure | Verify each boundary in isolation | `curl` each endpoint, check env vars |
| Build failure | Read the FIRST error only | `bun run build 2>&1 \| head -30` |

See `reference/debugging-patterns.md` for detailed workflows per symptom type.

## The Four Phases

You MUST complete each phase before proceeding to the next.

### Phase 1: Reproduce

**BEFORE attempting ANY fix, get a single command that triggers the bug.**

1. **Read Error Messages Carefully**
   - Read stack traces completely -- don't skip past them
   - Note line numbers, file paths, error codes
   - The error often contains the exact solution

   ```bash
   # Get full error with context
   bun test path/to/failing.test.ts 2>&1 | tail -80

   # For deep stack traces
   NODE_OPTIONS='--stack-trace-limit=50' bun run script.ts
   ```

2. **Reproduce Consistently**
   - Can you trigger it reliably with a single command?
   - If not reproducible: gather more data, don't guess

   ```bash
   # Run the specific failing test in isolation
   bun test path/to/failing.test.ts --bail

   # Reproduce with clean state
   rm -rf node_modules/.cache dist/
   bun install && bun test path/to/failing.test.ts
   ```

3. **Check Recent Changes**

   ```bash
   # What changed since last green state?
   git log --oneline -20
   git diff HEAD~3 -- src/

   # Binary search for breaking commit
   git bisect start && git bisect bad HEAD && git bisect good <good-commit>
   git bisect run bun test path/to/failing.test.ts
   ```

### Phase 2: Isolate

**Narrow from "something broke" to "THIS line is the cause."**

1. **Trace Data Flow**
   - Where does the bad value originate?
   - What called this with the bad value?
   - Keep tracing backward until you find the source
   - Fix at source, not at symptom

   ```bash
   # Find all assignments to the suspect variable
   rg 'myVariable\s*=' --type ts -n

   # Find all callers of the broken function
   sg run -p 'brokenFunction($$$ARGS)' --lang typescript

   # Find where an error is thrown
   rg 'throw new.*ErrorMessage' --type ts -n -C 3
   ```

   See `root-cause-tracing.md` for the complete backward tracing technique.

2. **Gather Evidence at Boundaries (multi-component systems)**

   When system has multiple components (CI --> build --> signing, API --> service --> database):

   ```
   For EACH component boundary:
     - Log what data enters
     - Log what data exits
     - Verify environment/config propagation

   Run once --> read output --> identify failing boundary
   ```

   ```bash
   # Example: identify which layer fails
   echo "=== Layer 1: Workflow ==="
   echo "IDENTITY: ${IDENTITY:+SET}${IDENTITY:-UNSET}"

   echo "=== Layer 2: Build script ==="
   env | grep IDENTITY || echo "IDENTITY not in environment"

   echo "=== Layer 3: Signing ==="
   security find-identity -v
   ```

3. **Find Working Examples and Compare**
   - Locate similar working code in same codebase
   - List every difference, however small
   - Don't assume "that can't matter"

   ```bash
   # Find working examples of the same pattern
   sg run -p 'workingPattern($$$ARGS)' --lang typescript
   rg 'similarFunction\(' --type ts --files-with-matches
   ```

### Phase 3: Hypothesize and Test

**Scientific method -- one variable at a time.**

1. **Form Single Hypothesis**
   - State clearly: "I think X is the root cause because evidence Y shows Z"
   - Be specific, not vague

2. **Test Minimally**
   - Make the SMALLEST possible change to test the hypothesis
   - One variable at a time
   - Don't fix multiple things at once

3. **Evaluate**
   - Did it work? Yes --> Phase 4
   - Didn't work? Form NEW hypothesis with the new evidence
   - DON'T stack fixes on top of each other

4. **When You Don't Know**
   - Say "I don't understand X" -- don't pretend
   - Research more before guessing

### Phase 4: Fix and Verify

**Fix the root cause, not the symptom.**

1. **Create Failing Test Case**

   ```bash
   # Write the test
   # (use maestro skill maestro:tdd for proper TDD technique)

   # MUST fail before you fix
   bun test path/to/module.test.ts --bail
   # Expected: FAIL
   ```

2. **Implement Single Fix**
   - Address the root cause identified in Phase 2
   - ONE change at a time
   - No "while I'm here" improvements
   - No bundled refactoring

3. **Verify Completely**

   ```bash
   # Regression test passes
   bun test path/to/module.test.ts --bail

   # Full suite passes
   bun test

   # Build succeeds
   bun run build

   # For flaky bugs: run multiple times
   for i in {1..20}; do bun test path/to/module.test.ts || break; done

   # Check for sibling bugs (same broken pattern elsewhere)
   rg 'the-broken-pattern' --type ts --files-with-matches
   ```

4. **If Fix Doesn't Work**
   - STOP
   - Count: How many fixes have you tried?
   - If < 3: Return to Phase 1 with new evidence
   - **If >= 3: STOP -- this is architectural (see below)**
   - DON'T attempt Fix #4 without architectural discussion

5. **If 3+ Fixes Failed: Question Architecture**

   **Pattern indicating architectural problem:**
   - Each fix reveals new shared state / coupling in a different place
   - Fixes require "massive refactoring" to implement
   - Each fix creates new symptoms elsewhere

   **STOP and question fundamentals:**
   - Is this pattern fundamentally sound?
   - Are we "sticking with it through sheer inertia"?
   - Should we refactor architecture vs. continue fixing symptoms?

   **Discuss with your human partner before attempting more fixes.**

   This is NOT a failed hypothesis -- this is a wrong architecture.

## Red Flags -- STOP and Follow Process

### Thought Patterns (you catch yourself thinking)

- "Quick fix for now, investigate later"
- "Just try changing X and see if it works"
- "It's probably X, let me fix that"
- "I don't fully understand but this might work"
- "Here are the main problems: [lists fixes without investigation]"

### Action Patterns (you're already doing this)

- Making multiple changes before testing any of them
- Proposing a fix before tracing data flow
- Adding `as any`, `@ts-ignore`, or `// eslint-disable` to silence errors
- Adding defensive null checks instead of ensuring values exist at source
- Skipping test creation: "I'll manually verify"
- Copy-pasting a fix from StackOverflow without understanding it
- **Fix #3 on the same bug** (each revealing a new problem in a different place)

### Code Patterns (you see these in your diff)

| Pattern in Diff | What It Means |
|----------------|---------------|
| `as any` or `as unknown as X` | Silencing the type system, not fixing the mismatch |
| `?.` chains added defensively | Hiding null propagation instead of fixing the source |
| `try { } catch { }` with empty catch | Swallowing errors that need handling |
| `setTimeout` / `sleep` added | Papering over a race condition |
| `// TODO: fix later` | You know it's wrong and you're shipping it anyway |
| Multiple files changed for "one fix" | You're doing shotgun debugging |

**ALL of these mean: STOP. Return to Phase 1.**

**If 3+ fixes failed:** Question the architecture (see Phase 4, step 5).

## Your Human Partner's Signals

**Watch for these redirections:**
- "Is that not happening?" -- You assumed without verifying
- "Will it show us...?" -- You should have added evidence gathering
- "Stop guessing" -- You're proposing fixes without understanding
- "Ultrathink this" -- Question fundamentals, not just symptoms
- "We're stuck?" (frustrated) -- Your approach isn't working

**When you see these:** STOP. Return to Phase 1.

## Common Rationalizations

| Excuse | Reality |
|--------|---------|
| "Issue is simple, don't need process" | Simple issues have root causes too. Process is fast for simple bugs. |
| "Emergency, no time for process" | Systematic debugging is FASTER than guess-and-check thrashing. |
| "Just try this first, then investigate" | First fix sets the pattern. Do it right from the start. |
| "I'll write test after confirming fix works" | Untested fixes don't stick. Test first proves it. |
| "Multiple fixes at once saves time" | Can't isolate what worked. Causes new bugs. |
| "Reference too long, I'll adapt the pattern" | Partial understanding guarantees bugs. Read it completely. |
| "I see the problem, let me fix it" | Seeing symptoms ≠ understanding root cause. |
| "One more fix attempt" (after 2+ failures) | 3+ failures = architectural problem. Question pattern, don't fix again. |

## Quick Reference

| Phase | Key Activities | Success Criteria |
|-------|---------------|------------------|
| **1. Root Cause** | Read errors, reproduce, check changes, gather evidence | Understand WHAT and WHY |
| **2. Pattern** | Find working examples, compare | Identify differences |
| **3. Hypothesis** | Form theory, test minimally | Confirmed or new hypothesis |
| **4. Implementation** | Create test, fix, verify | Bug resolved, tests pass |

## When Process Reveals "No Root Cause"

If systematic investigation reveals issue is truly environmental, timing-dependent, or external:

1. You've completed the process
2. Document what you investigated
3. Implement appropriate handling (retry, timeout, error message)
4. Add monitoring/logging for future investigation

**But:** 95% of "no root cause" cases are incomplete investigation.

## Supporting Techniques

**In this directory:**

- **`reference/debugging-patterns.md`** - Concrete workflows per symptom type: runtime errors, type errors, test failures, performance, integration. Decision tree, tool commands, anti-patterns.
- **`root-cause-tracing.md`** - Trace bugs backward through call stack to find original trigger
- **`defense-in-depth.md`** - Add validation at multiple layers after finding root cause
- **`condition-based-waiting.md`** - Replace arbitrary timeouts with condition polling

**Related skills:**
- **maestro skill maestro:tdd** - For creating failing test case (Phase 4, Step 1)
- **maestro skill maestro:verification** - Verify fix worked before claiming success

## Real-World Impact

From debugging sessions:
- Systematic approach: 15-30 minutes to fix
- Random fixes approach: 2-3 hours of thrashing
- First-time fix rate: 95% vs 40%
- New bugs introduced: Near zero vs common
