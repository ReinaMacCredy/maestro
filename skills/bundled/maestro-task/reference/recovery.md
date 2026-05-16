# Task Recovery Patterns

When things go wrong: dead sessions, stale claims, stuck work, runaway local state.

## Dead session still owns tasks

A crashed or exited session can leave tasks claimed. Release them:

```bash
maestro task release-owned <deadSessionId>
```

Accepts bare session ids or canonical owner ids like `claude-code-pickup-1`. Manual operator sessions starting with `claude-` are preserved by `task ready` and need explicit cleanup.

## Stale claim (no heartbeat for a while)

Take over an aged-out claim:

```bash
maestro task claim <id> --stale-after 4h
```

Active contracts follow the new owner unless `contracts.staleReclaimContractPolicy: block` is set.

## Unclaim a single task

```bash
maestro task unclaim <id>
```

## Stuck tasks

In-progress tasks with no recent activity:

```bash
maestro task stuck [--older-than 4h]
```

Use this to find tasks that need a heartbeat bump or ownership transfer.

## Keep a long-running claim alive

```bash
maestro task heartbeat <id>
```

Run periodically from inside long work so the claim does not age out under the current `stale-after` threshold.

## Local artifact growth

`.maestro/tasks/candidates/` and `.maestro/tasks/continuations/completed/` are local-only (gitignored) and grow over time. Prune:

```bash
maestro task prune --dry-run                                # preview
maestro task prune                                          # keep newest 500 per kind (default)
maestro task prune --keep 1000                              # override cap
maestro task prune --candidates-only                        # only candidates
maestro task prune --continuations-only                     # only completed continuations
maestro task prune --all                                    # purge everything in those dirs
maestro task prune --json                                   # machine-readable output
```

## Force-delete a claimed task

```bash
maestro task delete <id> --force
```

Without `--force`, claimed tasks can only be deleted by their owner session.

## Reopening a completed task

```bash
maestro task reopen <id>
```

Reactivates an active contract, clears the stored verdict, preserves amendment history. Do this before resuming work on the task.

## Continuation intents in chat

When a session starts with an active task, maestro session hooks inject a continuation pointer. Saying `continue` or `resume` in chat loads the full continuation state (current state, next action, active decisions, recent history). These are chat intents, not CLI commands.

Inspect raw state:

```bash
maestro task show <id>
```

## Handoff transfer

To deliberately transfer work to another agent, do not manually unclaim. Run `maestro task block <id> --reason "<context>"`; that emits a handoff envelope at `.maestro/handoffs/<hnd-...>.json`. The receiving agent reads the envelope (see `maestro-handoff`) and re-claims the task.
