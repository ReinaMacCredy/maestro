---
name: beads
version: "2.2.0"
description: >
  Tracks complex, multi-session work using the Beads issue tracker and dependency graphs, and provides
  persistent memory that survives conversation compaction. Use when work spans multiple sessions, has
  complex dependencies, or needs persistent context across compaction cycles. Trigger with phrases like
  "create task for", "what's ready to work on", "show task", "track this work", "what's blocking", or
  "update status".
---

# Beads - Persistent Task Memory for AI Agents

Graph-based issue tracker that survives conversation compaction. Provides persistent memory for multi-session work with complex dependencies.

## Entry Points

| Trigger | Workflow | Action |
|---------|----------|--------|
| `bd`, `beads` | `workflows/beads/workflow.md` | Core CLI operations |
| `fb`, `file-beads` | `workflows/beads/references/FILE_BEADS.md` | File beads from plan |
| `rb`, `review-beads` | `workflows/beads/references/REVIEW_BEADS.md` | Review filed beads |

## Load Workflow

1. Identify trigger from user input
2. Load corresponding workflow file (see table above)
3. Follow instructions in loaded file

## Quick Decision

**Key Distinction**:
- **bd**: Multi-session work, dependencies, survives compaction, git-backed
- **TodoWrite**: Single-session tasks, linear execution, conversation-scoped

**When to Use bd vs TodoWrite**:
- ❓ "Will I need this context in 2 weeks?" → **YES** = bd
- ❓ "Could conversation history get compacted?" → **YES** = bd
- ❓ "Does this have blockers/dependencies?" → **YES** = bd
- ❓ "Is this fuzzy/exploratory work?" → **YES** = bd
- ❓ "Will this be done in this session?" → **YES** = TodoWrite
- ❓ "Is this just a task list for me right now?" → **YES** = TodoWrite

**Decision Rule**: If resuming in 2 weeks would be hard without bd, use bd.

## Core Capabilities

- 📊 **Dependency Graphs**: Track what blocks what (blocks, parent-child, discovered-from, related)
- 💾 **Compaction Survival**: Tasks persist when conversation history is compacted
- 🐙 **Git Integration**: Issues versioned in `.beads/issues.jsonl`, sync with `bd sync`
- 🔍 **Smart Discovery**: Auto-finds ready work (`bd ready`), blocked work (`bd blocked`)
- 📝 **Audit Trails**: Complete history of status changes, notes, and decisions
- 🏷️ **Rich Metadata**: Priority (P0-P4), types (bug/feature/task/epic), labels, assignees

## Conductor Integration

When used with Conductor, beads operations are **automated via a facade pattern**:

### Facade Abstraction

Conductor commands call beads through a unified facade that:
- Handles mode detection (SA vs MA)
- Manages retry logic and error recovery
- Persists failed operations for later replay
- Abstracts differences between CLI and Village MCP

**In the happy path, you never run manual bd commands** - Conductor handles:
- `preflight` → bd availability check
- `track-init` → create epic + issues from plan.md
- `claim` → bd update --status in_progress
- `close` → bd close --reason completed
- `sync` → bd sync with retry

### SA vs MA Mode

| Mode | Description | Operations |
|------|-------------|------------|
| **SA** (Single-Agent) | Direct `bd` CLI calls | Standard bd commands |
| **MA** (Multi-Agent) | Village MCP server | Atomic claims, file reservations, handoffs |

Mode is detected at session start and locked for the session.

### planTasks Mapping

`.fb-progress.json` contains bidirectional mapping between plan task IDs and bead IDs:

```json
{
  "planTasks": { "1.1.1": "bd-42", "1.2.1": "bd-43" },
  "beadToTask": { "bd-42": "1.1.1", "bd-43": "1.2.1" }
}
```

This enables:
- Track which plan tasks have beads
- Navigate from bead to plan context
- Detect orphan beads after plan revisions

### When Manual bd IS Appropriate

- Direct issue creation outside Conductor flow
- Ad-hoc queries (`bd search`, `bd list`)
- Debugging (`bd show <id>`)
- Recovery from failed automated operations

See [Beads Integration](../conductor/references/beads-integration.md) for all 13 integration points.

## Essential Commands Quick Reference

| Command | Purpose |
|---------|---------|
| `bd ready` | Show tasks ready to work on |
| `bd create "Title" -p 1` | Create new task |
| `bd show <id>` | View task details |
| `bd update <id> --status in_progress` | Start working |
| `bd update <id> --notes "Progress"` | Add progress notes |
| `bd close <id> --reason "Done"` | Complete task |
| `bd dep add <child> <parent>` | Add dependency |
| `bd list` | See all tasks |
| `bd search <query>` | Find tasks by keyword |
| `bd sync` | Sync with git remote |

## Session Start Protocol

1. **Run** `bd ready` first
2. **Pick** highest priority ready task
3. **Run** `bd show <id>` to get full context
4. **Update** status to `in_progress`
5. **Add notes** as you work (critical for compaction survival)

## Full Documentation

For complete instructions, load the workflow file: `workflows/beads/workflow.md`

Reference files in `workflows/beads/references/`:
- `AGENTS.md` - Agent integration patterns
- `BOUNDARIES.md` - Scope and boundary rules
- `CLI_REFERENCE.md` - Complete command syntax
- `CONFIG.md` - Configuration system
- `DAEMON.md` - Daemon management
- `DEPENDENCIES.md` - Dependency system deep dive
- `FILE_BEADS.md` - Filing beads from plans
- `GIT_INTEGRATION.md` - Git workflows
- `ISSUE_CREATION.md` - Creating issues properly
- `LABELS.md` - Label system and usage
- `RESUMABILITY.md` - Session resumption patterns
- `REVIEW_BEADS.md` - Reviewing filed beads
- `STATIC_DATA.md` - Static data handling
- `TROUBLESHOOTING.md` - Common issues
- `VILLAGE.md` - Multi-agent coordination
- `WORKFLOWS.md` - Detailed workflow patterns
