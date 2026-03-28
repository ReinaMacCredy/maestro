# Cross-Agent Handoff: retest-after-refactor

| Field | Value |
|-------|-------|
| From | standalone |
| To | codex |
| Created | 2026-03-27T10:05:19.730Z |
| maestro | 0.2.0 |

## Plan
## Discovery

Post-refactoring validation of the cross-agent handoff protocol. The codebase just had 102 files changed with domain boundary cleanup, error standardization, and DRY extraction. Need to verify handoff commands still work end to end.

### 1. Verify handoff-pickup output

Run maestro handoff-pickup --json and confirm the JSON response contains feature, plan, tasks, quickstart, and state fields.

### 2. Verify task-next works

Run maestro task-next --feature retest-after-refactor --json and confirm it returns the correct first runnable task.


## Tasks

| # | ID | Name | Status | Depends On |
|---|-----|------|--------|------------|
| 1 | maestro-2tu-verify-task-next-works | maestro-2tu-verify-task-next-works | pending | - |
| 2 | maestro-1ts-verify-handoff-pickup-output | maestro-1ts-verify-handoff-pickup-output | pending | - |

## Doctrine

- **prefer-markdown-storage**: Use markdown files with sidecar indexes instead of databases for agent state that needs to be human-readable and git-tracked.
- **embed-at-build-time**: When code uses Bun-only APIs (import.meta.dir, import.meta.file) that will run under Node.js in the MCP bundle, embed the data at build time via a generator script instead of runtime filesystem scanning.

## Modified Files

