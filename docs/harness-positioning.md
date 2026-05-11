# Harness positioning

Maestro is a **long-running agent harness**: the environment around an agent
that makes long-horizon software-engineering work survivable. It is local-first,
verb-driven, and stays passive — host runtimes (Claude Code, Codex, Cursor,
CI) call maestro verbs; maestro never schedules, daemonizes, or runs an LLM
itself.

This doc maps the five principles in OpenAI's
["Harness Engineering"](https://openai.com/index/harness-engineering/) note
to the maestro primitives that already implement them, so contributors and
agents can find the right surface without re-deriving the design.

For the verb surface, see `docs/cli-reference.md`. For passive scheduling
patterns, see `docs/schedule-recipes.md`. For the architecture rules that
keep maestro passive, see `docs/architecture-lints.md` (the
`no-runner-inversion` rule is the structural guarantee).

---

## 1. Context as a scarce resource

Long-horizon agent work fails when context windows fill with low-signal
tool output, stale plans, or redundant evidence. Maestro treats every
write to durable state as evidence with a witness level, and every read
that re-enters the agent's context (orient digests, task introspect,
plan-check) as a curated projection — not a dump.

**Pointers:**

- `src/features/task/usecases/introspect.ts` — read-only post-context-loss
  digest (spec criteria, latest verdict, cost-budget, open lints, last 5
  evidence rows, recent commits since `session-start` anchor).
- `src/features/session/usecases/run-session-start.ts` — writes
  `.maestro/runs/<task>/orient.md` and anchors `session-start` evidence so
  "recent commits" is bounded.
- `src/features/plan/usecases/check-plan.ts` — three deterministic
  pre-coding checks (`scope-widens`, `missing-proof`, `risk-class-too-low`)
  reduce context wasted on doomed plans.
- `docs/witness-levels.md` — the 4-level evidence ladder. Lower-witness
  rows can be filtered out at recall time.

Phase G (deferred) would add explicit tool-result distillation; for now,
context discipline lives in the verbs above and in the agent's own
prompting.

---

## 2. Per-worktree observability

Harnesses give agents a way to see what their code is doing without
booting an entire production stack. Maestro separates **dev-time
observability** (cheap, local, advisory) from **deploy-gating runtime
checks** (witnessed, policy-driven, gating).

**Pointers:**

- `src/features/runtime/ports/runtime-monitor.port.ts` — deploy-gate
  signal queries (Prometheus adapter today). Records `runtime-signal`
  evidence.
- `src/features/runtime/commands/runtime-check.command.ts` — `maestro
  runtime check`, advisory at L7, can be wired into `policies/risk.yaml`.
- `src/features/runtime/ports/dev-observability.port.ts` — dev-time
  metrics + log tail for the agent's own iteration loop. Documented in
  `docs/dev-observability.md`.
- `docs/runtime-monitoring.md` — adapter contract and provider precedence.

Dev-time observability is not a deploy gate. It exists so an agent can
answer "what does this look like right now" without producing
deploy-witness evidence.

---

## 3. Isolated worktrees

Long tasks must not collide on shared mutable state. Maestro keeps every
task's run-state in its own directory and provisions worktrees with
isolated maestro state so parallel agents do not corrupt each other.

**Pointers:**

- `src/features/worktree/commands/worktree-create.command.ts` — `maestro
  worktree create <slug>` wraps `git worktree add` and provisions a
  per-worktree `.maestro/runs/` so each agent's evidence, plan, orient,
  and progress files stay local to its branch.
- `src/features/task/adapters/fs-run-state-store.adapter.ts` — run state
  is persisted under `.maestro/runs/<task-id>/state.json` (gitignored)
  so two worktrees on the same repo share nothing.
- `src/features/recover/commands/recover.command.ts` — `maestro recover`
  resets the working tree to the last `PASS` verdict's tree and drops
  `.maestro/runs/<id>/`, restoring a known-good per-worktree state.

---

## 4. Continuous quality grading

