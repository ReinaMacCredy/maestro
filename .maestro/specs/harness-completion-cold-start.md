---
slug: harness-completion-cold-start
acceptance_criteria:
  - "`maestro status` (no flags) prints in this order: Maestro health -> Project verified state -> Active missions (with nested tasks) -> Next ready -> Recent transitions; each section emits 'no rows yet' inline hints when empty"
  - "`maestro status --json` returns a stable object with keys `maestro_health`, `project_state`, `missions`, `next_ready`, `recent_transitions`; consumed by tests"
  - "`maestro status --terse` collapses Maestro health to only failing rows and omits Recent transitions; `--terse` applies to plain output only and is ignored (with a stderr warning) when combined with `--json`"
  - "Active missions block shows tasks nested under their mission; tasks not bound to a mission appear under a synthetic `(unscoped)` bucket"
  - "Per-task signal in status reads the latest verdict for the task; when no verdict exists, falls back to the most recent transition evidence row"
  - "Recent transitions reads the last 10 evidence rows of `kind: 'transition'` from the evidence store (`evidenceStore.list({ kind: 'transition' })`), newest first; never shells out, never re-derives state"
  - "`maestro doctor` runs a fast clean-state check by default (scaffold, init.sh present + executable, latest verdict freshness) and prints PASS/WARN per dimension; exits non-zero only when scaffold is broken"
  - "`maestro doctor --full` additionally runs `bun run build` and `bun test` (both advisory; WARN on non-zero, never FAIL doctor); `init.sh` calls the fast form so cold-start stays sub-second"
  - "Verdict freshness threshold defaults to 30 days (matches the policy-loosening soak window in `docs/policy-format.md` -- 'stale' means 'older than a policy change could have rotated under you'); overridable via `MAESTRO_VERDICT_STALE_DAYS` env var or `config.doctor.verdictStaleDays`"
  - "`maestro setup` writes a project-owned `init.sh` at the repo root if and only if no `init.sh` already exists; never overwrites"
  - "The emitted `init.sh` template runs `maestro doctor` and `maestro status` as its last two steps; the template is documented in `docs/init-sh-template.md`"
  - "When `.maestro/` is missing, `maestro status` refuses with one line pointing at `maestro init`; no partial output"
  - "Plain output uses ASCII glyphs (`[ok]` `[!]` `[--]`). No unicode glyphs are rendered in this spec."
non_goals:
  - "Introduce a `session end` verb or any `session:*` handoff vocabulary (advisor-killed in design grill Q8; multi-worktree clobber is unsolvable in a single-session model)"
  - "Introduce a single-FSM file like `feature_list.json` (Maestro is append-only event-sourced; the course's single-file model is a single-agent assumption)"
  - "Enforce WIP=1 (Maestro is multi-agent by design; mission-decompose + worktrees fan out work)"
  - "Schedule, daemonize, or background any of the new primitives (enforced by `no-runner-inversion` lint)"
  - "Auto-author narrative summaries via LLM call (Maestro never makes model API calls)"
  - "Seed `.maestro/sessions/` or any session-log directory (no session-end -> no session log)"
risk_class: low
mode: light
work_type: feature
---

# Harness completion: cold-start contract

## Context

Maestro already positions itself as "the harness OS for agent-generated codebases" (`docs/harness-positioning.md:3`) and ships the harness primitives a multi-agent system needs: append-only task + mission stores, per-task evidence + observability logs, per-task handoff envelopes, isolated worktrees, principles, architecture lint, trust verifier, risk engine, deploy gate, runtime monitor.

What is missing is the **cold-start contract**: a new agent session should be able to resume work in roughly one file read. Today an agent has to piece together state from `.maestro/tasks/tasks.jsonl`, `.maestro/missions/missions.jsonl`, `.maestro/handoffs/*.json`, `.maestro/evidence/*.jsonl`, and `git log` -- each readable individually, none summarized in one place. `maestro status` exists (`src/infra/commands/status.command.ts`) but is shallow: it only reports init + git + legacy-handoff count.

This spec closes that gap. It deepens `status` into the cold-start view, adds `maestro doctor` as the clean-state check, and standardizes a project-owned `init.sh` template so the resume path is one command for humans and one well-known file for agents.

The walkinglabs harness-engineering course (see `/tmp/lhe/docs/en/resources/templates/`) proposes a single-FSM file (`feature_list.json`) plus a narrative session log (`claude-progress.md`). Both are designed for a single-agent loop. Maestro rejects both: the FSM lives in event-sourced jsonl logs, and the narrative is derived from per-transition handoffs rather than authored at session end. The course's *concepts* land; its *primitives* do not.

## Decisions (locked by design grill 2026-05-18)

| # | Decision | Choice | Rationale |
|---|---|---|---|
| Q2 | Per-task signal in status | Latest verdict, fall back to most recent transition evidence row | Verdicts are the binding statement; evidence rows are the witness ladder underneath |
| Q3 | Multi-mission display | Tasks nested under their mission; unscoped tasks under synthetic bucket | Mirrors the actual ownership graph |
| Q4 | Section order | Maestro health -> Project verified state -> Missions -> Next ready -> Recent transitions | Action-first: health gates everything else; "what should I do" precedes "what happened" |
| Q5 | `init.sh` shape | Project-owned at repo root; Maestro emits template once, never overwrites | Avoids hidden coupling between Maestro version and project boot |
| Q7 | Empty-state philosophy | Inline hints under each empty section; no seeded content | Honors the "no speculative abstractions" rule -- empty means empty |
| Q8 | Session-end scoping | **No `session end` verb.** Narrative derives from per-transition handoffs + evidence | Any session-end concept silently re-imports WIP=1; two agents in two worktrees both ending a session is unsolvable |
| Q9 | Baseline split | Two sections: Maestro health (from setup-check) and Project verified state (from verdict + evidence) | Answers "is it me or the project?" at a glance |

