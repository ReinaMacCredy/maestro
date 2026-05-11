# Maestro CLI reference

Verb-by-verb reference for the `maestro` CLI. Each section names the verb, its
flags, exit codes where relevant, and the canonical doc to read for the full
contract.

For agent-facing usage, prefer the bundled skills under
`skills/bundled/maestro-*` — they cross-reference these verbs in the right
order.

---

## Build and check helpers

```bash
bun run build
bun run check:boundaries
bun run check:skills
bun run check:bundled-skills
bun run test
./dist/maestro mission-control --render-check --size 120x40
bun run release:local
```

---

## evidence

```bash
maestro evidence record --task <id> --command "bun test" --exit 0
maestro evidence record --task <id> --kind manual-note --note "Verified manually"
maestro evidence record --task <id> --kind ai-review --reviewer <bug|security|architecture> --findings <inline-json-or-path> --confidence <0-1>
maestro evidence record --task <id> --kind threat-model --threat-model-file <path>
maestro evidence list --task <id>
maestro evidence show <evidence-id>
```

See `docs/witness-levels.md` for the 4-level witness ladder.

---

## contract (L2)

```bash
maestro contract show --task <id> [--at-version <n>]
maestro contract amend --task <id> --add-path <path> --reason "<why>"
maestro contract amend --task <id> --remove-path <path> --reason "<why>"
maestro contract history --task <id>
```

Amendments consume from `amendmentBudget` (Rules 3–7). The amend use-case
writes a `contract-amended` Evidence row automatically. See
`docs/sensitive-paths-defaults.md` and `docs/owners-yaml-format.md`.

---

## task verify (L2)

```bash
maestro task verify --task <id> [--base <git-ref>] [--json]
```

Runs 8 Trust Verifier checks (8th is architecture lints from
`bun run lint:arch`). Lint findings at `error` severity produce
`lint-violation` evidence rows queryable via `maestro task introspect`.

---

## task introspect (Phase 1)

```bash
maestro task introspect <id-or-slug> [--json]
```

Read-only digest: spec acceptance criteria + non-goals, latest verdict,
cost-budget status, open lint violations, active blockers, last 5 evidence
rows, recent commits since the last `session-start` anchor.

---

## session (Phase 1)

```bash
maestro session whoami [--json] [-q]
maestro session start <taskId> [--json]
maestro session exit <taskId> [--json]
```

`session start` writes `.maestro/runs/<taskId>/orient.md`, runs the baseline
arch-lint pass, optionally invokes `maestro:setup` and `maestro:verify`
package-json scripts, and records a `session-start` evidence row at
`witnessed-by-maestro`.

`session exit` re-runs the baseline arch-lint pass, reads the latest verdict,
writes `.maestro/runs/<taskId>/progress.md`, and records a `session-exit`
evidence row. Exit codes: 0 clean, 1 baseline regressed, 2 arch-lint errors.

---

## lint (Phase 1)

```bash
bun run lint:arch                        # standalone, no diff (3 file-scan rules)
bun run lint:arch -- --base main         # diff-aware (enables no-hand-edit-generated rule)
bun run lint:arch -- --json
```

Same library powers Trust Verifier's 8th check. See `docs/architecture-lints.md`.

---

## recover (Phase 2)

```bash
maestro recover --task <id> [--to <commit>] [--force] [--dry-run] [--json]
```

Resolves the last `PASS` verdict for the task, finds a commit whose tree
matches `verdict.subject.tree_sha`, runs `git reset --hard`, removes
`.maestro/runs/<id>/`, and records a `recovery` evidence row at
`witnessed-by-maestro`. Refuses dirty trees unless `--force`.

---

## ralph (Phase 2)

```bash
maestro ralph review --task <id> [--stuck-threshold <n>] [--json]
```

Convergence oracle. Aggregates arch-lint, verifier, AI review, and
threat-model findings; computes a stable `findingsHash`; records a
`ralph-iteration` evidence row. Exit codes: 0 converged, 1 not converged,
2 stuck.

---

## gc (Phase 2 / 4)

```bash
maestro gc doc-gardening [--task <id>] [--json]
maestro gc slop-cleanup [--min-severity info|warn|error] [--json]
maestro gc plan-regen --task <id> [--json]
```

`doc-gardening` scans repo docs for stale path references; `slop-cleanup`
groups arch-lint violations by file; `plan-regen` reports plan-vs-state drift.

---

## state (Phase 3)

```bash
maestro state since <iso> [--until <iso>] [--task <id>] [--json]
```

Streams a chronological event view (Evidence + Verdict) within the window.

---

## mission-control filter (Phase 3)

```bash
maestro mission-control --json --filter task=<id>
maestro mission-control --json --filter feature=<id> --filter task=<id>
maestro mission-control --screen <name> --size 120x40 --format plain
```

`--filter key=value` narrows JSON output; filters are AND-combined.
`--screen <name>` is an alias for `--preview <name>`.

---

## spec (L2)

```bash
maestro spec show --mission <id>
maestro spec edit --mission <id>
```

---

