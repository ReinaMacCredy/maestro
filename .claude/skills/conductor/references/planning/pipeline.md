# Planning Pipeline

The 6-phase pipeline for risk-based feature planning.

## Pipeline Overview

```
USER REQUEST → Discovery → Synthesis → Verification → Decomposition → Validation → Track Planning → Ready Plan
```

| Phase | Tool | Output |
|-------|------|--------|
| 1. Discovery | Parallel Task() agents | design.md Section 2 |
| 2. Synthesis | Oracle | design.md Section 3 (Gap + Risk Map) |
| 3. Verification | Spikes via Task() | design.md Section 5 |
| 4. Decomposition | fb (file-beads) | .beads/*.md |
| 5. Validation | bv + Oracle | Validated dependency graph |
| 6. Track Planning | bv --robot-plan | plan.md Track Assignments |

## Phase 1: Discovery

Launch parallel sub-agents to gather codebase intelligence:

```
Task() → Agent A: Architecture snapshot (Grep, finder, Read)
Task() → Agent B: Pattern search (find similar existing code)
Task() → Agent C: Constraints (package.json, tsconfig, deps)
Web search → External patterns ("how do similar projects do this?")
```

**Output**: design.md Section 2 with:
- Architecture Snapshot (relevant packages, key modules, entry points)
- Existing Patterns (similar implementations, reusable utilities)
- Technical Constraints (versions, dependencies, build requirements)
- External References (library docs, similar projects)

## Phase 2: Synthesis

Feed Discovery to Oracle for gap analysis:

```python
oracle(
  task="Analyze gap between current codebase and feature requirements",
  context="Discovery attached. User wants: <feature>",
  files=["conductor/tracks/<id>/design.md"]
)
```

**Oracle produces**:
1. **Gap Analysis** - What exists vs what's needed
2. **Approach Options** - 1-3 strategies with tradeoffs
3. **Risk Assessment** - LOW / MEDIUM / HIGH per component

**Output**: design.md Section 3 with Gap Analysis, Risk Map, Recommended Approach.

### Risk Classification

| Level | Criteria | Verification |
|-------|----------|--------------|
| LOW | Pattern exists in codebase | Proceed |
| MEDIUM | Variation of existing pattern | Interface sketch, type-check |
| HIGH | Novel or external integration | Spike required |

### Risk Indicators

```
Pattern exists in codebase? ─── YES → LOW base
                            └── NO  → MEDIUM+ base

External dependency? ─── YES → HIGH
                     └── NO  → Check blast radius

Blast radius >5 files? ─── YES → HIGH
                       └── NO  → MEDIUM
```

## Phase 3: Verification

For HIGH risk items, create and execute spikes.

See [spikes.md](spikes.md) for full spike workflow.

**Summary**:
1. Create spike bead and directory
2. Execute via Task() with time-box (30 min default)
3. Capture result (YES/NO + learnings)
4. Update design.md Section 5

**Output**: design.md Section 5, validated approach.

## Phase 4: Decomposition

Load file-beads skill and create beads with embedded learnings:

```bash
skill("beads")
```

Each bead MUST include:
- **Spike learnings** embedded in description (if applicable)
- **Reference to spike code** for HIGH risk items
- **Clear acceptance criteria**
- **File scope** for track assignment

**Output**: .beads/*.md with spike learnings embedded.

## Phase 5: Validation

### Run bv Analysis

```bash
bv --robot-suggest   # Find missing dependencies
bv --robot-insights  # Detect cycles, bottlenecks
bv --robot-priority  # Validate priorities
```

### Fix Issues

```bash
bd dep add <from> <to>      # Add missing deps
bd dep remove <from> <to>   # Break cycles
bd update <id> --priority X # Adjust priorities
```

### Oracle Final Review

```python
oracle(
  task="Review plan completeness and clarity",
  context="Plan ready. Check for gaps, unclear beads, missing deps.",
  files=[".beads/"]
)
```

**Output**: Validated dependency graph.

## Phase 6: Track Planning

Creates execution-ready plan for orchestrator.

### Step 1: Get Parallel Tracks

```bash
bv --robot-plan 2>/dev/null | jq '.plan.tracks'
```

### Step 2: Assign File Scopes

For each track, determine file scope based on beads. Rules:
- File scopes must NOT overlap between tracks
- Use glob patterns: `packages/sdk/**`, `apps/server/**`
- If overlap unavoidable, merge into single track

### Step 3: Generate Agent Names

Assign unique adjective+noun names:
- BlueLake, GreenCastle, RedStone, PurpleBear, etc.
- Names are memorable identifiers, NOT role descriptions

### Step 4: Create Track Assignments

Add to plan.md:

```markdown
## Track Assignments

| Track | Agent | Beads (in order) | File Scope |
|-------|-------|------------------|------------|
| A | BlueLake | bd-10 → bd-11 → bd-12 | `packages/sdk/**` |
| B | GreenCastle | bd-20 → bd-21 | `packages/cli/**` |
| C | RedStone | bd-30 → bd-31 → bd-32 | `apps/server/**` |
```

### Validation

```bash
# No cycles in the graph
bv --robot-insights 2>/dev/null | jq '.Cycles'

# All beads assigned to tracks
bv --robot-plan 2>/dev/null | jq '.plan.unassigned'
```

**Output**: plan.md with Track Assignments, design.md Section 6.

## State Machine

```
unplanned → discovery → synthesized → verified → decomposed → validated → track_planned → executing → complete
```

Tracked in `metadata.json`:
```json
{
  "planning": {
    "state": "track_planned",
    "phases_completed": ["discovery", "synthesis", "verification", "decomposition", "validation", "track_planning"]
  }
}
```

## Validation Gates

| Gate | After Phase | Enforcement |
|------|-------------|-------------|
| discovery-complete | 1 | WARN |
| risk-assessed | 2 | HALT if HIGH without spike |
| spikes-resolved | 3 | HALT if unresolved |
| execution-ready | 6 | HALT if missing learnings |

## Related

- [spikes.md](spikes.md) - Spike workflow details
- [design-template.md](design-template.md) - Unified design.md format
- [../../workflows/newtrack.md](../workflows/newtrack.md) - Track creation
