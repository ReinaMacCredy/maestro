# Scenario Tests

Behavioral gate for the Maestro harness.

Each scenario runs a full user → coding-agent → maestro → ship cycle in a
sandboxed project directory. Rubrics score deterministically against the
`.maestro/evidence/<date>.jsonl` trail — no LLM-as-judge.

## The 4 scenarios

| # | Name | Project | Familiarity | Mode | Task shape |
|---|------|---------|-------------|------|-----------|
| 1 | `greenfield-novice-light` | greenfield | novice | light | feature |
| 2 | `greenfield-novice-heavy` | greenfield | novice | heavy | feature (multi-PR) |
| 3 | `greenfield-expert-light` | greenfield | expert | light | bug |
| 4 | `greenfield-expert-heavy` | greenfield | expert | heavy | feature (lint-violation recovery) |

## Directory layout

```
tests/scenarios/
├── README.md                      (this file)
├── _helpers/
│   └── rubric-helpers.ts          (shared: loadEvidence, mustHave, mustNotHave, ...)
└── <scenario-name>/
    ├── scenario.md                (user-mock script + termination + expected evidence)
    ├── rubric.ts                  (deterministic checker, runnable standalone)
    └── agent-brief.md             (prompt handed to the spawned sub-agent)
```

## Rubric contract

Each `rubric.ts` exports:

```typescript
export interface RubricResult {
  readonly scenario: string;
  readonly projectDir: string;
  readonly pass: boolean;
  readonly checks: readonly CheckResult[];
}

export async function runRubric(projectDir: string): Promise<RubricResult>;
```

and has a `main` block that runs when executed directly:

```bash
bun tests/scenarios/<name>/rubric.ts <project-dir>
```

Exit code 0 = PASS, exit code 1 = FAIL. Every check prints `[PASS]` or
`[FAIL]` with its id and description. Failing checks include a note.

## Agent-brief format

Five sections (substitution placeholders `<SANDBOX_PATH>` and
`<MAESTRO_CHECKOUT>` are filled by the swarm dispatcher at dispatch time):

1. Identity and context
2. Operating mode (novice or expert)
3. User-mock script (ordered messages the agent simulates)
4. Termination contract (exit sentinel written to `.maestro/scenarios/sub-agent-exit.json`)
5. Self-check (run rubric as last step, print output)

## Running a single rubric (dry-run syntax check)

Pass a directory that has no `.maestro/` tree; all checks fail, none crash:

```bash
bun tests/scenarios/greenfield-novice-light/rubric.ts /tmp/empty-dir
# => all [FAIL], exits 1 — expected
```

## Running the full swarm

The swarm dispatcher lives in `scripts/scenarios/swarm.ts`. Run it from an
interactive Claude Code session:

```bash
bun scripts/scenarios/swarm.ts
```

## Evidence reading rules

Rubrics read `.maestro/evidence/<date>.jsonl`. The two row kinds are:

- `kind: "transition"` -- every task and plan state change. Fields: `task_id`,
  `mission_id`, `from_state`, `to_state`, `trigger_verb`, `verdict`, `reason`.
- `kind: "lint-violation"` -- architecture lint finding. Fields: `task_id`,
  `rule_id`, `severity`, `file`, `line`, `message`.

Distinguish task vs plan transitions by which of `task_id` / `mission_id` is set.

Task states: `draft | claimed | doing | verifying | blocked | ready | shipped | abandoned`

Plan states: `intake | specified | planned | in-progress | completed | cancelled`
