# Maestro Project State — Read Order

For any agent picking up work in this repo, read in order:

1. `.maestro/MAESTRO.md` (this file) — read order, lane policy, daily commands.
2. `AGENTS.md` (repo root) — code conventions, feature boundaries, build/test commands.
3. `.maestro/tasks/NOW.md` — what is currently in flight.
4. `maestro status --json` — live state across missions, tasks, pending loosenings.
5. `.maestro/policies/*.yaml` — risk, autopilot, release, sensitive-paths, owners.
6. `.maestro/specs/<id>/spec.json` — acceptance criteria for the active mission, if any.
7. `docs/` — concept references (witness levels, risk class derivation, deploy gate, …).

If two sources conflict, the lower-numbered file is operational; the higher-numbered file is informational.

## Two outputs per task

Every task close should answer two questions:

1. **Product delta** — what changed in user-facing or product behavior?
2. **Harness delta** — what should we change so the next agent has it easier? (memory ratchet, skill update, `maestro doctor` finding, friction note in this file). Answer "none" if truly nothing.

If the harness delta is non-trivial, capture it before the close so the next session inherits it.

## Daily commands

```bash
maestro status --json                                 # what is in flight
maestro task plan --file - --start <name>             # batch-create tasks atomically
maestro plan check --task <id> --plan-file <path>     # plan-time consistency check
maestro feature prompt <featureId> --mission <id>     # worker prompt with memory injected
maestro memory-correct "use bun not npm" --trigger "package,install,npm"
maestro doctor                                        # harness drift checks
```

## What lives here vs. AGENTS.md

- `MAESTRO.md` (this file): the operational entry point — read order, lane policy, two-output rule.
- `AGENTS.md`: code conventions, feature boundaries, anti-patterns, CLI verb references.

When updating: route process-of-work changes here; route code-shape changes to AGENTS.md.
