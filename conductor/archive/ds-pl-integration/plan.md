# Plan: DS-PL Integration

## Epic Overview

Integrate `pl` (Planning Pipeline) into `ds` (Design Session) for seamless design-to-planning flow.

## Tasks

### Task 1: Update design/SKILL.md
**File:** `~/.config/agents/skills/design/SKILL.md`
**Priority:** P0
**Effort:** S

Changes:
1. Add step 7 in "Session Flow" section:
   ```markdown
   7. **Plan** - In FULL mode with APPROVED verdict, auto-run pl pipeline (6 phases) → [planning/PIPELINE.md](../conductor/references/planning/PIPELINE.md)
   ```

2. Update "Next Steps" section:
   ```markdown
   > **Note:** In FULL mode with Oracle approval, pl pipeline runs automatically.
   > Manual `cn` only needed for SPEED mode or interrupted sessions.
   ```

3. Add row to "Research & Validation Triggers" table:
   ```markdown
   | Post-CP4 (FULL) | pl Discovery/Synthesis | Execution planning |
   ```

### Task 2: Update double-diamond.md
**File:** `~/.config/agents/skills/design/references/double-diamond.md`
**Priority:** P0
**Effort:** S

Add after "Exit: Design verified and approved":
```markdown
## Post-DELIVER: Planning Pipeline (FULL Mode Only)

After Oracle Audit passes with APPROVED verdict in FULL mode:

1. Display transition:
   ```
   ✅ Design approved. Transitioning to Planning Pipeline...
   ```

2. Execute full pl pipeline (all 6 phases):
   - Phase 1: Discovery
   - Phase 2: Synthesis
   - Phase 3: Verification (spikes if HIGH risk)
   - Phase 4: Decomposition (fb)
   - Phase 5: Validation (bv)
   - Phase 6: Track Planning

3. Update metadata.json with planning state

**SPEED Mode:** Suggest `cn` command instead of auto-trigger.

**Failure:** Show failed phase; resume via `cn`.
```

### Task 3: Update routing-table.md
**File:** `~/.config/agents/skills/maestro-core/references/routing-table.md`
**Priority:** P1
**Effort:** XS

Update row for `ds`:
```markdown
| `ds` | design | 3 | FULL mode: auto-chains to pl after CP4 |
```

Update "Design vs Planning" section:
```markdown
| Auto-chain | No | Yes (after FULL ds) |
```

### Task 4: Create metadata.json for track
**File:** `conductor/tracks/ds-pl-integration/metadata.json`
**Priority:** P1
**Effort:** XS

```json
{
  "track_id": "ds-pl-integration",
  "created": "2025-01-04",
  "status": "planning",
  "workflow": {
    "state": "PLANNING"
  }
}
```

## Verification

```bash
# After implementation, verify:
# 1. Run ds in FULL mode → should auto-trigger pl
# 2. Run ds in SPEED mode → should suggest cn
# 3. Run pl standalone → should work unchanged
```

## Dependencies

- None (all tasks are independent documentation updates)

## Track Assignments

| Track | Agent | Beads (in order) | File Scope |
|-------|-------|------------------|------------|
| A | SingleTrack | Task 1 → Task 2 → Task 3 → Task 4 | `skills/design/**`, `skills/maestro-core/**` |
