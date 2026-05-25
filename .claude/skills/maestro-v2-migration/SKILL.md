---
name: maestro-v2-migration
description: "Guide for working with maestroCLI's v2 architecture. Covers the context-to-memory rename, 4-state task model, plain file backend, pre-agent hooks, research phase, and memory promotion. Use when touching v2 code, encountering legacy patterns (e.g. 'context' instead of 'memory', old task states), adding new v2 features, or debugging v2 behavior. Also use when you see imports from adapters/fs/context, references to 'contextAdapter', or task states that don't match the 4-state model."
---

# maestroCLI v2 Architecture Guide

## Why This Exists

maestroCLI went through a significant architectural shift in v2. Sessions that touch v2 code without understanding these changes waste time rediscovering what was renamed, what was removed, and what the new patterns look like. This guide prevents that.

## The v2 Changes (in dependency order)

### 1. Context --> Memory Rename

Everything called "context" in v1 is now "memory" in v2.

| v1 (deprecated) | v2 (current) |
|-----------------|--------------|
| `ContextPort` | `MemoryPort` |
| `FsContextAdapter` | `FsMemoryAdapter` |
| `contextAdapter` | `memoryAdapter` |
| `context-write` command | `memory-write` command (but `context-write` still works as CLI alias) |
| `src/adapters/fs/context.ts` | `src/adapters/fs/memory.ts` |
| `src/ports/context.ts` | `src/ports/memory.ts` |
| `.maestro/features/<name>/context/` | `.maestro/features/<name>/memory/` (filesystem path unchanged for backward compat) |

If you see `context` in imports or variable names in core code, it is legacy. The filesystem directories under `.maestro/features/` may still use `context/` for backward compatibility -- that is intentional, not a bug.

### 2. Execution Layer Stripped

v1 had an execution layer that managed worktrees, delegation, and worker lifecycle. v2 stripped this entirely. The orchestrator (Claude Code, Codex, or the user) now manages execution directly.

**What was removed:**
- Worktree creation/management
- Worker prompt generation
- Delegation protocol
- Execution state tracking

**What replaced it:**
- The MCP server exposes task state transition tools
- The orchestrator calls `task_claim`, `task_done`, `task_block`, `task_unblock` directly
- No intermediate execution layer between the orchestrator and task state

### 3. Four-State Task Model

v1 had a complex task lifecycle. v2 uses exactly 4 states:

```
open --> claimed --> done
                \-> blocked --> (unblock) --> open
```

| State | Meaning | Transition |
|-------|---------|------------|
| `open` | Available for work | `task_claim` --> `claimed` |
| `claimed` | An agent is working on it | `task_done` --> `done`, `task_block` --> `blocked` |
| `done` | Work completed | Terminal |
| `blocked` | Cannot proceed, needs decision | `task_unblock` --> `open` |

There are no other states. If you see `pending`, `in_progress`, `failed`, `partial`, or `stale` in task code, that is v1 legacy and should be migrated.

### 4. Plain File Task Backend (Default)

v1 used `beads_rust` (`br`) as the task backend. v2 defaults to a plain filesystem backend (`FsTaskAdapter`).

| Backend | Config value | Adapter | When to use |
|---------|-------------|---------|-------------|
| Plain files | `"fs"` (default) | `FsTaskAdapter` | Default for all new projects |
| beads_rust | `"br"` | `BrTaskAdapter` | Legacy projects, explicit opt-in |

The backend is selected via `configAdapter.get().taskBackend` in `services.ts`. Both backends implement the same `TaskPort` interface -- code above the adapter layer should not care which backend is active.

### 5. Pre-Agent Hook for Task Spec Injection

v2 added a `pre-agent` hook (`hooks/pre-agent.mjs`) that runs before a worker agent starts. It injects the task specification into the agent's context automatically.

This means worker prompts no longer need to manually include task specs -- the hook handles it. If you are adding new context that workers need, consider whether it belongs in:
- The task spec (injected by pre-agent hook)
- The worker prompt template
- A memory file (loaded on demand)

### 6. Research Phase with External Tool Detection

v2 added a research phase that detects external tools available to the agent (MCP servers, CLI tools) and incorporates them into task planning. This runs during `task_claim` or early in the task lifecycle.

The research phase scans for:
- Available MCP server tools
- CLI tools in PATH
- Project-specific tooling (test runners, linters, build tools)

### 7. Memory Promotion on Feature Complete

When `feature_complete` is called, v2 suggests promoting valuable feature-scoped memories to project-level memories via `memory_promote`. This ensures learnings from a feature survive beyond the feature's lifecycle.

The `suggestPromote` logic runs automatically -- it scans feature memory files and suggests which ones contain decisions or constraints that apply project-wide.

## How to Apply This Guide

**When adding a new feature:**
- Use `MemoryPort` (not `ContextPort`)
- Use the 4-state task model
- Implement against `TaskPort` interface (agnostic to backend)
- Consider whether the pre-agent hook should inject your data

**When encountering legacy code:**
- Check if it uses v1 patterns (context, old task states, execution layer)
- Migrate to v2 patterns if the file is being modified anyway
- Do not migrate files you are not otherwise touching -- scope discipline

**When debugging:**
- Check which task backend is active (`configAdapter.get().taskBackend`)
- Check task state transitions match the 4-state model
- Check that memory (not context) adapters are being used
- Check pre-agent hook execution order
