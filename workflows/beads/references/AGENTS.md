# Agent Instructions

This file provides core instructions for AI agents using beads.

See the [workflow.md](../workflow.md) file for full beads skill instructions.

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:

```bash
git pull --rebase
bd sync
git push
git status  # MUST show "up to date with origin"
```

5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**

- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds

## Quick Reference

### Session Start

```bash
bd ready --json           # Find available work
bd list --status in_progress --json  # Check active work
bd show <id>              # Read notes for context
```

### During Work

```bash
bd update <id> --status in_progress --json  # Claim work
bd update <id> --notes "Progress update..."  # Checkpoint
bd create "Found issue" -t bug -p 1 --deps discovered-from:<parent> --json
```

### Session End

```bash
bd close <id> --reason "Completed" --json  # Close finished work
bd sync                   # Force sync to git
git push                  # Push to remote
```

## See Also

- [workflow.md](../workflow.md) - Complete beads skill documentation
- [CLI_REFERENCE.md](CLI_REFERENCE.md) - Full command reference
- [WORKFLOWS.md](WORKFLOWS.md) - Common workflow patterns
- [DAEMON.md](DAEMON.md) - Daemon management
