# Design: Orchestrator Skill Improvements

## Problem Statement

Current orchestrator skill lacks:
1. **Track threads** for bead-to-bead context passing
2. **Explicit per-bead loop** (START → WORK → COMPLETE → NEXT)
3. **AGENTS.md tool preferences** in worker prompts
4. **Auto-detect parallel routing** based on bead independence
5. **Lingering beads verification** before epic close

## Design Decisions

### 1. Track Thread Pattern (Ephemeral)

- Thread ID format: `track:{AGENT_NAME}:{EPIC_ID}`
- Cleared per epic (ephemeral, not persistent)
- Workers self-message learnings/gotchas between beads
- Workers read track thread via `summarize_thread()` at bead start

### 2. Per-Bead Loop Protocol

For EACH bead in track:
```
START:    register, read track thread, reserve files, claim bead
WORK:     implement, check inbox periodically  
COMPLETE: close bead, report to orchestrator, save context to track thread, release
NEXT:     loop to next bead
```

### 3. AGENTS.md Tool Preferences

Add explicit section to worker prompt:
```markdown
## Tool Preferences (from AGENTS.md)
- Codebase exploration: {tools}
- File editing: {tools}
- Web search: {tools}
```

### 4. Auto-Detect Parallel Routing

```
/conductor-implement
       ↓
  Has "## Track Assignments"? ─── YES ──→ Use orchestrator
       │
       NO
       ↓
  Read metadata.json.beads.planTasks (B: saved by fb)
       ↓
  Verify with `bd list --json` (A: runtime check)
       ↓
  ≥2 independent beads? ─── YES ──→ Auto-generate tracks → Use orchestrator
       │
       NO
       ↓
  Sequential execution
```

### 5. Enhanced Monitoring

- Primary: `bv --robot-triage --graph-root <epic-id> | jq '.quick_ref'`
- Secondary: `fetch_inbox`, `search_messages`

### 6. Lingering Beads Check

Before epic close:
```bash
OPEN_COUNT=$(bd list --parent=<epic-id> --status=open --json | jq 'length')
if [ "$OPEN_COUNT" -gt 0 ]; then
  echo "⚠️ Lingering beads found: $OPEN_COUNT"
  # Prompt user to close or skip
fi
```

### 7. Fix metadata.json planTasks Population

Ensure `fb` command saves:
```json
{
  "beads": {
    "planTasks": { "1.1": "bead-id-1", "1.2": "bead-id-2" },
    "beadToTask": { "bead-id-1": "1.1", "bead-id-2": "1.2" },
    "crossTrackDeps": []
  }
}
```

## Research Validation

| Check | Status |
|-------|--------|
| Track thread self-messaging | ✅ VALID - Agent Mail supports |
| Worker prompt conflicts | ✅ NO CONFLICTS - fits in STEP 2 loop |
| Auto-detect via metadata.json | ⚠️ Need to ensure fb saves planTasks |

## Out of Scope

- Cross-epic persistent learnings (future enhancement)
- Worker retry/respawn on failure (existing fallback is sufficient)
