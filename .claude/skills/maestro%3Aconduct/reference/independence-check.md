# Independence Check

Before dispatching two or more agents in parallel, verify they are independent. This is the dispatch gate -- skip it and you get merge conflicts, wasted work, or silent bugs.

## The Five Tests

Two tasks are independent only if ALL five pass:

| # | Test | How to check |
|---|------|-------------|
| 1 | **No shared files** | List the files each task will create or modify. Any file appearing in both lists fails this test. |
| 2 | **No shared type mutations** | If both tasks modify the same interface, type, or schema, they are coupled even if they edit different files. |
| 3 | **No import dependencies** | If task A's output will be imported by task B's output, they must run sequentially (A first). |
| 4 | **Tests run in isolation** | Each task's tests must pass without the other task's changes present. |
| 5 | **Merge order invariant** | The final result must be the same regardless of which task's changes land first. |

**One failure = sequential, not parallel.** Do not dispatch in parallel and hope for the best.

## Decision Table

| Condition | Dispatch strategy |
|-----------|------------------|
| All 5 tests pass for every pair | Parallel -- dispatch all at once |
| Tests 1-2 pass but test 3 fails (import dependency) | Pipeline -- dispatch A, wait for completion, then dispatch B |
| Test 1 fails (shared files) | Sequential -- one at a time, same order |
| Unsure about any test | Sequential until you can verify |

## Quick File-Overlap Grid

For N tasks, build an NxN grid of file lists:

```
             Task A files    Task B files    Task C files
Task A       src/auth.ts     --              --
Task B       --              src/api.ts      --
Task C       --              --              src/db.ts
                                              src/api.ts  [!]
```

Task B and Task C both touch `src/api.ts` -- they cannot run in parallel.
Task A is independent of both -- it can run in parallel with either (but B and C must be sequential).

Dispatch plan: A + B in parallel, then C after B completes.

## Worked Example

**Scenario:** Conductor has 3 features to dispatch from a milestone.

- Feature 1: "Add rate limiting middleware" -- touches `src/middleware/rate-limit.ts` (new), `src/middleware/index.ts`
- Feature 2: "Add request logging" -- touches `src/middleware/logger.ts` (new), `src/middleware/index.ts`
- Feature 3: "Add health check endpoint" -- touches `src/routes/health.ts` (new), `src/routes/index.ts`

**Check:**
- Features 1 and 2: both modify `src/middleware/index.ts` --> test 1 FAILS --> sequential
- Features 1 and 3: no shared files, no shared types, no import deps --> all tests PASS --> parallel
- Features 2 and 3: no shared files --> all tests PASS --> parallel

**Dispatch plan:**
- Wave 1: Feature 1 + Feature 3 (parallel)
- Wave 2: Feature 2 (after Feature 1 completes, since they share `middleware/index.ts`)

**Report to user before dispatching:**

```
Independence check complete:
- Feature 1 (rate limiting) + Feature 3 (health check): independent -- parallel dispatch
- Feature 2 (request logging): depends on Feature 1 (shared middleware/index.ts) -- sequential after Feature 1
Dispatch plan: Wave 1 = [1, 3], Wave 2 = [2]
Proceed?
```

Always show the dispatch plan and get user confirmation before sending agents out.