- `.beads/issues.jsonl`
- `.claude/pending-merges.md`
- `src/__tests__/e2e/cli-agent-friendly-regression.test.ts`
- `src/app/handoff/crossagent.ts`
- `src/app/memory/execution/inference.ts`
- `src/app/workflow/stages.ts`
- `src/domain/errors.ts`
- `src/domain/ports/host.ts`
- `src/domain/types.ts`
- `src/infra/adapters/handoff/shared.ts`
- `src/infra/adapters/memory/adapter.ts`
- `src/infra/utils/fs-io.ts`
- `src/infra/utils/host-detect.ts`
- `src/infra/utils/time-utils.ts`
- `src/surfaces/cli/handlers/_task-factory.ts`
- `src/surfaces/cli/handlers/agents-md.ts`
- `src/surfaces/cli/handlers/config/config.ts`
- `src/surfaces/cli/handlers/config/get.ts`
- `src/surfaces/cli/handlers/config/set.ts`
- `src/surfaces/cli/handlers/dcp.ts`
- `src/surfaces/cli/handlers/debug-visual.ts`
- `src/surfaces/cli/handlers/doctor.ts`
- `src/surfaces/cli/handlers/doctrine/deprecate.ts`
- `src/surfaces/cli/handlers/doctrine/doctrine.ts`
- `src/surfaces/cli/handlers/doctrine/list.ts`
- `src/surfaces/cli/handlers/doctrine/read.ts`
- `src/surfaces/cli/handlers/doctrine/suggest.ts`
- `src/surfaces/cli/handlers/doctrine/write.ts`
- `src/surfaces/cli/handlers/execution-insights.ts`
- `src/surfaces/cli/handlers/feature/complete.ts`
- `src/surfaces/cli/handlers/feature/create.ts`
- `src/surfaces/cli/handlers/feature/feature.ts`
- `src/surfaces/cli/handlers/feature/info.ts`
- `src/surfaces/cli/handlers/feature/list.ts`
- `src/surfaces/cli/handlers/graph/discovery.ts`
- `src/surfaces/cli/handlers/graph/graph.ts`
- `src/surfaces/cli/handlers/graph/next.ts`
- `src/surfaces/cli/handlers/graph/plan.ts`
- `src/surfaces/cli/handlers/graph/reserve.ts`
- `src/surfaces/cli/handlers/handoff/handoff.ts`
- `src/surfaces/cli/handlers/handoff/list.ts`
- `src/surfaces/cli/handlers/handoff/pickup.ts`
- `src/surfaces/cli/handlers/handoff/plan.ts`
- `src/surfaces/cli/handlers/handoff/read.ts`
- `src/surfaces/cli/handlers/handoff/receive.ts`
- `src/surfaces/cli/handlers/handoff/report.ts`
- `src/surfaces/cli/handlers/handoff/send.ts`
- `src/surfaces/cli/handlers/handoff/status.ts`
- `src/surfaces/cli/handlers/history.ts`
- `src/surfaces/cli/handlers/init.ts`
- `src/surfaces/cli/handlers/install.ts`
- `src/surfaces/cli/handlers/memory/compile.ts`
- `src/surfaces/cli/handlers/memory/compress.ts`
- `src/surfaces/cli/handlers/memory/connect.ts`
- `src/surfaces/cli/handlers/memory/consolidate.ts`
- `src/surfaces/cli/handlers/memory/delete.ts`
- `src/surfaces/cli/handlers/memory/insights.ts`
- `src/surfaces/cli/handlers/memory/list.ts`
- `src/surfaces/cli/handlers/memory/memory.ts`
- `src/surfaces/cli/handlers/memory/promote.ts`
- `src/surfaces/cli/handlers/memory/read.ts`
- `src/surfaces/cli/handlers/memory/stats.ts`
- `src/surfaces/cli/handlers/memory/write.ts`
- `src/surfaces/cli/handlers/ping.ts`
- `src/surfaces/cli/handlers/plan/comment.ts`
- `src/surfaces/cli/handlers/plan/comments-clear.ts`
- `src/surfaces/cli/handlers/plan/plan.ts`
- `src/surfaces/cli/handlers/plan/read.ts`
- `src/surfaces/cli/handlers/plan/revoke.ts`
- `src/surfaces/cli/handlers/plan/write.ts`
- `src/surfaces/cli/handlers/search/search.ts`
- `src/surfaces/cli/handlers/search/sessions.ts`
- `src/surfaces/cli/handlers/search/similar.ts`
- `src/surfaces/cli/handlers/skill/create.ts`
- `src/surfaces/cli/handlers/skill/install.ts`
- `src/surfaces/cli/handlers/skill/load.ts`
- `src/surfaces/cli/handlers/skill/remove.ts`
- `src/surfaces/cli/handlers/skill/skill.ts`
- `src/surfaces/cli/handlers/skill/sync.ts`
- `src/surfaces/cli/handlers/stage/back.ts`
- `src/surfaces/cli/handlers/stage/jump.ts`
- `src/surfaces/cli/handlers/stage/skip.ts`
- `src/surfaces/cli/handlers/status.ts`
- `src/surfaces/cli/handlers/task/accept.ts`
- `src/surfaces/cli/handlers/task/brief.ts`
- `src/surfaces/cli/handlers/task/claim.ts`
- `src/surfaces/cli/handlers/task/done.ts`
- `src/surfaces/cli/handlers/task/list.ts`
- `src/surfaces/cli/handlers/task/next.ts`
- `src/surfaces/cli/handlers/task/reject.ts`
- `src/surfaces/cli/handlers/task/sync.ts`
- `src/surfaces/cli/handlers/task/task.ts`
- `src/surfaces/cli/handlers/task/unblock.ts`
- `src/surfaces/cli/handlers/toolbox/add.ts`
- `src/surfaces/cli/handlers/toolbox/create.ts`
- `src/surfaces/cli/handlers/toolbox/install.ts`
- `src/surfaces/cli/handlers/toolbox/remove.ts`
- `src/surfaces/cli/handlers/toolbox/test.ts`
- `src/surfaces/cli/handlers/toolbox/toolbox.ts`
- `src/surfaces/cli/handlers/update/self.ts`
- `src/surfaces/cli/handlers/visual.ts`
- `src/version.ts`

## Quickstart

This project uses `maestro` for agent coordination. Always pass `--json` to all commands.

### 1. Find the next runnable task
```
maestro task-next --feature retest-after-refactor --json
```
This returns the next task whose dependencies are satisfied.

### 2. Claim and implement
```
maestro task-claim --feature retest-after-refactor --task maestro-2tu-verify-task-next-works --agent-id <your-id> --json
```

### 3. Mark done
```
maestro task-done --feature retest-after-refactor --task maestro-2tu-verify-task-next-works --content "summary of work" --json
```

### 4. Repeat until all tasks done
```
maestro task-next --feature retest-after-refactor --json
```
When task-next returns no runnable tasks, all work is done.

### 5. Report completion
```
maestro handoff-report --feature retest-after-refactor --content "Summary of all work done" --json
```

### Tips
- Run `maestro status --feature retest-after-refactor --json` anytime to orient
- If a task is blocked: `maestro task-block --feature retest-after-refactor --task <id> --reason "..." --json`
- Always use task-next to find runnable tasks -- it respects dependency order