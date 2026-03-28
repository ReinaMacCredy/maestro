# Cross-Agent Handoff: codex-impl-test

| Field | Value |
|-------|-------|
| From | standalone |
| To | codex |
| Created | 2026-03-27T07:10:05.195Z |
| maestro | 0.2.0 |

## Plan
## Discovery

The maestro CLI currently has 92 commands but the CLAUDE.md documentation lists only 70 in the "CLI Commands" section. 
We investigated and found that the count is stale from before the cross-agent handoff commands were added.
The handoff domain section lists only 3 commands but now has 9 (send, receive, ack, list, read, status, plan, pickup, report).
The total count in the "CLI Harness" description line also says 89 but should say 92.

### 1. Update CLAUDE.md command counts

Update the CLAUDE.md file at project root:
- Change "89 CLI commands" to "92 CLI commands" in the Architecture section
- Change "CLI Commands (70)" heading to the correct count
- Update the Handoff section from 3 to 9 commands: add `handoff-plan`, `handoff-pickup`, `handoff-report`, `handoff-list`, `handoff-read`, `handoff-status`
- Update the Other section count if needed (currently 13)

### 2. Update total in CLI Reference section

In the same CLAUDE.md file, the "CLI Reference (Agent Use)" section mentions commands.
Add entries for the 3 new cross-agent handoff commands under a new subsection or in the existing Handoff entries:
- `maestro handoff-plan --to <agent> --json` -- export plan for another agent
- `maestro handoff-pickup --json` -- discover pending handoff  
- `maestro handoff-report --content "..." --json` -- report completion


## Tasks

| # | ID | Name | Status | Depends On |
|---|-----|------|--------|------------|
| 1 | maestro-1i8-update-total-in-cli-reference-section | maestro-1i8-update-total-in-cli-reference-section | pending | - |
| 2 | maestro-8wm-update-claudemd-command-counts | maestro-8wm-update-claudemd-command-counts | pending | - |

## Doctrine

- **prefer-markdown-storage**: Use markdown files with sidecar indexes instead of databases for agent state that needs to be human-readable and git-tracked.
- **embed-at-build-time**: When code uses Bun-only APIs (import.meta.dir, import.meta.file) that will run under Node.js in the MCP bundle, embed the data at build time via a generator script instead of runtime filesystem scanning.

## Modified Files

- `.beads/issues.jsonl`

## Additional Context

Simple docs update -- just edit CLAUDE.md to fix stale command counts. Safe change, no code modifications needed.

## Quickstart

This project uses `maestro` for agent coordination. Always pass `--json` to all commands.

### 1. Claim a task
```
maestro task-claim --task maestro-1i8-update-total-in-cli-reference-section --agent-id <your-id> --json
```

### 2. Implement, then mark done
```
maestro task-done --task maestro-1i8-update-total-in-cli-reference-section --content "summary of work" --json
```

### 3. Check remaining work
```
maestro task-list --feature codex-impl-test --json
```

### 4. Report completion
```
maestro handoff-report --feature codex-impl-test --content "Summary of all work done" --json
```

### Tips
- Run `maestro status --json` anytime to orient
- If a task is blocked: `maestro task-block --task <id> --reason "..." --json`
- Tasks have dependencies -- claim only tasks with no pending deps