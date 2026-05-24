---
name: maestro-task
description: Use at the start of any multi-step work in a maestro-initialized project, and throughout task execution. Claim one task at a time, iterate through the verify → block / ship loop. `claim` and `block` automatically emit a handoff envelope to `.maestro/handoffs/<hnd-...>.json`; see `maestro-handoff` for the read side. Auto-invokes whenever a `.maestro/` directory is present in the current working tree or an ancestor.
---

# Maestro Task

You are working in a maestro project. Author a product-spec, materialize one task per PR, claim it, iterate until `task verify` returns PASS, then ship.

This skill covers the **single-task path** (light-mode specs, one PR). For multi-PR work, see `maestro-mission` — heavy-mode specs decompose into a batch of child tasks via `mission from-spec` → `mission decompose`.

---

## When to activate

Auto-activate when:

1. The user asks for a multi-step implementation.
2. The user names a task id (`tsk-...`) or says "resume this task".
3. Starting a fresh session in a maestro project (`.maestro/specs/` exists).

Do not activate for one-liner edits or read-only questions.

---

## Hard rules

1. **One task at a time** unless the user explicitly directs otherwise.
2. **Every spec is authored through `maestro-design`** — do not write spec markdown by hand without grilling.
3. **`claim` and `block` emit a handoff envelope** to `.maestro/handoffs/<hnd-...>.json`. This is automatic; do not invent parallel handoff files. `ship`, `verify`, and `abandon` do not emit yet.
4. **Blockers carry a `--reason`.** Same for abandon and `verify --verdict {human,block}`.
5. **Heavy-mode specs do not flow through this skill.** They go to `maestro-mission`.

---

## The single-task loop

```
spec new (light)  →  task from-spec  →  claim  →  [doing → verify ↔ blocked]  →  ready  →  ship
```

### 1. Author the spec

```bash
maestro spec new <slug>                      # mode defaults to light
maestro spec new <slug> --mode heavy         # promote when multi-PR
maestro spec new <slug> --title "Human-readable title"
```

The grill protocol in `maestro-design` walks the spec through `acceptance_criteria` + `non_goals` + `work_type` before this verb runs. If you found yourself at this skill without a spec, jump to `maestro-design` first.

### 2. Materialize the task

```bash
maestro task from-spec .maestro/specs/<slug>.md
```

Creates a task in `draft`. Print the returned `tsk-...` id back to the user.

### 3. Claim it

```bash
maestro claim <id> --agent <agent-id>
# heavy-mode specs auto-create a worktree under <parent>/<repo>-<task_id>
maestro claim <id> --skip-worktree           # opt out of auto-worktree
```

The hot-path alias `claim` is identical to `task claim`. `--agent` is recorded on the task and the transition evidence.

### 4. Iterate

```bash
maestro verify <id>
maestro verify <id> --json                   # full JSON envelope
maestro verify <id> --verdict human --reason "needs UX call"     # stays at verifying, exit 2
maestro verify <id> --verdict block  --reason "infra missing"    # → blocked, exit 3
```

Exit codes (see `maestro-verify` for the full protocol):

- `0` PASS — the task auto-advances `verifying → ready`. Move to step 5.
- `1` FAIL — fix the cited violations, edit, then `verify` again. The task stays at `verifying`.
- `2` HUMAN — explicit human verdict, task stays at `verifying`. Hand off and stop.
- `3` BLOCK — task → `blocked`. Surface the reason; do not retry without guidance.

### 5. Block / abandon when you can't proceed

```bash
maestro block    <id> --reason "missing-credentials"
maestro abandon  <id> --reason "out-of-scope after grill"
```

Both transitions emit an evidence row and an observability row. `block` also emits a handoff envelope; `abandon` does not.

### 6. Ship

```bash
maestro ship <id>
maestro ship <id> --pr-url https://github.com/owner/repo/pull/123
```

`ship` is the manual `ready → shipped` flip.

### 7. Split when scope grows

If, mid-claim, the task turns out to be two or three smaller pieces, split it
rather than overloading the verify loop:

```bash
maestro task split <parent-id> "Wire the API" "Wire the UI" "Add tests"
maestro task split <parent-id> "first half" "second half" --parallel
maestro task split <parent-id> "title" --agent <agent-id>      # assert claimant
```

- Parent must be `claimed` or `doing`. Children are created as `draft` with
  slugs `<parent.slug>-1`, `<parent.slug>-2`, …, and inherit `mission_id`,
  `spec_path`, and `worktree_path` from the parent.
- Default chain is sequential: `child[i].blocked_by = [child[i-1]]`. Pass
  `--parallel` to give every child an empty `blocked_by`.
- Parent's `blocked_by` gains the new child ids, so the parent stays in flight
  while you claim and ship each child individually. `task get <parent>` now
  shows a `children:` section and an `external blocked_by:` section.
- When abandoning a parent with active children, pass `--cascade` to abandon
  the descendants post-order; without it, non-terminal children block the
  abandon with `TASK_ABANDON_CASCADE_BLOCKED`.

```bash
maestro abandon <parent-id> --reason "scope dropped" --cascade
```

---

## What `task claim` does for you

- Records the claim transition in `.maestro/evidence/<date>.jsonl`.
- Mirrors the transition into `.maestro/runs/<task-id>/observability.jsonl`.
- For `mode: heavy` specs (and only those), creates a worktree at `<parent>/<repo>-<task_id>` on a branch `feat/<slug>` off `main`. Persists the record at `.maestro/worktrees/<task-id>.json` on the primary repo. The path is printed in the `(worktree …)` suffix.
- Emits a `task:claim` handoff envelope at `.maestro/handoffs/<id>.json` carrying `agent_id`, `worktree_path`, and `spec_path`.

