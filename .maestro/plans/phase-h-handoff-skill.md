# Phase H — Resurrect Agent Awareness of v2 Passive-Handoff Model via Lean Skill

Branch: `harness-os` (PR #64)
Generated: 2026-05-16
Owner: Plan agent (opus) per master-plan loop

## Background

The v1 handoff feature was deleted in commit `2af5fe37`:
- `src/features/handoff/` (whole feature) — deleted
- MCP handoff tools (`maestro_handoff_list/show/open_for_task/pickup`) — deleted
- `maestro handoff` CLI verb — deleted

But the v2 passive-handoff model survives:
- `.maestro/handoffs/<id>.json` envelopes are still emitted by v2 lifecycle verbs
- Read paths exist in `src/repo/handoff-emitter.port.ts` (FsHandoffEmitter)

User decision (AskUserQuestion 2026-05-16): **Skill only**, no MCP tools.

## Investigation findings

**Actual emitters today** (not "all transitions" as maestro-task SKILL.md currently claims):
- `task-claim.usecase.ts` — emits `task:claim`
- `task-block.usecase.ts` — emits `task:block`
- `task-ship.usecase.ts` — does NOT emit (no `emitHandoff` call)
- `task-verify.usecase.ts` — does NOT emit
- `task-abandon.usecase.ts` — does NOT emit

The `HandoffTrigger` type in `handoff-emitter.port.ts` declares all five verbs as valid, so ship/verify/abandon were planned but never wired. → Roadmap for Phase H+1.

**Envelope schema** (`src/repo/handoff-emitter.port.ts`):
`{ id: "hnd-<base36>-<rand>", task_id, trigger_verb, created_at, agent_id?, worktree_path?, spec_path?, reason? }`

Filename is the envelope id (`hnd-...`), not the task id. Discovery requires `ls .maestro/handoffs/*.json` and filtering by `task_id` inside each file.

**Stale v1 artifacts in `.maestro/handoffs/`** — 343 entries (directory-shaped, pre-v2). `maestro doctor` flags them as legacy but does not error. Out of scope for this phase.

## Done-state criteria

1. `skills/bundled/maestro-handoff/SKILL.md` exists with `name: maestro-handoff` frontmatter, ≤80 lines, no `/Users/` paths.
2. `src/infra/domain/bundled-skill-templates.ts` regenerated via `bun run sync:bundled-skills`, contains `"maestro-handoff"`.
3. `bun run check:bundled-skills` exits 0.
4. `tests/unit/infra/domain/bundled-skill-templates.test.ts` line 76-82 updated: expects 6 items including `"maestro-handoff"`.
5. Chain-consistency negative assertions (lines 115, 129: `not.toContain("maestro-handoff")`) remain intact — cross-reference is one-directional (handoff → verify only).
6. `bun test` exits 0.
7. `bun run build` exits 0.
8. Smoke: `maestro task claim <id>` then `cat .maestro/handoffs/<hnd-...>.json` returns a `task:claim` envelope with matching `task_id`.
9. Smoke: `maestro task block <id> --reason "x"` then `cat` returns a `task:block` envelope.
10. Stale-reference fixes landed: 3 doc files, 2 maestro-task skill files.
11. Phase H+1 follow-up captured (wire emitHandoff into ship/verify/abandon).
12. User approves SKILL.md draft before it lands in `skills/bundled/`.

## Files to create

- `skills/bundled/maestro-handoff/SKILL.md`

## Files to modify

**Test:**
- `tests/unit/infra/domain/bundled-skill-templates.test.ts` — line 76-82, add `"maestro-handoff"` to sorted list.

**Regenerated automatically (do not hand-edit):**
- `src/infra/domain/bundled-skill-templates.ts` — via `bun run sync:bundled-skills`.

**maestro-task skill corrections:**
- `skills/bundled/maestro-task/SKILL.md`
  - Line 30: narrow "every state transition emits" → "claim and block emit; ship/verify/abandon do not yet (roadmap)."
  - Line 93: "block and abandon both emit" → "block emits; abandon does not yet."
  - Line 178 (MCP table): remove deleted v1 tool rows; replace with "no MCP tools for handoffs; read `.maestro/handoffs/*.json` directly."
- `skills/bundled/maestro-task/reference/recovery.md`
  - Line 91: "Use `maestro-handoff` instead..." → "Use `maestro task block --reason <reason>`; receiving agent reads `.maestro/handoffs/<hnd-...>.json`. See `maestro-handoff` skill."

**Doc fixes:**
- `docs/edge-cases.md` lines 104, 110 — remove `maestro handoff create`, replace with `task block --reason`.
- `docs/providers.md` line 100 — remove `maestro handoff --model` and `maestro handoff pickup`.
- `docs/token-budget.md` lines 43, 50 — remove `handoff list --json` row and `maestro_handoff_list` entry.

## Skill content outline (≤80 lines)

```
---
name: maestro-handoff
description: ...
---
```

- **When to read** — session start, task pickup, debugging missed pickup
- **The passive model** — no `maestro handoff` verb; envelopes auto-emit from lifecycle verbs
- **Which verbs emit today** — claim, block (only). ship/verify/abandon do NOT (roadmap)
- **Envelope schema** — path `.maestro/handoffs/<hnd-...>.json`, fields list
- **How to find envelopes** — `ls .maestro/handoffs/*.json` + inspect `task_id`
- **Pickup protocol** — read envelope, verify task_id, check trigger_verb, re-claim
- **See also** — maestro-task, maestro-verify (one-directional ref only)

## Verification ladder

```bash
wc -l skills/bundled/maestro-handoff/SKILL.md        # must be ≤80
bun run sync:bundled-skills
bun run check:bundled-skills
bun test
bun run build
./dist/maestro task from-spec .maestro/specs/<slug>.md
./dist/maestro task claim <tsk-id> --agent smoke
ls -1t .maestro/handoffs/*.json | head -1 | xargs cat   # task:claim envelope
./dist/maestro task block <tsk-id> --reason smoke
ls -1t .maestro/handoffs/*.json | head -1 | xargs cat   # task:block envelope
./dist/maestro doctor
```

## Task decomposition

| # | Name | Title | Blocked by |
|---|---|---|---|
| 1 | phase-h-01-investigate | Confirm emitter wiring and stale-ref inventory | — |
| 2 | phase-h-02-draft-skill | Draft `maestro-handoff/SKILL.md` for user approval | 01 |
| 3 | phase-h-03-approval | User approves SKILL.md draft | 02 |
| 4 | phase-h-04-create-skill | Write approved SKILL.md to `skills/bundled/` | 03 |
| 5 | phase-h-05-fix-maestro-task | Fix false claims in `maestro-task/SKILL.md` and `recovery.md` | 03 |
| 6 | phase-h-06-fix-docs | Fix stale v1 refs in `docs/edge-cases.md`, `docs/providers.md`, `docs/token-budget.md` | 03 |
| 7 | phase-h-07-update-test | Add `maestro-handoff` to `bundled-skill-templates.test.ts` expected list | 04 |
| 8 | phase-h-08-sync-verify | Run sync, check, bun test, bun build, integration smoke | 04, 05, 06, 07 |
| 9 | phase-h-09-followup | Create Phase H+1 task: wire emitHandoff into ship/verify/abandon | 08 |
| 10 | phase-h-10-commit | Commit Phase H | 09 |

## Risks

- **Test breaks before phase-h-07.** Sync regenerates `bundled-skill-templates.ts` with 6 skills; old test expects 5. Sequence tightly — do not run `bun test` between phase-h-04 and phase-h-07.
- **Chain-consistency test trips if reverse-cross-ref added.** Keep handoff → verify ref one-directional.
- **Legacy directory-shaped handoffs (343 entries) confuse smoke.** Always use `*.json` glob.
- **maestro-task SKILL.md edits re-trigger drift.** Re-sync after maestro-task edits.
- **Approval beat is hard gate.** phase-h-04 blocked by phase-h-03; do not skip.

## Rollback

```bash
git revert HEAD --no-edit
bun run sync:bundled-skills
bun test
```

## Follow-ups (Phase H+1, H+2)

- **H+1: wire emitHandoff into `task ship`, `task verify --verdict {human,block}`, and `task abandon`.** Today only `claim` and `block` emit. The `maestro-handoff` SKILL.md flags the others as roadmap. Once wired, update the table row + update the MCP `maestro_handoff_emit` description to remove the "ship/verify/abandon do not emit on their own" stanza.
- **H+1: stale rows elsewhere in `maestro-task/SKILL.md` MCP table.** Audit the other rows (verdict, contract, evidence) for accuracy; rewrite in a single pass.
- **H+2: rename `HandoffEmitterPort` → `HandoffStorePort`.** The port now owns read+write+pickup; the "emitter" label is misleading. Touch only port + adapter + DI wiring.
- **H+2: `~/.hermes/skills/maestro` provider entry.** `docs/providers.md` Hermes section still has the "Runtime: yes" column claim; passive harness pivot may invalidate it. Audit the providers table separately from this PR's handoff scope.
