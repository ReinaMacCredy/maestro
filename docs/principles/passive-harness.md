# passive-harness

## Rule

Maestro never schedules background work. No `setInterval`, no detached daemons, no LLM calls, no auto-launching subprocesses. Every state transition must be the direct result of an agent invoking a CLI verb.

## Rationale

"Passive harness" is the load-bearing harness invariant. It is what lets a single repo-tracked state directory be safely shared across multiple agents and human operators: nothing is mutating the world while you read it. The moment Maestro starts its own timers or workers, that contract breaks and recovery becomes guesswork.

## Scan Command

! rg -n "setInterval|setTimeout|child_process\.fork|spawn.*detached|new Worker\(" --glob 'src/**' --glob '!**/*.test.ts' --glob '!src/repo/bun-process-runner.adapter.ts'

## Fix Recipe

1. Remove the background scheduler.
2. Turn the work into a CLI verb the agent (or external cron) invokes explicitly.
3. Record the resulting state change as an evidence row so the next verb call can see it.
4. Re-run the scan; the file should drop out of the result set.
