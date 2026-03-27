# Cross-Agent Handoff: handoff-pipeline-test

| Field | Value |
|-------|-------|
| From | codex |
| To | claude |
| Created | 2026-03-27T14:12:13.493Z |
| maestro | 0.2.0 |

## Plan
## Discovery
The current feature needs a cross-agent handoff to Claude, but Maestro's handoff flow only works after a valid plan exists, the plan has been approved, and the derived tasks have been synced. This repository's current active feature is `handoff-pipeline-test`, and `maestro status --json` showed that the feature was still in `planning` with no plan file and no tasks. The requested work is therefore to convert the provided handoff outline into a valid Maestro plan so the CLI can export a Claude-ready handoff artifact safely and deterministically.

### 1. Write plan into Maestro state
Use the repository root as the working directory and write this handoff-focused plan for the active feature so Maestro has a valid plan file with numbered task headings.

### 2. Approve the plan
Approve the written plan for `handoff-pipeline-test` so the workflow can advance beyond planning and allow downstream task generation.

### 3. Sync tasks from the plan
Run task sync so the numbered plan sections become executable Maestro tasks, which is a required prerequisite for `handoff-plan`.

### 4. Generate the Claude handoff
Create a cross-agent handoff targeted at `claude` for the `handoff-pipeline-test` feature and capture the resulting handoff artifact path and summary.

### 5. Verify Claude pickup
Verify that `maestro handoff-pickup --feature handoff-pipeline-test --json` succeeds and returns the quickstart instructions Claude should follow.


## Tasks

| # | ID | Name | Status | Depends On |
|---|-----|------|--------|------------|
| 1 | maestro-1tf-verify-claude-pickup | maestro-1tf-verify-claude-pickup | pending | - |
| 2 | maestro-2n8-generate-the-claude-handoff | maestro-2n8-generate-the-claude-handoff | pending | - |
| 3 | maestro-1s9-sync-tasks-from-the-plan | maestro-1s9-sync-tasks-from-the-plan | pending | - |
| 4 | maestro-3uk-approve-the-plan | maestro-3uk-approve-the-plan | pending | - |
| 5 | maestro-2t5-write-plan-into-maestro-state | maestro-2t5-write-plan-into-maestro-state | pending | - |

## Doctrine

- **prefer-markdown-storage**: Use markdown files with sidecar indexes instead of databases for agent state that needs to be human-readable and git-tracked.
- **embed-at-build-time**: When code uses Bun-only APIs (import.meta.dir, import.meta.file) that will run under Node.js in the MCP bundle, embed the data at build time via a generator script instead of runtime filesystem scanning.

## Modified Files

- `.beads/issues.jsonl`
- `.claude/pending-merges.md`
- `.maestro/features/handoff-pipeline-test/feature.json`
- `src/version.ts`

## Quickstart

This project uses `maestro` for agent coordination. Always pass `--json` to all commands.

### 1. Find the next runnable task
```
maestro task-next --feature handoff-pipeline-test --json
```
This returns the next task whose dependencies are satisfied.

### 2. Claim and implement
```
maestro task-claim --feature handoff-pipeline-test --task maestro-1tf-verify-claude-pickup --agent-id <your-id> --json
```

### 3. Mark done
```
maestro task-done --feature handoff-pipeline-test --task maestro-1tf-verify-claude-pickup --content "summary of work" --json
```

### 4. Repeat until all tasks done
```
maestro task-next --feature handoff-pipeline-test --json
```
When task-next returns no runnable tasks, all work is done.

### 5. Report completion
```
maestro handoff-report --feature handoff-pipeline-test --content "Summary of all work done" --json
```

### Tips
- Run `maestro status --feature handoff-pipeline-test --json` anytime to orient
- If a task is blocked: `maestro task-block --feature handoff-pipeline-test --task <id> --reason "..." --json`
- Always use task-next to find runnable tasks -- it respects dependency order