Agents need a verdict surface that is deterministic, evidenced, and
re-derivable. Maestro grades work via Verdicts (`PASS`/`FAIL`/`HUMAN`/
`BLOCK`) bound to (PR, tree_sha) so squashes survive and force-pushes to
a different tree invalidate the grade.

**Pointers:**

- `src/features/verdict/usecases/request-verdict.ts` — the decision tree
  that maps spec acceptance criteria + evidence + risk class + policy to
  a verdict.
- `src/features/verify/usecases/run-trust-verifier.ts` — eight checks
  (scope, lockfile, generated, sensitive-paths, commit-metadata, secrets,
  proof-map, architecture-lints) feeding the verdict.
- `src/features/verify/usecases/proof-map.ts` — joins
  `Spec.acceptance_criteria` with evidence rows so coverage is provable,
  not asserted.
- `src/features/ralph/commands/ralph.command.ts` — `maestro ralph
  review` is the convergence oracle: hashes findings and detects
  stuck-iteration via `--stuck-threshold`.
- `docs/risk-class-derivation.md` and `docs/policy-format.md` —
  deterministic risk-class derivation and policy mapping.

---

## 5. Building blocks (design / code / review / test)

OpenAI's harness post frames agent work as four blocks: design, code,
review, test. Maestro exposes each as named verbs, not implicit modes.

**Pointers:**

- **Design** — `src/features/spec/commands/spec.command.ts` and
  `src/features/plan/commands/plan-check.command.ts`. Specs hold
  `acceptance_criteria` + `non_goals`; plan-check guards them before
  coding.
- **Code** — `src/features/task/commands/contract.command.ts`. Contracts
  declare allowed paths + sensitive-path consent + amendment budget.
- **Review** — `src/features/verdict/`, `src/features/risk/`,
  `src/features/verify/`. Trust Verifier + Risk Engine + ProofMap +
  verdict request.
- **Test** — `src/features/evidence/` (record runs as evidence rows) +
  `src/features/ci/commands/ci-verify.command.ts` (authoritative CI
  gate). Local maestro is advisory; CI maestro is authoritative.

Per-block verbs make each phase a separable, evidenced step instead of an
opaque agent monologue.

---

## External triggers

Maestro never schedules itself. Anything that needs to run on a clock
lives outside the binary and calls maestro verbs as a subprocess. The
three canonical shapes are:

1. **GitHub Actions cron** — nightly `maestro gc doc-gardening --json`,
   weekly `maestro gc slop-cleanup`, etc.
2. **Host-runtime session hooks** — `.claude/settings.json`,
   `.codex/settings.json`, `.cursor/settings.json` `SessionStart` /
   `SessionEnd` hooks calling `maestro session start/exit "$TASK_ID"`.
3. **Agent skill prompts** — a local skill instructs `maestro task
   verify --task <id>` after a substantive edit batch. Timing is
   contextual; the skill decides, not a scheduler.

Full recipes: `docs/schedule-recipes.md`.

---

## What this is not

Maestro deliberately is **not**:

- **A scheduler.** No cron, no daemon, no background process inside
  maestro. Scheduling lives outside maestro (external cron, host-runtime
  hooks, agent skills).
- **A daemon.** The binary runs only when invoked and exits.
- **An LLM client.** Maestro never makes model API calls; agents do, and
  call maestro verbs in between.
- **A background process.** No watcher, no file-system poller, no
  long-lived state machine.

The structural guarantee is `no-runner-inversion` in
`docs/architecture-lints.md`: maestro code may not invoke schedulers or
spin up persistent loops. The lint enforces it at `error` severity.

---

## Where to read next

- Verb surface: `docs/cli-reference.md`
- Witness ladder: `docs/witness-levels.md`
- Risk derivation: `docs/risk-class-derivation.md`
- Policy format: `docs/policy-format.md`
- CI integration: `docs/ci-integration.md`
- Architecture lints: `docs/architecture-lints.md`
- Schedule recipes: `docs/schedule-recipes.md`
