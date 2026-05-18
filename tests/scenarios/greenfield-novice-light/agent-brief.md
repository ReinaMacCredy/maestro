## Identity and context

You are a software coding agent working in a real project directory.
Your working directory is: <SANDBOX_PATH>
All `maestro` commands run from that directory.

Your primary guidance comes from the 5 maestro skill files. Load them now:
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-setup/SKILL.md
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-design/SKILL.md
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-mission/SKILL.md
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-task/SKILL.md
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-verify/SKILL.md

## Operating mode

**Novice.** The user does not know maestro verb names. Infer what to run from
their intent. Do not prompt the user to name a command -- translate their words
into the right maestro verb and proceed.

The project directory starts as a bare `git init` with no files. Run
`maestro setup` to scaffold the v2 layout before doing any other
maestro work.

## User-mock script

Simulate the following user messages in order, one per turn. For each message,
act on it fully (run the appropriate commands, write code, etc.) before
reading the next.

User message 1: "I want to set up this project with maestro so I can track my work."

User message 2: "I need to add a greeting endpoint that returns 'Hello, <name>'."

User message 3: "Go ahead and get started on it."

User message 4: "Looks good. Ship it."

## Termination contract

Track consecutive verify failures. After each `maestro verify <id>` (or
`maestro task verify <id>`):
- If the exit code is 0 (PASS), the task auto-advances to `ready`. Proceed to
  `maestro ship <id>` then write the exit sentinel.
- If the exit code is 1 (FAIL), increment the failure counter. If the counter
  reaches 3 with no state change between runs (check by reading
  `.maestro/tasks/tasks.jsonl` and finding the task's current `state`
  field), write the exit sentinel with status `fail-budget` and stop.

After 20 minutes of wall-clock time from the start of this brief, write the
sentinel with status `timeout` and stop.

Before writing the sentinel, ensure the directory exists:

```bash
mkdir -p <SANDBOX_PATH>/.maestro/scenarios
```

Write to `<SANDBOX_PATH>/.maestro/scenarios/sub-agent-exit.json`:

```json
{
  "status": "pass" | "fail-budget" | "timeout",
  "final_verify_exit_code": <number> | null,
  "evidence_row_count": <number>,
  "terminated_at": "<ISO timestamp>"
}
```

## Self-check

After writing the sentinel, run:

```bash
bun <MAESTRO_CHECKOUT>/tests/scenarios/greenfield-novice-light/rubric.ts <SANDBOX_PATH>
```

Print the full rubric output. This output is consumed by the outer dispatcher.
