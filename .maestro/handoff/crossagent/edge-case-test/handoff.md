# Cross-Agent Handoff: edge-case-test

| Field | Value |
|-------|-------|
| From | standalone |
| To | codex |
| Created | 2026-03-27T07:49:40.409Z |
| maestro | 0.2.0 |

## Plan
## Discovery

Testing edge cases in the cross-agent handoff protocol. We need to verify that Codex can handle various scenarios correctly including claiming tasks with dependencies, handling blocked tasks, and reporting partial completion.

### 1. Create test file

Create a file called `HANDOFF-TEST.md` in the project root with the text "Cross-agent handoff verified".

### 2. Verify test file

Read the file `HANDOFF-TEST.md` and confirm it contains the expected text. Report the result.

### 3. Clean up test file

Delete the file `HANDOFF-TEST.md` that was created in task 1.


## Tasks

| # | ID | Name | Status | Depends On |
|---|-----|------|--------|------------|
| 1 | maestro-1j8-clean-up-test-file | maestro-1j8-clean-up-test-file | pending | - |
| 2 | maestro-2p7-verify-test-file | maestro-2p7-verify-test-file | pending | - |
| 3 | maestro-3sg-create-test-file | maestro-3sg-create-test-file | pending | - |

## Doctrine

- **prefer-markdown-storage**: Use markdown files with sidecar indexes instead of databases for agent state that needs to be human-readable and git-tracked.
- **embed-at-build-time**: When code uses Bun-only APIs (import.meta.dir, import.meta.file) that will run under Node.js in the MCP bundle, embed the data at build time via a generator script instead of runtime filesystem scanning.

## Modified Files

- `.beads/issues.jsonl`
- `src/version.ts`

## Additional Context

Edge case test: tasks have dependencies (2 depends on 1, 3 depends on 2). You must respect the dependency order.

## Quickstart

This project uses `maestro` for agent coordination. Always pass `--json` to all commands.

### 1. Claim a task
```
maestro task-claim --feature edge-case-test --task maestro-1j8-clean-up-test-file --agent-id <your-id> --json
```

### 2. Implement, then mark done
```
maestro task-done --feature edge-case-test --task maestro-1j8-clean-up-test-file --content "summary of work" --json
```

### 3. Check remaining work
```
maestro task-list --feature edge-case-test --json
```

### 4. Report completion
```
maestro handoff-report --feature edge-case-test --content "Summary of all work done" --json
```

### Tips
- Run `maestro status --json` anytime to orient
- If a task is blocked: `maestro task-block --task <id> --reason "..." --json`
- Tasks have dependencies -- claim only tasks with no pending deps