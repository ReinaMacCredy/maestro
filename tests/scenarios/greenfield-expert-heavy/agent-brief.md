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

The project directory starts as a bare `git init` with no files.

## User-mock script

Simulate the following user messages in order, one per turn. For each message,
act on it fully before reading the next.

User message 1: "Run `maestro setup bootstrap`, then author a heavy-mode spec for a feature: a data pipeline that ingests CSV files, transforms rows, and writes output. Break it into 3 tasks."

User message 2: "Run `plan from-spec`, then `plan decompose` with at least 3 child tasks. Show me the plan."

User message 3: "Claim task-1. Before implementing, create `docs/architecture.yaml` with a `passive_harness` forbidden pattern `pollInterval`. Implement the ingest step using that pattern so verify catches it."

User message 4: "Now fix the violation -- remove `pollInterval` from the code -- and re-verify task-1. Ship it when ready."

Implementation notes for message 3:
- Create `docs/architecture.yaml` in the sandbox project with:
  ```yaml
  version: 1
  forward_only: true
  layers: [types, service]
  cross_cutting: []
  lint_scope:
    - "src/**/*.ts"
  passive_harness:
    forbidden_patterns:
      - pollInterval
  ```
- Create `src/ingest.ts` (or similar) that contains the literal word `pollInterval` (e.g.,
  as a variable name or comment).
- Run `maestro task verify <id>` -- it should exit 1 (FAIL) with a lint-violation row
  in evidence for `rule_id: "passive-harness"`.

Implementation notes for message 4:
- Remove `pollInterval` from `src/ingest.ts` (or wherever it appears).
- Re-run `maestro verify <id>` -- should now exit 0 (PASS).
- Run `maestro ship <id>`.

## Termination contract

Track consecutive verify failures AFTER the intentional first failure
(message 3). Reset the counter when a state change occurs or when the
deliberate FAIL in message 3 is acknowledged.

After the intentional FAIL in message 3:
- Proceed to message 4 (fix and re-verify).
- If the re-verify exits 0 (PASS): run `maestro ship <id>` then write sentinel
  with status `pass`.
- If re-verify exits 1 (FAIL) again: this is an unexpected failure. Attempt to
  fix. If 3 consecutive FAILs occur with no state change (check the task
  `state` field in `.maestro/tasks/tasks.v2.jsonl`), write sentinel with status
  `fail-budget` and stop.

After 20 minutes of wall-clock time, write sentinel with status `timeout` and
stop.

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
bun <MAESTRO_CHECKOUT>/tests/scenarios/greenfield-expert-heavy/rubric.ts <SANDBOX_PATH>
```

Print the full rubric output. This output is consumed by the outer dispatcher.
