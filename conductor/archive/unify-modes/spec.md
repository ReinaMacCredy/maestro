# Specification: Unify SA/MA into FULL Mode

## Overview

Merge Single-Agent (SA) and Multi-Agent (MA) execution modes into a single unified **FULL mode**. All execution flows through the orchestrator with Agent Mail MCP coordination. Village MCP is removed entirely.

## Background

The current system has two distinct execution paths:
- **SA (Single-Agent)**: Sequential TDD, direct `bd` CLI commands
- **MA (Multi-Agent)**: Parallel dispatch via Village MCP coordination

This dual-mode architecture creates:
1. Branching logic throughout ~55 files
2. Cognitive overhead for developers ("which mode am I in?")
3. Two coordination mechanisms to maintain (Village + Agent Mail)
4. Graceful fallback complexity (MA → SA degradation)

## Functional Requirements

### FR-1: Unified Execution Mode
- Remove SA/MA mode detection logic
- Always use orchestrator, even for single-task execution
- Single-task execution spawns 1 worker for consistency

### FR-2: Agent Mail Coordination Only
- Remove all Village MCP references (`bv` commands, `.beads-village/`)
- Use Agent Mail MCP for all coordination:
  - `register_agent` for identity
  - `file_reservation_paths` for file locking
  - `send_message`/`fetch_inbox` for communication
  - `release_file_reservations` for explicit release

### FR-3: Worker Protocol (per bead)
| Step | Action |
|------|--------|
| 1. Register | `register_agent(name, program, model)` |
| 2. Check inbox | `fetch_inbox()` |
| 3. Load context | `summarize_thread(track:{AGENT}:{EPIC})` |
| 4. Claim task | `bd update <id> --status in_progress` |
| 5. Reserve files | `file_reservation_paths(paths=[...])` |
| 6. Work | TDD cycle |
| 7. Close task | `bd close <id> --reason completed` |
| 8. Release files | `release_file_reservations()` |
| 9. Notify | `send_message(to=[orchestrator], ...)` |
| 10. Save context | `send_message(to=[self], thread_id=track:...)` |

### FR-4: Thread Structure
| Thread | Purpose |
|--------|---------|
| `{EPIC_ID}` | Epic-wide coordination (orchestrator ↔ workers) |
| `track:{AGENT}:{EPIC_ID}` | Bead-to-bead context (worker ↔ self) |

### FR-5: Preflight Simplification
- Remove mode detection step
- Remove Village MCP availability check
- Add Agent Mail registration check
- Update session state schema (remove `mode` field)

### FR-6: Fallback Policy Change
- Current: Village unavailable → degrade to SA mode
- New: Agent Mail unavailable → HALT (no fallback)

## Non-Functional Requirements

### NFR-1: File Changes
- ~55 files require updates (see design.md for full list)
- Delete: `docs/VILLAGE.md`
- Major rewrites: 8 core skill references
- SA/MA/Mode refs: 18 files
- Village/bv refs: 10 files
- Root docs: 13 files

### NFR-2: Backward Compatibility
- Existing tracks with no `## Track Assignments` still work (orchestrator with 1 worker)
- Beads CLI (`bd`) commands unchanged
- TDD checkpoint behavior unchanged

### NFR-3: Schema Updates
Remove `session.mode` from metadata.json:
```json
{
  "session": {
    "bound_bead": "...",
    "tdd_phase": "...",
    "agent_mail_registered": true
  }
}
```

## Acceptance Criteria

1. **AC-1**: No references to "SA", "MA", "SINGLE_AGENT", or "mode detection" in skill files
2. **AC-2**: No references to Village MCP (`bv`, `.beads-village/`, `init team`)
3. **AC-3**: `/conductor-implement` always invokes orchestrator
4. **AC-4**: Single-task execution completes successfully via orchestrator + 1 worker
5. **AC-5**: Agent Mail failure causes HALT, not degradation
6. **AC-6**: All existing tests pass
7. **AC-7**: `./scripts/validate-links.sh .` passes
8. **AC-8**: `./scripts/validate-anchors.sh .` passes

## Out of Scope

- Changing orchestrator's wave execution logic
- Modifying TDD checkpoint behavior
- Changing beads CLI (`bd`) commands
- Changing SPEED/FULL design complexity routing (unrelated to SA/MA)
