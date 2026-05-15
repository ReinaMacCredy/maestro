# Phase 4 — Done

Phase 4 was scoped as docs polish + skill-surface cleanup (master plan §10). PRs 36-40 cover that stated scope. PRs 41-44 pulled the ADR-0015 + ADR-0018 sunset deletions forward where they had zero v2 dependencies, so Phase 5 can focus on the entangled cuts (mission, spec, verify, setup, task v1 stores) and the MCP v2 verb pass.

## PRs

| PR  | Scope                                                                  | Commit |
| --- | ---------------------------------------------------------------------- | ------ |
| 36  | docs(cli-reference) rewrite against v2 verbs only                      | (Phase 4 batch) |
| 37  | docs(harness-positioning) rewrite around v2 primitives                 | (Phase 4 batch) |
| 38  | docs root + setup templates v2 pass                                    | (Phase 4 batch) |
| 39  | feat(skills,test) five-skill v2 bundle + delete 6 absorbed skills      | (Phase 4 batch) |
| 40  | feat(skills) delete `skills/built-in` colon-tier                       | 7404c29c |
| 41  | feat(harness-os) delete v1 `memory`, `memory-ratchet`, `agent`         | f9e30e0a |
| 42  | feat(harness-os) delete v1 `graph`, `session`, `notes`, `intake`       | 88f19303 |
| 43  | feat(harness-os) delete v1 `ralph`, `inspect`, `state` + `lint:arch:v1` | 08b4e7bb |
| 44  | refactor(mcp) rename `missionId` → `plan_id` on `task_list` filter     | (PR 44 commit) |

## What landed

### Documentation pass (PRs 36-38)
- `docs/cli-reference.md` lists only v2 verbs; v1 verb tables removed.
- `docs/harness-positioning.md` reframed around v2 primitives (Spec, Plan, Task, Evidence, Verdict, Principle).
- `AGENTS.md` / `CLAUDE.md` / `.maestro/docs/` setup templates updated to v2 vocab. The `WHERE TO LOOK` table still points at some v1 paths (mission, spec, verify, setup) since those feature dirs are alive; the rewrite is Phase 5 scope.

### Skill-surface cleanup (PRs 39-40)
- Five-skill bundle finalized: `maestro-design`, `maestro-plan`, `maestro-task`, `maestro-verify`, `maestro-setup`. Each cross-references `maestro-verify` as the canonical verification protocol.
- Absorbed v1 skills deleted from `skills/bundled/` and `skills/built-in/`.
- Colon-namespaced `skills/built-in/maestro:*` tier deleted.

### v1 feature deletions (PRs 41-43)
Master-plan Phase 5 owns the bulk v1 source-tree sunset. PRs 41-43 pulled forward the deletions with zero v2 dependencies — the ADR-0015 absorbed list. Specifically:

- **PR 41**: `memory`, `memory-ratchet`, `agent` — absorbed by the v2 principle subsystem.
- **PR 42**: `graph`, `session`, `notes`, `intake` — dropped from the harness OS per ADR-0015. Session-detect autodetection retired in favor of `MAESTRO_AGENT` / `MAESTRO_SESSION_ID` env vars; per-user synthesized fallback session id retained.
- **PR 43**: `ralph`, `inspect`, `state` — dropped (loop primitive owns iterate-until-PASS; per-primitive `show` verbs replace `inspect`/`state`). Also removed the v1 arch lint script `lint:arch:v1` and `scripts/lint/run-arch-lint.ts`; `lint:arch` now points at the v2 runner.

The remaining v1 dirs (`mission`, `spec`, `verify`, `setup`, `task`) all have live v2 surfaces *and* v1 surfaces sharing process state; Phase 5 retires the v1 halves.

### MCP wire-parameter rename (PR 44)
- `maestro_task_list` filter parameter `missionId` → `plan_id`; regex tightened to `^pln-[a-z0-9]+-[a-z0-9]+$` to match v2 exec-plan ids.
- Strict-mode rejection: clients sending `missionId` now get a tool error, no silent drop. Migrate to `plan_id`.
- Wire-output asymmetry retained intentionally: v1 `Task` domain still carries `missionId` in its JSON body; Phase 5 aligns input + output when MCP rewires onto v2 use cases that return v2 `Task` with `plan_id`.

## Dogfood evidence

Run on commit `08b4e7bb` (before PR 44), then re-validated post-PR-44. Binary built via `bun run release:local`.

### Read-only checks (live repo, cwd = maestro repo root)