## verdict (L3)

```bash
maestro verdict show --task <id> [--at-version <id>] [--pr <number>]
maestro verdict request --task <id> [--json]
```

Exit codes for `verdict request`: 0 = PASS, 1 = FAIL, 2 = HUMAN, 3 = BLOCK.

`--pr <n>` filters by current HEAD tree SHA.

---

## policy (L3)

```bash
maestro policy check --task <id>
maestro policy pending
```

---

## task proof (L3)

```bash
maestro task proof --task <id> [--json]
```

---

## plan (L4)

```bash
maestro plan check --task <id> --plan-file <path> [--json]
```

Exit code always 0; agents react to findings. A clean plan-check does not
guarantee a passing verdict.

---

## task budget (L4)

```bash
maestro task budget --task <id> [--json]
```

Exit always 0; read-only. Once any budget limit is exceeded, the next
`verdict request` returns BLOCK (exit 3).

---

## ci (L5)

```bash
maestro ci verify [--pr <n>] [--task <id>] [--base <ref>] [--json]
```

Reads CI env (`GITHUB_*`) by default; flags override. Runs Trust Verifier,
ingests CI job-result file as `witnessed-by-ci` Evidence, computes Verdict,
writes outputs, and posts a GitHub Check (when token present).

Exit codes: 0 PASS / 1 FAIL / 2 HUMAN / 3 BLOCK.

See `docs/ci-integration.md`.

---

## merge auto (L6)

```bash
maestro merge auto --pr <number> --task <id> [--base <ref>] [--repo <owner/name>] [--json]
```

Runs 8 eligibility predicates. Exit 0 + `gh pr merge --auto` on pass; exit
1 listing failing codes on fail. Requires `autoMergeAllowed.<riskClass>:
true` in `autopilot.yaml`. See `docs/auto-merge-eligibility.md`.

---

## verdict override (L6)

```bash
maestro verdict override --task <id> --pr <number> --reason "<text>" [--verdict <id>] [--base <ref>] [--json]
```

Records `verdict-override` Evidence at `agent-claimed-and-not-reproducible`.
Requires the invoker in `owners.yaml.sensitive_waiver` (loaded from base
branch). Does not flip the PR check conclusion. See `docs/override-flow.md`.

---

## review ack (L6)

```bash
maestro review ack --task <id> --verdict <id> --criterion "<text>" [--criterion "<text>" ...] [--json]
```

Records `review-ack` Evidence at `agent-claimed-locally`. Required when the
verdict is `HUMAN` at `>=medium` risk before `merge auto` can succeed.
`--criterion` is repeatable.

---

## deploy (L7)

```bash
maestro deploy gate --task <id> [--base <ref>] [--json]
maestro deploy rollback --task <id> --command <cmd> [--json]
```

`deploy gate` runs 4 checks (feature_flag, canary_plan, rollback, owner) and
records `deploy-readiness` Evidence. Exits 0 pass, 1 fail. Does NOT mutate
the Verdict.

`deploy rollback` runs the command and records `rollback-exercised` Evidence
at `witnessed-by-ci` (in CI) or `witnessed-by-maestro` (locally).

See `docs/deploy-gate.md`.

---

## runtime (L7)

```bash
maestro runtime check --task <id> [--provider-base-url <url>] [--json]
```

Queries each signal declared in `Spec.runtime_signals` via the configured
provider (Prometheus today). Records `runtime-signal` Evidence per signal.
Exit code always 0; `pass=false` rows are advisory at L7.

Provider base URL precedence: `--provider-base-url` → `MAESTRO_PROMETHEUS_URL`
→ `http://localhost:9090`. See `docs/runtime-monitoring.md`.

---

## contract sprint (Phase 4)

```bash
maestro contract sprint --task <id> [--propose <text>] [--proposed-by <actor>] [--json]
```

Sprint snapshot. With `--propose`, records a `manual-note` evidence row
tagged as a sprint-contract proposal. Does NOT mutate the contract.

---

## inspect (Phase 5)

```bash
maestro inspect <taskId> [--tail <n>] [--json]
```

Read-only post-mortem snapshot of `.maestro/runs/<taskId>/{orient,progress,plan}.md`
+ `state.json` + last `--tail` evidence rows + verdict history.

---

## worktree (Phase 5)

```bash
maestro worktree create <slug> [--base <branch>] [--prefix <pre>] [--json]
```

Wraps `git worktree add -b <prefix>/<slug>` and provisions an isolated
`.maestro/runs/` inside the worktree. Default base `main`, default prefix `feat`.

---

## setup (Phase B / harness pivot)

```bash
maestro setup --check [--json]
maestro setup --self-test [--json]
maestro setup --install-hooks [--json]
```

`--check` audits AGENTS.md size, host-runtime detection, owners.yaml roles,
required docs, orphan run dirs, and bundled-skill verb drift vs the binary.
`--self-test` runs an isolated tmpdir smoke pass. `--install-hooks` installs
`SessionStart`/`SessionEnd` hooks into detected host runtimes (Claude Code,
Codex, Cursor).
