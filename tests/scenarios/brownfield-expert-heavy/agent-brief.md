## Identity and context

You are a software coding agent working in a real project directory.
Your working directory is: <SANDBOX_PATH>
All `maestro` commands run from that directory.

Your primary guidance comes from the 5 maestro skill files. Load them now:
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-setup/SKILL.md
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-design/SKILL.md
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-plan/SKILL.md
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-task/SKILL.md
- <MAESTRO_CHECKOUT>/skills/bundled/maestro-verify/SKILL.md

## Operating mode

**Expert.** The user names maestro verbs explicitly. Execute them as directed.

The project directory has a v1 `.maestro/` tree. The user's first message
explicitly asks you to migrate.

## User-mock script

Simulate the following user messages in order, one per turn. For each message,
act on it fully before reading the next.

User message 1: "Migrate to v2 first: run `maestro setup`."

User message 2: "Author a heavy spec for a reporting module: weekly digest, PDF export, and email delivery."

User message 3: "Run `mission from-spec` then `mission decompose` -- break into at least 3 tasks."

User message 4: "Claim task-1. Implement it. Then run verify with `--verdict block --reason 'email service not configured'`."

User message 5: "Email service is now available. Re-verify task-1 and ship it."

Implementation notes for message 4:
- Claim the first child task: `maestro claim <tsk-id> --skip-worktree`
- Do some minimal implementation work (create a stub file).
- Run: `maestro verify <tsk-id> --verdict block --reason 'email service not configured'`
  This should exit 3 (BLOCK) and write a `transition` row with `to_state=blocked`
  and `verdict=BLOCK`.

Implementation notes for message 5:
- The task is currently `blocked`. First reclaim it from blocked:
  `maestro claim <tsk-id> --skip-worktree` (transitions `blocked -> claimed`).
- Then run verify again normally (no --verdict flag):
  `maestro verify <tsk-id>`
- If exit code 0 (PASS): run `maestro ship <tsk-id>` then write sentinel.
- If exit code 1 (FAIL): fix any issues and retry.

## Termination contract

For message 5's re-verify and beyond:
- Exit code 0 (PASS): run `maestro ship <id>` then write sentinel with status
  `pass`.
- Exit code 1 (FAIL): increment failure counter. If counter reaches 3 with no
  task state change (check the `state` field in `.maestro/tasks/tasks.jsonl`
  for this task), write sentinel with status `fail-budget` and stop.

After 20 minutes of wall-clock time from session start, write sentinel with
status `timeout` and stop.

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
bun <MAESTRO_CHECKOUT>/tests/scenarios/brownfield-expert-heavy/rubric.ts <SANDBOX_PATH>
```

Print the full rubric output. This output is consumed by the outer dispatcher.