## Implementation outline

### 1. `maestro status` (deepened)

File: `src/infra/commands/status.command.ts` (replace current shallow implementation).

Sections in order, each with an empty-state inline hint:

1. **Maestro health** -- delegates to existing `setup-check.usecase.ts`. Rows: `.maestro/` scaffolded, principles seeded count, config valid, git available. Empty-state never applies (always at least one row).
2. **Project verified state** -- derived. Rows: most recent verdict + age, count of tasks stuck in `verifying` > 24h, count of stale handoffs (no pickup sidecar > 24h). Hint when no verdict ever: `no verdict yet -- run 'maestro task verify <id>'`.
3. **Active missions** -- reads `missions.jsonl`, filters to non-terminal states. For each mission, nests its tasks (joined on `mission_id`). Unscoped tasks under `(unscoped)`. Hint when none: `no active missions -- 'maestro mission new'`.
4. **Next ready** -- the highest-priority task in `ready` state (existing `task ready` logic, single row). Hint when none: `no tasks ready to ship`.
5. **Recent transitions** -- last 10 rows from `evidenceStore.list({ kind: "transition" })`, newest first, formatted `<ts>  <task-id>  <trigger_verb>  <verdict|to_state>`. Source is the evidence log, not handoffs (only `claim` and `block` emit handoffs today per `docs/harness-positioning.md:85`; the evidence log covers all five triggers). Hint when none: `no transitions yet`.

JSON mode (`--json`) returns the same shape under stable keys. Terse mode (`--terse`) collapses Maestro health to only failing rows and omits Recent transitions.

Hard-refuse when `.maestro/` is missing: print one line `not initialized -- run 'maestro init'` and exit non-zero. No partial output.

Reads only. Never writes. Never shells out for state (uses ports for tasks/missions/evidence/handoffs).

### 2. `maestro doctor` (new verb)

File: `src/infra/commands/doctor.command.ts` (new).

**Fast form (default; what `init.sh` calls):** runs three dimensions, sub-second on any repo:

1. **Scaffold** -- `.maestro/` directories present, principles seeded, config valid (reuses `setup-check`).
2. **Init script** -- repo-root `init.sh` exists and is executable.
3. **Verdict freshness** -- latest verdict age; WARN if older than the staleness threshold, FAIL if no verdict ever AND tasks exist.

**Full form (`--full`):** the three above plus:

4. **Build** -- runs `bun run build` (advisory; WARN on non-zero exit, never FAIL doctor on build).
5. **Tests** -- runs `bun test` (advisory; WARN on non-zero exit).

Exit code: non-zero only when **Scaffold** is broken. Build/tests/verdict produce WARN, not exit failure -- doctor is a status check, not a gate. The fast form is what cold-start pays for; `--full` is what a human runs when they want pre-commit confidence.

**Verdict staleness threshold:** defaults to **30 days**. Override via `MAESTRO_VERDICT_STALE_DAYS` env var (numeric, days) or `config.doctor.verdictStaleDays`. The 30-day default matches the policy-loosening soak window in `docs/policy-format.md`, so "stale" carries a defensible meaning: "older than a policy change could have rotated under you."

### 3. `init.sh` template

File: `.maestro/docs/INIT.sh.template` (new).

`maestro setup` (`src/service/setup.usecase.ts`) copies the template to repo-root `init.sh` and `chmod +x` it on first run. Never overwrites.

Template body (verbatim):

```bash
#!/usr/bin/env bash
# Project init -- regenerated by `maestro setup` only if missing.
# Edit freely; Maestro will not overwrite.
set -euo pipefail

# Health gate -- exits non-zero if .maestro/ scaffold is broken.
maestro doctor

# Cold-start view -- prints the one-screen resume snapshot.
maestro status
```

Document the template at `docs/init-sh-template.md` so users can rebuild it by hand if they delete it.

### 4. Skill alignment

`skills/bundled/maestro-setup/SKILL.md` -- mention the `init.sh` emission step.
`skills/bundled/maestro-verify/SKILL.md` -- mention that `maestro doctor` precedes verification when resuming a cold session.

No verb-vocabulary changes. No new ports. No new state files.

## Cold-start contract (the user-facing claim)

After this spec lands, an agent or human resuming work runs **one command**:

```bash
./init.sh
```

That command runs `maestro doctor` (fast form: scaffold + init script + verdict freshness; sub-second) and `maestro status` (health + project state + missions + next ready + recent transitions). The combined output is the cold-start snapshot. No file reads, no `git log` archaeology, no jsonl spelunking required to know what to do next. Build + tests are explicitly **not** in the cold-start path; they live in `maestro doctor --full` and run only when a human asks for pre-commit confidence.

## Risk + rollback

Risk class is low: all changes are read-side except the one-shot `init.sh` emission, which is gated on file non-existence. Rollback is `git revert` of the implementing commits plus deletion of the emitted `init.sh` (and the user's repo gets one back next time they run `maestro setup`).
