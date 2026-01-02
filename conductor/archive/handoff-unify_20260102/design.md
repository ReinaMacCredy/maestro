# Design: Unified Handoff System

## Problem Statement

The current handoff system is scattered across 6 files (~1100 lines) and missing key features from the Conductor-Beads reference implementation:

1. **No parallel execution check** - Handoffs don't detect running parallel workers
2. **No Beads sync** - Handoff context isn't saved to Beads CLI for compaction-proof resumability
3. **No progress tracking** - No section count, progress %, or handoff history in metadata.json
4. **Scattered documentation** - 6 separate files make it hard to understand the full workflow

## Reference

Based on: https://github.com/NguyenSiTrung/Conductor-Beads/blob/main/.claude/commands/conductor-handoff.md

## Solution

Consolidate into 2 unified files and add missing features.

---

## Design Decisions

### D1: Single Command with Subcommands

**Decision:** Replace `/create_handoff` and `/resume_handoff` with unified `/conductor-handoff`

**Rationale:** 
- Matches Conductor-Beads pattern
- Auto-detect reduces cognitive load
- Backward-compatible aliases maintained

**Command syntax:**
```
/conductor-handoff              # Auto-detect mode
/conductor-handoff create       # Force create
/conductor-handoff resume       # Force resume
/conductor-handoff resume <path|track>
```

**Auto-detect logic:**
```
IF session_first_message AND recent_handoff_exists(<7d)
  → RESUME mode
ELSE
  → CREATE mode
```

### D2: Merge Progress Tracking into metadata.json

**Decision:** Add `handoff` section to metadata.json instead of separate implement_state.json fields

**Rationale:**
- Single source of truth per track
- Already have workflow, beads, generation sections
- Reduces state file proliferation

**Schema:**
```json
{
  "handoff": {
    "status": "active | handed_off",
    "section_count": 2,
    "progress_percent": 45,
    "last_handoff": "ISO8601",
    "history": [
      {
        "section": 1,
        "timestamp": "ISO8601",
        "trigger": "epic-end",
        "bead_id": "E1-jwt-core",
        "phase_at_handoff": "Phase 2",
        "tasks_completed": 5,
        "tasks_total": 12,
        "file": "filename.md"
      }
    ]
  }
}
```

### D3: Beads Sync Protocol

**Decision:** Add `bd update` call in create workflow with structured notes

**Rationale:**
- Notes survive context compaction
- Enables seamless session resume via `bd show`
- Matches Conductor-Beads protocol

**Note format:**
```
COMPLETED: Tasks 1-N (X% of track)
KEY DECISIONS: [list]
IN PROGRESS: <current_task>
NEXT: <next_task>
BLOCKER: <if any>
HANDOFF: Section N saved at <path>
```

### D4: Parallel Execution Check

**Decision:** Add Step 1a to detect parallel_state.json before handoff

**Rationale:**
- Prevents losing worker context during handoff
- Gives user choice: wait, proceed, or cancel

**Prompt:**
```
⚠️ Parallel workers running: [worker-1, worker-2]

[A] Wait for completion
[B] Handoff anyway (include worker state)
[C] Cancel
```

### D5: File Consolidation

**Decision:** Delete 5 files, keep 1, create 2

**Files to DELETE:**
- `handoff/create.md` → merged
- `handoff/resume.md` → merged
- `handoff/triggers.md` → merged
- `handoff/idle-detection.md` → merged
- `handoff/agent-mail-format.md` → merged

**Files to KEEP:**
- `handoff/template.md` - Still needed for markdown format

**Files to CREATE:**
- `commands/handoff.toml` - Command definition (~30 lines)
- `workflows/handoff.md` - Full workflow reference (~300 lines)

---

## Workflows

### CREATE Mode (9 Steps)

| Step | Action | New |
|------|--------|:---:|
| 1 | Detect context (track or general) | |
| 1a | Parallel worker check | ⭐ |
| 2 | Gather metadata (git, validation) | |
| 3 | Scan for secrets | |
| 4 | Send to Agent Mail | |
| 5 | Beads sync (`bd update --notes`) | ⭐ |
| 6 | Write markdown file | |
| 7 | Update metadata.json.handoff | ⭐ |
| 8 | Update index.md | |
| 9 | Touch .last_activity | |

### RESUME Mode (9 Steps)

| Step | Action | New |
|------|--------|:---:|
| 1 | Parse input (path, track, or auto) | |
| 2 | Agent Mail lookup (primary) | |
| 3 | File discovery (fallback) | |
| 4 | Load handoff content | |
| 5 | Beads context (`bd show <epic>`) | ⭐ |
| 6 | Validate git state (branch, drift) | |
| 7 | Present analysis | |
| 8 | Create TodoWrite items | |
| 9 | Update metadata.json.handoff.status | ⭐ |

### Integration: /conductor-implement Phase 0.5

Enhanced to include:
1. Run `/conductor-handoff resume` internally
2. Load beads context (`bd show <epic>`) ⭐
3. Calculate and display progress % ⭐
4. Create epic-start handoff

---

## Triggers (6 Auto-Triggers)

| Trigger | When | Auto |
|---------|------|:----:|
| `design-end` | After `/conductor-newtrack` | ✅ |
| `epic-start` | Before each epic | ✅ |
| `epic-end` | After epic closes | ✅ |
| `pre-finish` | Start of `/conductor-finish` | ✅ |
| `manual` | User runs command | ❌ |
| `idle` | After 30min gap | ✅ |

---

## Non-Goals

- Not changing handoff file format (template.md stays same)
- Not changing Agent Mail integration (already primary)
- Not adding new triggers

---

## Success Criteria

1. Single `/conductor-handoff` command works with auto-detect
2. Beads sync saves context on every handoff
3. Progress % displays on resume
4. Parallel workers detected before handoff
5. 5 files deleted, 2 created (net -3 files, -700 lines)