Worktree failures (git not initialized, branch already exists, etc.) are logged to stderr but never block the claim — the task still reaches `claimed`.

---

## Evidence

Every transition writes an evidence row. For verification commands that do not run through `task verify`, record explicitly:

```bash
maestro evidence record --task <id> --command "bun test" --exit 0
maestro evidence record --task <id> --kind manual-note --note "Verified UI on staging 1280x800"
```

For AI review / threat-model rows, see `maestro-verify`.

---

## Discovery

```bash
maestro task list                            # all tasks
maestro task list --state claimed
maestro task list --mission-id pln-...       # children of a mission
maestro task get <id>
```

---

## Dev-time observability

`maestro task observe` is the ad-hoc inspection verb for the agent's own worktree: one-shot metric query or last-N log lines. It does **not** gate any verdict — that is `runtime check`'s job (see `maestro-verify`).

```bash
maestro task observe metrics 'up' --prometheus-url http://localhost:9090
maestro task observe metrics 'rate(errors[5m])' --json
maestro task observe logs --log-file ./app.log --lines 50 --filter error
```

Flags:

- `--prometheus-url <url>` overrides `MAESTRO_PROMETHEUS_URL`.
- `--log-file <path>` overrides `MAESTRO_DEV_LOG_FILE`.
- `--lines N` (logs only, default 100).
- `--filter <substring>` (logs only, plain substring match).
- `--json` emits a JSON envelope instead of plain text.
- `--record --task <id>` writes a `manual-note` evidence row tagged `[dev-observation:metrics]` / `[dev-observation:logs]` so the observation appears in `evidence list`.

Exit codes: `0` success, `1` config error (missing URL/path, `--record` without `--task`), `2` backend unreachable / empty vector / fs read error. The verb is one-shot — there is no `--follow`.

---

## Recovery and worktrees

```bash
maestro recover --task <id>                  # reset working tree to last PASS verdict's tree
maestro recover --task <id> --dry-run
maestro worktree create <slug>               # explicit worktree (use when task claim's auto-create did not run)
```

`recover` finds the latest PASS verdict for the task, runs `git reset --hard` to its `tree_sha`, removes `.maestro/runs/<id>/`, and records a `recovery` evidence row at `witnessed-by-maestro`. Refuses dirty trees unless `--force`.

---

## When verify keeps failing

If `task verify` repeats the same violations after two iterations, the loop is stuck. Choose one:

- Read the failing rule's `Fix Recipe` in `docs/principles/<rule>.md`.
- Run `maestro gc slop-cleanup` to see the violation grouped with the rest of the codebase.
- Decompose into a plan: promote the spec to `mode: heavy` and use `maestro-mission` to break the work into smaller verifiable PRs.
- Block the task with a precise reason and hand off.

---

## MCP tools (when available)

The MCP tool surface mirrors the CLI:

| MCP tool                            | CLI equivalent                       |
| ----------------------------------- | ------------------------------------ |
| `maestro_task_list`, `maestro_task_get` | `maestro task list`, `maestro task get` |
| `maestro_task_create`               | `maestro task from-spec`             |
| `maestro_task_claim`                | `maestro task claim`                 |
| `maestro_task_block`, `maestro_task_unblock` | `maestro task block`, `maestro task abandon` (no native unblock; re-claim from blocked) |
| `maestro_task_abandon`              | `maestro task abandon` — accepts `cascade: true` to recursively abandon non-terminal split-children |
| `maestro_task_split`                | `maestro task split` — `parent_id`, `titles[]`, optional `parallel`, optional `agent_id` |
| `maestro_task_complete`             | `maestro task ship`                  |
| `maestro_evidence_record`, `maestro_evidence_list` | `maestro evidence record`, `maestro evidence list` |
| `maestro_contract_show`, `maestro_contract_amend` | `maestro contract show`, `maestro contract amend` |
| `maestro_verdict_show`, `maestro_verdict_request` | `maestro verdict show`, `maestro verdict request` |
| `maestro_handoff_list`, `maestro_handoff_show` | `.maestro/handoffs/*.json` (read directly; see `maestro-handoff`) |
| `maestro_handoff_emit`, `maestro_handoff_pickup` | emit envelope outside lifecycle / mark picked up (see `maestro-handoff`) |

`maestro_task_list` accepts `mission_id` to filter to a mission's children. The MCP schema is strict — unknown fields fail rather than getting silently dropped.

If MCP is unavailable, fall back to the CLI verbs above.

---

## Hand off cleanly

The next phase after this skill depends on where the single-task loop exits:

- `ship` → loop is done. Surface the PR URL; no downstream skill.
- `block` → the next agent enters via `maestro-handoff` and reads the `task:block` envelope. Surface `block_reason` and stop.
- `verify --verdict human` → surface the reason to the user; do not retry. The user (or a follow-up agent via `maestro-handoff`) decides next.
- Pre-ship verification is in-loop, not a downstream skill — `maestro-verify` is the protocol you run *inside* this skill at step 4, not a handoff target.

Pass a claimed task with a clean evidence trail — not an in-flight scratchpad. Do not invoke spec authoring or planning from this skill.

---

## See also

- `maestro-design` — grill-protocol spec authoring (run before `task from-spec`).
- `maestro-mission` — heavy-mode multi-PR work (skip `task from-spec`; use `mission from-spec` + `mission decompose`).
- `maestro-verify` — canonical verification protocol (PASS / FAIL / HUMAN / BLOCK routing).
- `maestro-setup` — `setup` (idempotent default action) and `setup check`.
- `docs/cli-reference.md` — verb-by-verb reference.