```
$ ./dist/maestro --version
0.83.0.1778869913-gfa707eb (released 2026-05-15T18:31:53.617Z, 10s ago)

$ ./dist/maestro mission-control --render-check --size 120x40 --json | jq .summary
{
  "passed": 14,
  "failed": 0,
  "skipped": 0
}

$ ./dist/maestro setup --check
Setup audit — ISSUES
  Host runtimes detected: claude-code, codex
  Skills checked: 5 (0 drift)
  [error] agents-md-too-large: AGENTS.md is 173 lines, exceeds hard limit 160
  [info] owners-role-missing: owners.yaml has no deploy_approver entries

$ ./dist/maestro skills list 2>&1 | head -2
[!] SKILL.md frontmatter is malformed: Nested mappings are not allowed in compact mappings at line 2, column 14
[ok] 132 skill(s)
```

### Happy-path cycle (sandbox)

```
$ TMP=$(mktemp -d) && cd "$TMP" && git init -q && git commit --allow-empty -qm "root"

$ maestro setup bootstrap
created 5, skipped 0
  created .maestro/tasks
  created .maestro/plans
  created .maestro/evidence
  created .maestro/runs
  created docs/principles

$ maestro setup check
[ok]   .maestro/tasks
[ok]   .maestro/plans
[ok]   .maestro/evidence
[ok]   .maestro/runs
[ok]   docs/principles
[warn] docs/principles/*.md — no principles found
[warn] .maestro/config.yaml — config.yaml not present (optional)
setup check: OK

$ maestro spec new phase-4-dogfood --title "Phase 4 dogfood smoke" --mode light
Created spec at .maestro/specs/phase-4-dogfood.md

$ maestro task create "fresh task"
[ok] Task created: implement/fresh-task

$ maestro task list
1 task(s)
implement/fresh-task  P2  pending       fresh task

$ maestro task claim tsk-35daa3
maestro task claim: Task tsk-35daa3 not found
```

### Known finding: v1 / v2 task-store overlap

Sandbox dogfood surfaced the expected v1/v2 store split:

- `maestro task create` writes through the v1 adapter to `.maestro/tasks/tasks.jsonl`.
- `maestro task claim` is the v2 verb (registered later in `src/index.ts`, overrides v1) and reads through `src/v2/repo/jsonl-task-store.adapter.ts` from `.maestro/tasks/tasks.v2.jsonl`.
- The v2 read sees an empty store and returns `Task not found`.

This is the v1/v2 process-state split that Phase 5 retires: once v1 `task create` is deleted and the only path is `task from-spec` writing to the v2 store, the overlap disappears. Not a regression — it is the precise scenario ADR-0018 names as the Phase 5 cut.

The fix is deferred to Phase 5; no fix lands in Phase 4 because Phase 4 deliberately leaves the v1 task feature alive (it is one of the entangled dirs Phase 5 owns).

## Deferred to Phase 5 (and beyond)

- **`src/features/mission/` directory**: entangled with TUI snapshot model, `replyStore`, `principleStore`, `bundle`, `handoff`. ADR-0015 keeps reply + principle alive; both currently live inside mission and must be extracted before the parent dir can go. The TUI rewires off v1 mission shapes (`Mission`, `Feature`, `Assertion`, `Checkpoint`, `MissionStorePort`, etc.) onto exec-plan shapes from `src/v2/types/` in the same wave.
- **`src/features/{spec,verify,setup,task}/` v1 halves**: each has a v2 counterpart already shipping; Phase 5 deletes the v1 halves and consolidates onto the v2 store.
- **MCP server v2 pass**: rename `task_complete` → `task_ship`; delete `task_unblock`; replace `task_create` / `task_plan` with `task_from_spec`; add `principle_promote`, `setup_check`, `setup_migrate_v2`. Phase 4 PR 44 only renamed the `missionId` filter on `task_list`.
- **README v2 sweep**: post-sunset audit that no v1 verb survives in root docs.
- **install-smoke workflow v2 verbs**: extend `.github/workflows/install-smoke.yml` to exercise `setup check` + `spec new` + `task from-spec` + `claim` + `verify` + `ship`.
- **§9 migration mapping audit** and **AGENTS.md `WHERE TO LOOK` rewrite**.
- **§11 non-goals stay**: `handoff`, `bundle`, `evidence`, `verdict`, `policy`, `plan-check`, `worktree`.

## Operational invariants

- Passive harness preserved: no scheduler, no daemon, no `setInterval`/`setTimeout` introduced.
- Forward-only layers: v2 arch lint clean.
- Local-first: all writes under `.maestro/` in the consumer repo. No network calls inside maestro.
- Test suite: 2864 pass / 0 fail / 112 skip on PR 44 head.

## What's next

Phase 4 is closed. The next batch of work continues v1 sunset and consolidates onto v2: extract reply + principle stores, rewire TUI onto exec-plan shapes, delete v1 `mission`/`spec`/`verify`/`setup`/`task` halves, ship the MCP v2 verb pass, sweep README, extend install-smoke, then cut 2.0.
